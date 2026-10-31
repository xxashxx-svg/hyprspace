// The app updating itself, after the Tauri app's updater store and toast (src/stores/updater.ts,
// src/components/Updater.tsx): it checks on launch, every 6 hours, and when the window comes back
// after 15 minutes away, quietly. When a release is out, a small card in the corner offers to
// restart and update; Settings, About shows the same state with a button to check now. The
// engine does the downloading and installing (engine/src/update.rs).
//
// After an update the first launch shows what's new in this version, from the changelog built
// into the app, like the Tauri app's WhatsNew.tsx.
//
// The same checks ask about Claude and Codex (`agents.rs`). "Restart and update" updates any agent
// that's behind first, then the app.

mod agents;
mod changelog;
mod toast;

pub(crate) use agents::{names, said, versions};

pub use toast::overlay;

use std::time::{Duration, Instant};

use gpui::{Context, Task};
use hyprspace_proto::update::{Release, Step};
use hyprspace_proto::{
    Agent, AgentUpdateCommand, AgentUpdateEvent, Client, Command, UpdateCommand, UpdateEvent,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const RECHECK: Duration = Duration::from_secs(6 * 60 * 60);
const FOCUS_THROTTLE: Duration = Duration::from_secs(15 * 60);
const FOLD: Duration = Duration::from_secs(6);

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
    pub(crate) confirm: Option<usize>,
    last_check: Option<Instant>,
    /// What's new in this version, until it is closed.
    pub(crate) whats_new: Option<Vec<String>>,
    pub(crate) swipe: Option<Swipe>,
    pub(crate) agents: agents::Agents,
    /// The npm check's switch, unknown until the saved state loads so a check never runs before
    /// it can be honored.
    agent_checks: Option<bool>,
    /// "Restart and update" is waiting for the agent updates it started.
    install_after: bool,
    /// The corner chip is grown into its list.
    pub(crate) open: bool,
    /// How many things the update running now covers, for "1 of 3".
    batch: usize,
    fold: Option<Task<()>>,
    _poll: Task<()>,
}

#[derive(Clone, Copy)]
pub(crate) struct Swipe {
    pub(crate) news: bool,
    pub(crate) from: f32,
    pub(crate) dx: f32,
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
            swipe: None,
            confirm: None,
            last_check: None,
            whats_new: None,
            agents: Default::default(),
            agent_checks: None,
            install_after: false,
            open: false,
            batch: 0,
            fold: None,
            _poll: poll,
        }
    }

    pub fn check(&mut self, silent: bool, cx: &mut Context<Self>) {
        if let Some(latest) = self.agent_checks {
            self.check_agents(latest);
        }
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

    pub fn install(&mut self, working: usize, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Busy(_) | Phase::Unmanaged) {
            return;
        }
        self.dismissed = false;
        if working > 0 && self.confirm.is_none() {
            self.confirm = Some(working);
            self.open = true;
            cx.notify();
            return;
        }
        self.confirm = None;
        self.open = false;
        if self.install_after {
            return;
        }
        let behind: Vec<Agent> = self.agents.behind().into_iter().map(|(a, _)| a).collect();
        if !behind.is_empty() {
            self.install_after = true;
            self.update_agents(&behind, true, cx);
            return;
        }
        self.start_install(cx);
    }

    fn start_install(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Busy(Step::Preparing);
        self.client.send(Command::Update(UpdateCommand::Install));
        cx.notify();
    }

    /// The saved switch for the npm check. The first time it's known, the agents get checked.
    pub fn set_agent_checks(&mut self, on: bool) {
        let first = self.agent_checks.is_none();
        self.agent_checks = Some(on);
        if first || on {
            self.check_agents(on);
        }
    }

    fn check_agents(&self, latest: bool) {
        self.client
            .send(Command::AgentUpdate(AgentUpdateCommand::Check { latest }));
    }

    /// Settings, Agents opened: how each one is installed now, without asking npm again.
    pub fn look_at_agents(&self) {
        self.check_agents(false);
    }

    /// `announce` puts the results on the corner chip.
    pub fn update_agents(&mut self, list: &[Agent], announce: bool, cx: &mut Context<Self>) {
        if announce {
            self.open = false;
            self.fold = None;
            self.batch = list.len() + usize::from(self.install_after);
        }
        for &agent in list {
            if self.agents.running.contains(&agent) {
                continue;
            }
            self.agents.start(agent, announce);
            self.client
                .send(Command::AgentUpdate(AgentUpdateCommand::Run { agent }));
        }
        cx.notify();
    }

    pub fn revert_agent(&mut self, agent: Agent, version: String, cx: &mut Context<Self>) {
        if self.agents.running.contains(&agent) {
            return;
        }
        self.agents.start(agent, false);
        self.client
            .send(Command::AgentUpdate(AgentUpdateCommand::Revert {
                agent,
                version,
            }));
        cx.notify();
    }

    pub fn agent_event(&mut self, e: AgentUpdateEvent, cx: &mut Context<Self>) {
        self.agents.event(e);
        if self.install_after && self.agents.running.is_empty() {
            self.install_after = false;
            self.start_install(cx);
        }
        self.fold_when_done(cx);
        cx.notify();
    }

    /// A batch that all went well folds its "Updated" chip away by itself after a few seconds; a
    /// failure stays until it's read and closed.
    fn fold_when_done(&mut self, cx: &mut Context<Self>) {
        let done = self.agents.announced();
        let clean = !done.is_empty()
            && self.agents.running.is_empty()
            && !done
                .iter()
                .any(|(_, o)| matches!(o, hyprspace_proto::agent_update::Outcome::Failed { .. }));
        if !clean || self.fold.is_some() {
            return;
        }
        self.fold = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FOLD).await;
            let _ = this.update(cx, |u, cx| {
                u.fold = None;
                if !u.open {
                    u.agents.close_announced();
                    cx.notify();
                }
            });
        }));
    }

    /// The batch running now: how many are done, out of how many.
    pub(crate) fn tally(&self) -> Option<(usize, usize)> {
        let left = self.agents.running.len() + usize::from(self.install_after);
        (self.batch > 1 && left <= self.batch).then(|| (self.batch - left, self.batch))
    }

    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.open == open {
            return;
        }
        self.open = open;
        if !open {
            self.confirm = None;
        }
        cx.notify();
    }

    /// The chip's close: everything it offered waits for the next version, and results go.
    pub fn dismiss_all(&mut self, cx: &mut Context<Self>) {
        self.dismissed = true;
        self.confirm = None;
        self.open = false;
        self.agents.dismiss();
        self.agents.close_announced();
        cx.notify();
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.dismissed = true;
        self.confirm = None;
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

    /// This version's notes again, from Settings, About.
    pub fn show_whats_new(&mut self, cx: &mut Context<Self>) {
        self.whats_new = Some(changelog::notes(VERSION));
        cx.notify();
    }

    pub(crate) fn end_swipe(&mut self, news: bool, cx: &mut Context<Self>) {
        let Some(s) = self.swipe.take().filter(|s| s.news == news) else {
            return;
        };
        if s.dx >= toast::DISMISS {
            if news {
                self.close_whats_new(cx);
            } else {
                self.dismiss(cx);
            }
        }
        cx.notify();
    }

    pub fn close_whats_new(&mut self, cx: &mut Context<Self>) {
        self.whats_new = None;
        cx.notify();
    }

    /// The line under the version in Settings, About.
    pub(crate) fn status(&self) -> String {
        if let (Some(n), Phase::Available(_) | Phase::Failed(_)) = (self.confirm, &self.phase) {
            return running(n);
        }
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

    pub(crate) fn checked_by_hand(&self) -> bool {
        !self.silent
    }

    /// Download progress from 0 to 1, or None while the size is unknown.
    pub(crate) fn progress(&self) -> Option<f32> {
        match self.phase {
            Phase::Busy(Step::Downloading { percent: Some(p) }) => Some(p as f32 / 100.),
            _ => None,
        }
    }
}

/// Whether the built-in changelog has notes for this version. A build between releases has none.
pub(crate) fn has_notes() -> bool {
    !changelog::notes(VERSION).is_empty()
}

fn running(n: usize) -> String {
    match n {
        1 => "1 thread is still running. Updating stops it.".into(),
        n => format!("{n} threads are still running. Updating stops them."),
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
