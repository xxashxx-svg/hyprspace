#![allow(unused_imports)]
use super::*;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use serde::Serialize;
use serde_json::Value;
use base64::Engine;

#[derive(Serialize, Default)]
pub struct ProviderStatus {
    id: String,
    installed: bool,
    version: Option<String>,
    account: Option<String>,
    plan: Option<String>,
    detail: Option<String>,
}

fn cli_version(cli: &str) -> Option<String> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/c", cli, "--version"]);
        c
    } else {
        let mut c = Command::new(cli);
        c.arg("--version");
        c
    };
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    // first token that looks like a version number, e.g. "2.1.183 (Claude Code)" → 2.1.183
    for tok in s.split_whitespace() {
        let t = tok
            .trim_start_matches('v')
            .trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        let head = t.split('.').next().unwrap_or("");
        if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) && t.contains('.') {
            return Some(t.to_string());
        }
    }
    s.lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn decode_jwt(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[tauri::command]
pub async fn provider_status(id: String) -> ProviderStatus {
    tauri::async_runtime::spawn_blocking(move || provider_status_blocking(&id))
        .await
        .unwrap_or_default()
}

fn provider_status_blocking(id: &str) -> ProviderStatus {
    let mut st = ProviderStatus {
        id: id.to_string(),
        ..Default::default()
    };
    let cli = match id {
        "claude" | "gemini" | "codex" => id,
        _ => return st,
    };
    st.version = cli_version(cli);
    st.installed = st.version.is_some();
    if !st.installed {
        st.detail = Some(format!("`{cli}` is not installed or not on PATH."));
        return st;
    }

    let home = home_dir();
    match id {
        "claude" => {
            if let Some(v) = read_json(home.join(".claude.json")) {
                st.account = v["oauthAccount"]["emailAddress"].as_str().map(String::from);
            }
            if let Some(v) = read_json(home.join(".claude").join(".credentials.json")) {
                if let Some(sub) = v["claudeAiOauth"]["subscriptionType"].as_str() {
                    st.plan = Some(format!("Claude {} Subscription", title_case(sub)));
                }
            }
            if st.account.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        "codex" => {
            if let Some(v) = read_json(home.join(".codex").join("auth.json")) {
                if let Some(tok) = v["tokens"]["id_token"].as_str() {
                    if let Some(p) = decode_jwt(tok) {
                        st.account = p["email"].as_str().map(String::from);
                        if let Some(plan) =
                            p["https://api.openai.com/auth"]["chatgpt_plan_type"].as_str()
                        {
                            st.plan = Some(format!("ChatGPT {} Subscription", title_case(plan)));
                        }
                    }
                }
                if st.account.is_none() && v["OPENAI_API_KEY"].as_str().is_some() {
                    st.detail = Some("Authenticated via API key".into());
                }
            }
            if st.account.is_none() && st.detail.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        "gemini" => {
            if let Some(v) = read_json(home.join(".gemini").join("google_accounts.json")) {
                st.account = v["active"].as_str().map(String::from);
            }
            if st.account.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        _ => {}
    }
    st
}

// ---- MCP servers: read/write Claude's ~/.claude.json top-level "mcpServers" ----
