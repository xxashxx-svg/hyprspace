// Stop's last resort. Each harness kills a CLI that ignores an interrupt for 5 seconds, but only
// from its own loop, and a loop stuck awaiting the CLI never gets there. A run still going after
// `wait` has a session like that, so the engine ends its task, which kills the CLI, and reports
// the run stopped the way the harness would have.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyprspace_proto::{Agent, RunEvent, RunStatus};
use tokio::task::AbortHandle;

use crate::{Say, lock};

pub(crate) const WAIT: Duration = Duration::from_secs(10);

/// The run that Stop was pressed on: its slot while live, the count of runs that have ended and
/// what it was at Stop, and when the run started.
pub(crate) struct Stopped {
    pub run: Arc<Mutex<Option<String>>>,
    pub ended: Arc<AtomicU64>,
    pub seen: u64,
    pub since: Instant,
}

pub(crate) async fn watch(
    wait: Duration,
    stopped: Stopped,
    task: AbortHandle,
    say: Say,
    agent: Agent,
) {
    tokio::time::sleep(wait).await;
    let over = || stopped.ended.load(Ordering::SeqCst) != stopped.seen;
    if over() || lock(&stopped.run).is_none() {
        return;
    }
    task.abort();
    while !task.is_finished() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // the run may have ended on its own while the task was going down
    if over() {
        return;
    }
    lock(&stopped.run).take();
    say(RunEvent::Finished {
        status: RunStatus::Interrupted,
        ms: stopped.since.elapsed().as_millis() as u64,
        text: String::new(),
        error: None,
    });
    say(RunEvent::Failed {
        message: format!(
            "{} did not stop when asked, so the session was closed.",
            agent.name()
        ),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard() -> (Say, Arc<Mutex<Vec<RunEvent>>>) {
        let got = Arc::new(Mutex::new(Vec::new()));
        let into = got.clone();
        (Arc::new(move |e| lock(&into).push(e)), got)
    }

    fn stopped(live: bool) -> Stopped {
        Stopped {
            run: Arc::new(Mutex::new(live.then(String::new))),
            ended: Arc::new(AtomicU64::new(0)),
            seen: 0,
            since: Instant::now(),
        }
    }

    #[tokio::test]
    async fn a_run_stuck_past_stop_is_ended_and_its_session_closed() {
        let stuck = tokio::spawn(std::future::pending::<()>());
        let (say, got) = heard();
        let s = stopped(true);
        let run = s.run.clone();
        watch(
            Duration::from_millis(20),
            s,
            stuck.abort_handle(),
            say,
            Agent::Codex,
        )
        .await;
        assert!(stuck.is_finished());
        assert!(lock(&run).is_none());
        let got = lock(&got);
        assert!(matches!(
            got.as_slice(),
            [RunEvent::Finished { status: RunStatus::Interrupted, .. }, RunEvent::Failed { message }]
                if message.starts_with("Codex did not stop")
        ));
    }

    #[tokio::test]
    async fn a_run_that_stopped_on_its_own_is_left_alone() {
        let live = tokio::spawn(std::future::pending::<()>());
        let (say, got) = heard();
        let s = stopped(true);
        let ended = s.ended.clone();
        let watching = tokio::spawn(watch(
            Duration::from_millis(50),
            s,
            live.abort_handle(),
            say,
            Agent::Claude,
        ));
        ended.fetch_add(1, Ordering::SeqCst);
        watching.await.unwrap();
        assert!(!live.is_finished());
        assert!(lock(&got).is_empty());
        live.abort();
    }

    #[tokio::test]
    async fn stop_between_runs_does_nothing() {
        let idle = tokio::spawn(std::future::pending::<()>());
        let (say, got) = heard();
        watch(
            Duration::from_millis(20),
            stopped(false),
            idle.abort_handle(),
            say,
            Agent::Claude,
        )
        .await;
        assert!(!idle.is_finished());
        assert!(lock(&got).is_empty());
        idle.abort();
    }
}
