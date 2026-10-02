use std::io::Cursor;

use base64::Engine as _;
use hyprspace_update::{Update, newer, parse, swap_bundle, verify, verify_with};

const FEED: &str = include_str!("fixtures/latest-0.21.1.json");

/// A fresh key pair and a signature over `data`, both encoded the way Tauri's CI encodes them:
/// base64 of the minisign file text.
fn signed(data: &[u8]) -> (String, String) {
    let b64 = base64::engine::general_purpose::STANDARD;
    let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
    let pk = kp.pk.to_box().unwrap().to_string();
    let sig = minisign::sign(Some(&kp.pk), &kp.sk, Cursor::new(data), None, None)
        .unwrap()
        .to_string();
    (b64.encode(pk), b64.encode(sig))
}

#[test]
fn compares_versions() {
    assert!(newer("0.21.1", "0.21.2"));
    assert!(newer("0.21.1", "v0.22.0"));
    assert!(newer("0.21.1", "1.0.0"));
    assert!(!newer("0.21.1", "0.21.1"));
    assert!(!newer("0.21.1", "0.20.9"));
    assert!(!newer("0.21.1", "not a version"));
}

#[test]
fn reads_the_real_feed() {
    let up = parse(FEED, "0.21.0", "windows-x86_64").unwrap().unwrap();
    assert_eq!(up.version, "0.21.1");
    assert!(up.url.ends_with("HyprSpace_0.21.1_x64-setup.exe"));
    assert!(
        parse(FEED, "0.21.0", "darwin-aarch64")
            .unwrap()
            .unwrap()
            .url
            .ends_with(".app.tar.gz")
    );
    // already current, or a platform the release doesn't carry: nothing to do
    assert_eq!(parse(FEED, "0.21.1", "windows-x86_64").unwrap(), None);
    assert_eq!(parse(FEED, "0.21.0", "plan9-mips").unwrap(), None);
}

#[test]
fn accepts_a_good_signature_and_refuses_a_tampered_file() {
    let data = b"pretend installer";
    let (pk, sig) = signed(data);
    verify_with(&pk, data, &sig).unwrap();
    assert!(verify_with(&pk, b"pretend installer, with a virus", &sig).is_err());
    // signed by someone else
    let (other_pk, _) = signed(data);
    assert!(verify_with(&other_pk, data, &sig).is_err());
}

#[test]
fn swaps_a_mac_bundle_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("HyprSpace.app");
    std::fs::create_dir_all(bundle.join("Contents")).unwrap();
    std::fs::write(bundle.join("Contents/version"), "old").unwrap();

    // the release's .app.tar.gz: one top-level HyprSpace.app
    let mut tarball = Vec::new();
    {
        let gz = flate2::write::GzEncoder::new(&mut tarball, flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        let mut h = tar::Header::new_gnu();
        h.set_size(3);
        h.set_mode(0o644);
        h.set_cksum();
        tar.append_data(&mut h, "HyprSpace.app/Contents/version", &b"new"[..])
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }

    swap_bundle(&tarball, &bundle).unwrap();
    assert_eq!(
        std::fs::read_to_string(bundle.join("Contents/version")).unwrap(),
        "new"
    );
    // nothing left over beside it
    let left: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left, vec![std::ffi::OsString::from("HyprSpace.app")]);
}

/// The real thing: the v0.21.1 Windows installer from GitHub, its signature from the real feed,
/// and HyprSpace's real key. Proves this crate accepts exactly what CI signs.
/// `cargo test -p hyprspace-update -- --ignored` (downloads about 10 MB).
#[tokio::test]
#[ignore = "network"]
async fn verifies_a_real_release() {
    let up: Update = parse(FEED, "0.21.0", "windows-x86_64").unwrap().unwrap();
    let bytes = reqwest::get(&up.url).await.unwrap().bytes().await.unwrap();
    verify(&bytes, &up.signature).unwrap();
    let mut flipped = bytes.to_vec();
    flipped[1000] ^= 1;
    assert!(verify(&flipped, &up.signature).is_err());
}
