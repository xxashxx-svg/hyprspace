// One real session through a harness, printing every event: the check that an adapter still
// speaks the installed CLI's protocol. It spends the user's plan, so keep the prompt tiny.
//
//   cargo run -p hyprspace-harness --example live -- claude "Reply with just: hi"
//   cargo run -p hyprspace-harness --example live -- codex "Reply with just: hi" [--resume ID]

use hyprspace_harness::Emit;
use hyprspace_proto::{Agent, Launch, Prompt, RunEvent};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let agent = match args.first().map(String::as_str) {
        Some("codex") => Agent::Codex,
        Some("claude") => Agent::Claude,
        _ => {
            eprintln!("usage: live claude|codex PROMPT [--resume ID] [--model M] [--effort E]");
            std::process::exit(2);
        }
    };
    let prompt = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "Reply with just: hi".into());
    let flag = |name: &str| {
        let at = args.iter().position(|a| a == name)?;
        args.get(at + 1).cloned()
    };
    let launch = Launch {
        model: flag("--model"),
        effort: flag("--effort"),
        resume: flag("--resume"),
        ..Launch::new(agent, std::env::current_dir().unwrap())
    };

    let (tx, mut rx) = mpsc::unbounded_channel();
    let emit: Emit = Box::new(move |e| {
        let _ = tx.send(e);
    });
    let session = hyprspace_harness::for_agent(agent)
        .start(launch, emit)
        .expect("start the CLI");
    session.send(Prompt::text(prompt));
    while let Some(event) = rx.recv().await {
        println!("{}", serde_json::to_string(&event).unwrap());
        // a tool approval would wait forever here; this check never allows one
        if let RunEvent::Approval { request, .. } = &event {
            session.answer(request.clone(), hyprspace_proto::Answer::Deny);
        }
        if matches!(event, RunEvent::Finished { .. } | RunEvent::Failed { .. }) {
            break;
        }
    }
}
