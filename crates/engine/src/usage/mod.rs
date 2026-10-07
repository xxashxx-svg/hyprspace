// Usage for the meter and the settings tab. `local` reads what the CLIs write on disk; `live`
// asks each provider's usage endpoint for the account's real limits, under the poll rules in
// that file; `status` reads the limits Claude's status line pushes in terminal sessions.

pub mod live;
pub mod local;
pub mod status;

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::{Agent, Event, UsageCommand, UsageEvent};

/// Answers a usage request off the command loop: the live ones wait on the network, the local
/// ones read a lot of files.
pub fn handle(cmd: UsageCommand, tx: UnboundedSender<Event>) {
    match cmd {
        UsageCommand::Live { agent } => {
            tokio::spawn(async move {
                let usage = match agent {
                    Agent::Claude => live::claude().await,
                    Agent::Codex => live::codex().await,
                };
                let _ = tx.unbounded_send(Event::Usage(UsageEvent::Live {
                    agent,
                    usage: Box::new(usage),
                }));
            });
        }
        UsageCommand::Local { provider } => {
            tokio::task::spawn_blocking(move || {
                let usage = local::provider_usage(&provider).map(Box::new);
                let _ = tx.unbounded_send(Event::Usage(UsageEvent::Local { provider, usage }));
            });
        }
    }
}
