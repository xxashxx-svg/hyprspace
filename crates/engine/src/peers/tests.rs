use std::path::PathBuf;
use std::time::Duration;

use hyprspace_proto::peer::{PeerCommand, PeerEvent};
use hyprspace_proto::phone::{
    Board, BoardKind, BoardSpace, BoardThread, Network, PhoneCommand, PhoneEvent,
};
use hyprspace_proto::{
    Command, Entry, Event, Events, FolderCommand, FolderEvent, Prompt, SessionId,
};

use crate::Engine;

fn wait<T>(events: &mut Events, mut pick: impl FnMut(Event) -> Option<T>) -> T {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
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

#[test]
fn a_computer_pairs_drives_a_terminal_reads_a_journal_and_a_folder() {
    // SAFETY: every test that touches this sets it to the same value
    unsafe { std::env::set_var("HYPRSPACE_PHONE_BIND", "127.0.0.1") };
    let host_dir = tempfile::tempdir().unwrap();
    {
        let journal = crate::journal::Journal::open(&crate::journal::path(
            &host_dir.path().join("journals"),
            "thread-8",
        ));
        journal.record(Entry::Prompt {
            prompt: Prompt::text("explain the redirect"),
        });
    }
    let (host, hc, mut he) = Engine::start_in(host_dir.path().into()).unwrap();
    hc.send(Command::Phone(PhoneCommand::Enable {
        on: true,
        network: Network::Everywhere,
    }));
    wait(&mut he, |e| match e {
        Event::Phone(PhoneEvent::Status { status }) if status.on => Some(()),
        _ => None,
    });
    let thread = |id, kind| BoardThread {
        id,
        space: 1,
        title: "t".into(),
        kind,
        live: true,
        ..Default::default()
    };
    hc.send(Command::Phone(PhoneCommand::Board {
        board: Box::new(Board {
            spaces: vec![BoardSpace {
                id: 1,
                name: "w".into(),
                path: "/w".into(),
                ..Default::default()
            }],
            threads: vec![
                thread(7, BoardKind::Terminal),
                thread(8, BoardKind::Structured),
            ],
            ..Default::default()
        }),
    }));
    hc.send(Command::Phone(PhoneCommand::Sessions {
        sessions: vec![(7, SessionId(1))],
    }));
    hc.send(Command::OpenTerminal {
        id: SessionId(1),
        cwd: std::env::temp_dir(),
        cols: 120,
        rows: 30,
        run: None,
        prompt: None,
    });
    // ConPTY asks where the cursor is before the shell starts; the host UI's emulator answers
    if cfg!(windows) {
        wait(&mut he, |e| match e {
            Event::TerminalOutput { bytes, .. } if bytes.windows(3).any(|w| w == b"[6n") => {
                Some(())
            }
            _ => None,
        });
        hc.send(Command::WriteTerminal {
            id: SessionId(1),
            bytes: b"\x1b[1;1R".to_vec(),
        });
    }
    hc.send(Command::Phone(PhoneCommand::Pair));
    let link = wait(&mut he, |e| match e {
        Event::Phone(PhoneEvent::Pairing { pairing: Some(p) }) => Some(p.link),
        _ => None,
    });

    let client_dir = tempfile::tempdir().unwrap();
    let (client, cc, mut ce) = Engine::start_in(client_dir.path().into()).unwrap();
    cc.send(Command::Peer(PeerCommand::Pair { link }));
    let peer = wait(&mut ce, |e| match e {
        Event::Peer(PeerEvent::Peers { peers }) => peers.into_iter().find(|p| p.online),
        Event::Peer(PeerEvent::PairFailed { message }) => panic!("{message}"),
        _ => None,
    })
    .id;
    wait(&mut ce, |e| match e {
        Event::Peer(PeerEvent::Board { board, .. }) if board.threads.len() == 2 => Some(()),
        _ => None,
    });
    assert!(
        std::fs::read_to_string(client_dir.path().join("peers.json"))
            .unwrap()
            .contains(&peer)
    );

    // the terminal: typed here, run there, its output back here
    let term = SessionId(50);
    cc.send(Command::Peer(PeerCommand::Bind {
        id: term,
        peer: peer.clone(),
        thread: 7,
    }));
    cc.send(Command::OpenTerminal {
        id: term,
        cwd: PathBuf::new(),
        cols: 90,
        rows: 25,
        run: None,
        prompt: None,
    });
    cc.send(Command::WriteTerminal {
        id: term,
        bytes: b"echo computer-was-here\r".to_vec(),
    });
    let mut seen = Vec::new();
    wait(&mut ce, |e| match e {
        Event::TerminalOutput { id, bytes } if id == term => {
            seen.extend(bytes);
            let text = String::from_utf8_lossy(&seen);
            (text.matches("computer-was-here").count() >= 2).then_some(())
        }
        _ => None,
    });
    let by = wait(&mut he, |e| match e {
        Event::Phone(PhoneEvent::Fit {
            phone: true, by, ..
        }) => Some(by),
        _ => None,
    });
    assert_eq!(by, crate::phone::host_name());

    // a structured thread's journal
    let chat = SessionId(51);
    cc.send(Command::Peer(PeerCommand::Bind {
        id: chat,
        peer: peer.clone(),
        thread: 8,
    }));
    cc.send(Command::LoadJournal {
        id: chat,
        journal: "unused".into(),
    });
    let entries = wait(&mut ce, |e| match e {
        Event::Journal { id, entries } if id == chat => Some(entries),
        _ => None,
    });
    assert!(
        matches!(&entries[..], [Entry::Prompt { prompt }] if prompt.text == "explain the redirect")
    );

    // a folder on the host
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(folder.path().join("notes.md"), "hi").unwrap();
    cc.send(Command::Peer(PeerCommand::Folder {
        peer: peer.clone(),
        cmd: FolderCommand::ListDir {
            path: folder.path().into(),
        },
    }));
    let names = wait(&mut ce, |e| match e {
        Event::Peer(PeerEvent::Folder {
            event:
                FolderEvent::Dir {
                    entries: Ok(entries),
                    ..
                },
            ..
        }) => Some(entries.into_iter().map(|e| e.name).collect::<Vec<_>>()),
        _ => None,
    });
    assert_eq!(names, ["notes.md"]);

    // unpairing here unpairs there
    cc.send(Command::Peer(PeerCommand::Forget { peer }));
    wait(&mut he, |e| match e {
        Event::Phone(PhoneEvent::Status { status }) if status.devices.is_empty() => Some(()),
        _ => None,
    });
    client.shutdown();
    host.shutdown();
}
