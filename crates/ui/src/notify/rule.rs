use std::collections::{HashMap, HashSet};

use hyprspace_proto::NotifyPrefs;

use crate::transcript::Status;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Waiting,
    Failed,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reach {
    Nothing,
    Toast,
    Os,
}

impl Kind {
    // only a live run moves out of Working; a view loading its journal starts from no status or Idle
    pub fn of(before: Option<Status>, now: Status) -> Option<Kind> {
        match (before?, now) {
            (Status::Working, Status::Waiting) => Some(Kind::Waiting),
            (Status::Working | Status::Waiting, Status::Failed) => Some(Kind::Failed),
            (Status::Working | Status::Waiting, Status::Done) => Some(Kind::Done),
            _ => None,
        }
    }

    pub fn on(self, prefs: &NotifyPrefs) -> bool {
        prefs.on
            && match self {
                Kind::Waiting => prefs.waiting,
                Kind::Failed => prefs.failed,
                Kind::Done => prefs.finished,
            }
    }

    pub fn status(self) -> Status {
        match self {
            Kind::Waiting => Status::Waiting,
            Kind::Failed => Status::Failed,
            Kind::Done => Status::Done,
        }
    }

    pub fn body(self, detail: Option<String>) -> String {
        match (self, detail) {
            (Kind::Waiting, Some(d)) => format!("Waiting on you. {d}"),
            (Kind::Waiting, None) => "Waiting on you.".into(),
            (Kind::Failed, Some(d)) => format!("Failed: {d}"),
            (Kind::Failed, None) => "Failed.".into(),
            (Kind::Done, Some(d)) => format!("Done. {d}"),
            (Kind::Done, None) => "Done.".into(),
        }
    }

    pub fn toast(self) -> &'static str {
        match self {
            Kind::Waiting => "Waiting on you",
            Kind::Failed => "Failed",
            Kind::Done => "Done",
        }
    }
}

pub fn reach(focused: bool, on_screen: bool) -> Reach {
    match (focused, on_screen) {
        (false, _) => Reach::Os,
        (true, false) => Reach::Toast,
        (true, true) => Reach::Nothing,
    }
}

pub fn need_you(unseen: &HashSet<u64>, status: &HashMap<u64, Status>) -> usize {
    let waiting = status
        .iter()
        .filter(|(id, s)| **s == Status::Waiting && !unseen.contains(id))
        .count();
    unseen.len() + waiting
}

#[cfg(test)]
mod tests {
    use super::*;
    use Status::*;

    #[test]
    fn only_a_run_moving_on_fires() {
        assert_eq!(Kind::of(Some(Working), Waiting), Some(Kind::Waiting));
        assert_eq!(Kind::of(Some(Working), Failed), Some(Kind::Failed));
        assert_eq!(Kind::of(Some(Waiting), Failed), Some(Kind::Failed));
        assert_eq!(Kind::of(Some(Working), Done), Some(Kind::Done));
        assert_eq!(Kind::of(Some(Waiting), Done), Some(Kind::Done));
        assert_eq!(Kind::of(Some(Working), Idle), None);
        assert_eq!(Kind::of(Some(Working), Working), None);
    }

    #[test]
    fn a_journal_replayed_or_a_repeat_stays_quiet() {
        for now in [Waiting, Failed, Done] {
            assert_eq!(Kind::of(None, now), None);
            assert_eq!(Kind::of(Some(Idle), now), None);
            assert_eq!(Kind::of(Some(now), now), None);
        }
        assert_eq!(Kind::of(Some(Done), Waiting), None);
        assert_eq!(Kind::of(Some(Failed), Done), None);
    }

    #[test]
    fn nothing_notifies_until_switched_on() {
        let off = NotifyPrefs::default();
        assert!(!Kind::Waiting.on(&off) && !Kind::Failed.on(&off) && !Kind::Done.on(&off));
        let prefs = NotifyPrefs { on: true, ..off };
        assert!(Kind::Waiting.on(&prefs) && Kind::Failed.on(&prefs));
        assert!(!Kind::Done.on(&prefs));
        let quiet = NotifyPrefs {
            waiting: false,
            ..prefs
        };
        assert!(!Kind::Waiting.on(&quiet));
    }

    #[test]
    fn the_thread_in_front_of_you_never_notifies() {
        assert_eq!(reach(true, true), Reach::Nothing);
        assert_eq!(reach(true, false), Reach::Toast);
        assert_eq!(reach(false, true), Reach::Os);
        assert_eq!(reach(false, false), Reach::Os);
    }

    #[test]
    fn the_badge_counts_each_thread_that_needs_you_once() {
        let unseen = HashSet::from([1, 2]);
        let status = HashMap::from([(2, Waiting), (3, Waiting), (4, Working), (5, Done)]);
        assert_eq!(need_you(&unseen, &status), 3);
        assert_eq!(need_you(&HashSet::new(), &HashMap::new()), 0);
    }

    #[test]
    fn the_body_says_what_happened() {
        assert_eq!(
            Kind::Waiting.body(Some("Run cargo test?".into())),
            "Waiting on you. Run cargo test?"
        );
        assert_eq!(Kind::Failed.body(Some("exit 1".into())), "Failed: exit 1");
        assert_eq!(Kind::Done.body(None), "Done.");
    }
}
