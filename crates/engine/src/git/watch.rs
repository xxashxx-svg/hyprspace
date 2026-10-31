// What a scheduled task's watch looks for on the repo's GitHub, through the user's own `gh`:
// failed CI runs, open pull requests, or open issues with a label.

use std::path::Path;

use hyprspace_proto::schedule::{Happening, When};
use serde_json::Value;

/// Newest first. Empty when gh is missing, signed out, or the repo isn't on GitHub.
pub fn happened(cwd: &Path, when: &When) -> Vec<Happening> {
    match when {
        When::CiFails => list(
            cwd,
            &[
                "run",
                "list",
                "--limit",
                "20",
                "--json",
                "databaseId,conclusion,workflowName,headBranch,displayTitle,url",
            ],
            |v| {
                (v["conclusion"] == "failure").then(|| Happening {
                    id: v["databaseId"].as_u64().unwrap_or(0),
                    line: format!(
                        "{} failed on {}: {}",
                        text(&v["workflowName"]),
                        text(&v["headBranch"]),
                        text(&v["displayTitle"])
                    ),
                    url: text(&v["url"]),
                })
            },
        ),
        When::PrOpens => list(
            cwd,
            &[
                "pr",
                "list",
                "--state",
                "open",
                "--limit",
                "30",
                "--json",
                "number,title,url",
            ],
            numbered,
        ),
        When::Labeled { label } => list(
            cwd,
            &[
                "issue",
                "list",
                "--state",
                "open",
                "--label",
                label,
                "--limit",
                "30",
                "--json",
                "number,title,url",
            ],
            numbered,
        ),
        _ => Vec::new(),
    }
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn numbered(v: &Value) -> Option<Happening> {
    Some(Happening {
        id: v["number"].as_u64()?,
        line: format!("#{} {}", v["number"], text(&v["title"])),
        url: text(&v["url"]),
    })
}

fn list(cwd: &Path, args: &[&str], each: impl Fn(&Value) -> Option<Happening>) -> Vec<Happening> {
    let Ok(out) = super::gh(cwd).args(args).output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    serde_json::from_slice::<Value>(&out.stdout)
        .ok()
        .and_then(|v| {
            v.as_array()
                .map(|all| all.iter().filter_map(&each).collect())
        })
        .unwrap_or_default()
}
