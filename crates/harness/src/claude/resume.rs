// `claude --resume <id>` only finds a conversation from the folder it started in, so a resume
// runs there whatever folder the caller passed. Claude keeps each conversation as
// `<config>/projects/<encoded folder>/<id>.jsonl`, and its lines carry the real `cwd`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Claude's config folder: `CLAUDE_CONFIG_DIR` when set, else `~/.claude`.
pub(crate) fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(dir.into());
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join(".claude"))
}

/// The folder `thread` started in, or None when no transcript for it exists.
pub(crate) fn origin(config: &Path, thread: &str) -> Option<PathBuf> {
    // an id is a uuid; anything else could walk out of the projects folder
    if thread.is_empty()
        || !thread
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    let name = format!("{thread}.jsonl");
    for entry in std::fs::read_dir(config.join("projects")).ok()?.flatten() {
        let Ok(file) = std::fs::File::open(entry.path().join(&name)) else {
            continue;
        };
        // the first lines are summaries and snapshots; the cwd shows up within a few
        for line in BufReader::new(file).lines().take(50).map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && let Some(cwd) = v["cwd"].as_str().filter(|c| !c.is_empty())
            {
                return Some(PathBuf::from(cwd));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_folder_a_conversation_started_in() {
        let config = tempfile::tempdir().unwrap();
        let dir = config.path().join("projects").join("C--work-app");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("abc-123.jsonl"),
            "{\"type\":\"summary\"}\nnot json\n{\"type\":\"user\",\"cwd\":\"C:\\\\work\\\\app\"}\n",
        )
        .unwrap();
        assert_eq!(
            origin(config.path(), "abc-123"),
            Some(PathBuf::from(r"C:\work\app"))
        );
        assert_eq!(origin(config.path(), "gone"), None);
        assert_eq!(origin(config.path(), "../abc-123"), None);
    }
}
