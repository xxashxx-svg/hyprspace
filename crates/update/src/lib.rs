//! Self-update, compatible with the Tauri app's release pipeline on purpose.
//!
//! CI publishes `latest.json` on each GitHub release (`scripts/ci-build-latest.mjs`) and signs
//! every installer with the minisign key whose public half the Tauri app shipped in its
//! `src-tauri/tauri.conf.json` (see the v0.21.1 tag). The Tauri app updates from that same file
//! and key, and so does this crate: one feed, one key, so the last Tauri version can install the
//! first GPUI release and the GPUI app keeps updating from the same place afterwards
//! (docs/internals/updates.md).
//!
//! Flow: [`check`] reads the feed and returns an [`Update`] when it's newer, [`download`] fetches
//! the artifact and refuses it unless the signature verifies, [`stage`] writes it to disk, then
//! [`install`] hands it to the OS and the caller quits. Only a copy the installer put in place
//! ([`installed`]) replaces itself; a build folder never does.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use base64::Engine as _;
use futures::StreamExt as _;
use serde::Deserialize;

/// The feed both apps read. `latest` redirects to the newest published release.
///
/// A test build can point at a local feed by setting `HYPRSPACE_UPDATE_FEED` while it compiles.
/// It is read at build time, never at run time, so a shipped build can't be redirected.
pub const ENDPOINT: &str = match option_env!("HYPRSPACE_UPDATE_FEED") {
    Some(feed) => feed,
    None => "https://github.com/xxashxx-svg/hyprspace/releases/latest/download/latest.json",
};

/// The updater's public key, byte for byte the `plugins.updater.pubkey` in the Tauri app's
/// `src-tauri/tauri.conf.json` (v0.21.1): base64 of a minisign public key file. A test build swaps
/// in a test key with `HYPRSPACE_UPDATE_PUBKEY` at build time, like [`ENDPOINT`].
pub const PUBKEY: &str = match option_env!("HYPRSPACE_UPDATE_PUBKEY") {
    Some(key) => key,
    None => {
        "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEIxNDU5OTExNkNENjQ3MTAKUldRUVI5WnNFWmxGc1pzSjU3SEJWMWo3ODM5T0xQREZVT3FCdktJSTVtSkNBVlpCRDJ0SnBFNHMK"
    }
};

/// The arguments the Tauri app's updater passes the NSIS installer (tauri-plugin-updater 2.10,
/// install mode "passive"): a progress bar and no questions, restart the app when done, and
/// update mode, which keeps the user's shortcuts as they are. This crate passes the same ones,
/// so the installer only ever has one way of being run by an updater.
pub const INSTALLER_ARGS: [&str; 4] = ["/P", "/R", "/UPDATE", "/ARGS"];

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
    check_feed(ENDPOINT, current, platform).await
}

/// [`check`] against any feed, for tests.
pub async fn check_feed(
    url: &str,
    current: &str,
    platform: &str,
) -> anyhow::Result<Option<Update>> {
    let feed = reqwest::get(url).await?.error_for_status()?.text().await?;
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
    progress: impl FnMut(u64, Option<u64>),
) -> anyhow::Result<Vec<u8>> {
    download_with(PUBKEY, update, progress).await
}

/// [`download`] verified against any key, for tests.
pub async fn download_with(
    pubkey: &str,
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
    verify_with(pubkey, &bytes, &update.signature)?;
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

/// A folder an updater left in the temp folder: ours from [`stage`], or the Tauri app's
/// (`HyprSpace-<version>-updater-*`), which it never removes on Windows because it exits straight
/// after starting the installer.
fn leftover(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("hyprspace-update-")
        || (name.starts_with("hyprspace-") && name.contains("-updater-"))
}

/// Remove the installers earlier updates left in `temp`. Returns how many are still there, such
/// as the installer that just ran and hasn't exited yet.
pub fn sweep(temp: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(temp) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_str().is_some_and(leftover))
        .filter(|e| std::fs::remove_dir_all(e.path()).is_err())
        .count()
}

/// How this copy of the app was put on disk, when it can replace itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Install {
    /// Windows: the folder the NSIS installer wrote, with its `uninstall.exe` beside the app.
    Windows { dir: PathBuf },
    /// macOS: the `.app` bundle the app runs from.
    Mac { bundle: PathBuf },
}

/// This copy's install, or None for a build folder (`cargo run`) that must never be replaced
/// by a release.
pub fn installed() -> Option<Install> {
    install_of(&std::env::current_exe().ok()?)
}

/// [`installed`] for any executable path.
pub fn install_of(exe: &Path) -> Option<Install> {
    if cfg!(windows) {
        let dir = exe.parent()?;
        dir.join("uninstall.exe")
            .is_file()
            .then(|| Install::Windows { dir: dir.into() })
    } else if cfg!(target_os = "macos") {
        // <Name>.app/Contents/MacOS/<binary>
        let bundle = exe.parent()?.parent()?.parent()?;
        let shaped = exe.parent()?.ends_with("Contents/MacOS")
            && bundle.extension().is_some_and(|x| x == "app");
        shaped.then(|| Install::Mac {
            bundle: bundle.into(),
        })
    } else {
        None
    }
}

/// Hand a staged update to the OS. The caller quits right after: the installer waits for the
/// app to exit, and the relaunch waits for this process.
///
/// Windows: runs the NSIS installer with [`INSTALLER_ARGS`]; it installs over the old app and
/// starts the new one. macOS: unpacks the `.app.tar.gz` over the running bundle, then opens the
/// new bundle once this process is gone.
pub fn install(staged: &Path, install: &Install) -> anyhow::Result<()> {
    match install {
        Install::Windows { .. } => {
            std::process::Command::new(staged)
                .args(INSTALLER_ARGS)
                .spawn()
                .context("couldn't start the installer")?;
            Ok(())
        }
        Install::Mac { bundle } => {
            swap_bundle(&std::fs::read(staged)?, bundle)?;
            relaunch_after_exit(bundle);
            Ok(())
        }
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

/// Open `bundle` once this process has exited, from a detached shell that polls for our pid.
/// Adapted from zeron's `relaunch_after_exit` (crates/update/src/lib.rs).
#[cfg(unix)]
fn relaunch_after_exit(bundle: &Path) {
    use std::os::unix::process::CommandExt as _;
    let script =
        r#"while /bin/kill -0 "$1" 2>/dev/null; do sleep 0.2; done; exec /usr/bin/open "$2""#;
    let _ = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .arg("hyprspace-relaunch")
        .arg(std::process::id().to_string())
        .arg(bundle)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn();
}

#[cfg(not(unix))]
fn relaunch_after_exit(_: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_takes_only_updater_leftovers() {
        let temp = tempfile::tempdir().unwrap();
        let dir = |name: &str| {
            let d = temp.path().join(name);
            std::fs::create_dir(&d).unwrap();
            std::fs::write(d.join("setup.exe"), "x").unwrap();
            d
        };
        let ours = dir("hyprspace-update-a1b2");
        let tauri = dir("HyprSpace-0.21.1-updater-x9y8");
        let other = dir("hyprspace-notes");
        let tool = dir("SomeApp-1.0-updater-z");
        std::fs::write(temp.path().join("hyprspace-update-file"), "x").unwrap();

        assert_eq!(sweep(temp.path()), 0);
        assert!(!ours.exists() && !tauri.exists());
        assert!(other.exists() && tool.exists());
        assert!(temp.path().join("hyprspace-update-file").exists());
        assert_eq!(sweep(&temp.path().join("missing")), 0);
    }

    // A running installer can't be deleted on Windows; the sweep reports it so the caller retries.
    #[cfg(windows)]
    #[test]
    fn sweep_counts_what_it_could_not_remove() {
        use std::os::windows::fs::OpenOptionsExt as _;
        let temp = tempfile::tempdir().unwrap();
        let d = temp.path().join("hyprspace-update-busy");
        std::fs::create_dir(&d).unwrap();
        // no FILE_SHARE_DELETE, the way a running .exe is held
        let held = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .open(d.join("setup.exe"))
            .unwrap();
        assert_eq!(sweep(temp.path()), 1);
        drop(held);
        assert_eq!(sweep(temp.path()), 0);
        assert!(!d.exists());
    }

    #[test]
    fn only_an_installed_copy_replaces_itself() {
        let dir = tempfile::tempdir().unwrap();
        if cfg!(windows) {
            let exe = dir.path().join("hyprspace.exe");
            assert_eq!(install_of(&exe), None);
            std::fs::write(dir.path().join("uninstall.exe"), "").unwrap();
            assert_eq!(
                install_of(&exe),
                Some(Install::Windows {
                    dir: dir.path().into()
                })
            );
        } else if cfg!(target_os = "macos") {
            let bundle = dir.path().join("HyprSpace.app");
            let exe = bundle.join("Contents/MacOS/hyprspace");
            assert_eq!(
                install_of(&exe),
                Some(Install::Mac {
                    bundle: bundle.clone()
                })
            );
            assert_eq!(
                install_of(&dir.path().join("target/release/hyprspace")),
                None
            );
        }
    }
}
