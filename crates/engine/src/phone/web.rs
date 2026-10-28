use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

use super::Phone;
use super::server;

const HEAD_MAX: usize = 16 << 10;

macro_rules! page {
    ($($name:literal),*) => {
        &[$((concat!("/", $name), include_bytes!(concat!("../../web/", $name)) as &[u8])),*]
    };
}

macro_rules! ui {
    ($($name:literal),*) => {
        &[$((concat!("/", $name), include_bytes!(concat!("../../../ui/assets/", $name)) as &[u8])),*]
    };
}

const PAGE: &[(&str, &[u8])] = page!(
    "index.html",
    "style.css",
    "app.js",
    "link.js",
    "board.js",
    "chat.js",
    "transcript.js",
    "markdown.js",
    "term.js",
    "dom.js",
    "icons.js"
);

const ASSETS: &[(&str, &[u8])] = ui!(
    "fonts/Geist.ttf",
    "fonts/Geist-Medium.ttf",
    "fonts/Geist-SemiBold.ttf",
    "fonts/GeistMono.ttf",
    "fonts/JetBrainsMonoNerdFontMono-Regular.ttf",
    "brand/claude.svg",
    "brand/openai.svg",
    "icons/plus.svg",
    "icons/search.svg",
    "icons/git-branch.svg",
    "icons/folder.svg",
    "icons/list-checks.svg",
    "icons/hand.svg",
    "icons/file-pen-line.svg",
    "icons/lock-open.svg",
    "icons/arrow-up.svg",
    "icons/square.svg",
    "icons/chevron-right.svg",
    "icons/x.svg",
    "icons/copy.svg",
    "icons/check.svg",
    "icons/terminal.svg",
    "icons/paperclip.svg",
    "icons/circle-alert.svg",
    "icons/pencil.svg",
    "icons/gauge.svg",
    "icons/bot.svg",
    "icons/zap.svg",
    "icons/message-square.svg"
);

fn find(path: &str) -> Option<(&'static str, &'static [u8], bool)> {
    let path = if path == "/" { "/index.html" } else { path };
    if let Some((_, body)) = PAGE.iter().find(|(p, _)| *p == path) {
        let kind = match path.rsplit('.').next() {
            Some("html") => "text/html; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            _ => "text/javascript; charset=utf-8",
        };
        return Some((kind, body, false));
    }
    let (_, body) = ASSETS.iter().find(|(p, _)| *p == path)?;
    let kind = if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "font/ttf"
    };
    Some((kind, body, true))
}

pub async fn listen(phone: Phone, listener: TcpListener) {
    let waiting = Arc::new(Semaphore::new(server::WAITING));
    loop {
        let Ok((tcp, _)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        };
        let Ok(permit) = waiting.clone().try_acquire_owned() else {
            continue;
        };
        let _ = tcp.set_nodelay(true);
        tokio::spawn(handle(phone.clone(), tcp, permit, true));
    }
}

/// A request on either listener: a WebSocket for a phone or a browser, or one of the page's
/// files once the page is switched on.
pub(super) async fn handle<S>(phone: Phone, mut stream: S, permit: OwnedSemaphorePermit, web: bool)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Ok(Some(head)) = timeout(server::HANDSHAKE, read_head(&mut stream)).await else {
        return;
    };
    let text = String::from_utf8_lossy(&head).to_string();
    let mut first = text.lines().next().unwrap_or("").split_whitespace();
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let path = path.split('?').next().unwrap_or("");
    let upgrade = text.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("upgrade:") && l.contains("websocket")
    });
    if upgrade {
        let stream = Prefixed {
            head,
            at: 0,
            inner: stream,
        };
        server::accept(phone, stream, permit, web).await;
        return;
    }
    if let Some(token) = path.strip_prefix("/media/") {
        lend(&phone, &mut stream, method, token).await;
        return;
    }
    if !phone.serves_page() {
        return;
    }
    let reply = match find(path) {
        Some((kind, body, keep)) if method == "GET" || method == "HEAD" => {
            let body = if method == "HEAD" { &[][..] } else { body };
            response("200 OK", kind, body, keep)
        }
        Some(_) => response("405 Method Not Allowed", "text/plain", b"", false),
        None => response("404 Not Found", "text/plain", b"Not found", false),
    };
    let _ = stream.write_all(&reply).await;
    let _ = stream.shutdown().await;
}

/// A file lent to a phone by its token, streamed rather than read whole: a video can be large.
async fn lend<S: AsyncWrite + Unpin>(phone: &Phone, stream: &mut S, method: &str, token: &str) {
    let opened = match phone.lent(token) {
        Some(file) if method == "GET" => tokio::fs::File::open(&file).await.ok().map(|f| (file, f)),
        _ => None,
    };
    let Some((file, mut body)) = opened else {
        let _ = stream
            .write_all(&response(
                "404 Not Found",
                "text/plain",
                b"Not found",
                false,
            ))
            .await;
        let _ = stream.shutdown().await;
        return;
    };
    let len = body.metadata().await.map(|m| m.len()).unwrap_or(0);
    let kind = match file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        _ => "video/mp4",
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {len}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(head.as_bytes()).await.is_ok() {
        let _ = tokio::io::copy(&mut body, stream).await;
    }
    let _ = stream.shutdown().await;
}

fn response(status: &str, kind: &str, body: &[u8], keep: bool) -> Vec<u8> {
    let cache = if keep { "max-age=604800" } else { "no-cache" };
    let mut out = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: {cache}\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none'\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

async fn read_head<S: AsyncRead + Unpin>(stream: &mut S) -> Option<Vec<u8>> {
    let mut head = Vec::new();
    let mut buf = [0u8; 2048];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).await.ok()?;
        if n == 0 || head.len() + n > HEAD_MAX {
            return None;
        }
        head.extend_from_slice(&buf[..n]);
    }
    Some(head)
}

// the request head was already read off the socket, and the WebSocket handshake reads it again
struct Prefixed<S> {
    head: Vec<u8>,
    at: usize,
    inner: S,
}

impl<S: AsyncRead + Unpin> AsyncRead for Prefixed<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.at < self.head.len() {
            let at = self.at;
            let n = (self.head.len() - at).min(buf.remaining());
            buf.put_slice(&self.head[at..at + n]);
            self.at += n;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Prefixed<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
