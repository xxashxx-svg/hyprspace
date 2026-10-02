// The Claude harness against the fake CLI (fixtures/fake_cli/claude.rs): every capability the
// Harness trait offers, end to end through a real child process.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{count, events, fake, finished, text};
use hyprspace_harness::{Claude, Harness, Session};
use hyprspace_proto::run::{ChangeKind, FileChange};
use hyprspace_proto::{Agent, Launch, Permission, Prompt, RunEvent, RunStatus, Tool};

fn claude() -> Claude {
    Claude::default().with_program(fake())
}

fn start(harness: &Claude, cwd: &Path) -> (Session, common::Events) {
    let (emit, events) = events();
    let session = harness
        .start(Launch::new(Agent::Claude, cwd), emit)
        .expect("the fake starts");
    (session, events)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    std::fs::canonicalize(a).unwrap() == std::fs::canonicalize(b).unwrap()
}

#[tokio::test]
async fn streams_a_reply_with_thinking_and_tool_calls() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("hello"));
    let run = events.run().await;

    let RunEvent::Started {
        agent,
        model,
        thread,
        cwd,
    } = &run[0]
    else {
        panic!("first event is Started: {run:?}")
    };
    assert_eq!((*agent, model.as_str()), (Agent::Claude, "claude-fake"));
    assert_eq!(thread, "fake-session");
    assert!(same_dir(cwd, dir.path()));
    assert!(run.contains(&RunEvent::Thinking {
        text: "pondering".into()
    }));
    // subagent text stays out of the main reply
    assert_eq!(text(&run), "Hello");
    assert!(run.contains(&RunEvent::Tool {
        id: "t1".into(),
        tool: Tool::Command {
            command: "ls".into()
        }
    }));
    assert!(run.contains(&RunEvent::Tool {
        id: "t2".into(),
        tool: Tool::Edit {
            changes: vec![FileChange {
                path: "/a.rs".into(),
                kind: ChangeKind::Update,
                diff: "@@\n-a\n+b".into()
            }]
        }
    }));
    assert!(run.contains(&RunEvent::ToolDone {
        id: "t1".into(),
        ok: true,
        output: "file.txt".into()
    }));
    assert!(run.contains(&RunEvent::ToolDone {
        id: "t2".into(),
        ok: false,
        output: "no such file".into()
    }));
    assert!(run.contains(&RunEvent::Usage {
        input: 10,
        output: 20
    }));
    assert_eq!(finished(&run), (RunStatus::Done, "Hello".into(), None));
}

#[tokio::test]
async fn sends_more_runs_to_the_same_session() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("first"));
    let one = events.run().await;
    session.send(Prompt::text("second"));
    let two = events.run().await;
    assert_eq!(text(&one), "echo: first");
    assert_eq!(text(&two), "echo: second");
    // the CLI sends init every turn; the session started once
    let started = |e: &RunEvent| matches!(e, RunEvent::Started { .. });
    assert_eq!((count(&one, started), count(&two, started)), (1, 0));
}

#[tokio::test]
async fn passes_model_effort_permission_and_resumes_in_the_original_folder() {
    let config = tempfile::tempdir().unwrap();
    let origin = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let project = config.path().join("projects").join("encoded");
    std::fs::create_dir_all(&project).unwrap();
    let line = serde_json::json!({ "type": "user", "cwd": origin.path() });
    std::fs::write(project.join("abc-1.jsonl"), format!("{line}\n")).unwrap();

    let harness = claude().with_config_dir(config.path());
    let (emit, mut events) = events();
    let launch = Launch {
        model: Some("claude-opus-5-5".into()),
        effort: Some("high".into()),
        permission: Permission::Auto,
        resume: Some("abc-1".into()),
        ..Launch::new(Agent::Claude, elsewhere.path())
    };
    let session = harness.start(launch, emit).unwrap();
    session.send(Prompt::text("args"));
    let run = events.run().await;
    let RunEvent::Started { thread, cwd, .. } = &run[0] else {
        panic!("{run:?}")
    };
    assert_eq!(thread, "abc-1");
    assert!(same_dir(cwd, origin.path()), "{cwd:?}");

    let args = text(&run);
    for want in [
        "--print",
        "--input-format stream-json",
        "--output-format stream-json",
        "--verbose",
        "--permission-prompt-tool stdio",
        "--model claude-opus-5-5",
        "--effort high",
        "--permission-mode acceptEdits",
        "--resume abc-1",
    ] {
        assert!(args.contains(want), "{want} missing from {args}");
    }
    let name = origin.path().file_name().unwrap().to_string_lossy();
    assert!(
        args.contains(&*name),
        "ran outside the origin folder: {args}"
    );
}

#[tokio::test]
async fn approvals_wait_for_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("approve"));
    let first = events
        .until(|e| matches!(e, RunEvent::Approval { .. }))
        .await;
    assert_eq!(
        first.last(),
        Some(&RunEvent::Approval {
            request: "r1".into(),
            tool: Tool::Command {
                command: "ls".into()
            },
            reason: None
        })
    );
    // an answer to a request nobody asked is ignored
    session.answer("nope".into(), true);
    session.answer("r1".into(), true);
    let second = events
        .until(|e| matches!(e, RunEvent::Approval { .. }))
        .await;
    let Some(RunEvent::Approval { request, tool, .. }) = second.last() else {
        panic!()
    };
    assert_eq!(request, "r2");
    assert!(matches!(tool, Tool::Edit { changes } if changes[0].kind == ChangeKind::Add));
    session.answer("r2".into(), false);
    let run = events.run().await;
    assert_eq!(text(&run), "approvals ok");
}

#[tokio::test]
async fn a_prompt_mid_run_steers_it() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("steer"));
    events
        .until(|e| {
            *e == RunEvent::Text {
                text: "first".into(),
            }
        })
        .await;
    session.send(Prompt::text("go left"));
    let run = events.run().await;
    assert_eq!(count(&run, |e| *e == RunEvent::Steered), 1);
    // no tool was open, so the steer cut in at once
    assert_eq!(text(&run), "steered(now): go left");
    assert_eq!(finished(&run).0, RunStatus::Done);
    // the session takes the next run as usual
    session.send(Prompt::text("after"));
    assert_eq!(text(&events.run().await), "echo: after");
}

#[tokio::test]
async fn a_steer_waits_for_an_open_tool() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("steer-tool"));
    events.until(|e| matches!(e, RunEvent::Tool { .. })).await;
    session.send(Prompt::text("go right"));
    let run = events.run().await;
    assert_eq!(count(&run, |e| *e == RunEvent::Steered), 1);
    assert_eq!(text(&run), "steered(next): go right");
}

#[tokio::test]
async fn a_steer_the_cli_never_echoes_still_ends_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let harness = claude().with_patience(Duration::from_millis(300));
    let (session, mut events) = start(&harness, dir.path());
    session.send(Prompt::text("absorb"));
    session.send(Prompt::text("also this"));
    let run = events.run().await;
    assert_eq!(count(&run, |e| *e == RunEvent::Steered), 1);
    assert_eq!(finished(&run), (RunStatus::Done, "absorbed".into(), None));
}

#[tokio::test]
async fn interrupt_ends_the_run_and_keeps_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    // nothing to stop yet
    session.interrupt();
    session.send(Prompt::text("interrupt"));
    events
        .until(|e| {
            *e == RunEvent::Text {
                text: "working".into(),
            }
        })
        .await;
    session.interrupt();
    let run = events.run().await;
    assert_eq!(
        finished(&run),
        (RunStatus::Interrupted, String::new(), None)
    );
    session.send(Prompt::text("again"));
    assert_eq!(text(&events.run().await), "echo: again");
}

#[tokio::test]
async fn an_ignored_interrupt_closes_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let harness = claude().with_patience(Duration::from_millis(300));
    let (session, mut events) = start(&harness, dir.path());
    session.send(Prompt::text("wedge"));
    events
        .until(|e| {
            *e == RunEvent::Text {
                text: "working".into(),
            }
        })
        .await;
    session.interrupt();
    let run = events.run().await;
    assert_eq!(finished(&run).0, RunStatus::Interrupted);
    let RunEvent::Failed { message } = events.next().await else {
        panic!("the session closes")
    };
    assert!(message.contains("did not stop"), "{message}");
}

#[tokio::test]
async fn images_go_inline() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("shot.png");
    std::fs::write(&png, [0x89, b'P', b'N', b'G', 1, 2, 3]).unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt {
        text: "image".into(),
        images: vec![png],
    });
    assert_eq!(text(&events.run().await), "images: image/png");
}

#[tokio::test]
async fn a_crash_fails_the_run_and_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("crash"));
    let run = events.run().await;
    let (status, _, error) = finished(&run);
    assert_eq!(status, RunStatus::Failed);
    assert!(error.unwrap().contains("fake claude crashed"));
    let RunEvent::Failed { message } = events.next().await else {
        panic!("the session closes")
    };
    assert!(message.contains("code 3"), "{message}");
    events.quiet().await;
    assert!(session.is_closed());
}

#[tokio::test]
async fn a_failed_result_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("fail"));
    let (status, _, error) = finished(&events.run().await);
    assert_eq!(status, RunStatus::Failed);
    assert_eq!(
        error.as_deref(),
        Some("The run hit the maximum number of turns.")
    );
}

#[tokio::test]
async fn a_missing_cli_fails_to_start() {
    let harness = Claude::default().with_program(PathBuf::from("hyprspace-no-such-cli"));
    let (emit, mut events) = events();
    match harness.start(Launch::new(Agent::Claude, std::env::temp_dir()), emit) {
        Err(e) => assert_eq!(e.kind(), std::io::ErrorKind::NotFound),
        // on Windows the cmd.exe retry starts, then reports the missing command
        Ok(_session) => assert!(matches!(events.next().await, RunEvent::Failed { .. })),
    }
}
