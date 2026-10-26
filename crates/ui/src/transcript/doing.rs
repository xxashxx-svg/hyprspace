// What a live run is doing, in one line under the reply: each label stays up long enough to
// read, and the next one rolls up into its place.

use std::time::{Duration, Instant};

use super::model::{Status, Transcript};
use crate::slide::animations;

const HOLD: Duration = Duration::from_millis(850);
const ROLL: Duration = Duration::from_millis(340);

pub fn label(t: &Transcript, stopping: bool) -> String {
    if stopping {
        return "Stopping".into();
    }
    if t.compacting {
        return "Compacting".into();
    }
    if t.status() == Status::Waiting {
        return "Waiting for you".into();
    }
    "Working".into()
}

#[derive(Default)]
pub struct Doing {
    pub shown: String,
    pub prev: Option<String>,
    /// How far the roll from `prev` to `shown` has gone.
    pub roll: f32,
    since: Option<Instant>,
}

impl Doing {
    /// Takes `label` once what shows has been up for its hold, or at once when `now`. True
    /// while a later frame has more to show.
    pub fn step(&mut self, label: String, now: bool) -> bool {
        let at = Instant::now();
        let held = self.since.is_none_or(|s| at - s >= HOLD);
        if label != self.shown && (held || now || !animations()) {
            let old = std::mem::replace(&mut self.shown, label.clone());
            self.prev = (self.since.is_some() && animations()).then_some(old);
            self.since = Some(at);
        }
        self.roll = self
            .since
            .map_or(1., |s| (at - s).as_secs_f32() / ROLL.as_secs_f32())
            .min(1.);
        if self.roll >= 1. {
            self.prev = None;
        }
        self.prev.is_some() || label != self.shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprspace_proto::{Prompt, RunEvent, Tool};

    #[test]
    fn leaves_the_live_step_to_the_row_above_it() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("go"));
        t.apply(RunEvent::Thinking { text: "hm".into() });
        assert_eq!(label(&t, false), "Working");
        t.apply(RunEvent::Tool {
            id: "c1".into(),
            tool: Tool::Command {
                command: "cargo test".into(),
            },
        });
        assert_eq!(label(&t, false), "Working");
        assert_eq!(label(&t, true), "Stopping");
    }

    #[test]
    fn a_label_holds_before_the_next_takes_over() {
        let mut d = Doing::default();
        d.step("Working".into(), false);
        assert_eq!(d.shown, "Working");
        assert!(d.step("Writing".into(), false));
        assert_eq!(d.shown, "Working");
        d.step("Stopping".into(), true);
        assert_eq!(d.shown, "Stopping");
    }
}
