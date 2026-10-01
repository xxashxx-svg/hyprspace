// Install and sign-in checks for the provider CLIs, copied from src-tauri/src/devtools. Reads
// only the display fields the CLIs keep on disk; no token is used for anything.

use std::path::Path;
use std::process::Command;

use hyprspace_proto::agents::ProviderStatus;

use crate::util::{decode_jwt, home_dir, no_window, read_json, title_case};

/// Install and sign-in state for one provider: claude, codex, gemini, opencode or grok. Blocks
/// on `<cli> --version`, so run it off the UI thread.
pub fn status(id: &str) -> ProviderStatus {
    let mut st = ProviderStatus {
        id: id.to_string(),
        ..Default::default()
    };
    if !matches!(id, "claude" | "gemini" | "codex" | "opencode" | "grok") {
        return st;
    }
    st.version = cli_version(id);
    st.installed = st.version.is_some();
    if !st.installed {
        st.detail = Some(format!("`{id}` is not installed or not on PATH."));
        return st;
    }
    sign_in(&home_dir(), &mut st, std::env::var("XAI_API_KEY").is_ok());
    st
}

// run "<cli> --version". On Windows go through `cmd /c` so a .cmd or .ps1 shim on PATH resolves,
// but pass the args separately (never build a shell string) so `cli` can't be misread as syntax.
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
    let out = no_window(&mut cmd).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_version(&String::from_utf8_lossy(&out.stdout))
}

// first token that looks like a version number, e.g. "2.1.183 (Claude Code)" gives 2.1.183
fn parse_version(s: &str) -> Option<String> {
    for tok in s.split_whitespace() {
        let t = tok
            .trim_start_matches('v')
            .trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        let head = t.split('.').next().unwrap_or("");
        if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) && t.contains('.') {
            return Some(t.to_string());
        }
    }
    s.lines()
        .next()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

// account, plan and a detail line from the files each CLI writes when you sign in
fn sign_in(home: &Path, st: &mut ProviderStatus, xai_key: bool) {
    match st.id.as_str() {
        "claude" => {
            if let Some(v) = read_json(&home.join(".claude.json")) {
                st.account = v["oauthAccount"]["emailAddress"].as_str().map(String::from);
            }
            if let Some(v) = read_json(&home.join(".claude").join(".credentials.json"))
                && let Some(sub) = v["claudeAiOauth"]["subscriptionType"].as_str()
            {
                st.plan = Some(format!("Claude {} Subscription", title_case(sub)));
            }
            if st.account.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        "codex" => {
            if let Some(v) = read_json(&home.join(".codex").join("auth.json")) {
                if let Some(p) = v["tokens"]["id_token"].as_str().and_then(decode_jwt) {
                    st.account = p["email"].as_str().map(String::from);
                    if let Some(plan) =
                        p["https://api.openai.com/auth"]["chatgpt_plan_type"].as_str()
                    {
                        st.plan = Some(format!("ChatGPT {} Subscription", title_case(plan)));
                    }
                }
                if st.account.is_none() && v["OPENAI_API_KEY"].as_str().is_some() {
                    st.detail = Some("Authenticated with an API key".into());
                }
            }
            if st.account.is_none() && st.detail.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        "gemini" => {
            if let Some(v) = read_json(&home.join(".gemini").join("google_accounts.json")) {
                st.account = v["active"].as_str().map(String::from);
            }
            if st.account.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        "opencode" => {
            // bring your own model: auth.json holds one entry per configured model provider
            let auth = home
                .join(".local")
                .join("share")
                .join("opencode")
                .join("auth.json");
            let names: Vec<String> = read_json(&auth)
                .and_then(|v| {
                    v.as_object()
                        .map(|o| o.keys().map(|k| provider_name(k)).collect())
                })
                .unwrap_or_default();
            st.detail = Some(if names.is_empty() {
                "No model providers. Run `opencode auth login`.".into()
            } else {
                format!(
                    "{} model provider{}: {}",
                    names.len(),
                    if names.len() == 1 { "" } else { "s" },
                    names.join(", ")
                )
            });
        }
        "grok" => {
            // `grok login` keeps one entry per auth scope in ~/.grok/auth.json, each with a key.
            // The folder alone proves nothing: the installer creates it before any login.
            let login = read_json(&home.join(".grok").join("auth.json")).and_then(|v| {
                v.as_object()?
                    .values()
                    .find(|e| e["key"].is_string())
                    .cloned()
            });
            if xai_key {
                st.detail = Some("Authenticated with XAI_API_KEY".into());
            } else if let Some(entry) = login {
                st.account = entry["email"].as_str().map(String::from);
                st.detail = Some("Signed in".into());
            } else {
                st.detail = Some("Not signed in. Run `grok` or set XAI_API_KEY.".into());
            }
        }
        _ => {}
    }
}

fn provider_name(k: &str) -> String {
    match k {
        "opencode" => "OpenCode Zen".into(),
        "openai" => "OpenAI".into(),
        "gmicloud" => "GMI Cloud".into(),
        "anthropic" => "Anthropic".into(),
        "google" | "gemini" => "Google".into(),
        "openrouter" => "OpenRouter".into(),
        other => title_case(other),
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use serde_json::json;

    use super::*;

    fn write(home: &Path, rel: &[&str], v: serde_json::Value) {
        let p = rel.iter().fold(home.to_path_buf(), |p, s| p.join(s));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, v.to_string()).unwrap();
    }

    fn check(home: &Path, id: &str) -> ProviderStatus {
        let mut st = ProviderStatus {
            id: id.into(),
            ..Default::default()
        };
        sign_in(home, &mut st, false);
        st
    }

    #[test]
    fn parses_version_lines() {
        assert_eq!(
            parse_version("2.1.183 (Claude Code)").as_deref(),
            Some("2.1.183")
        );
        assert_eq!(
            parse_version("codex-cli v0.150.1\n").as_deref(),
            Some("0.150.1")
        );
        assert_eq!(parse_version("weird\n").as_deref(), Some("weird"));
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn reads_claude_and_codex_sign_in() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        assert_eq!(check(h, "claude").detail.as_deref(), Some("Not signed in"));

        write(
            h,
            &[".claude.json"],
            json!({ "oauthAccount": { "emailAddress": "a@b.c" } }),
        );
        write(
            h,
            &[".claude", ".credentials.json"],
            json!({ "claudeAiOauth": { "subscriptionType": "max" } }),
        );
        let st = check(h, "claude");
        assert_eq!(st.account.as_deref(), Some("a@b.c"));
        assert_eq!(st.plan.as_deref(), Some("Claude Max Subscription"));

        let claims = json!({ "email": "x@y.z", "https://api.openai.com/auth": { "chatgpt_plan_type": "plus" } });
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        write(
            h,
            &[".codex", "auth.json"],
            json!({ "tokens": { "id_token": format!("h.{payload}.s") } }),
        );
        let st = check(h, "codex");
        assert_eq!(st.account.as_deref(), Some("x@y.z"));
        assert_eq!(st.plan.as_deref(), Some("ChatGPT Plus Subscription"));
    }

    #[test]
    fn opencode_lists_its_model_providers() {
        let home = tempfile::tempdir().unwrap();
        write(
            home.path(),
            &[".local", "share", "opencode", "auth.json"],
            json!({ "openrouter": {} }),
        );
        assert_eq!(
            check(home.path(), "opencode").detail.as_deref(),
            Some("1 model provider: OpenRouter")
        );
    }

    #[test]
    fn unknown_ids_are_left_alone() {
        let st = status("notepad");
        assert!(!st.installed);
        assert_eq!(st.detail, None);
    }
}
