use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures::{FutureExt, SinkExt, StreamExt};
use hyprspace_proto::peer::Found;
use hyprspace_proto::phone::{Down, OUTPUT, PROTOCOL, Up, unframe};
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::crypto::{
    CryptoProvider, verify_tls12_signature, verify_tls13_signature,
};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{ClientConfig, DigitallySignedStruct, Error, SignatureScheme};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use super::{Peers, Saved};
use crate::phone::squeeze::Unsqueeze;
use crate::phone::{host_name, prove};

type Ws = WebSocketStream<TlsStream<TcpStream>>;

const DIAL: Duration = Duration::from_secs(6);
const MAX_MESSAGE: usize = 64 << 20;

pub(super) async fn run(peers: Peers, id: String) {
    let mut wait = 1;
    loop {
        let Some(saved) = peers.saved(&id) else {
            return;
        };
        match dial(&saved).await {
            Ok(ws) => {
                wait = 1;
                if !session(&peers, &saved, ws).await {
                    return;
                }
            }
            Err(e) => {
                peers.offline(&id, Some(e));
                if let Some(f) = find(&id, Duration::from_secs(3)).await {
                    peers.moved(&id, f.hosts, f.port);
                    continue;
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(wait)).await;
        wait = (wait * 2).min(30);
    }
}

/// One connection, from hello to close. False when the host forgot this computer.
async fn session(peers: &Peers, saved: &Saved, ws: Ws) -> bool {
    let (mut sink, mut stream) = ws.split();
    let hello = Up::Hello {
        token: saved.token.clone(),
        device: host_name(),
        protocol: PROTOCOL,
        app: env!("CARGO_PKG_VERSION").into(),
        computer: true,
    };
    if sink.send(text(&hello)).await.is_err() {
        peers.offline(&saved.id, Some(format!("Can't reach {}.", saved.name)));
        return true;
    }
    let version = match timeout(DIAL, stream.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<Down>(&t) {
            Ok(Down::Welcome { version, .. }) => version,
            Ok(Down::Denied { forget: true, .. }) => {
                peers.forget(&saved.id);
                return false;
            }
            Ok(Down::Denied { message, .. }) => {
                peers.offline(&saved.id, Some(message));
                return true;
            }
            _ => String::new(),
        },
        _ => {
            peers.offline(&saved.id, Some(format!("Can't reach {}.", saved.name)));
            return true;
        }
    };
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    peers.online(&saved.id, tx, version);
    let mut unsqueeze = Unsqueeze::default();
    let mut ping = tokio::time::interval(Duration::from_secs(20));
    let mut forgotten = false;
    loop {
        tokio::select! {
            msg = stream.next() => match msg {
                Some(Ok(Message::Text(t))) => {
                    if let Ok(down) = serde_json::from_str::<Down>(&t) {
                        forgotten = matches!(down, Down::Denied { forget: true, .. });
                        peers.heard(&saved.id, down);
                        if forgotten {
                            break;
                        }
                    }
                }
                Some(Ok(Message::Binary(b))) => {
                    if let Some((OUTPUT, thread, squeezed)) = unframe(&b)
                        && let Some(bytes) = unsqueeze.run(squeezed)
                    {
                        peers.output(&saved.id, thread, bytes);
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            out = rx.next() => match out {
                Some(msg) => {
                    if sink.send(msg).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
            _ = ping.tick() => {
                if sink.send(text(&Up::Ping)).await.is_err() {
                    break;
                }
            }
        }
    }
    let _ = sink.close().await;
    peers.offline(&saved.id, None);
    !forgotten
}

fn text(up: &Up) -> Message {
    Message::Text(serde_json::to_string(up).unwrap_or_default().into())
}

/// Pairs from a host's link: `hyprspace://pair?n=&h=&p=&f=&c=`. The code never travels; each
/// side proves it knows the code over the certificate the client actually got.
pub(super) async fn pair(link: &str) -> Result<(Saved, String), String> {
    let bad = || "That isn't a pairing link from HyprSpace.".to_string();
    let query = link
        .trim()
        .strip_prefix("hyprspace://pair?")
        .ok_or_else(bad)?;
    let fields: HashMap<&str, String> = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k, decode(v)))
        .collect();
    let get = |k: &str| fields.get(k).cloned().filter(|v| !v.is_empty());
    let fingerprint = get("f").ok_or_else(bad)?;
    let code: String = get("c")
        .ok_or_else(bad)?
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let code = if code.len() == 14 && code.matches('-').count() == 2 {
        code.replace('-', "").to_ascii_uppercase()
    } else {
        code
    };
    let saved = Saved {
        id: fingerprint.clone(),
        name: get("n").unwrap_or_else(|| "Computer".into()),
        hosts: get("h")
            .ok_or_else(bad)?
            .split(',')
            .map(str::to_string)
            .collect(),
        port: get("p").and_then(|p| p.parse().ok()).ok_or_else(bad)?,
        token: String::new(),
    };
    let ws = dial(&saved).await?;
    let (mut sink, mut stream) = ws.split();
    let ask = Up::Pair {
        proof: prove(&code, &fingerprint),
        device: host_name(),
        protocol: PROTOCOL,
        app: env!("CARGO_PKG_VERSION").into(),
        computer: true,
    };
    sink.send(text(&ask))
        .await
        .map_err(|_| format!("Can't reach {}.", saved.name))?;
    let reply = timeout(DIAL, stream.next()).await;
    let _ = sink.close().await;
    match reply {
        Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<Down>(&t) {
            Ok(Down::Welcome {
                desktop,
                version,
                token: Some(token),
                proof: Some(proof),
            }) if proof == prove(&code, &format!("desktop {fingerprint}")) => Ok((
                Saved {
                    name: desktop,
                    token,
                    ..saved
                },
                version,
            )),
            Ok(Down::Denied { message, .. }) => Err(message),
            _ => Err("That computer didn't answer the way HyprSpace does.".into()),
        },
        _ => Err(format!("{} didn't answer.", saved.name)),
    }
}

/// The first of the host's addresses that answers, each tried a quarter second after the last.
async fn dial(saved: &Saved) -> Result<Ws, String> {
    let tls = TlsConnector::from(config(&saved.id));
    let port = saved.port;
    let tries = saved.hosts.iter().enumerate().map(|(i, host)| {
        let (tls, host) = (tls.clone(), host.clone());
        async move {
            tokio::time::sleep(Duration::from_millis(250 * i as u64)).await;
            connect(tls, &host, port).await
        }
        .boxed()
    });
    let unreachable = format!("Can't reach {}.", saved.name);
    if saved.hosts.is_empty() {
        return Err(unreachable);
    }
    match timeout(DIAL, futures::future::select_ok(tries)).await {
        Ok(Ok((ws, _))) => Ok(ws),
        _ => Err(unreachable),
    }
}

async fn connect(tls: TlsConnector, host: &str, port: u16) -> Result<Ws, ()> {
    let ip: std::net::IpAddr = host.parse().map_err(|_| ())?;
    let tcp = TcpStream::connect((ip, port)).await.map_err(|_| ())?;
    let _ = tcp.set_nodelay(true);
    let name = ServerName::try_from("hyprspace").map_err(|_| ())?;
    let stream = tls.connect(name, tcp).await.map_err(|_| ())?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let (ws, _) =
        tokio_tungstenite::client_async_with_config("wss://hyprspace/", stream, Some(config))
            .await
            .map_err(|_| ())?;
    Ok(ws)
}

fn config(fingerprint: &str) -> Arc<ClientConfig> {
    let provider = Arc::new(tokio_rustls::rustls::crypto::aws_lc_rs::default_provider());
    let verifier = Arc::new(Pinned {
        fingerprint: fingerprint.to_string(),
        provider: provider.clone(),
    });
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("the default protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Arc::new(config)
}

/// Trusts exactly the certificate the pairing named. No authority vouches for a host's
/// certificate, so its fingerprint is what proves who it is.
#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        if URL_SAFE_NO_PAD.encode(Sha256::digest(end.as_ref())) == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(Error::General("not the paired computer".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Hosts announcing themselves on the local network, reported as they turn up.
pub(super) async fn look(peers: Peers) {
    let _ = tokio::task::spawn_blocking(move || {
        let Ok(daemon) = mdns_sd::ServiceDaemon::new() else {
            return;
        };
        let Ok(events) = daemon.browse(crate::phone::SERVICE) else {
            return;
        };
        let mut seen: HashMap<String, Found> = HashMap::new();
        // the task is aborted when Settings stops looking; the receiver closing ends it too
        while let Ok(e) = events.recv() {
            if let Some(f) = resolved(e) {
                seen.insert(f.fingerprint.clone(), f);
                peers.found(seen.values().cloned().collect());
            }
        }
        let _ = daemon.shutdown();
    })
    .await;
}

/// Looks for one host by its fingerprint for a little while.
async fn find(fingerprint: &str, wait: Duration) -> Option<Found> {
    let fingerprint = fingerprint.to_string();
    tokio::task::spawn_blocking(move || {
        let daemon = mdns_sd::ServiceDaemon::new().ok()?;
        let events = daemon.browse(crate::phone::SERVICE).ok()?;
        let until = std::time::Instant::now() + wait;
        let mut out = None;
        while let Some(left) = until.checked_duration_since(std::time::Instant::now()) {
            match events.recv_timeout(left) {
                Ok(e) => {
                    if let Some(f) = resolved(e).filter(|f| f.fingerprint == fingerprint) {
                        out = Some(f);
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = daemon.shutdown();
        out
    })
    .await
    .ok()
    .flatten()
}

fn resolved(e: mdns_sd::ServiceEvent) -> Option<Found> {
    let mdns_sd::ServiceEvent::ServiceResolved(info) = e else {
        return None;
    };
    let fingerprint = info.txt_properties.get_property_val_str("f")?.to_string();
    let name = info
        .txt_properties
        .get_property_val_str("n")
        .unwrap_or("Computer")
        .to_string();
    let mut hosts: Vec<String> = info
        .get_addresses_v4()
        .iter()
        .map(|ip| ip.to_string())
        .collect();
    hosts.sort();
    Some(Found {
        name,
        hosts,
        port: info.get_port(),
        fingerprint,
    })
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(b) => {
                    out.push(b);
                    i += 3;
                    continue;
                }
                Err(_) => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_fields_decode() {
        assert_eq!(decode("Ash%27s%20PC"), "Ash's PC");
        assert_eq!(decode("100.64.1.2,192.168.1.5"), "100.64.1.2,192.168.1.5");
        assert_eq!(decode("50%"), "50%");
    }
}
