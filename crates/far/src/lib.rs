//! Reaching a computer from any network: iroh dials it by its key, straight through both routers
//! when it can and through n0's public relays when it can't. The desktop's engine and the phone
//! app's native library both dial through here. docs/internals/machines.md has the reasons.

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use iroh::endpoint::{Connection, RecvStream, SendStream, presets};
use iroh::{Endpoint, EndpointId, SecretKey};
use tokio::io::{AsyncRead, AsyncWrite, Join, ReadBuf};
use tokio::sync::OnceCell;

pub const ALPN: &[u8] = b"hyprspace/bridge/1";

/// An endpoint that answers dials, under `key`.
pub async fn host(key: SecretKey) -> Result<Endpoint, String> {
    Endpoint::builder(presets::N0)
        .secret_key(key)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(|e| format!("Couldn't reach n0's relays: {e}"))
}

static CLIENT: OnceCell<Endpoint> = OnceCell::const_new();

/// A stream to the computer whose key is `id`, once iroh finds a way there. The caller bounds
/// how long that takes.
pub async fn dial(id: &str) -> Option<Stream> {
    let id: EndpointId = id.parse().ok()?;
    let endpoint = CLIENT
        .get_or_try_init(|| Endpoint::bind(presets::N0))
        .await
        .ok()?;
    loop {
        // a computer that just started listening takes a few seconds to show up in the lookup
        if let Ok(conn) = endpoint.connect(id, ALPN).await {
            let (send, recv) = conn.open_bi().await.ok()?;
            return Some(Stream::new(conn, send, recv));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// One QUIC stream, read and written like a TCP connection. It holds the connection so the
/// connection lives as long as the stream does.
pub struct Stream {
    io: Join<RecvStream, SendStream>,
    _conn: Connection,
}

impl Stream {
    pub fn new(conn: Connection, send: SendStream, recv: RecvStream) -> Self {
        Stream {
            io: tokio::io::join(recv, send),
            _conn: conn,
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}
