// Shared by the adapter tests: the fake CLI's path and a way to wait for events.

#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Duration;

use hyprspace_harness::Emit;
use hyprspace_proto::{RunEvent, RunStatus};
use tokio::sync::mpsc;

pub fn fake() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fake-cli"))
}

pub struct Events(mpsc::UnboundedReceiver<RunEvent>);

pub fn events() -> (Emit, Events) {
    let (tx, rx) = mpsc::unbounded_channel();
    let emit: Emit = Box::new(move |e| {
        let _ = tx.send(e);
    });
    (emit, Events(rx))
}

impl Events {
    pub async fn next(&mut self) -> RunEvent {
        tokio::time::timeout(Duration::from_secs(20), self.0.recv())
            .await
            .expect("an event within 20s")
            .expect("the session is still sending")
    }

    /// Every event up to and including the first that matches.
    pub async fn until(&mut self, want: impl Fn(&RunEvent) -> bool) -> Vec<RunEvent> {
        let mut out = Vec::new();
        loop {
            let e = self.next().await;
            let done = want(&e);
            out.push(e);
            if done {
                return out;
            }
        }
    }

    /// Every event up to and including the run's `Finished`.
    pub async fn run(&mut self) -> Vec<RunEvent> {
        self.until(|e| matches!(e, RunEvent::Finished { .. })).await
    }

    /// No event arrives for a moment, or the session is gone.
    pub async fn quiet(&mut self) {
        let got = tokio::time::timeout(Duration::from_millis(300), self.0.recv()).await;
        assert!(!matches!(got, Ok(Some(_))), "unexpected event {got:?}");
    }
}

pub fn text(events: &[RunEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            RunEvent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

pub fn finished(events: &[RunEvent]) -> (RunStatus, String, Option<String>) {
    let finishes: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RunEvent::Finished {
                status,
                text,
                error,
                ..
            } => Some((*status, text.clone(), error.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(finishes.len(), 1, "one Finished per run: {events:?}");
    finishes.into_iter().next().unwrap()
}

pub fn count(events: &[RunEvent], want: impl Fn(&RunEvent) -> bool) -> usize {
    events.iter().filter(|e| want(e)).count()
}
