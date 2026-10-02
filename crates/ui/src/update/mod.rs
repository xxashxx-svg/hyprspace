// The app updating itself, after the Tauri app's updater store and toast (src/stores/updater.ts,
// src/components/Updater.tsx): it checks on launch, every 6 hours, and when the window comes back
// after 15 minutes away, quietly. When a release is out, a small card in the corner offers to
// restart and update; Settings, General shows the same state with a button to check now. The
// engine does the downloading and installing (engine/src/update.rs).
//
// After an update the first launch shows what's new in this version, from the changelog built
// into the app, like the Tauri app's WhatsNew.tsx.

mod changelog;
mod toast;

pub use toast::overlay;

use std::time::{Duration, Instant};

use gpui::{Context, Task};
use hyprspace_proto::update::{Release, Step};
use hyprspace_proto::{Client, Command, UpdateCommand, UpdateEvent};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const RECHECK: Duration = Duration::from_secs(6 * 60 * 60);
const FOCUS_THROTTLE: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Checking,
    Available(Release),
    UpToDate,
    Busy(Step),
    Failed(String),
    /// A build folder, which never updates itself.
    Unmanaged,
}

pub struct Updater {
    client: Client,
    pub(crate) phase: Phase,
    /// The automatic checks stay quiet when they fail; only "Check for updates" says so.
    silent: bool,
    /// The card was closed. It comes back for the next release or error.
    dismissed: bool,
    last_check: Option<Instant>,
    /// What's new in this version, until it is closed.
    pub(crate) whats_new: Option<Vec<String>>,
    _poll: Task<()>,
}

impl Updater {
    pub fn new(client: Client, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                if this.update(cx, |u, cx| u.check(true, cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(RECHECK).await;
            }
        });
        Self {
            client,
            phase: Phase::Idle,
            silent: true,
            dismissed: false,
            last_check: None,
            whats_new: None,
            _poll: poll,
        }
    }

    pub fn check(&mut self, silent: bool, cx: &mut Context<Self>) {
        if matches!(
            self.phase,
            Phase::Checking | Phase::Busy(_) | Phase::Unmanaged
        ) {
            return;
        }
        self.silent = silent;
        self.last_check = Some(Instant::now());
        self.phase = Phase::Checking;
        self.client.send(Command::Update(UpdateCommand::Check));
        cx.notify();
    }

    /// The window came back to the front: check again if it has been a while.
    pub fn focused(&mut self, cx: &mut Context<Self>) {
        if self
            .last_check
            .is_none_or(|t| t.elapsed() >= FOCUS_THROTTLE)
        {
            self.check(true, cx);
        }
    }

    pub fn install(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Busy(_) | Phase::Unmanaged) {
            return;
        }
        self.dismissed = false;
        self.phase = Phase::Busy(Step::Preparing);
        self.client.send(Command::Update(UpdateCommand::Install));
        cx.notify();
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.dismissed = true;
        cx.notify();
    }

    pub fn event(&mut self, e: UpdateEvent, cx: &mut Context<Self>) {
        self.phase = match e {
            UpdateEvent::Checked { release } => {
                self.dismissed = false;
                match release {
                    Some(r) => Phase::Available(r),
                    None => Phase::UpToDate,
                }
            }
            UpdateEvent::Unmanaged => Phase::Unmanaged,
            UpdateEvent::Step { step } => Phase::Busy(step),
            // Root quits the app; the installer takes it from here
            UpdateEvent::Quit => Phase::Busy(Step::Installing),
            UpdateEvent::Failed { message, install } => {
                if !install && self.silent {
                    Phase::Idle
                } else {
                    self.dismissed = false;
                    Phase::Failed(message)
                }
            }
        };
        cx.notify();
    }

    /// Called once the saved state has loaded with the version that ran last. Returns true when
    /// that differs, so the caller saves this version as seen.
    pub fn launched_after(&mut self, seen: &str, cx: &mut Context<Self>) -> bool {
        if seen == VERSION {
            return false;
        }
        // a first-ever run has nothing to compare and stays quiet
        if !seen.is_empty() {
            self.whats_new = Some(changelog::notes(VERSION));
            cx.notify();
        }
        true
    }

    pub fn close_whats_new(&mut self, cx: &mut Context<Self>) {
        self.whats_new = None;
        cx.notify();
    }

    /// The line under the version in Settings, General.
    pub(crate) fn status(&self) -> String {
        match &self.phase {
            Phase::Idle => "Checks for updates on launch".into(),
            Phase::Checking => "Checking for updates".into(),
            Phase::Available(r) => format!("Version {} is ready to install", r.version),
            Phase::UpToDate => "You're on the latest version".into(),
            Phase::Busy(step) => step_text(*step),
            Phase::Failed(m) => m.clone(),
            Phase::Unmanaged => {
                "This copy runs from a build folder, so it doesn't update itself".into()
            }
        }
    }

    /// Download progress from 0 to 1, or None while the size is unknown.
    pub(crate) fn progress(&self) -> Option<f32> {
        match self.phase {
            Phase::Busy(Step::Downloading { percent: Some(p) }) => Some(p as f32 / 100.),
            _ => None,
        }
    }
}

fn step_text(step: Step) -> String {
    match step {
        Step::Preparing => "Checking for the latest version".into(),
        Step::Downloading { percent: Some(p) } => format!("Downloading {p}%"),
        Step::Downloading { percent: None } => "Downloading".into(),
        Step::Installing => "Installing".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_read_like_the_tauri_app() {
        assert_eq!(
            step_text(Step::Downloading { percent: Some(40) }),
            "Downloading 40%"
        );
        assert_eq!(step_text(Step::Installing), "Installing");
    }
}
