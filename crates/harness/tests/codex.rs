// The Codex harness against the fake app server (fixtures/fake_cli/codex.rs): every capability
// the Harness trait offers, end to end through a real child process.

mod common;

use std::path::Path;
use std::time::Duration;

use common::{count, events, fake, finished, text};
use hyprspace_harness::{Codex, Harness, Session};
use hyprspace_proto::run::{ChangeKind, FileChange};
use hyprspace_proto::{Agent, Answer, Launch, Permission, Prompt, RunEvent, RunStatus, Tool};
use serde_json::Value;

fn codex() -> Codex {
    Codex::default().with_program(fake())
}

fn start_with(harness: &Codex, launch: Launch) -> (Session, common::Events) {
    let (emit, events) = events();
    let session = harness.start(launch, emit).expect("the fake starts");
    (session, events)
}

fn start(cwd: &Path) -> (Session, common::Events) {
    start_with(&codex(), Launch::new(Agent::Codex, cwd))
}

#[tokio::test]
async fn streams_a_reply_with_reasoning_and_tool_items() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(dir.path());
    let RunEvent::Started {
        agent,
        model,
        thread,
        ..
    } = events.next().await
    else {
        panic!("first event is Started")
    };
    assert_eq!(
        (agent, model.as_str(), thread.as_str()),
        (Agent::Codex, "gpt-fake", "th-1")
    );

    session.send(Prompt::text("hello"));
    let run = events.run().await;
    let thinking: String = run
        .iter()
        .filter_map(|e| match e {
            RunEvent::Thinking { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(thinking, "plan\n\nmore");
    // a subagent thread's text stays out; two messages get a paragraph break
    assert_eq!(text(&run), "Hello\n\nDone.");
    assert!(run.contains(&RunEvent::Tool {
        id: "c1".into(),
        tool: Tool::Command {
            command: "ls".into()
        }
    }));
    assert!(run.contains(&RunEvent::ToolDone {
        id: "c1".into(),
        ok: true,
        output: "a.txt".into()
    }));
    assert!(run.contains(&RunEvent::Tool {
        id: "f1".into(),
        tool: Tool::Edit {
            changes: vec![FileChange {
                path: "/a.rs".into(),
                kind: ChangeKind::Update,
                diff: "@@ -1 +1 @@\n-a\n+b".into()
            }]
        }
    }));
    assert!(run.contains(&RunEvent::Usage {
        input: 10,
        output: 20
    }));
    assert!(run.contains(&RunEvent::Context {
        used: 30,
        window: 1000
    }));
    // a retried error is not the user's problem
    assert_eq!(count(&run, |e| matches!(e, RunEvent::Error { .. })), 0);
    assert_eq!(finished(&run), (RunStatus::Done, "Done.".into(), None));

    session.send(Prompt::text("second"));
    assert_eq!(text(&events.run().await), "echo: second");
}

#[tokio::test]
async fn passes_model_effort_and_permission() {
    let dir = tempfile::tempdir().unwrap();
    let launch = Launch {
        model: Some("gpt-5.5".into()),
        effort: Some("high".into()),
        permission: Permission::Auto,
        ..Launch::new(Agent::Codex, dir.path())
    };
    let (session, mut events) = start_with(&codex(), launch);
    session.send(Prompt::text("args"));
    let run = events.run().await;
    let seen: Value = serde_json::from_str(&text(&run)).unwrap();
    assert_eq!(seen["thread"]["model"], "gpt-5.5");
    assert_eq!(seen["thread"]["approvalPolicy"], "on-request");
    assert_eq!(seen["thread"]["sandbox"], "workspace-write");
    assert_eq!(seen["thread"]["cwd"].as_str(), dir.path().to_str());
    assert_eq!(seen["turn"]["effort"], "high");
    assert_eq!(seen["turn"]["summary"], "auto");
    assert_eq!(seen["turn"]["threadId"], "th-1");
}

#[tokio::test]
async fn resumes_a_thread_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let launch = Launch {
        resume: Some("th-9".into()),
        ..Launch::new(Agent::Codex, dir.path())
    };
    let (session, mut events) = start_with(&codex(), launch);
    session.send(Prompt::text("args"));
    let run = events.run().await;
    assert!(matches!(&run[0], RunEvent::Started { thread, .. } if thread == "th-9"));
    let seen: Value = serde_json::from_str(&text(&run)).unwrap();
    assert_eq!(seen["thread"]["threadId"], "th-9");
    assert_eq!(seen["thread"]["excludeTurns"], true);
    assert_eq!(seen["turn"]["threadId"], "th-9");
}

#[tokio::test]
async fn a_thread_that_is_gone_fails_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let launch = Launch {
        resume: Some("gone".into()),
        ..Launch::new(Agent::Codex, dir.path())
    };
    let harness = codex().with_patience(Duration::from_millis(300));
    let (_session, mut events) = start_with(&harness, launch);
    let RunEvent::Failed { message } = events.next().await else {
        panic!("resume fails")
    };
    assert!(message.contains("no rollout found"), "{message}");
}

#[tokio::test]
async fn approvals_wait_for_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(dir.path());
    session.send(Prompt::text("approve"));
    let first = events
        .until(|e| matches!(e, RunEvent::Approval { .. }))
        .await;
    assert_eq!(
        first.last(),
        Some(&RunEvent::Approval {
            request: "100".into(),
            tool: Tool::Command {
                command: "rm x".into()
            },
            reason: Some("needs write access".into()),
            always: true,
        })
    );
    session.answer("100".into(), Answer::AllowAlways);
    let second = events
        .until(|e| matches!(e, RunEvent::Approval { .. }))
        .await;
    let Some(RunEvent::Approval { request, tool, .. }) = second.last() else {
        panic!()
    };
    assert_eq!(request, "req-101");
    // the request names only the item; its edits come from item/started
    assert!(matches!(tool, Tool::Edit { changes } if changes[0].path == "/b.rs"));
    session.answer("req-101".into(), Answer::Deny);
    assert_eq!(text(&events.run().await), "approvals ok");
}

#[tokio::test]
async fn a_prompt_mid_turn_steers_it() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(dir.path());
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
    assert_eq!(text(&run), "steered(same turn): go left");
    assert_eq!(finished(&run).0, RunStatus::Done);
}

#[tokio::test]
async fn a_steer_that_misses_the_turn_starts_the_next_one() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(dir.path());
    session.send(Prompt::text("late-steer"));
    session.send(Prompt::text("go right"));
    let run = events
        .until(|e| matches!(e, RunEvent::Finished { .. }))
        .await;
    assert_eq!(count(&run, |e| *e == RunEvent::Steered), 1);
    assert_eq!(text(&run), "first\n\nlate: go right");
    assert_eq!(finished(&run).0, RunStatus::Done);
}

#[tokio::test]
async fn interrupt_ends_the_turn_and_keeps_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(dir.path());
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
    assert_eq!(finished(&run).0, RunStatus::Interrupted);
    session.send(Prompt::text("again"));
    assert_eq!(text(&events.run().await), "echo: again");
}

#[tokio::test]
async fn an_ignored_interrupt_closes_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let harness = codex().with_patience(Duration::from_millis(300));
    let (session, mut events) = start_with(&harness, Launch::new(Agent::Codex, dir.path()));
    session.send(Prompt::text("wedge"));
    events
        .until(|e| {
            *e == RunEvent::Text {
                text: "working".into(),
            }
        })
        .await;
    session.interrupt();
    assert_eq!(finished(&events.run().await).0, RunStatus::Interrupted);
    let RunEvent::Failed { message } = events.next().await else {
        panic!("the session closes")
    };
    assert!(message.contains("did not stop"), "{message}");
}

#[tokio::test]
async fn images_go_as_local_files() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("shot.png");
    let (session, mut events) = start(dir.path());
    session.send(Prompt {
        text: "image".into(),
        images: vec![png.clone()],
    });
    let reply = text(&events.run().await);
    assert_eq!(reply, format!("images: {}", png.display()));
}

#[tokio::test]
async fn a_crash_fails_the_run_and_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(dir.path());
    session.send(Prompt::text("crash"));
    let (status, _, error) = finished(&events.run().await);
    assert_eq!(status, RunStatus::Failed);
    assert!(error.unwrap().contains("fake codex crashed"));
    assert!(matches!(events.next().await, RunEvent::Failed { .. }));
}

#[tokio::test]
async fn a_failed_turn_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(dir.path());
    session.send(Prompt::text("fail"));
    let (status, _, error) = finished(&events.run().await);
    assert_eq!(
        (status, error.as_deref()),
        (RunStatus::Failed, Some("boom"))
    );
}
