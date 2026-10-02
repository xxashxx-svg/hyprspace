// The app updating itself, on hyprspace-update: the same feed, key and installer arguments the
// Tauri app's updater uses (docs/adr/0011-updater.md). A check answers with what the feed says;
// an install checks again, downloads, verifies, starts the installer (or swaps the macOS bundle)
// and tells the UI to quit so the install can finish.

use std::sync::atomic::{AtomicBool, Ordering};

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::update::{Release, Step};
use hyprspace_proto::{Event, UpdateCommand, UpdateEvent};

/// The running version: every crate shares the workspace's.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One install at a time. A second click while one runs is ignored.
static INSTALLING: AtomicBool = AtomicBool::new(false);

pub fn handle(cmd: UpdateCommand, tx: UnboundedSender<Event>) {
    let send = move |e: UpdateEvent| {
        let _ = tx.unbounded_send(Event::Update(e));
    };
    let Some(install) = hyprspace_update::installed() else {
        send(UpdateEvent::Unmanaged);
        return;
    };
    match cmd {
        UpdateCommand::Check => {
            tokio::spawn(async move {
                send(match hyprspace_update::check(VERSION).await {
                    Ok(up) => UpdateEvent::Checked {
                        release: up.map(release),
                    },
                    Err(e) => UpdateEvent::Failed {
                        message: format!("Couldn't reach the update server: {e:#}"),
                        install: false,
                    },
                });
            });
        }
        UpdateCommand::Install => {
            if INSTALLING.swap(true, Ordering::SeqCst) {
                return;
            }
            tokio::spawn(async move {
                let end = run(&install, &send).await;
                INSTALLING.store(false, Ordering::SeqCst);
                send(match end {
                    Ok(Some(())) => UpdateEvent::Quit,
                    Ok(None) => UpdateEvent::Checked { release: None },
                    Err(e) => UpdateEvent::Failed {
                        message: failure(&format!("{e:#}")),
                        install: true,
                    },
                });
            });
        }
    }
}

/// Clears the installers earlier updates left in the temp folder. Runs at launch, which right
/// after an update means the installer just started us and may still hold its own file for a
/// moment, so whatever is busy gets a few more tries.
pub fn sweep() {
    if hyprspace_update::installed().is_none() {
        return;
    }
    std::thread::spawn(|| {
        let temp = std::env::temp_dir();
        for _ in 0..15 {
            if hyprspace_update::sweep(&temp) == 0 {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });
}

fn release(up: hyprspace_update::Update) -> Release {
    Release {
        version: up.version,
        notes: up.notes,
    }
}

/// Some(()) once the install is handed off, None when there turned out to be nothing newer.
async fn run(
    install: &hyprspace_update::Install,
    send: &impl Fn(UpdateEvent),
) -> anyhow::Result<Option<()>> {
    send(UpdateEvent::Step {
        step: Step::Preparing,
    });
    let Some(up) = hyprspace_update::check(VERSION).await? else {
        return Ok(None);
    };
    let mut shown = None;
    let bytes = hyprspace_update::download(&up, |got, total| {
        let percent = total
            .filter(|t| *t > 0)
            .map(|t| (got * 100 / t).min(100) as u8);
        // one event per percent, not per chunk
        if shown != Some(percent) {
            shown = Some(percent);
            send(UpdateEvent::Step {
                step: Step::Downloading { percent },
            });
        }
    })
    .await?;
    send(UpdateEvent::Step {
        step: Step::Installing,
    });
    let install = install.clone();
    tokio::task::spawn_blocking(move || {
        let staged = hyprspace_update::stage(&up, &bytes)?;
        hyprspace_update::install(&staged, &install)
    })
    .await??;
    Ok(Some(()))
}

/// Says what went wrong in words that suggest the next step, like the Tauri app's updater did.
fn failure(msg: &str) -> String {
    let lower = msg.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    if has(&["signature", "verif"]) {
        "That download didn't verify. It may be incomplete. Try again.".into()
    } else if has(&[
        "permission",
        "denied",
        "access",
        "in use",
        "os error 5",
        "os error 32",
    ]) {
        "Couldn't replace the app. Close other copies of HyprSpace and try again.".into()
    } else if has(&[
        "connect",
        "timed out",
        "dns",
        "tls",
        "certificate",
        "network",
    ]) {
        "Couldn't download the update. Check your connection.".into()
    } else {
        let short: String = msg.chars().take(120).collect();
        format!("Update failed: {short}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_suggest_the_next_step() {
        assert!(failure("the download doesn't match its signature").contains("didn't verify"));
        assert!(failure("Access is denied. (os error 5)").contains("Close other copies"));
        assert!(failure("error sending request: connection refused").contains("connection"));
        assert_eq!(failure("odd"), "Update failed: odd");
    }

    #[test]
    fn a_build_folder_never_installs() {
        // the test binary runs from target/, which no installer wrote
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        handle(UpdateCommand::Install, tx);
        assert_eq!(
            rx.try_recv().ok(),
            Some(Event::Update(UpdateEvent::Unmanaged))
        );
    }
}
