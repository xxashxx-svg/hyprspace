use std::path::Path;
use std::time::Duration;

use hyprspace_proto::Agent;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::spawn::{Proc, spawn};
use crate::{Claude, Codex};

const ASK: &str = "Name this coding conversation for a sidebar. Reply with the title only: 2 to 6 \
    words, sentence case, no quotes, no period at the end. Say what the task is. Don't use any \
    tools or run any commands.";

const PATIENCE: Duration = Duration::from_secs(90);

pub fn ask(text: &str, previous: Option<&str>) -> String {
    let now = previous
        .map(|p| {
            format!(
                "\n\nIts title now is \"{p}\". Write a better one if the conversation has moved on."
            )
        })
        .unwrap_or_default();
    format!("{ASK}{now}\n\nThe conversation:\n\n{text}")
}

pub fn clean(said: &str) -> Option<String> {
    let line = said.lines().map(str::trim).find(|l| !l.is_empty())?;
    let line = line.strip_prefix("Title:").unwrap_or(line);
    let line = line
        .trim_matches(|c: char| c.is_whitespace() || "\"'`*#".contains(c))
        .trim_end_matches('.');
    if line.is_empty() {
        return None;
    }
    Some(if line.chars().count() > 60 {
        format!(
            "{}...",
            line.chars().take(57).collect::<String>().trim_end()
        )
    } else {
        line.to_string()
    })
}

impl Claude {
    pub async fn title(&self, cwd: &Path, model: Option<String>, ask: &str) -> Option<String> {
        let quiet = std::env::temp_dir().join("hyprspace-title-settings.json");
        std::fs::write(&quiet, r#"{"disableAllHooks":true}"#).ok()?;
        let args = [
            "-p",
            "--output-format",
            "text",
            "--model",
            model.as_deref().unwrap_or("haiku"),
            "--no-session-persistence",
            "--max-turns",
            "1",
            "--tools",
            "",
            "--strict-mcp-config",
            "--settings",
            &quiet.to_string_lossy(),
        ]
        .map(String::from);
        let Proc {
            child: _child,
            mut stdin,
            mut stdout,
            ..
        } = spawn(&self.program, &args, cwd).ok()?;
        let said = tokio::time::timeout(PATIENCE, async move {
            stdin.write_all(ask.as_bytes()).await.ok()?;
            drop(stdin);
            let mut said = String::new();
            stdout.read_to_string(&mut said).await.ok()?;
            Some(said)
        })
        .await
        .ok()??;
        clean(&said)
    }
}

impl Codex {
    pub async fn title(&self, cwd: &Path, model: Option<String>, ask: &str) -> Option<String> {
        let out =
            std::env::temp_dir().join(format!("hyprspace-title-{}.txt", uuid::Uuid::new_v4()));
        let mut args: Vec<String> = [
            "exec",
            "--ephemeral",
            "--skip-git-repo-check",
            "-s",
            "read-only",
            "-c",
            "notify=[]",
            "-c",
            "model_reasoning_effort=low",
            "-o",
            &out.to_string_lossy(),
        ]
        .map(String::from)
        .to_vec();
        if let Some(model) = model {
            args.extend(["-m".into(), model]);
        }
        args.push("-".into());
        let Proc {
            mut child,
            mut stdin,
            ..
        } = spawn(&self.program, &args, cwd).ok()?;
        let ended = tokio::time::timeout(PATIENCE, async move {
            stdin.write_all(ask.as_bytes()).await.ok()?;
            drop(stdin);
            child.wait().await.ok()
        })
        .await;
        let said = std::fs::read_to_string(&out).ok();
        let _ = std::fs::remove_file(&out);
        ended.ok()??;
        clean(&said?)
    }
}

pub async fn title(
    agent: Agent,
    cwd: &Path,
    model: Option<String>,
    text: &str,
    previous: Option<&str>,
) -> Option<String> {
    let ask = ask(text, previous);
    match agent {
        Agent::Claude => Claude::default().title(cwd, model, &ask).await,
        Agent::Codex => Codex::default().title(cwd, model, &ask).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_loses_its_dressing() {
        assert_eq!(
            clean("\n  \"Fix the login bug.\"\n"),
            Some("Fix the login bug".into())
        );
        assert_eq!(
            clean("Title: **Speed up the sidebar**"),
            Some("Speed up the sidebar".into())
        );
        assert_eq!(clean("  \n "), None);
        assert_eq!(clean(&"word ".repeat(30)).unwrap().chars().count(), 60);
    }
}
