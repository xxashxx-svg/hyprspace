// The bridge's listener: TLS under the certificate the phone pinned when it paired, a WebSocket
// over it, one JSON message per text frame. A connection has ten seconds to say hello or pair,
// at most sixteen wait to at once, and one drops after a minute without a word (the phone pings
// every twenty seconds).

use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use hyprspace_proto::phone::{Down, Up};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::{Instant, timeout};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use super::Phone;
use super::store::Identity;

const HANDSHAKE: Duration = Duration::from_secs(10);
const WAITING: usize = 16;
const SILENCE: Duration = Duration::from_secs(60);
/// The biggest message a phone sends is a photo, shrunk on the phone first.
const MAX_MESSAGE: usize = 8 << 20;

pub fn acceptor(id: &Identity) -> anyhow::Result<TlsAcceptor> {
    let provider = Arc::new(tokio_rustls::rustls::crypto::aws_lc_rs::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(id.cert.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(id.key.clone())),
        )?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

pub async fn listen(phone: Phone, listener: TcpListener, tls: TlsAcceptor) {
    let waiting = Arc::new(Semaphore::new(WAITING));
    loop {
        let Ok((tcp, _)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        };
        let Ok(permit) = waiting.clone().try_acquire_owned() else {
            continue;
        };
        tokio::spawn(serve(phone.clone(), tcp, tls.clone(), permit));
    }
}

async fn serve(phone: Phone, tcp: TcpStream, tls: TlsAcceptor, permit: OwnedSemaphorePermit) {
    let _ = tcp.set_nodelay(true);
    let Ok(Ok(stream)) = timeout(HANDSHAKE, tls.accept(tcp)).await else {
        return;
    };
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let Ok(Ok(ws)) = timeout(
        HANDSHAKE,
        tokio_tungstenite::accept_async_with_config(stream, Some(config)),
    )
    .await
    else {
        return;
    };
    let (mut sink, mut stream) = ws.split();
    let first = match timeout(HANDSHAKE, stream.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str::<Up>(&t).ok(),
        _ => None,
    };
    let Some(first) = first else {
        return;
    };
    drop(permit);
    let (tx, mut rx) = mpsc::unbounded();
    let conn = match phone.admit(first, tx) {
        Ok((conn, welcome)) => {
            if send(&mut sink, &welcome).await.is_err() {
                phone.left(conn);
                return;
            }
            conn
        }
        Err(denied) => {
            let _ = send(&mut sink, &denied).await;
            let _ = sink.close().await;
            return;
        }
    };
    let mut heard = Instant::now();
    let mut every = tokio::time::interval(Duration::from_secs(15));
    loop {
        tokio::select! {
            msg = stream.next() => match msg {
                Some(Ok(Message::Text(t))) => {
                    heard = Instant::now();
                    if let Ok(up) = serde_json::from_str::<Up>(&t) {
                        phone.handle(conn, up);
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => heard = Instant::now(),
            },
            down = rx.next() => match down {
                Some(down) => {
                    if send(&mut sink, &down).await.is_err() {
                        break;
                    }
                }
                // the hub dropped this phone: it was forgotten, or the bridge went off
                None => break,
            },
            _ = every.tick() => {
                if heard.elapsed() > SILENCE || sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
    let _ = sink.close().await;
    phone.left(conn);
}

async fn send<S>(sink: &mut S, down: &Down) -> Result<(), ()>
where
    S: futures::Sink<Message> + Unpin,
{
    let json = serde_json::to_string(down).map_err(|_| ())?;
    sink.send(Message::Text(json.into())).await.map_err(|_| ())
}
