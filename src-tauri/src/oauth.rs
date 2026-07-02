// One-shot loopback HTTP server for the OAuth redirect. After Google sign-in, the browser is sent
// to http://localhost:8765/?code=...; we capture that request line and hand the code back to the
// frontend, which exchanges it for a session. Async + spawn_blocking so the UI never stalls, and a
// timeout so an abandoned sign-in can't pin the port forever.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

#[tauri::command]
pub async fn oauth_listen() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(listen_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn listen_blocking() -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:8765")
        .map_err(|e| format!("couldn't open sign-in port 8765: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;

    // wait for the browser to come back, but don't hang forever if the user bails
    let start = Instant::now();
    let mut stream = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() > Duration::from_secs(180) {
                    return Err("sign-in timed out".into());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    stream.set_nonblocking(false).ok();
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    // read until we've got the whole header block (\r\n\r\n), or hit a 64KB cap / EOF. a single
    // read can truncate the request; the 5s read timeout above still bounds each read so a stalled
    // client can't pin us here.
    let mut data: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break, // EOF
            Ok(n) => {
                data.extend_from_slice(&chunk[..n]);
                if data.windows(4).any(|w| w == b"\r\n\r\n") || data.len() >= 64 * 1024 {
                    break;
                }
            }
            // timeout/other: bail with what we have, or surface the error if we got nothing
            Err(e) => {
                if data.is_empty() {
                    return Err(e.to_string());
                }
                break;
            }
        }
    }
    let req = String::from_utf8_lossy(&data);
    // request line: "GET /?code=...&state=... HTTP/1.1" — take the target (2nd token)
    let target = req
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("")
        .to_string();

    let html = "<!doctype html><meta charset=utf-8><title>HyprSpace</title>\
<body style=\"font-family:system-ui,Segoe UI,sans-serif;background:#0a0a0a;color:#ededed;\
display:flex;align-items:center;justify-content:center;height:100vh;margin:0\">\
<div style=\"text-align:center\"><h2 style=\"font-weight:600;margin:0 0 8px\">Signed in to HyprSpace</h2>\
<p style=\"color:#9e9e9e;margin:0\">You can close this tab and head back to the app.</p></div>";
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();

    if target.is_empty() {
        return Err("no redirect captured".into());
    }
    Ok(target)
}
