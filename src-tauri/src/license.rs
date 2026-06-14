use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Serialize;
use tauri::State;

use crate::persist::Store;

// Public half of the offline license keypair. The private key lives in
// ~/.hyprspace-signing/hyprspace-license.pem and is NEVER shipped. Regenerate the pair
// with scripts/gen-license-keypair.mjs (which invalidates every key already issued),
// mint keys with scripts/license-keygen.mjs.
const LICENSE_PUBKEY: &str = "64s7qDCG00hZyFFgBfbYS6J_Io--UiK9kofSkvo2L4I";

// Revoked keys: paste the payload segment (the part between "HSPACE-" and the ".") of any
// leaked key here and ship an update — it'll be rejected on next launch and re-activation.
const REVOKED: &[&str] = &[];

#[derive(Serialize, Clone)]
pub struct LicenseInfo {
    pub name: String,
    pub issued: String,
}

#[derive(serde::Deserialize)]
struct Payload {
    n: String,
    iss: String,
}

fn pubkey() -> Result<VerifyingKey, String> {
    let raw = URL_SAFE_NO_PAD
        .decode(LICENSE_PUBKEY)
        .map_err(|_| "bad embedded pubkey")?;
    let bytes: [u8; 32] = raw
        .as_slice()
        .try_into()
        .map_err(|_| "bad embedded pubkey length")?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| "bad embedded pubkey".into())
}

// Offline-verify a key shaped like  HSPACE-<payloadB64url>.<sigB64url>
// where the signature is over the ASCII bytes of <payloadB64url>.
fn verify(key: &str) -> Result<LicenseInfo, String> {
    let body = key.trim().strip_prefix("HSPACE-").ok_or("not a HyprSpace key")?;
    let (payload_b64, sig_b64) = body.split_once('.').ok_or("malformed key")?;

    if REVOKED.contains(&payload_b64) {
        return Err("this key has been revoked".into());
    }

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| "malformed key")?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "malformed key")?;
    let sig = Signature::from_bytes(&sig_arr);

    pubkey()?
        .verify(payload_b64.as_bytes(), &sig)
        .map_err(|_| "invalid license key")?;

    let json = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| "malformed key")?;
    let p: Payload = serde_json::from_slice(&json).map_err(|_| "malformed key")?;
    Ok(LicenseInfo {
        name: p.n,
        issued: p.iss,
    })
}

// Validate a key and, if good, persist it. Returns the license info for the UI.
#[tauri::command]
pub fn activate_license(store: State<Store>, key: String) -> Result<LicenseInfo, String> {
    let info = verify(&key)?;
    store.save("license", key.trim())?;
    Ok(info)
}

// None = not licensed (no key stored, or the stored key no longer verifies). The UI
// gates the whole app on this at launch.
#[tauri::command]
pub fn license_status(store: State<Store>) -> Result<Option<LicenseInfo>, String> {
    match store.load("license")? {
        Some(key) => Ok(verify(&key).ok()),
        None => Ok(None),
    }
}
