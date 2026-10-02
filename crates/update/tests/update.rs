use std::io::Cursor;

use base64::Engine as _;
use hyprspace_update::{
    Update, check_feed, download_with, newer, parse, stage, swap_bundle, verify, verify_with,
};

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

/// Serves the files `files(base)` lists over plain HTTP on a free local port until the test ends,
/// and returns the base URL. Just enough HTTP for reqwest: one request per connection, a
/// length, then close.
fn serve(files: impl FnOnce(&str) -> Vec<(&'static str, Vec<u8>)>) -> String {
    use std::io::{BufRead as _, BufReader, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let files = files(&base);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut line = String::new();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            reader.read_line(&mut line).unwrap();
            // drain the headers
            let mut h = String::new();
            while reader.read_line(&mut h).is_ok_and(|n| n > 2) {
                h.clear();
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/");
            let found = files.iter().find(|(p, _)| *p == path);
            let (status, body) = match found {
                Some((_, b)) => ("200 OK", b.clone()),
                None => ("404 Not Found", Vec::new()),
            };
            let head = format!(
                "HTTP/1.1 {status}
Content-Length: {}
Connection: close

",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    base
}

/// The whole flow against a local feed signed with a test key: the feed names a newer release,
/// the download verifies, it is staged under its own file name, and a file swapped on the
/// server after signing is refused.
#[tokio::test(flavor = "multi_thread")]
async fn updates_from_a_local_feed() {
    let installer = b"MZ pretend NSIS installer".to_vec();
    let (pk, sig) = signed(&installer);
    let feed = |base: &str| -> Vec<u8> {
        serde_json::json!({
            "version": "9.9.9",
            "notes": "- A new thing",
            "pub_date": "2026-10-02T00:00:00Z",
            "platforms": {
                "windows-x86_64": { "signature": sig, "url": format!("{base}/HyprSpace_9.9.9_x64-setup.exe") },
                "darwin-aarch64": { "signature": sig, "url": format!("{base}/HyprSpace.app.tar.gz") },
            }
        })
        .to_string()
        .into_bytes()
    };
    let mut tampered = installer.clone();
    tampered[3] ^= 1;
    let files = serve(|base| {
        vec![
            ("/latest.json", feed(base)),
            ("/HyprSpace_9.9.9_x64-setup.exe", installer.clone()),
            ("/tampered.exe", tampered),
        ]
    });
    let up = check_feed(&format!("{files}/latest.json"), "0.21.1", "windows-x86_64")
        .await
        .unwrap()
        .expect("the feed is newer");
    assert_eq!(up.version, "9.9.9");
    assert_eq!(up.notes, "- A new thing");

    let mut seen = 0;
    let bytes = download_with(&pk, &up, |got, _| seen = got).await.unwrap();
    assert_eq!(bytes, installer);
    assert_eq!(seen, installer.len() as u64);
    let staged = stage(&up, &bytes).unwrap();
    assert_eq!(
        staged.file_name().unwrap().to_str().unwrap(),
        "HyprSpace_9.9.9_x64-setup.exe"
    );
    assert_eq!(std::fs::read(&staged).unwrap(), installer);
    std::fs::remove_dir_all(staged.parent().unwrap()).unwrap();

    // already on it: nothing to do
    assert_eq!(
        check_feed(&format!("{files}/latest.json"), "9.9.9", "windows-x86_64")
            .await
            .unwrap(),
        None
    );

    // a tampered installer behind the same feed entry
    let tampered = Update {
        url: format!("{files}/tampered.exe"),
        ..up
    };
    let err = download_with(&pk, &tampered, |_, _| {}).await.unwrap_err();
    assert!(format!("{err:#}").contains("signature"), "{err:#}");
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
