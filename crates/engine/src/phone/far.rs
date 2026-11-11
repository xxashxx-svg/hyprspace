// Reaching a computer from any network: iroh dials it by its key, straight through both routers
// when it can and through n0's public relays when it can't. The bridge's own TLS runs inside the
// QUIC stream, so a relay passes along bytes it can't read and the pinned certificate still
// decides which computer answered.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use iroh::endpoint::{Connection, presets};
use iroh::{Endpoint, EndpointId, SecretKey};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite, Join, ReadBuf};
use tokio::sync::{OnceCell, Semaphore};
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

use super::Phone;
use super::server::{self, HANDSHAKE, WAITING};
use super::store::Identity;

const ALPN: &[u8] = b"hyprspace/bridge/1";

/// The computer's key, made from its certificate's so it stays the same as long as pairings do.
pub fn key(id: &Identity) -> SecretKey {
    let seed: [u8; 32] = Sha256::new()
        .chain_update(b"hyprspace far")
        .chain_update(&id.key)
        .finalize()
        .into();
    SecretKey::from_bytes(&seed)
}

pub async fn listen(phone: Phone, key: SecretKey, tls: TlsAcceptor) -> Result<(), String> {
    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(key)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(|e| format!("Couldn't reach n0's relays: {e}"))?;
    let waiting = Arc::new(Semaphore::new(WAITING));
    while let Some(incoming) = endpoint.accept().await {
        let Ok(permit) = waiting.clone().try_acquire_owned() else {
            continue;
        };
        let (phone, tls) = (phone.clone(), tls.clone());
        tokio::spawn(async move {
            let Ok(Ok(conn)) = timeout(HANDSHAKE, incoming).await else {
                return;
            };
            let Ok(Ok((send, recv))) = timeout(HANDSHAKE, conn.accept_bi()).await else {
                return;
            };
            server::serve(phone, Stream::new(conn, send, recv), tls, permit).await;
        });
    }
    Ok(())
}

static CLIENT: OnceCell<Endpoint> = OnceCell::const_new();

/// A stream to the computer whose key is `id`, once iroh finds a way there. The caller bounds
/// how long that takes.
pub async fn dial(id: &str) -> Result<Stream, ()> {
    let id: EndpointId = id.parse().map_err(|_| ())?;
    let endpoint = CLIENT
        .get_or_try_init(|| Endpoint::bind(presets::N0))
        .await
        .map_err(|_| ())?;
    loop {
        // a computer that just started listening takes a few seconds to show up in the lookup
        if let Ok(conn) = endpoint.connect(id, ALPN).await {
            let (send, recv) = conn.open_bi().await.map_err(|_| ())?;
            return Ok(Stream::new(conn, send, recv));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// One QUIC stream, read and written like a TCP connection. It holds the connection so the
/// connection lives as long as the stream does.
pub struct Stream {
    io: Join<iroh::endpoint::RecvStream, iroh::endpoint::SendStream>,
    _conn: Connection,
}

impl Stream {
    fn new(
        conn: Connection,
        send: iroh::endpoint::SendStream,
        recv: iroh::endpoint::RecvStream,
    ) -> Self {
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
