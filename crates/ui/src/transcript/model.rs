// What a transcript holds, built from prompts, answers and run events. Live events and a
// journal replay go through the same calls, so a thread reopened after a restart looks the way
// it did. No GPUI here, so all of it is unit tested.

use std::path::PathBuf;
use std::time::Instant;

use hyprspace_proto::{Answer, Prompt, RunEvent, RunStatus, Tool};

use crate::markdown::{self, Block};

pub enum Item {
    User {
        text: String,
        images: Vec<PathBuf>,
        /// Sent while a run was live, so it joined that run.
        steer: bool,
    },
    Text {
        source: String,
        /// Parsed lazily: a streaming reply changes many times between paints.
        blocks: Option<Vec<Block>>,
    },
    Thinking {
        text: String,
        open: bool,
    },
    Tool {
        id: String,
        tool: Tool,
        /// (ok, output) once the call ended.
        done: Option<(bool, String)>,
        open: bool,
    },
    Approval {
        request: String,
        tool: Tool,
        reason: Option<String>,
        always: bool,
        answer: Option<Answer>,
        /// The session ended before anyone answered.
        expired: bool,
    },
    Error(String),
    Finished {
        status: RunStatus,
        ms: u64,
        error: Option<String>,
        tokens: Option<(u64, u64)>,
    },
    Note(String),
}

/// Where a thread stands, for the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Working,
    Waiting,
    Done,
    Failed,
}

struct Run {
    since: Instant,
    tokens: Option<(u64, u64)>,
    said: bool,
}

#[derive(Default)]
pub struct Transcript {
    pub items: Vec<Item>,
    run: Option<Run>,
    /// The CLI's own name for its model, once it started.
    pub model: Option<String>,
    /// The id `Launch::resume` takes, once the CLI started.
    pub thread: Option<String>,
    pub cwd: Option<PathBuf>,
    last: Option<RunStatus>,
    failed: bool,
}

impl Transcript {
    pub fn running(&self) -> bool {
        self.run.is_some()
    }

    /// Seconds the live run has taken so far.
    pub fn elapsed(&self) -> Option<u64> {
        self.run.as_ref().map(|r| r.since.elapsed().as_secs())
    }

    pub fn status(&self) -> Status {
        let waiting = self.items.iter().any(|i| {
            matches!(
                i,
                Item::Approval {
                    answer: None,
                    expired: false,
                    ..
                }
            )
        });
        if waiting {
            Status::Waiting
        } else if self.run.is_some() {
            Status::Working
        } else if self.failed || self.last == Some(RunStatus::Failed) {
            Status::Failed
        } else if self.last.is_some() {
            Status::Done
        } else {
            Status::Idle
        }
    }

    /// A prompt the user sent. Mid-run it steers; otherwise it starts a run.
    pub fn prompt(&mut self, prompt: &Prompt) {
        self.items.push(Item::User {
            text: prompt.text.clone(),
            images: prompt.images.clone(),
            steer: self.run.is_some(),
        });
        if self.run.is_none() {
            self.failed = false;
            self.run = Some(Run {
                since: Instant::now(),
                tokens: None,
                said: false,
            });
        }
    }

    pub fn answered(&mut self, request: &str, answer: Answer) {
        for item in self.items.iter_mut().rev() {
            if let Item::Approval {
                request: r,
                answer: a,
                ..
            } = item
                && r == request
            {
                *a = Some(answer);
                return;
            }
        }
    }

    /// The engine could not start or reach the session.
    pub fn fail(&mut self, message: String) {
        self.items.push(Item::Error(message));
        self.end_session();
    }

    pub fn note(&mut self, text: impl Into<String>) {
        self.items.push(Item::Note(text.into()));
    }

    pub fn apply(&mut self, event: RunEvent) {
        match event {
            RunEvent::Started {
                model, thread, cwd, ..
            } => {
                self.model = Some(model);
                self.thread = Some(thread);
                self.cwd = Some(cwd);
            }
            RunEvent::Text { text } => {
                if let Some(run) = self.run.as_mut() {
                    run.said = true;
                }
                match self.items.last_mut() {
                    Some(Item::Text { source, blocks }) => {
                        source.push_str(&text);
                        *blocks = None;
                    }
                    _ => self.items.push(Item::Text {
                        source: text,
                        blocks: None,
                    }),
                }
            }
            RunEvent::Thinking { text } => match self.items.last_mut() {
                Some(Item::Thinking { text: t, .. }) => t.push_str(&text),
                _ => self.items.push(Item::Thinking { text, open: false }),
            },
            RunEvent::Tool { id, tool } => match self.tool(&id) {
                Some(Item::Tool { tool: t, .. }) => *t = tool,
                _ => self.items.push(Item::Tool {
                    id,
                    tool,
                    done: None,
                    open: false,
                }),
            },
            RunEvent::ToolDone { id, ok, output } => {
                if let Some(Item::Tool { done, .. }) = self.tool(&id) {
                    *done = Some((ok, output));
                }
            }
            RunEvent::Approval {
                request,
                tool,
                reason,
                always,
            } => self.items.push(Item::Approval {
                request,
                tool,
                reason,
                always,
                answer: None,
                expired: false,
            }),
            RunEvent::Steered => {}
            RunEvent::Usage { input, output } => {
                if let Some(run) = self.run.as_mut() {
                    run.tokens = Some((input, output));
                }
            }
            RunEvent::Error { message } => self.items.push(Item::Error(message)),
            RunEvent::Finished {
                status,
                ms,
                text,
                error,
            } => {
                let run = self.run.take();
                if !text.is_empty() && !run.as_ref().is_some_and(|r| r.said) {
                    self.items.push(Item::Text {
                        source: text,
                        blocks: None,
                    });
                }
                self.expire();
                self.last = Some(status);
                // the CLI often reports a failure as an error and again as the run's end
                let said = |e: &String| {
                    self.items
                        .iter()
                        .rev()
                        .take_while(|i| !matches!(i, Item::User { .. }))
                        .any(|i| matches!(i, Item::Error(m) if m == e))
                };
                let error = error.filter(|e| !said(e));
                self.items.push(Item::Finished {
                    status,
                    ms,
                    error,
                    tokens: run.and_then(|r| r.tokens).filter(|t| *t != (0, 0)),
                });
            }
            RunEvent::Failed { message } => {
                self.items.push(Item::Error(message));
                self.end_session();
            }
        }
    }

    /// After a journal replay: whatever was live when the app closed is over now.
    pub fn settle(&mut self) {
        if self.run.take().is_some() {
            self.note("The app closed before this run finished.");
        }
        self.expire();
    }

    fn end_session(&mut self) {
        self.run = None;
        self.failed = true;
        self.expire();
    }

    // an approval left open when its run or session ended can't be answered any more
    fn expire(&mut self) {
        for item in &mut self.items {
            if let Item::Approval {
                answer: None,
                expired,
                ..
            } = item
            {
                *expired = true;
            }
        }
    }

    fn tool(&mut self, id: &str) -> Option<&mut Item> {
        self.items
            .iter_mut()
            .rev()
            .find(|i| matches!(i, Item::Tool { id: t, .. } if t == id))
    }

    /// Parses any reply text that changed since the last paint.
    pub fn parse(&mut self) {
        for item in &mut self.items {
            if let Item::Text { source, blocks } = item
                && blocks.is_none()
            {
                *blocks = Some(markdown::parse(source));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> RunEvent {
        RunEvent::Text { text: s.into() }
    }

    fn finished(status: RunStatus, text: &str) -> RunEvent {
        RunEvent::Finished {
            status,
            ms: 1200,
            text: text.into(),
            error: None,
        }
    }

    fn approval(request: &str) -> RunEvent {
        RunEvent::Approval {
            request: request.into(),
            tool: Tool::Command {
                command: "ls".into(),
            },
            reason: None,
            always: true,
        }
    }

    #[test]
    fn a_run_streams_then_finishes() {
        let mut t = Transcript::default();
        assert_eq!(t.status(), Status::Idle);
        t.prompt(&Prompt::text("hi"));
        assert_eq!(t.status(), Status::Working);
        t.apply(text("Hel"));
        t.apply(text("lo"));
        t.apply(RunEvent::Usage {
            input: 10,
            output: 2,
        });
        t.apply(finished(RunStatus::Done, "Hello"));
        assert_eq!(t.status(), Status::Done);
        // the final text is not added twice when it already streamed
        let texts: Vec<_> = t
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Text { source, .. } => Some(source.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["Hello"]);
        assert!(matches!(
            t.items.last(),
            Some(Item::Finished {
                tokens: Some((10, 2)),
                ..
            })
        ));
    }

    #[test]
    fn steers_join_the_live_run_and_approvals_wait() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("go"));
        t.prompt(&Prompt::text("left"));
        assert!(matches!(&t.items[1], Item::User { steer: true, .. }));
        t.apply(approval("r1"));
        assert_eq!(t.status(), Status::Waiting);
        t.answered("r1", Answer::AllowAlways);
        assert_eq!(t.status(), Status::Working);
        t.apply(approval("r2"));
        t.apply(finished(RunStatus::Done, "ok"));
        assert!(matches!(
            &t.items[3],
            Item::Approval {
                expired: true,
                answer: None,
                ..
            }
        ));
        // a reply that never streamed shows from the final text
        assert!(matches!(&t.items[4], Item::Text { source, .. } if source == "ok"));
        // the next prompt starts a fresh run, not a steer
        t.prompt(&Prompt::text("again"));
        assert!(matches!(
            t.items.last(),
            Some(Item::User { steer: false, .. })
        ));
    }

    #[test]
    fn tools_update_in_place_and_failures_end_the_session() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("x"));
        let tool = |c: &str| RunEvent::Tool {
            id: "t1".into(),
            tool: Tool::Command { command: c.into() },
        };
        t.apply(tool("l"));
        t.apply(tool("ls"));
        t.apply(RunEvent::ToolDone {
            id: "t1".into(),
            ok: true,
            output: "a.txt".into(),
        });
        let tools: Vec<_> = t
            .items
            .iter()
            .filter(|i| matches!(i, Item::Tool { .. }))
            .collect();
        assert_eq!(tools.len(), 1);
        assert!(
            matches!(tools[0], Item::Tool { done: Some((true, _)), tool: Tool::Command { command }, .. } if command == "ls")
        );
        t.apply(RunEvent::Failed {
            message: "gone".into(),
        });
        assert_eq!(t.status(), Status::Failed);
        assert!(!t.running());
    }

    #[test]
    fn a_failure_is_said_once() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("x"));
        t.apply(RunEvent::Error {
            message: "model refused".into(),
        });
        t.apply(RunEvent::Usage {
            input: 0,
            output: 0,
        });
        t.apply(RunEvent::Finished {
            status: RunStatus::Failed,
            ms: 5,
            text: String::new(),
            error: Some("model refused".into()),
        });
        assert!(matches!(
            t.items.last(),
            Some(Item::Finished {
                error: None,
                tokens: None,
                ..
            })
        ));
    }

    #[test]
    fn a_replay_cut_off_mid_run_settles() {
        let mut t = Transcript::default();
        t.apply(RunEvent::Started {
            agent: hyprspace_proto::Agent::Claude,
            model: "m".into(),
            thread: "th".into(),
            cwd: PathBuf::from("/w"),
        });
        t.prompt(&Prompt::text("x"));
        t.apply(approval("r1"));
        t.settle();
        assert_eq!(t.thread.as_deref(), Some("th"));
        assert!(!t.running());
        assert!(matches!(t.items.last(), Some(Item::Note(_))));
        assert_ne!(t.status(), Status::Waiting);
        t.parse();
    }
}
