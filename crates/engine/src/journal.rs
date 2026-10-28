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
pub type First<'a> = Box<dyn FnOnce(&Path) + 'a>;

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

    /// Adds a watcher. With `first`, hands it the file holding every entry recorded so far, under
    /// the same lock `record` takes, so the watcher misses nothing, hears nothing twice and hears
    /// it in order.
    pub fn watch(&self, first: Option<First<'_>>, watcher: Watcher) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(first) = first {
            // a reply still streaming is written out, to go on in a line of its own
            inner.flush_pending();
            first(&inner.path);
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

/// The journal's last whole prompts that fit in about `budget` bytes, and at least the last, with
/// how many prompts came before them. Without a budget, all of it.
pub fn tail(file: &Path, budget: Option<usize>) -> (u32, Vec<Entry>) {
    let Some(budget) = budget else {
        return (0, load(file));
    };
    let Ok(bytes) = std::fs::read(file) else {
        return (0, Vec::new());
    };
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let start = tail_start(&lines, budget);
    let before = lines[..start].iter().filter(|l| is_prompt(l)).count() as u32;
    let entries = lines[start..]
        .iter()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    (before, entries)
}

// told by how the line starts, so a tail parses only the lines it keeps
fn is_prompt(line: &str) -> bool {
    line.starts_with(r#"{"type":"prompt""#)
}

fn tail_start(lines: &[&str], budget: usize) -> usize {
    let mut size = 0;
    let mut start = None;
    for (i, line) in lines.iter().enumerate().rev() {
        size += line.len();
        if is_prompt(line) {
            if size > budget
                && let Some(start) = start
            {
                return start;
            }
            start = Some(i);
        }
    }
    0
}

pub fn markdown(title: &str, entries: &[Entry]) -> String {
    let mut out = format!("# {title}\n");
    let mut said = false;
    for entry in entries {
        match entry {
            Entry::Prompt { prompt } => {
                let asked = prompt.text.trim();
                out.push_str(&format!("\n## You\n\n{asked}\n\n## Agent\n\n"));
                said = false;
            }
            Entry::Run {
                event: RunEvent::Text { text },
            } => {
                out.push_str(text);
                said = true;
            }
            Entry::Run {
                event: RunEvent::Finished { text, .. },
            } => {
                if !said {
                    out.push_str(text);
                }
                out.push('\n');
            }
            _ => {}
        }
    }
    out
}

pub fn import(file: &Path, conversation: &str) {
    let Some(all) = hyprspace_harness::claude::history(None, conversation) else {
        return;
    };
    let have = load(file)
        .iter()
        .filter(|e| matches!(e, Entry::Prompt { .. }))
        .count();
    let mut seen = 0;
    let journal = Journal::open(file);
    for entry in all.into_iter().skip_while(|e| {
        if matches!(e, Entry::Prompt { .. }) {
            seen += 1;
        }
        seen <= have
    }) {
        journal.record(entry);
    }
}

pub fn trim(file: &Path, keep: usize) {
    if let Some(kept) = head(file, keep) {
        let tmp = file.with_extension("jsonl.tmp");
        if std::fs::write(&tmp, kept).is_ok() {
            let _ = std::fs::rename(&tmp, file);
        }
    }
}

pub fn copy(from: &Path, to: &Path, keep: usize) {
    if let Some(kept) = head(from, keep) {
        let _ = std::fs::write(to, kept);
    }
}

/// The newest line of the file's messages and replies holding `query`, which is lowercase, cut
/// around the match.
pub fn search(file: &Path, query: &str) -> Option<String> {
    let f = File::open(file).ok()?;
    let mut last = None;
    let mut reply = String::new();
    let mut look = |text: &str| {
        if let Some(line) = snippet(text, query) {
            last = Some(line);
        }
    };
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if !line.starts_with(r#"{"type":"prompt""#)
            && !line.contains(r#""type":"text""#)
            && !line.contains(r#""type":"finished""#)
        {
            continue;
        }
        match serde_json::from_str::<Entry>(&line) {
            Ok(Entry::Prompt { prompt }) => {
                look(&std::mem::take(&mut reply));
                look(&prompt.text);
            }
            Ok(Entry::Run {
                event: RunEvent::Text { text },
            }) => reply.push_str(&text),
            Ok(Entry::Run {
                event: RunEvent::Finished { .. },
            }) => look(&std::mem::take(&mut reply)),
            _ => {}
        }
    }
    look(&reply);
    last
}

fn snippet(text: &str, query: &str) -> Option<String> {
    let line = text
        .lines()
        .rev()
        .find(|l| l.to_lowercase().contains(query))?;
    let lower = line.to_lowercase();
    let chars: Vec<char> = line.trim_end().chars().collect();
    let at = lower[..lower.find(query)?].chars().count().min(chars.len());
    let from = at.saturating_sub(24);
    let to = (from + 72).min(chars.len());
    let cut: String = chars[from..to].iter().collect();
    let cut = cut.trim();
    Some(match (from > 0, to < chars.len()) {
        (true, true) => format!("...{cut}..."),
        (true, false) => format!("...{cut}"),
        (false, true) => format!("{cut}..."),
        (false, false) => cut.to_string(),
    })
}

fn head(file: &Path, keep: usize) -> Option<String> {
    let f = File::open(file).ok()?;
    let mut prompts = 0;
    let mut kept = String::new();
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        if let Ok(Entry::Prompt { .. }) = serde_json::from_str::<Entry>(&line) {
            if prompts == keep {
                break;
            }
            prompts += 1;
        }
        kept.push_str(&line);
        kept.push('\n');
    }
    Some(kept)
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
    fn a_trim_keeps_the_prompts_before_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path(), "thread-2");
        let j = Journal::open(&file);
        for p in ["one", "two", "three"] {
            j.record(Entry::Prompt {
                prompt: Prompt::text(p),
            });
            j.record(run(text(p)));
        }
        drop(j);
        trim(&file, 1);
        assert_eq!(
            load(&file),
            vec![
                Entry::Prompt {
                    prompt: Prompt::text("one")
                },
                run(text("one")),
            ]
        );
        trim(&file, 0);
        assert!(load(&file).is_empty());
    }

    #[test]
    fn a_copy_takes_the_start_and_leaves_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (path(dir.path(), "thread-2"), path(dir.path(), "thread-3"));
        let j = Journal::open(&from);
        for p in ["one", "two"] {
            j.record(Entry::Prompt {
                prompt: Prompt::text(p),
            });
            j.record(run(text(p)));
        }
        drop(j);
        copy(&from, &to, 1);
        assert_eq!(load(&to).len(), 2);
        assert_eq!(load(&from).len(), 4);
    }

    #[test]
    fn search_finds_the_newest_line_across_streamed_text() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path(), "thread-5");
        let j = Journal::open(&file);
        j.record(Entry::Prompt {
            prompt: Prompt::text("fix the Login page"),
        });
        j.record(run(RunEvent::ToolDone {
            id: "t".into(),
            ok: true,
            output: "login in a tool's output".into(),
            images: Vec::new(),
        }));
        j.record(run(text("The log")));
        j.record(run(text("in form posts twice.\nUnrelated line.")));
        drop(j);
        assert_eq!(
            search(&file, "login").as_deref(),
            Some("The login form posts twice.")
        );
        assert_eq!(search(&file, "page").as_deref(), Some("fix the Login page"));
        assert_eq!(search(&file, "output"), None);
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
            Some(Box::new(|file| before = load(file))),
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
    fn an_export_reads_as_prompts_and_replies() {
        let entries = [
            Entry::Prompt {
                prompt: Prompt::text("fix it "),
            },
            run(text("Fixed")),
            run(text(".")),
            run(RunEvent::Finished {
                status: hyprspace_proto::RunStatus::Done,
                ms: 1,
                text: "Fixed.".into(),
                error: None,
            }),
            Entry::Prompt {
                prompt: Prompt::text("thanks"),
            },
            run(RunEvent::Finished {
                status: hyprspace_proto::RunStatus::Done,
                ms: 1,
                text: "Any time.".into(),
                error: None,
            }),
        ];
        assert_eq!(
            markdown("Auth", &entries),
            "# Auth\n\n## You\n\nfix it\n\n## Agent\n\nFixed.\n\n## You\n\nthanks\n\n## Agent\n\nAny time.\n"
        );
    }

    #[test]
    fn a_tail_keeps_whole_prompts_from_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path(), "thread-2");
        let j = Journal::open(&file);
        let prompt = |t: &str| Entry::Prompt {
            prompt: Prompt::text(t),
        };
        for (p, n) in [("a", 500), ("b", 50), ("c", 50)] {
            j.record(prompt(p));
            j.record(run(RunEvent::Error {
                message: "x".repeat(n),
            }));
        }
        drop(j);
        assert!(is_prompt(&serde_json::to_string(&prompt("a")).unwrap()));
        let texts = |(before, e): (u32, Vec<Entry>)| {
            let firsts: Vec<_> = e
                .iter()
                .filter_map(|e| match e {
                    Entry::Prompt { prompt } => Some(prompt.text.clone()),
                    _ => None,
                })
                .collect();
            (before, firsts)
        };
        assert_eq!(
            texts(tail(&file, Some(400))),
            (1, vec!["b".into(), "c".into()])
        );
        assert_eq!(texts(tail(&file, Some(10_000))).0, 0);
        // the last prompt comes whole, however big
        assert_eq!(texts(tail(&file, Some(10))), (2, vec!["c".into()]));
        assert_eq!(tail(&file, None).1.len(), 6);
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
