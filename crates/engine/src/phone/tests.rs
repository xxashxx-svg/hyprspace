// The bridge end to end: a real engine, a phone's side of TLS that pins the certificate from
// the pairing link, and the WebSocket over it. Everything stays on loopback.

use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use hyprspace_proto::phone::{
    Ask, Board, BoardKind, BoardSpace, BoardThread, Down, Network, PROTOCOL, PhoneCommand,
    PhoneEvent, Up,
};
use hyprspace_proto::{Command, Event, Events, SessionId};
use sha2::{Digest, Sha256};
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use tokio_tungstenite::tungstenite::Message;

use crate::Engine;

/// Trusts exactly one certificate, the way the phone does after pairing.
#[derive(Debug)]
struct Pinned(String);

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        cert: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, tokio_rustls::rustls::Error> {
        use base64::Engine as _;
        let got = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(cert));
        if got == self.0 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(tokio_rustls::rustls::Error::General(
                "not the pinned certificate".into(),
            ))
        }
    }
    fn verify_tls12_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        let p = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider();
        tokio_rustls::rustls::crypto::verify_tls12_signature(
            m,
            c,
            d,
            &p.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        m: &[u8],
        c: &CertificateDer<'_>,
        d: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        let p = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider();
        tokio_rustls::rustls::crypto::verify_tls13_signature(
            m,
            c,
            d,
            &p.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>;

async fn connect(port: u16, fingerprint: &str) -> Ws {
    let provider = Arc::new(tokio_rustls::rustls::crypto::aws_lc_rs::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned(fingerprint.into())))
        .with_no_client_auth();
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let tls = TlsConnector::from(Arc::new(config))
        .connect(ServerName::try_from("hyprspace").unwrap(), tcp)
        .await
        .unwrap();
    let (ws, _) = tokio_tungstenite::client_async("wss://hyprspace/", tls)
        .await
        .unwrap();
    ws
}

async fn say(ws: &mut Ws, up: &Up) {
    ws.send(Message::Text(serde_json::to_string(up).unwrap().into()))
        .await
        .unwrap();
}

async fn hear(ws: &mut Ws) -> Down {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let msg = tokio::time::timeout_at(deadline, ws.next())
            .await
            .expect("the desktop went quiet")
            .unwrap()
            .unwrap();
        if let Message::Text(t) = msg {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

/// Waits for an event the test cares about, skipping the rest.
fn wait<T>(events: &mut Events, mut pick: impl FnMut(Event) -> Option<T>) -> T {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        assert!(std::time::Instant::now() < deadline, "no such event");
        if let Ok(e) = events.try_recv()
            && let Some(t) = pick(e)
        {
            return t;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn query<'a>(link: &'a str, key: &str) -> &'a str {
    link.split(['?', '&'])
        .find_map(|kv| kv.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
        .unwrap()
}

#[test]
fn a_phone_pairs_watches_a_terminal_types_and_comes_back() {
    // SAFETY: every test that touches this sets it to the same value
    unsafe { std::env::set_var("HYPRSPACE_PHONE_BIND", "127.0.0.1") };
    let dir = tempfile::tempdir().unwrap();
    let (engine, client, mut events) = Engine::start_in(dir.path().into()).unwrap();
    client.send(Command::Phone(PhoneCommand::Enable {
        on: true,
        network: Network::Everywhere,
    }));
    let port = wait(&mut events, |e| match e {
        Event::Phone(PhoneEvent::Status { status }) if status.on => Some(status.port),
        _ => None,
    });
    let board = Board {
        spaces: vec![BoardSpace {
            id: 1,
            name: "w".into(),
            path: "/w".into(),
            ..Default::default()
        }],
        threads: vec![BoardThread {
            id: 7,
            space: 1,
            title: "Shell".into(),
            kind: BoardKind::Terminal,
            live: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    client.send(Command::Phone(PhoneCommand::Board {
        board: Box::new(board),
    }));
    client.send(Command::Phone(PhoneCommand::Sessions {
        sessions: vec![(7, SessionId(1))],
    }));
    client.send(Command::OpenTerminal {
        id: SessionId(1),
        cwd: std::env::temp_dir(),
        cols: 120,
        rows: 30,
        run: None,
        prompt: None,
    });
    // ConPTY asks where the cursor is before the shell starts; the UI's emulator answers that.
    // Other platforms' shells don't ask.
    if cfg!(windows) {
        wait(&mut events, |e| match e {
            Event::TerminalOutput { bytes, .. } if bytes.windows(4).any(|w| w == b"[6n") => {
                Some(())
            }
            _ => None,
        });
        client.send(Command::WriteTerminal {
            id: SessionId(1),
            bytes: b"[1;1R".to_vec(),
        });
    }
    client.send(Command::Phone(PhoneCommand::Pair));
    let link = wait(&mut events, |e| match e {
        Event::Phone(PhoneEvent::Pairing { pairing: Some(p) }) => Some(p.link),
        _ => None,
    });
    let fingerprint = query(&link, "f").to_string();
    let secret = query(&link, "c").to_string();
    assert_eq!(query(&link, "p"), port.to_string());

    let rt = tokio::runtime::Runtime::new().unwrap();
    let token = rt.block_on(async {
        let mut ws = connect(port, &fingerprint).await;
        say(
            &mut ws,
            &Up::Pair {
                proof: super::prove(&secret, &fingerprint),
                device: "Test phone".into(),
                protocol: PROTOCOL,
                app: String::new(),
            },
        )
        .await;
        let token = match hear(&mut ws).await {
            Down::Welcome { token, proof, .. } => {
                let ours = super::prove(&secret, &format!("desktop {fingerprint}"));
                assert_eq!(proof, Some(ours), "the computer proves it knows the code");
                token.expect("a token for a new pairing")
            }
            other => panic!("{other:?}"),
        };
        assert!(matches!(hear(&mut ws).await, Down::Board { board } if board.threads.len() == 1));
        say(&mut ws, &Up::Watch { thread: 7 }).await;
        say(
            &mut ws,
            &Up::Fit {
                thread: 7,
                cols: 40,
                rows: 20,
            },
        )
        .await;
        say(
            &mut ws,
            &Up::Keys {
                thread: 7,
                text: "echo phone-was-here\r".into(),
            },
        )
        .await;
        // the phone's copy of the screen, built from frames the way the app builds it
        let mut lines: Vec<String> = Vec::new();
        let mut fitted = false;
        loop {
            if let Down::Term { frame } = hear(&mut ws).await {
                if frame.reset {
                    lines.clear();
                }
                lines.drain(..(frame.drop as usize).min(lines.len()));
                lines.resize(frame.len as usize, String::new());
                for (i, spans) in frame.lines {
                    lines[i as usize] = spans.iter().map(|s| s.t.as_str()).collect();
                }
                fitted |= frame.fit && frame.cols == 40;
                // the echo's output, on a line of its own after the command
                if fitted
                    && lines
                        .iter()
                        .filter(|l| l.contains("phone-was-here"))
                        .count()
                        >= 2
                {
                    break;
                }
            }
        }
        say(
            &mut ws,
            &Up::Ask {
                ask: Ask::Settle {
                    thread: 7,
                    on: true,
                },
            },
        )
        .await;
        // a folder that isn't a space yet: the phone sends space 0 with it
        say(
            &mut ws,
            &Up::Ask {
                ask: Ask::New {
                    space: 0,
                    folder: Some("/somewhere/new".into()),
                    start: hyprspace_proto::phone::NewThread {
                        agent: None,
                        model: String::new(),
                        effort: String::new(),
                        permission: hyprspace_proto::Permission::Ask,
                        terminal: true,
                        prompt: String::new(),
                    },
                },
            },
        )
        .await;
        say(&mut ws, &Up::Ping).await;
        loop {
            match hear(&mut ws).await {
                Down::Pong => break,
                Down::Failed { message, .. } => panic!("{message}"),
                _ => {}
            }
        }
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("proj")).unwrap();
        std::fs::create_dir(root.path().join(".hidden")).unwrap();
        std::fs::write(root.path().join("notes.txt"), "x").unwrap();
        say(
            &mut ws,
            &Up::Folders {
                path: root.path().display().to_string(),
            },
        )
        .await;
        loop {
            if let Down::Folders { dirs, parent, .. } = hear(&mut ws).await {
                assert_eq!(dirs, vec![root.path().join("proj").display().to_string()]);
                assert!(parent.is_some());
                break;
            }
        }
        token
    });
    wait(&mut events, |e| match e {
        Event::Phone(PhoneEvent::Ask {
            ask: Ask::Settle {
                thread: 7,
                on: true,
            },
        }) => Some(()),
        _ => None,
    });
    wait(&mut events, |e| match e {
        Event::Phone(PhoneEvent::Ask {
            ask: Ask::New {
                folder: Some(f), ..
            },
        }) if f == "/somewhere/new" => Some(()),
        _ => None,
    });
    // the phone sized the terminal, and leaving gave it back
    wait(&mut events, |e| match e {
        Event::Phone(PhoneEvent::Fit { phone: false, .. }) => Some(()),
        _ => None,
    });

    rt.block_on(async {
        let mut ws = connect(port, &fingerprint).await;
        say(
            &mut ws,
            &Up::Hello {
                token: token.clone(),
                device: "Test phone".into(),
                protocol: PROTOCOL,
                app: String::new(),
            },
        )
        .await;
        assert!(matches!(
            hear(&mut ws).await,
            Down::Welcome { token: None, .. }
        ));
        // the phone forgets the computer, and the computer forgets it back
        say(&mut ws, &Up::Leave).await;
        let mut ws = connect(port, &fingerprint).await;
        say(
            &mut ws,
            &Up::Hello {
                token: token.clone(),
                device: "Test phone".into(),
                protocol: PROTOCOL,
                app: String::new(),
            },
        )
        .await;
        assert!(matches!(
            hear(&mut ws).await,
            Down::Denied { forget: true, .. }
        ));

        // the code worked once and is spent
        let mut ws = connect(port, &fingerprint).await;
        say(
            &mut ws,
            &Up::Pair {
                proof: super::prove(&secret, &fingerprint),
                device: "Thief".into(),
                protocol: PROTOCOL,
                app: String::new(),
            },
        )
        .await;
        assert!(matches!(hear(&mut ws).await, Down::Denied { .. }));

        let mut ws = connect(port, &fingerprint).await;
        say(
            &mut ws,
            &Up::Hello {
                token: "made-up".into(),
                device: "Thief".into(),
                protocol: PROTOCOL,
                app: String::new(),
            },
        )
        .await;
        assert!(matches!(hear(&mut ws).await, Down::Denied { .. }));
    });

    // revoking a connected phone cuts it off now, and its token stops working
    client.send(Command::Phone(PhoneCommand::Pair));
    let link = wait(&mut events, |e| match e {
        Event::Phone(PhoneEvent::Pairing { pairing: Some(p) }) => Some(p.link),
        _ => None,
    });
    let secret = query(&link, "c").to_string();
    let (mut ws, token) = rt.block_on(async {
        let mut ws = connect(port, &fingerprint).await;
        say(
            &mut ws,
            &Up::Pair {
                proof: super::prove(&secret, &fingerprint),
                device: "Second phone".into(),
                protocol: PROTOCOL,
                app: String::new(),
            },
        )
        .await;
        match hear(&mut ws).await {
            Down::Welcome { token, .. } => (ws, token.expect("a token")),
            other => panic!("{other:?}"),
        }
    });
    let device = wait(&mut events, |e| match e {
        Event::Phone(PhoneEvent::Status { status }) => status
            .devices
            .iter()
            .find(|d| d.name == "Second phone" && d.online)
            .map(|d| d.id.clone()),
        _ => None,
    });
    client.send(Command::Phone(PhoneCommand::Forget { device }));
    rt.block_on(async {
        loop {
            match hear(&mut ws).await {
                Down::Denied { forget, .. } => {
                    assert!(forget);
                    break;
                }
                _ => continue,
            }
        }
        let mut ws = connect(port, &fingerprint).await;
        say(
            &mut ws,
            &Up::Hello {
                token,
                device: "Second phone".into(),
                protocol: PROTOCOL,
                app: String::new(),
            },
        )
        .await;
        assert!(matches!(
            hear(&mut ws).await,
            Down::Denied { forget: true, .. }
        ));
    });
    client.send(Command::Close { id: SessionId(1) });
    engine.shutdown();
}

#[test]
fn proofs_match_the_phones() {
    // the same vector as the app's ProofTest
    assert_eq!(
        super::prove("K7MXQ2RTH9WP", "fp-example"),
        "Ty027fUmm9y1FhIoARQBY5b7pqndj_4jToliCUSyavk"
    );
}
