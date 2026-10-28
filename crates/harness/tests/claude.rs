// The Claude harness against the fake CLI (fixtures/fake_cli/claude.rs): every capability the
// Harness trait offers, end to end through a real child process.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{count, events, fake, finished, text};
use hyprspace_harness::{Claude, Harness, Session};
use hyprspace_proto::run::{ChangeKind, FileChange};
use hyprspace_proto::{
    Agent, Answer, Aside, Launch, Permission, Prompt, RunEvent, RunStatus, Task, TaskKind, Tool,
};
use serde_json::json;

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

    // the command list answers the session's first request, before the CLI says it started
    let listed = run
        .iter()
        .position(|e| matches!(e, RunEvent::Commands { .. }));
    assert_eq!(listed, Some(0), "{run:?}");
    let RunEvent::Commands { info, .. } = &run[0] else {
        unreachable!()
    };
    let skills: Vec<&str> = info
        .iter()
        .filter(|c| c.skill)
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(
        skills,
        ["show-me"],
        "built-ins and MCP prompts only start a message"
    );
    let RunEvent::Started {
        agent,
        model,
        thread,
        cwd,
    } = &run[1]
    else {
        panic!("Started follows: {run:?}")
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
        output: "file.txt".into(),
        images: Vec::new(),
    }));
    assert!(run.contains(&RunEvent::ToolDone {
        id: "t2".into(),
        images: Vec::new(),
        ok: false,
        output: "no such file".into()
    }));
    assert!(run.contains(&RunEvent::Usage {
        input: 10,
        output: 20
    }));
    // the window only comes with the result, so the first turn reports at its end
    assert!(run.contains(&RunEvent::Context {
        used: 420,
        window: 1000
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
    let Some(RunEvent::Started { thread, cwd, .. }) =
        run.iter().find(|e| matches!(e, RunEvent::Started { .. }))
    else {
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
async fn a_fork_never_takes_over_the_original_session() {
    let config = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let project = config.path().join("projects").join("encoded");
    std::fs::create_dir_all(&project).unwrap();
    let lines = [
        serde_json::json!({ "type": "user", "cwd": dir.path(), "message": { "content": "first" } }),
        serde_json::json!({ "type": "user", "parentUuid": "a1", "message": { "content": "go" } }),
    ];
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(project.join("abc-1.jsonl"), body).unwrap();
    let harness = claude().with_config_dir(config.path());
    let fork = |text: &str| Launch {
        resume: Some("abc-1".into()),
        rewind: Some(hyprspace_proto::Rewind {
            text: text.into(),
            nth: 0,
            fork: true,
        }),
        ..Launch::new(Agent::Claude, dir.path())
    };

    let (emit, mut seen) = events();
    let session = harness.start(fork("go"), emit).unwrap();
    session.send(Prompt::text("args"));
    let args = text(&seen.run().await);
    assert!(
        args.contains("--resume-session-at a1 --fork-session"),
        "{args}"
    );

    let (emit, mut seen) = events();
    let session = harness.start(fork("never sent"), emit).unwrap();
    session.send(Prompt::text("args"));
    let run = seen.run().await;
    let args = text(&run);
    assert!(args.contains("--resume abc-1"), "{args}");
    assert!(args.contains("--fork-session"), "{args}");
    assert!(!args.contains("--resume-session-at"), "{args}");
    assert!(run.iter().any(|e| matches!(e, RunEvent::Error { .. })));
}

#[tokio::test]
async fn changes_apply_to_the_running_cli() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("args"));
    let run = events.run().await;
    assert!(text(&run).contains("--allow-dangerously-skip-permissions"));
    let controls = |run: &[RunEvent]| -> Vec<serde_json::Value> {
        text(run)
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };

    session.configure(Launch {
        model: Some("claude-opus-5-5[1m]".into()),
        effort: Some("low".into()),
        fast: true,
        permission: Permission::Bypass,
        ..Launch::new(Agent::Claude, dir.path())
    });
    session.send(Prompt::text("controls"));
    let applied = controls(&events.run().await);
    for want in [
        json!({"subtype": "set_model", "model": "claude-opus-5-5[1m]"}),
        json!({"subtype": "apply_flag_settings", "settings": {"effortLevel": "low"}}),
        json!({"subtype": "apply_flag_settings", "settings": {"fastMode": true}}),
        json!({"subtype": "set_permission_mode", "mode": "bypassPermissions"}),
    ] {
        assert!(applied.contains(&want), "{want} missing from {applied:?}");
    }

    session.configure(Launch::new(Agent::Claude, dir.path()));
    session.send(Prompt::text("controls"));
    let cleared = controls(&events.run().await);
    for want in [
        json!({"subtype": "apply_flag_settings", "settings": {"effortLevel": null}}),
        json!({"subtype": "apply_flag_settings", "settings": {"fastMode": false}}),
    ] {
        assert!(cleared.contains(&want), "{want} missing from {cleared:?}");
    }
}

#[tokio::test]
async fn a_refused_change_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.configure(Launch {
        model: Some("nope".into()),
        ..Launch::new(Agent::Claude, dir.path())
    });
    session.send(Prompt::text("hi"));
    let run = events.run().await;
    assert!(
        run.contains(&RunEvent::Error {
            message: "Claude refused the change: unknown model nope".into()
        }),
        "{run:?}"
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
            reason: None,
            always: true,
        })
    );
    // an answer to a request nobody asked is ignored
    session.answer("nope".into(), Answer::Allow, Vec::new());
    session.answer("r1".into(), Answer::AllowAlways, Vec::new());
    let second = events
        .until(|e| matches!(e, RunEvent::Approval { .. }))
        .await;
    let Some(RunEvent::Approval {
        request,
        tool,
        always,
        ..
    }) = second.last()
    else {
        panic!()
    };
    assert_eq!(request, "r2");
    // no suggested rules, so nothing to remember
    assert!(!always);
    assert!(matches!(tool, Tool::Edit { changes } if changes[0].kind == ChangeKind::Add));
    session.answer("r2".into(), Answer::Deny, Vec::new());
    let run = events.run().await;
    assert_eq!(text(&run), "approvals ok");
}

#[tokio::test]
async fn a_question_is_answered_with_the_picks() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("question"));
    let asked = events
        .until(|e| matches!(e, RunEvent::Approval { .. }))
        .await;
    let Some(RunEvent::Approval { request, tool, .. }) = asked.last() else {
        panic!()
    };
    assert!(matches!(tool, Tool::Other { name, .. } if name == "AskUserQuestion"));
    session.answer(
        request.clone(),
        Answer::Allow,
        vec![("Which db?".into(), "SQLite".into())],
    );
    let run = events.run().await;
    assert_eq!(text(&run), "picked SQLite");
}

#[tokio::test]
async fn chat_about_a_question_turns_it_down_with_the_answers_so_far() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("question"));
    let asked = events
        .until(|e| matches!(e, RunEvent::Approval { .. }))
        .await;
    let Some(RunEvent::Approval { request, .. }) = asked.last() else {
        panic!()
    };
    session.answer(
        request.clone(),
        Answer::Deny,
        vec![("Which db?".into(), String::new())],
    );
    let run = events.run().await;
    assert_eq!(text(&run), "denied: (No answer provided)");
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
async fn a_steer_waiting_on_an_approval_keeps_the_run_open() {
    let dir = tempfile::tempdir().unwrap();
    let harness = claude().with_patience(Duration::from_millis(200));
    let (session, mut events) = start(&harness, dir.path());
    session.send(Prompt::text("steer-gate"));
    events
        .until(|e| {
            *e == RunEvent::Text {
                text: "first".into(),
            }
        })
        .await;
    session.send(Prompt::text("go left"));
    let asked = events
        .until(|e| matches!(e, RunEvent::Approval { .. }))
        .await;
    assert!(!asked.iter().any(|e| matches!(e, RunEvent::Finished { .. })));
    // longer than the patience a held run end gets
    events.quiet().await;
    events.quiet().await;
    session.answer("r1".into(), Answer::Allow, Vec::new());
    let run = events.run().await;
    assert_eq!(
        finished(&run),
        (RunStatus::Done, "after the gate".into(), None)
    );
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

fn agent_done(events: &[RunEvent], agent: &str) -> Vec<(bool, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            RunEvent::ToolDone { id, ok, output, .. } if id == agent => Some((*ok, output.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_subagent_reports_under_its_call() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("subagent"));
    let run = events.run().await;

    assert!(run.contains(&RunEvent::Tool {
        id: "toolu_agent".into(),
        tool: Tool::Agent {
            description: "Write a short poem".into(),
            agent_type: "general-purpose".into(),
            prompt: "Four lines.".into(),
        }
    }));
    // nothing the subagent says reaches the main reply
    assert_eq!(text(&run), "Wrote it.");
    let sub = |id: &str, tool: Tool| RunEvent::SubagentTool {
        parent: "toolu_agent".into(),
        id: id.into(),
        tool,
    };
    assert!(run.contains(&sub(
        "s1",
        Tool::Command {
            command: "ls".into()
        }
    )));
    assert!(run.contains(&RunEvent::SubagentToolDone {
        parent: "toolu_agent".into(),
        id: "s1".into(),
        ok: true,
        output: "a.txt".into(),
        images: Vec::new(),
    }));
    assert!(run.contains(&RunEvent::SubagentText {
        parent: "toolu_agent".into(),
        text: "Let me look.".into(),
    }));
    // a subagent's own subagent reports under the call the user sees
    assert!(run.contains(&sub(
        "s3",
        Tool::Read {
            path: "/a.txt".into()
        }
    )));
    // the call ends once, with the report and none of the CLI's framing
    assert_eq!(
        agent_done(&run, "toolu_agent"),
        [(true, "Roses are red.\nDone.".to_string())]
    );
    assert_eq!(finished(&run), (RunStatus::Done, "Wrote it.".into(), None));
}

#[tokio::test]
async fn a_background_subagent_outlives_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("background"));
    let first = events.run().await;
    assert!(
        first
            .iter()
            .any(|e| matches!(e, RunEvent::Tool { id, .. } if id == "toolu_bg"))
    );
    // the launch note is not the end of the subagent
    assert!(agent_done(&first, "toolu_bg").is_empty());
    assert_eq!(text(&first), "Started it.");

    let later = events.run().await;
    assert!(later.contains(&RunEvent::SubagentText {
        parent: "toolu_bg".into(),
        text: "Found a.txt.".into(),
    }));
    assert_eq!(
        agent_done(&later, "toolu_bg"),
        [(true, "Found a.txt.".to_string())]
    );
    // only the subagent's notification counts; the shell command's is left alone
    assert_eq!(count(&later, |e| matches!(e, RunEvent::ToolDone { .. })), 1);
    // the CLI's own turn after it is a run of its own
    assert_eq!(count(&later, |e| *e == RunEvent::Woke), 1);
    assert_eq!(text(&later), "It found a.txt.");
    assert_eq!(finished(&later).0, RunStatus::Done);
    session.send(Prompt::text("after"));
    assert_eq!(text(&events.run().await), "echo: after");
}

#[tokio::test]
async fn a_turn_without_an_init_still_wakes() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("bgquiet"));
    assert_eq!(text(&events.run().await), "Started it.");
    let later = events.run().await;
    assert_eq!(count(&later, |e| *e == RunEvent::Woke), 1);
    assert_eq!(text(&later), "It found a.txt.");
    assert_eq!(finished(&later).0, RunStatus::Done);
}

#[tokio::test]
async fn background_tasks_show_until_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("bgtask"));
    let run = events.run().await;
    let lists: Vec<_> = run
        .iter()
        .filter_map(|e| match e {
            RunEvent::Background { tasks } => Some(tasks.clone()),
            _ => None,
        })
        .collect();
    // the CLI's ambient watcher is not activity, and the task's call comes after its listing
    assert_eq!(
        lists.last().cloned(),
        Some(vec![Task {
            id: "b1".into(),
            kind: TaskKind::Command,
            label: "Dev server".into(),
            tool: Some("tb".into()),
        }])
    );
    session.stop_task("b1".into());
    let after = events
        .until(|e| matches!(e, RunEvent::Background { .. }))
        .await;
    assert_eq!(after.last(), Some(&RunEvent::Background { tasks: vec![] }));
}

#[tokio::test]
async fn a_background_task_s_output_is_read_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let (session, mut events) = start(&claude(), dir.path());
    session.send(Prompt::text("bgwedge"));
    events
        .until(|e| matches!(e, RunEvent::Background { .. }))
        .await;
    session.task_output("b1".into());
    let got = events
        .until(|e| matches!(e, RunEvent::TaskOutput { .. }))
        .await;
    let Some(RunEvent::TaskOutput { task, output, .. }) = got.last() else {
        unreachable!()
    };
    assert_eq!(task, "b1");
    assert!(output.contains("VITE") && output.contains("GET /"));
}

async fn aside(launch: Launch, question: &str) -> Vec<Aside> {
    let mut heard = Vec::new();
    claude()
        .aside(launch, question.into(), |a| heard.push(a))
        .await;
    heard
}

#[tokio::test]
async fn a_side_question_forks_once_and_streams_its_answer() {
    let dir = tempfile::tempdir().unwrap();
    let mut launch = Launch::new(Agent::Claude, dir.path());
    launch.resume = Some("fake-session".into());
    launch.permission = Permission::Bypass;
    let said = aside(launch.clone(), "what are you doing?").await;
    let text: String = said
        .iter()
        .filter_map(|a| match a {
            Aside::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    // Full access drops to Ask, and the fork isn't saved
    assert_eq!(text, "Side: forked once");
    assert_eq!(said.last(), Some(&Aside::Done));

    let said = aside(launch.clone(), "grab-a-tool").await;
    assert!(
        matches!(said.last(), Some(Aside::Failed { message }) if message.contains("needs a tool"))
    );

    launch.resume = None;
    let said = aside(launch, "anything").await;
    assert!(
        matches!(said.as_slice(), [Aside::Failed { message }] if message.contains("Send a message first"))
    );
}

#[tokio::test]
async fn a_title_comes_from_one_quiet_run_with_the_conversation_on_stdin() {
    let dir = tempfile::tempdir().unwrap();
    let ask = hyprspace_harness::title::ask("fix login", None);
    let title = claude().title(dir.path(), None, &ask).await;
    assert_eq!(title.as_deref(), Some("Fix the login bug"));
}
