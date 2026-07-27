// Mobile bridge — the LAN server the HyprSpace Android app (see ../../mobile) talks to.
//
// Shape of it: a small hand-rolled WebSocket server on the local network. The phone connects,
// authenticates with a pairing token, and then
//   * receives a mirror of the app's own state (spaces, panes, agent status, automations), which the
//     frontend PUSHES here on every change — Rust never introspects the store,
//   * subscribes to panes and gets their PTY output live (plus a replay of the recent tail so the
//     screen is drawn immediately), and writes keystrokes back,
//   * makes request/response calls for anything that needs the app itself (git, usage, launching a
//     pane…) — those are forwarded to the frontend as a Tauri event and answered via bridge_reply.
//
// Why hand-rolled: the framing we need is ~100 lines and it keeps a network-facing dependency tree
// out of a desktop app that otherwise has none.
//
// The mirror is best-effort by design: a slow phone gets frames DROPPED rather than being allowed to
// backpressure the PTY coalescer. Nothing on this path may ever stall a desktop terminal.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::pty::{PtyManager, Tap};

/// Bumped whenever the message shape changes incompatibly. The phone refuses to connect on a
/// mismatch instead of half-working — a silently incompatible mirror is worse than no mirror.
pub const PROTOCOL: u32 = 1;

pub const DEFAULT_PORT: u16 = 6768;

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
// a phone only ever sends control JSON and keystrokes; anything big is someone else knocking
const MAX_FRAME: usize = 1024 * 1024;
// how long a fresh connection has to send its `hello` before we hang up
const AUTH_SECS: u64 = 8;
// per-peer outbound queue. full = the phone can't keep up, so we drop (see module note).
const OUT_QUEUE: usize = 512;

// ---------------------------------------------------------------- websocket framing

fn accept_key(key: &str) -> String {
    let mut h = Sha1::new();
    h.update(key.as_bytes());
    h.update(WS_GUID.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(h.finalize())
}

/// Read the HTTP upgrade request. Byte-at-a-time is fine — it happens once per connection.
fn read_headers(s: &mut TcpStream) -> Option<String> {
    let mut buf: Vec<u8> = Vec::with_capacity(512);
    let mut b = [0u8; 1];
    while buf.len() < 8192 {
        match s.read(&mut b) {
            Ok(1) => buf.push(b[0]),
            _ => return None,
        }
        if buf.ends_with(b"\r\n\r\n") {
            return String::from_utf8(buf).ok();
        }
    }
    None
}

fn header<'a>(req: &'a str, name: &str) -> Option<&'a str> {
    req.lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case(name))
        .map(|(_, v)| v.trim())
}

fn write_frame(s: &mut TcpStream, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut h = Vec::with_capacity(10);
    h.push(0x80 | opcode); // FIN + opcode; we never fragment outbound
    let n = payload.len();
    if n < 126 {
        h.push(n as u8);
    } else if n <= 0xffff {
        h.push(126);
        h.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        h.push(127);
        h.extend_from_slice(&(n as u64).to_be_bytes());
    }
    s.write_all(&h)?;
    s.write_all(payload)?;
    s.flush()
}

struct Frame {
    opcode: u8,
    fin: bool,
    payload: Vec<u8>,
}

fn read_n(s: &mut TcpStream, n: usize) -> Option<Vec<u8>> {
    let mut v = vec![0u8; n];
    s.read_exact(&mut v).ok()?;
    Some(v)
}

fn read_frame(s: &mut TcpStream) -> Option<Frame> {
    let h = read_n(s, 2)?;
    let fin = h[0] & 0x80 != 0;
    let opcode = h[0] & 0x0f;
    let masked = h[1] & 0x80 != 0;
    let mut len = (h[1] & 0x7f) as usize;
    if len == 126 {
        let e = read_n(s, 2)?;
        len = u16::from_be_bytes([e[0], e[1]]) as usize;
    } else if len == 127 {
        let e = read_n(s, 8)?;
        len = u64::from_be_bytes(e.try_into().ok()?) as usize;
    }
    if len > MAX_FRAME {
        return None; // refuse rather than allocate on a stranger's say-so
    }
    let mask = if masked { Some(read_n(s, 4)?) } else { None };
    let mut payload = read_n(s, len)?;
    if let Some(m) = mask {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= m[i % 4];
        }
    }
    Some(Frame { opcode, fin, payload })
}

// ---------------------------------------------------------------- state

enum Out {
    Text(String),
    Bin(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

struct Peer {
    id: u64,
    name: String,
    addr: String,
    since: u64,
    tx: SyncSender<Out>,
    subs: Arc<Mutex<HashSet<String>>>,
}

#[derive(Serialize, Clone)]
pub struct PeerInfo {
    pub id: u64,
    pub name: String,
    pub addr: String,
    pub since: u64,
}

#[derive(Serialize, Clone)]
pub struct BridgeInfo {
    pub running: bool,
    pub port: u16,
    pub protocol: u32,
    pub host: String,
    /// LAN address the phone should dial, when we can work one out
    pub address: Option<String>,
    pub peers: Vec<PeerInfo>,
}

#[derive(Default)]
struct Inner {
    running: bool,
    port: u16,
    token: String,
    state: String, // last snapshot the frontend published, verbatim JSON
    peers: Vec<Peer>,
    stop: Option<Arc<AtomicBool>>,
}

#[derive(Default, Clone)]
pub struct Bridge {
    inner: Arc<Mutex<Inner>>,
    next_id: Arc<AtomicU64>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()))
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "desktop".into())
}

/// The IP a phone on the same wifi should dial. A connected UDP socket sends nothing — it just makes
/// the OS pick the default route's interface, which beats guessing among a machine's many addresses
/// (Hyper-V, WSL and VPN adapters all show up in a naive enumeration).
fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    let ip = s.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() {
        return None;
    }
    Some(ip.to_string())
}

/// Compare without an early return, so the time taken doesn't leak how much of the token matched.
fn token_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

impl Bridge {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn info(&self) -> BridgeInfo {
        let g = self.inner();
        BridgeInfo {
            running: g.running,
            port: g.port,
            protocol: PROTOCOL,
            host: hostname(),
            address: if g.running { lan_ip() } else { None },
            peers: g
                .peers
                .iter()
                .map(|p| PeerInfo { id: p.id, name: p.name.clone(), addr: p.addr.clone(), since: p.since })
                .collect(),
        }
    }

    /// Bind and start accepting. `token` is minted and persisted by the frontend (which owns the
    /// pairing UI); we only ever compare against it.
    pub fn start(&self, app: AppHandle, port: u16, token: String) -> Result<BridgeInfo, String> {
        if token.len() < 16 {
            return Err("pairing token is too short".into());
        }
        {
            let mut g = self.inner();
            if g.running {
                // already up — just adopt the (possibly regenerated) token
                g.token = token;
                drop(g);
                return Ok(self.info());
            }
        }
        let want = if port == 0 { DEFAULT_PORT } else { port };
        // walk a few ports so a stale instance (or another app on 6768) doesn't block pairing
        let (listener, bound) = (0..8)
            .find_map(|i| {
                let p = want.saturating_add(i);
                TcpListener::bind(("0.0.0.0", p)).ok().map(|l| (l, p))
            })
            .ok_or_else(|| format!("couldn't bind a port near {want}"))?;

        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut g = self.inner();
            g.running = true;
            g.port = bound;
            g.token = token;
            g.stop = Some(stop.clone());
        }

        let me = self.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(sock) = conn else { continue };
                let me = me.clone();
                let app = app.clone();
                std::thread::spawn(move || me.serve(app, sock));
            }
        });
        Ok(self.info())
    }

    pub fn stop(&self) {
        let (stop, port, peers) = {
            let mut g = self.inner();
            if !g.running {
                return;
            }
            g.running = false;
            let peers: Vec<SyncSender<Out>> = g.peers.drain(..).map(|p| p.tx).collect();
            (g.stop.take(), g.port, peers)
        };
        for tx in peers {
            let _ = tx.try_send(Out::Close);
        }
        if let Some(s) = stop {
            s.store(true, Ordering::Relaxed);
            // incoming() is blocking — poke it so it notices the flag and the thread ends
            let _ = TcpStream::connect(("127.0.0.1", port));
        }
    }

    /// The frontend's state snapshot (spaces, panes, agent status, automations). Stored verbatim and
    /// fanned out to every peer.
    pub fn publish(&self, state: String) {
        let msg = format!("{{\"t\":\"state\",\"d\":{}}}", if state.is_empty() { "null" } else { &state });
        let mut g = self.inner();
        g.state = state;
        for p in &g.peers {
            let _ = p.tx.try_send(Out::Text(msg.clone()));
        }
    }

    /// Answer a `req` the frontend handled.
    pub fn reply(&self, peer: u64, rid: u64, ok: bool, data: Value) {
        let msg = json!({ "t": "res", "id": rid, "ok": ok, "d": data }).to_string();
        let g = self.inner();
        if let Some(p) = g.peers.iter().find(|p| p.id == peer) {
            let _ = p.tx.try_send(Out::Text(msg));
        }
    }

    /// PTY fan-out. Called from the coalescer thread for EVERY session — must be cheap and must
    /// never block, hence try_send + an early bail when nobody's listening.
    pub fn on_pty(&self, pane: &str, tap: Tap<'_>) {
        let g = self.inner();
        if g.peers.is_empty() {
            return;
        }
        // build the payload once, only if some peer actually wants this pane
        let mut framed: Option<Vec<u8>> = None;
        let mut text: Option<String> = None;
        for p in &g.peers {
            let wants = p.subs.lock().unwrap_or_else(|e| e.into_inner()).contains(pane);
            if !wants {
                continue;
            }
            match tap {
                Tap::Data(bytes) => {
                    let buf = framed.get_or_insert_with(|| pane_frame(pane, bytes));
                    let _ = p.tx.try_send(Out::Bin(buf.clone()));
                }
                Tap::Size(cols, rows) => {
                    let m = text.get_or_insert_with(|| {
                        json!({ "t": "size", "pane": pane, "cols": cols, "rows": rows }).to_string()
                    });
                    let _ = p.tx.try_send(Out::Text(m.clone()));
                }
                Tap::Exit(code) => {
                    let m = text.get_or_insert_with(|| {
                        json!({ "t": "exit", "pane": pane, "code": code }).to_string()
                    });
                    let _ = p.tx.try_send(Out::Text(m.clone()));
                }
            }
        }
    }

    fn drop_peer(&self, app: &AppHandle, id: u64) {
        let mut g = self.inner();
        let before = g.peers.len();
        g.peers.retain(|p| p.id != id);
        let changed = g.peers.len() != before;
        drop(g);
        if changed {
            let _ = app.emit("bridge://peers", self.info());
        }
    }

    // one connection, start to finish: upgrade → authenticate → serve until it goes away
    fn serve(&self, app: AppHandle, mut sock: TcpStream) {
        let addr = sock.peer_addr().map(|a| a.ip().to_string()).unwrap_or_else(|_| "?".into());
        let _ = sock.set_nodelay(true);
        let _ = sock.set_read_timeout(Some(Duration::from_secs(AUTH_SECS)));

        let Some(req) = read_headers(&mut sock) else { return };
        let Some(key) = header(&req, "sec-websocket-key") else {
            let _ = sock.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            return;
        };
        let resp = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            accept_key(key)
        );
        if sock.write_all(resp.as_bytes()).is_err() {
            return;
        }

        // first frame must be a valid hello; anything else and we're done
        let Some(f) = read_frame(&mut sock) else { return };
        let hello: Value = match f.opcode {
            1 => serde_json::from_slice(&f.payload).unwrap_or(Value::Null),
            _ => Value::Null,
        };
        let token = hello.get("token").and_then(|v| v.as_str()).unwrap_or("");
        let their_protocol = hello.get("protocol").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let name = hello
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.chars().filter(|c| !c.is_control()).take(48).collect::<String>())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Phone".into());

        let ours = self.inner().token.clone();
        if !token_eq(token, &ours) {
            let _ = write_frame(&mut sock, 1, br#"{"t":"error","code":"auth"}"#);
            let _ = write_frame(&mut sock, 8, &[]);
            return;
        }
        if their_protocol != PROTOCOL {
            let msg = json!({ "t": "error", "code": "protocol", "need": PROTOCOL }).to_string();
            let _ = write_frame(&mut sock, 1, msg.as_bytes());
            let _ = write_frame(&mut sock, 8, &[]);
            return;
        }

        // authenticated: no more deadline, but keep a long one so a phone that drops off wifi
        // without a FIN (sleep, tunnel, airplane mode) doesn't leak this thread forever
        let _ = sock.set_read_timeout(Some(Duration::from_secs(180)));

        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (tx, rx) = sync_channel::<Out>(OUT_QUEUE);
        let subs: Arc<Mutex<HashSet<String>>> = Arc::default();

        // writer thread — the only place that writes to the socket, so nothing interleaves frames
        let Ok(mut wsock) = sock.try_clone() else { return };
        let writer = std::thread::spawn(move || {
            while let Ok(out) = rx.recv() {
                let ok = match out {
                    Out::Text(s) => write_frame(&mut wsock, 1, s.as_bytes()),
                    Out::Bin(b) => write_frame(&mut wsock, 2, &b),
                    Out::Pong(b) => write_frame(&mut wsock, 10, &b),
                    Out::Close => {
                        let _ = write_frame(&mut wsock, 8, &[]);
                        break;
                    }
                };
                if ok.is_err() {
                    break;
                }
            }
            let _ = wsock.shutdown(std::net::Shutdown::Both);
        });

        let version = app.package_info().version.to_string();
        let welcome = json!({
            "t": "welcome",
            "protocol": PROTOCOL,
            "app": "hyprspace",
            "version": version,
            "host": hostname(),
            "peer": id,
        })
        .to_string();
        let _ = tx.try_send(Out::Text(welcome));
        {
            let mut g = self.inner();
            let state = g.state.clone();
            g.peers.push(Peer { id, name, addr: addr.clone(), since: now_secs(), tx: tx.clone(), subs: subs.clone() });
            let _ = tx.try_send(Out::Text(format!(
                "{{\"t\":\"state\",\"d\":{}}}",
                if state.is_empty() { "null" } else { &state }
            )));
        }
        let _ = app.emit("bridge://peers", self.info());

        // read loop
        let mut acc: Vec<u8> = Vec::new(); // continuation frames (a long paste can arrive split)
        loop {
            let Some(f) = read_frame(&mut sock) else { break };
            match f.opcode {
                8 => break,               // close
                9 => {
                    let _ = tx.try_send(Out::Pong(f.payload));
                    continue;
                }
                10 => continue, // pong
                0 => {
                    acc.extend_from_slice(&f.payload);
                    if !f.fin {
                        continue;
                    }
                    let whole = std::mem::take(&mut acc);
                    if !self.handle(&app, id, &subs, &tx, &whole) {
                        break;
                    }
                }
                1 | 2 => {
                    if !f.fin {
                        acc.clear();
                        acc.extend_from_slice(&f.payload);
                        continue;
                    }
                    if !self.handle(&app, id, &subs, &tx, &f.payload) {
                        break;
                    }
                }
                _ => break,
            }
        }

        let _ = tx.try_send(Out::Close);
        drop(tx);
        let _ = writer.join();
        self.drop_peer(&app, id);
    }

    /// Handle one client message. Returns false to hang up.
    fn handle(
        &self,
        app: &AppHandle,
        peer: u64,
        subs: &Arc<Mutex<HashSet<String>>>,
        tx: &SyncSender<Out>,
        raw: &[u8],
    ) -> bool {
        let Ok(m) = serde_json::from_slice::<Value>(raw) else { return true }; // ignore junk, stay up
        let t = m.get("t").and_then(|v| v.as_str()).unwrap_or("");
        let pane = m.get("pane").and_then(|v| v.as_str()).unwrap_or("").to_string();
        match t {
            // mirror a pane: replay the recent tail so the phone paints a screen immediately,
            // then stream live
            "sub" => {
                if pane.is_empty() {
                    return true;
                }
                subs.lock().unwrap_or_else(|e| e.into_inner()).insert(pane.clone());
                let pty = app.state::<PtyManager>();
                match pty.replay(&pane) {
                    Some((buf, cols, rows)) => {
                        let head =
                            json!({ "t": "size", "pane": &pane, "cols": cols, "rows": rows }).to_string();
                        let _ = tx.try_send(Out::Text(head));
                        // chunked: one 64K frame is fine on the wire but the phone's terminal
                        // parses more smoothly fed in pieces
                        for part in buf.chunks(16 * 1024) {
                            let _ = tx.try_send(Out::Bin(pane_frame(&pane, part)));
                        }
                    }
                    None => {
                        let _ = tx.try_send(Out::Text(
                            json!({ "t": "gone", "pane": &pane }).to_string(),
                        ));
                    }
                }
            }
            "unsub" => {
                subs.lock().unwrap_or_else(|e| e.into_inner()).remove(&pane);
            }
            // keystrokes, base64 like the desktop's own write_pty
            "in" => {
                let d = m.get("d").and_then(|v| v.as_str()).unwrap_or("");
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(d) {
                    let _ = app.state::<PtyManager>().write(&pane, &bytes);
                }
            }
            // anything that needs the app itself — the frontend answers via bridge_reply
            "req" => {
                let rid = m.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let method = m.get("m").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let params = m.get("p").cloned().unwrap_or(Value::Null);
                let _ = app.emit(
                    "bridge://req",
                    json!({ "peer": peer, "id": rid, "m": method, "p": params }),
                );
            }
            "ping" => {
                let _ = tx.try_send(Out::Text(r#"{"t":"pong"}"#.into()));
            }
            "bye" => return false,
            _ => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener as TL;

    // the handshake digest is the one thing a client can't be lenient about — this vector is
    // straight from RFC 6455 §1.3
    #[test]
    fn handshake_accept_key() {
        assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let req = "GET / HTTP/1.1\r\nHost: x\r\nSec-WebSocket-Key: abc\r\n\r\n";
        assert_eq!(header(req, "sec-websocket-key"), Some("abc"));
        assert_eq!(header(req, "SEC-WEBSOCKET-KEY"), Some("abc"));
        assert_eq!(header(req, "nope"), None);
    }

    #[test]
    fn token_compare() {
        assert!(token_eq("0123456789abcdef", "0123456789abcdef"));
        assert!(!token_eq("0123456789abcdef", "0123456789abcdee"));
        assert!(!token_eq("short", "0123456789abcdef"));
        assert!(!token_eq("", ""));
    }

    #[test]
    fn pane_frame_layout() {
        let f = pane_frame("abc", b"hi");
        assert_eq!(f, vec![3, b'a', b'b', b'c', b'h', b'i']);
    }

    /// Write a frame the way the server does, read it back the way the server reads a client's —
    /// masking it first, since client frames always are. Covers all three length encodings.
    fn round_trip(payload: &[u8]) {
        let l = TL::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let body = payload.to_vec();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).unwrap();
            // hand-mask it: header with the mask bit, then key, then xored bytes
            let mut h = vec![0x82u8]; // FIN + binary
            let n = body.len();
            if n < 126 {
                h.push(0x80 | n as u8);
            } else if n <= 0xffff {
                h.push(0x80 | 126);
                h.extend_from_slice(&(n as u16).to_be_bytes());
            } else {
                h.push(0x80 | 127);
                h.extend_from_slice(&(n as u64).to_be_bytes());
            }
            let key = [0x37u8, 0xfa, 0x21, 0x3d];
            h.extend_from_slice(&key);
            let masked: Vec<u8> = body.iter().enumerate().map(|(i, b)| b ^ key[i % 4]).collect();
            h.extend_from_slice(&masked);
            c.write_all(&h).unwrap();
            c.flush().unwrap();
        });

        let (mut server, _) = l.accept().unwrap();
        let f = read_frame(&mut server).expect("frame");
        assert!(f.fin);
        assert_eq!(f.opcode, 2);
        assert_eq!(f.payload, payload);
        t.join().unwrap();
    }

    #[test]
    fn frames_round_trip_at_every_length_encoding() {
        round_trip(b"short");
        round_trip(&vec![7u8; 300]); // 16-bit length
        round_trip(&vec![9u8; 70_000]); // 64-bit length
    }

    #[test]
    fn oversized_frame_is_refused_rather_than_allocated() {
        let l = TL::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let t = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).unwrap();
            // claims 2 GB; we must bail on the header alone
            let mut h = vec![0x82u8, 0x80 | 127];
            h.extend_from_slice(&(2u64 << 30).to_be_bytes());
            let _ = c.write_all(&h);
        });
        let (mut server, _) = l.accept().unwrap();
        assert!(read_frame(&mut server).is_none());
        t.join().unwrap();
    }
}

/// binary pane payload: [id length][pane id][raw pty bytes]
fn pane_frame(pane: &str, data: &[u8]) -> Vec<u8> {
    let id = pane.as_bytes();
    let mut v = Vec::with_capacity(1 + id.len() + data.len());
    v.push(id.len().min(255) as u8);
    v.extend_from_slice(&id[..id.len().min(255)]);
    v.extend_from_slice(data);
    v
}

// ---------------------------------------------------------------- tauri commands

#[tauri::command]
pub fn bridge_status(bridge: State<Bridge>) -> BridgeInfo {
    bridge.info()
}

#[tauri::command]
pub fn bridge_start(
    app: AppHandle,
    bridge: State<Bridge>,
    port: u16,
    token: String,
) -> Result<BridgeInfo, String> {
    bridge.start(app, port, token)
}

#[tauri::command]
pub fn bridge_stop(bridge: State<Bridge>) {
    bridge.stop();
}

#[tauri::command]
pub fn bridge_publish(bridge: State<Bridge>, state: String) {
    bridge.publish(state);
}

#[tauri::command]
pub fn bridge_reply(bridge: State<Bridge>, peer: u64, id: u64, ok: bool, data: Value) {
    bridge.reply(peer, id, ok, data);
}

/// Boot the PTY→bridge tap. Registered once at startup; it's a no-op while no phone is subscribed.
pub fn attach(app: &AppHandle) {
    let bridge = app.state::<Bridge>().inner().clone();
    app.state::<PtyManager>()
        .set_tap(Arc::new(move |pane: &str, tap: Tap<'_>| bridge.on_pty(pane, tap)));
}
