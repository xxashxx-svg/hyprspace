// The bridge's side of iroh (hyprspace-far). Its own TLS runs inside the QUIC stream, so a relay
// passes along bytes it can't read and the pinned certificate still decides which computer
// answered.

use std::sync::Arc;

use iroh::SecretKey;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

use super::Phone;
use super::server::{self, HANDSHAKE, WAITING};
use super::store::Identity;

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
    let endpoint = hyprspace_far::host(key).await?;
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
            let stream = hyprspace_far::Stream::new(conn, send, recv);
            server::serve(phone, stream, tls, permit).await;
        });
    }
    Ok(())
}
