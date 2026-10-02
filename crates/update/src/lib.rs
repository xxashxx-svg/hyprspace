//! Self-update, compatible with the Tauri app's release pipeline on purpose.
//!
//! CI publishes `latest.json` on each GitHub release (`scripts/ci-build-latest.mjs`) and signs
//! every installer with the minisign key whose public half lives in `src-tauri/tauri.conf.json`.
//! The Tauri app updates from that same file and key, and so does this crate: one feed, one key,
//! so the last Tauri version can install the first GPUI release and the GPUI app keeps updating
//! from the same place afterwards (docs/REWRITE.md, "Upgrading from the Tauri app").
//!
//! Flow: [`check`] reads the feed and returns an [`Update`] when it's newer, [`download`] fetches
//! the artifact and refuses it unless the signature verifies, then [`install`] hands it to the OS.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use base64::Engine as _;
use futures::StreamExt as _;
use serde::Deserialize;

/// The feed both apps read. `latest` redirects to the newest published release.
pub const ENDPOINT: &str =
    "https://github.com/xxashxx-svg/hyprspace/releases/latest/download/latest.json";

/// The updater's public key, byte for byte the `plugins.updater.pubkey` in
/// `src-tauri/tauri.conf.json`: base64 of a minisign public key file.
pub const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEIxNDU5OTExNkNENjQ3MTAKUldRUVI5WnNFWmxGc1pzSjU3SEJWMWo3ODM5T0xQREZVT3FCdktJSTVtSkNBVlpCRDJ0SnBFNHMK";

/// This build's key in the feed's `platforms` map. None on a platform we don't ship.
pub fn platform_key() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows-x86_64"),
        ("macos", "aarch64") => Some("darwin-aarch64"),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    platforms: std::collections::BTreeMap<String, Artifact>,
}

#[derive(Debug, Deserialize)]
struct Artifact {
    /// base64 of a minisign signature file
    signature: String,
    url: String,
}

/// A release newer than the running one, for this platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub signature: String,
}

/// Is `candidate` newer than `current`? Both are semver, with or without a leading `v`. A version
/// that doesn't parse is never newer, so a malformed feed can't trigger an install.
pub fn newer(current: &str, candidate: &str) -> bool {
    let parse = |v: &str| semver::Version::parse(v.trim().trim_start_matches('v')).ok();
    match (parse(current), parse(candidate)) {
        (Some(c), Some(n)) => n > c,
        _ => false,
    }
}

/// Read a feed and pick this platform's artifact if the feed is newer than `current`.
pub fn parse(feed: &str, current: &str, platform: &str) -> anyhow::Result<Option<Update>> {
    let m: Manifest = serde_json::from_str(feed).context("latest.json isn't valid")?;
    if !newer(current, &m.version) {
        return Ok(None);
    }
    // a release can exist before this platform's build is in; wait for it rather than error
    let Some(a) = m.platforms.get(platform) else {
        return Ok(None);
    };
    Ok(Some(Update {
        version: m.version,
        notes: m.notes,
        url: a.url.clone(),
        signature: a.signature.clone(),
    }))
}

/// Ask the feed whether there's a newer release for this machine.
pub async fn check(current: &str) -> anyhow::Result<Option<Update>> {
    let Some(platform) = platform_key() else {
        return Ok(None);
    };
    let feed = reqwest::get(ENDPOINT)
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse(&feed, current, platform)
}

/// Check `bytes` against a Tauri-style signature (base64 of a minisign signature file) and a
/// Tauri-style public key (base64 of a minisign public key file).
pub fn verify_with(pubkey: &str, bytes: &[u8], signature: &str) -> anyhow::Result<()> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let text = |s: &str, what: &str| -> anyhow::Result<String> {
        let raw = b64
            .decode(s.trim())
            .with_context(|| format!("the {what} isn't base64"))?;
        String::from_utf8(raw).with_context(|| format!("the {what} isn't text"))
    };
    let pk = minisign_verify::PublicKey::decode(&text(pubkey, "public key")?)
        .context("bad public key")?;
    let sig = minisign_verify::Signature::decode(&text(signature, "signature")?)
        .context("bad signature")?;
    pk.verify(bytes, &sig, false)
        .context("the download doesn't match its signature")?;
    Ok(())
}

/// [`verify_with`] against HyprSpace's own key.
pub fn verify(bytes: &[u8], signature: &str) -> anyhow::Result<()> {
    verify_with(PUBKEY, bytes, signature)
}

/// Download the update and return it only if its signature checks out. `progress` gets the bytes
/// so far and the total when the server sends one.
pub async fn download(
    update: &Update,
    mut progress: impl FnMut(u64, Option<u64>),
) -> anyhow::Result<Vec<u8>> {
    let res = reqwest::get(&update.url).await?.error_for_status()?;
    let total = res.content_length();
    let mut bytes = Vec::with_capacity(total.unwrap_or(0) as usize);
    let mut stream = res.bytes_stream();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
        progress(bytes.len() as u64, total);
    }
    verify(&bytes, &update.signature)?;
    Ok(bytes)
}

/// Write verified bytes to a fresh temp folder under the URL's file name, which the installers
/// rely on (the extension tells Windows it's an .exe).
pub fn stage(update: &Update, bytes: &[u8]) -> anyhow::Result<PathBuf> {
    let name = update
        .url
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("hyprspace-update");
    let dir = tempfile::Builder::new()
        .prefix("hyprspace-update-")
        .tempdir()?
        .keep();
    let path = dir.join(name);
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Hand a staged update to the OS. The caller quits right after so the files can be replaced.
///
/// Windows: runs the NSIS installer silently (`/S`); it installs per user over the old app.
/// macOS: unpacks the `.app.tar.gz` and swaps it in for `app_bundle`, the running bundle.
pub fn install(staged: &Path, app_bundle: Option<&Path>) -> anyhow::Result<()> {
    if cfg!(windows) {
        std::process::Command::new(staged)
            .arg("/S")
            .spawn()
            .context("couldn't start the installer")?;
        Ok(())
    } else if cfg!(target_os = "macos") {
        let bundle = app_bundle.context("need the running .app bundle to replace")?;
        swap_bundle(&std::fs::read(staged)?, bundle)
    } else {
        bail!("this platform doesn't self-update")
    }
}

/// Unpack an `.app.tar.gz` next to `bundle`, then swap it in: old aside, new in place, old gone.
/// The unpack happens beside the target so the final step is a rename on one volume.
pub fn swap_bundle(tarball: &[u8], bundle: &Path) -> anyhow::Result<()> {
    let parent = bundle.parent().context("the bundle has no parent folder")?;
    let staging = tempfile::Builder::new()
        .prefix(".hyprspace-new-")
        .tempdir_in(parent)?;
    tar::Archive::new(flate2::read::GzDecoder::new(tarball)).unpack(staging.path())?;
    // the tarball holds one top-level `<Name>.app`
    let fresh = std::fs::read_dir(staging.path())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "app"))
        .context("the update has no .app inside")?;
    let aside = parent.join(format!(
        ".{}.old",
        bundle.file_name().and_then(|n| n.to_str()).unwrap_or("app")
    ));
    let _ = std::fs::remove_dir_all(&aside);
    if bundle.exists() {
        std::fs::rename(bundle, &aside)?;
    }
    if let Err(e) = std::fs::rename(&fresh, bundle) {
        // put the old app back rather than leave nothing to launch
        let _ = std::fs::rename(&aside, bundle);
        return Err(e.into());
    }
    let _ = std::fs::remove_dir_all(&aside);
    Ok(())
}
