// A thread's journal: one JSON line per prompt, answer and run event, appended as they happen,
// so the transcript can be rebuilt after a restart. Streamed text arrives a few characters at a
// time, so consecutive text (or thinking) pieces are joined into one line before they are
// written; a crash loses at most the reply that was still streaming. A phone watching the thread
// gets the journal so far and then each entry as it is recorded (`watch`).

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hyprspace_proto::{Entry, RunEvent};

pub struct Journal {
    inner: Mutex<Inner>,
}

/// Hears each entry as it is recorded, streamed text piece by piece. Returns false once it no
/// longer wants to, and is dropped.
pub type Watcher = Box<dyn FnMut(&Entry) -> bool + Send>;

struct Inner {
    path: PathBuf,
    file: Option<BufWriter<File>>,
    /// A text or thinking event still taking more pieces.
    pending: Option<RunEvent>,
    watchers: Vec<Watcher>,
}

/// Where the journal named `name` lives under `dir`. The name is reduced to one safe token, the
/// same way the state store does it, so it can't point outside `dir`.
pub fn path(dir: &Path, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join(format!("{safe}.jsonl"))
}

impl Journal {
    /// Opens for appending. A journal that can't be opened still works, it just keeps nothing:
    /// losing history must never stop a run.
    pub fn open(file: &Path) -> Self {
        let path = file.to_path_buf();
        let file = file
            .parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .ok()
            .and_then(|_| OpenOptions::new().create(true).append(true).open(file).ok())
            .map(BufWriter::new);
        Self {
            inner: Mutex::new(Inner {
                path,
                file,
                pending: None,
                watchers: Vec::new(),
            }),
        }
    }

    pub fn record(&self, entry: Entry) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.watchers.retain_mut(|w| w(&entry));
        if let Entry::Run { event } = &entry {
            match (&mut inner.pending, event) {
                (Some(RunEvent::Text { text: have }), RunEvent::Text { text })
                | (Some(RunEvent::Thinking { text: have }), RunEvent::Thinking { text }) => {
                    have.push_str(text);
                    return;
                }
                (_, RunEvent::Text { .. } | RunEvent::Thinking { .. }) => {
                    inner.flush_pending();
                    inner.pending = Some(event.clone());
                    return;
                }
                _ => {}
            }
        }
        inner.flush_pending();
        inner.write(&entry);
    }

    /// Adds a watcher. With `first`, hands it every entry recorded so far, under the same lock
    /// `record` takes, so the watcher misses nothing, hears nothing twice and hears it in order.
    pub fn watch(&self, first: Option<Box<dyn FnOnce(Vec<Entry>) + '_>>, watcher: Watcher) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(first) = first {
            // a reply still streaming is written out, to go on in a line of its own
            inner.flush_pending();
            first(load(&inner.path));
        }
        inner.watchers.push(watcher);
    }
}

impl Inner {
    fn flush_pending(&mut self) {
        if let Some(event) = self.pending.take() {
            self.write(&Entry::Run { event });
        }
    }

    fn write(&mut self, entry: &Entry) {
        let Some(file) = self.file.as_mut() else {
            return;
        };
        if let Ok(line) = serde_json::to_string(entry) {
            let _ = writeln!(file, "{line}").and_then(|_| file.flush());
        }
    }
}

impl Drop for Journal {
    fn drop(&mut self) {
        let inner = self.inner.get_mut().unwrap_or_else(|e| e.into_inner());
        inner.flush_pending();
    }
}

/// Every entry in the journal at `file`, oldest first. A line that does not parse (the torn last
/// line of a crash, or a shape from a newer build) is skipped.
pub fn load(file: &Path) -> Vec<Entry> {
    let Ok(f) = File::open(file) else {
        return Vec::new();
    };
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use hyprspace_proto::{Answer, Prompt, RunStatus};

    use super::*;

    fn run(event: RunEvent) -> Entry {
        Entry::Run { event }
    }

    fn text(s: &str) -> RunEvent {
        RunEvent::Text { text: s.into() }
    }

    #[test]
    fn joins_streamed_text_and_keeps_the_order() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path(), "thread-1");
        let j = Journal::open(&file);
        j.record(Entry::Prompt {
            prompt: Prompt::text("hi"),
        });
        j.record(run(RunEvent::Thinking { text: "hm".into() }));
        j.record(run(text("Hel")));
        j.record(run(text("lo")));
        j.record(Entry::Answer {
            request: "r1".into(),
            answer: Answer::Allow,
            answers: Vec::new(),
        });
        j.record(run(text("!")));
        drop(j);
        let got = load(&file);
        assert_eq!(
            got,
            vec![
                Entry::Prompt {
                    prompt: Prompt::text("hi")
                },
                run(RunEvent::Thinking { text: "hm".into() }),
                run(text("Hello")),
                Entry::Answer {
                    request: "r1".into(),
                    answer: Answer::Allow,
                    answers: Vec::new(),
                },
                run(text("!")),
            ]
        );
    }

    #[test]
    fn a_watcher_gets_the_history_then_every_piece() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::open(&path(dir.path(), "thread-2"));
        j.record(Entry::Prompt {
            prompt: Prompt::text("hi"),
        });
        j.record(run(text("Hel")));
        let heard = std::sync::Arc::new(Mutex::new(Vec::new()));
        let h = heard.clone();
        let mut before = Vec::new();
        j.watch(
            Some(Box::new(|e| before = e)),
            Box::new(move |e| {
                h.lock().unwrap().push(e.clone());
                true
            }),
        );
        assert_eq!(before.len(), 2);
        assert_eq!(before[1], run(text("Hel")));
        j.record(run(text("lo")));
        assert_eq!(*heard.lock().unwrap(), vec![run(text("lo"))]);
        drop(j);
        // the reply split where the watcher came in, and replays the same
        let all = load(&path(dir.path(), "thread-2"));
        assert_eq!(all[1..], [run(text("Hel")), run(text("lo"))]);
    }

    #[test]
    fn appends_across_opens_and_skips_torn_lines() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path(), "../evil");
        assert_eq!(file.parent(), Some(dir.path()));
        assert!(load(&file).is_empty());
        let finished = run(RunEvent::Finished {
            status: RunStatus::Done,
            ms: 5,
            text: String::new(),
            error: None,
        });
        Journal::open(&file).record(finished.clone());
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap()
            .write_all(b"{\"type\":\"run\",\"ev")
            .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        Journal::open(&file).record(finished.clone());
        assert_eq!(load(&file), vec![finished.clone(), finished]);
    }
}
