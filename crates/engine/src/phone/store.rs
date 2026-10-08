// What the bridge keeps between runs, in phone.json beside the saved state: the certificate the
// phones pin, the port they know, and each paired phone with a hash of its token. A token is
// shown once, to the phone that paired; the file only ever holds its SHA-256.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hyprspace_proto::phone::Device;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The port a bridge tries first, so a phone that paired once finds it again.
pub const PORT: u16 = 47821;

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Saved {
    cert: String,
    key: String,
    port: u16,
    devices: Vec<Saved1>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved1 {
    id: String,
    name: String,
    token: String,
    paired: u64,
    seen: u64,
    #[serde(default)]
    app: String,
}

pub struct Store {
    path: PathBuf,
    saved: Saved,
}

/// The certificate and its key, as rustls takes them.
pub struct Identity {
    pub cert: Vec<u8>,
    pub key: Vec<u8>,
    /// SHA-256 of the certificate, base64url: what the phone pins.
    pub fingerprint: String,
}

pub fn random(bytes: usize) -> Vec<u8> {
    let mut b = vec![0; bytes];
    getrandom::fill(&mut b).expect("the OS has no random numbers");
    b
}

fn hash(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

impl Store {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("phone.json");
        let saved = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, saved }
    }

    fn save(&self) {
        let Ok(json) = serde_json::to_vec_pretty(&self.saved) else {
            return;
        };
        let tmp = self.path.with_extension("json.tmp");
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }

    /// The certificate, made on first use. It never expires, so a paired phone keeps trusting it.
    pub fn identity(&mut self) -> anyhow::Result<Identity> {
        if self.saved.cert.is_empty() || self.saved.key.is_empty() {
            let made = rcgen::generate_simple_self_signed(vec!["hyprspace".to_string()])?;
            self.saved.cert = made.cert.pem();
            self.saved.key = made.signing_key.serialize_pem();
            self.save();
        }
        let cert = pem_der(&self.saved.cert, "CERTIFICATE")?;
        let key = pem_der(&self.saved.key, "PRIVATE KEY")?;
        let fingerprint = URL_SAFE_NO_PAD.encode(Sha256::digest(&cert));
        Ok(Identity {
            cert,
            key,
            fingerprint,
        })
    }

    pub fn port(&self) -> u16 {
        match self.saved.port {
            0 => PORT,
            p => p,
        }
    }

    pub fn set_port(&mut self, port: u16) {
        if self.saved.port != port {
            self.saved.port = port;
            self.save();
        }
    }

    /// Pairs a phone and returns its id and the token it keeps.
    pub fn add(&mut self, name: &str, app: &str, now: u64) -> (String, String) {
        let id = URL_SAFE_NO_PAD.encode(random(9));
        let token = URL_SAFE_NO_PAD.encode(random(32));
        self.saved.devices.push(Saved1 {
            id: id.clone(),
            name: name.to_string(),
            token: hash(&token),
            paired: now,
            seen: now,
            app: app.to_string(),
        });
        self.save();
        (id, token)
    }

    /// The paired phone holding `token`, marked seen now, with its saved name updated.
    pub fn check(&mut self, token: &str, name: &str, app: &str, now: u64) -> Option<String> {
        let h = hash(token);
        let d = self.saved.devices.iter_mut().find(|d| d.token == h)?;
        d.seen = now;
        d.app = app.to_string();
        if !name.is_empty() {
            d.name = name.to_string();
        }
        let id = d.id.clone();
        self.save();
        Some(id)
    }

    pub fn forget(&mut self, id: &str) {
        self.saved.devices.retain(|d| d.id != id);
        self.save();
    }

    pub fn devices(&self, online: impl Fn(&str) -> bool) -> Vec<Device> {
        self.saved
            .devices
            .iter()
            .map(|d| Device {
                id: d.id.clone(),
                name: d.name.clone(),
                paired: d.paired,
                seen: d.seen,
                online: online(&d.id),
                app: d.app.clone(),
            })
            .collect()
    }
}

/// The DER inside one PEM block.
fn pem_der(pem: &str, label: &str) -> anyhow::Result<Vec<u8>> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let body = pem
        .split_once(&begin)
        .and_then(|(_, rest)| rest.split_once(&end))
        .map(|(b, _)| b)
        .ok_or_else(|| anyhow::anyhow!("no {label} in the saved certificate"))?;
    let b64: String = body.split_whitespace().collect();
    Ok(base64::engine::general_purpose::STANDARD.decode(b64)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_works_until_its_phone_is_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::load(dir.path());
        let (id, token) = s.add("Pixel", "0.24.4", 1);
        assert!(
            !std::fs::read_to_string(dir.path().join("phone.json"))
                .unwrap()
                .contains(&token)
        );
        let mut s = Store::load(dir.path());
        assert_eq!(s.check(&token, "Pixel 9", "0.25.0", 2), Some(id.clone()));
        assert_eq!(s.check("nope", "", "", 2), None);
        assert_eq!(s.devices(|_| false)[0].app, "0.25.0");
        assert_eq!(s.devices(|_| false)[0].name, "Pixel 9");
        s.forget(&id);
        assert_eq!(s.check(&token, "", "", 3), None);
    }

    #[test]
    fn the_certificate_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let a = Store::load(dir.path()).identity().unwrap();
        let b = Store::load(dir.path()).identity().unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.cert, b.cert);
        assert_eq!(a.fingerprint.len(), 43);
    }
}
