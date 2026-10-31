// The models and effort levels each agent starts with, carried over from the Tauri app's
// src/lib/models.ts. Claude takes `--model` and `--effort low|medium|high|xhigh|max` (checked
// against `claude --help`). Codex's own list, with each model's levels, is the cache the CLI
// keeps at ~/.codex/models_cache.json; the static list is the fallback when that file is missing.
// There is no "Default" entry: the first model is the default, named and passed to the CLI, so
// what a thread says it runs is what it runs (`AgentCatalog::default_model`).

use std::path::Path;

use hyprspace_proto::Agent;
use hyprspace_proto::agents::{AgentCatalog, ModelInfo};
use serde_json::Value;

const CLAUDE_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const CODEX_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

fn model(id: &str, label: &str, note: Option<&str>, efforts: &[&str]) -> ModelInfo {
    ModelInfo {
        id: id.into(),
        label: label.into(),
        note: note.map(Into::into),
        efforts: efforts.iter().map(|e| e.to_string()).collect(),
        default_effort: None,
    }
}

/// The built-in catalog for `agent`.
pub fn fallback(agent: Agent) -> AgentCatalog {
    let ultra: Vec<&str> = CODEX_EFFORTS.iter().copied().chain(["ultra"]).collect();
    let (models, efforts) = match agent {
        Agent::Claude => (
            vec![
                ModelInfo {
                    default_effort: Some("medium".into()),
                    ..model("claude-opus-5-5", "Opus 5.5", None, &[])
                },
                model("claude-fable-5-1", "Fable 5.1", None, &[]),
                model("claude-fable-5", "Fable 5", None, &[]),
                model("claude-opus-5", "Opus 5", None, &[]),
                model("claude-opus-4-8", "Opus 4.8", None, &[]),
                model("claude-opus-4-7", "Opus 4.7", None, &[]),
                model("claude-opus-4-6", "Opus 4.6", None, &[]),
                model("claude-opus-4-5-20251101", "Opus 4.5", None, &[]),
                model("claude-sonnet-5-5", "Sonnet 5.5", None, &[]),
                model("claude-sonnet-5", "Sonnet 5", None, &[]),
                model("claude-sonnet-4-6", "Sonnet 4.6", None, &[]),
                model("claude-haiku-5-5", "Haiku 5.5", None, &[]),
                model("claude-haiku-4-5-20251001", "Haiku 4.5", None, &[]),
            ],
            CLAUDE_EFFORTS,
        ),
        Agent::Codex => (
            vec![
                model("gpt-6-astra", "GPT-6 Astra", None, &ultra),
                model("gpt-5.6-sol", "GPT-5.6 Sol", None, &ultra),
                model("gpt-5.6-terra", "GPT-5.6 Terra", None, &ultra),
                model("gpt-5.6-luna", "GPT-5.6 Luna", None, &[]),
                model(
                    "gpt-5.5",
                    "GPT-5.5",
                    None,
                    &["low", "medium", "high", "xhigh"],
                ),
            ],
            CODEX_EFFORTS,
        ),
    };
    AgentCatalog {
        agent,
        models,
        efforts: efforts.iter().map(|e| e.to_string()).collect(),
    }
}

/// Codex models a thread starts on when the config names none, newest first: the first one the
/// account lists.
const CODEX_PREFERRED: &[&str] = &["gpt-6-astra", "gpt-5.6-sol", "gpt-5.6-terra"];

/// The catalog to offer: Codex's own cache under `home` when it has models, else the fallback.
/// Each agent's default is the model its own config names, so a thread runs what the user set
/// up there: Claude's `settings.json` (an alias like `opus` takes that family's newest), Codex's
/// `config.toml`, else Opus 5.5 or the newest preferred Codex model the account lists.
pub fn catalog(agent: Agent, home: &Path) -> AgentCatalog {
    let mut out = fallback(agent);
    if agent == Agent::Claude {
        let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|d| !d.is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| home.join(".claude"));
        let settings: Option<Value> = std::fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok());
        let get = |key: &str| {
            settings
                .as_ref()
                .and_then(|s| s.get(key)?.as_str())
                .map(str::to_string)
        };
        if let Some(effort) = get("effortLevel").filter(|e| out.efforts.contains(e)) {
            for m in &mut out.models {
                m.default_effort = Some(effort.clone());
            }
        }
        let wanted = get("model").map(|m| m.split('[').next().unwrap_or(&m).to_string());
        let id = wanted.filter(|m| !m.is_empty() && m != "default").map(|m| {
            let family = format!("claude-{m}-");
            out.models
                .iter()
                .find(|x| x.id == m || x.id.starts_with(&family))
                .map_or(m, |x| x.id.clone())
        });
        if let Some(id) = id {
            match out.models.iter().position(|m| m.id == id) {
                Some(i) => {
                    let m = out.models.remove(i);
                    out.models.insert(0, m);
                }
                None => out
                    .models
                    .insert(0, model(&id, &id, Some("From your Claude settings"), &[])),
            }
        }
    }
    if agent == Agent::Codex {
        if let Some(models) = codex_cache(home) {
            out.models = models;
        }
        let first = match codex_config_model(home) {
            Some(model) => {
                if !out.models.iter().any(|m| m.id == model) {
                    let note = Some("From your Codex config");
                    out.models.push(self::model(&model, &model, note, &[]));
                }
                Some(model)
            }
            None => CODEX_PREFERRED
                .iter()
                .find(|p| out.models.iter().any(|m| m.id == **p))
                .map(|p| p.to_string()),
        };
        if let Some(i) = first.and_then(|id| out.models.iter().position(|m| m.id == id)) {
            let m = out.models.remove(i);
            out.models.insert(0, m);
        }
    }
    out
}

// The top-level `model = "..."` in ~/.codex/config.toml. Only that one key is read, so a line
// scan is enough and no TOML parser is needed; anything under a [table] is someone else's.
fn codex_config_model(home: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(home.join(".codex").join("config.toml")).ok()?;
    for line in raw.lines().map(str::trim) {
        if line.starts_with('[') {
            return None;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "model" {
            let value = value.split('#').next()?.trim().trim_matches('"');
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

// The CLI writes this file itself, so it is display-only data we never fetch. Only listed
// models, in the CLI's own priority order.
fn codex_cache(home: &Path) -> Option<Vec<ModelInfo>> {
    let raw = std::fs::read_to_string(home.join(".codex").join("models_cache.json")).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    let mut rows: Vec<&Value> = data["models"]
        .as_array()?
        .iter()
        .filter(|m| m["slug"].as_str().is_some_and(|s| !s.is_empty()))
        .filter(|m| m["visibility"].as_str() != Some("hide"))
        .collect();
    rows.sort_by_key(|m| m["priority"].as_i64().unwrap_or(999));
    let str_of = |v: &Value| v.as_str().map(str::to_string);
    let models: Vec<ModelInfo> = rows
        .into_iter()
        .map(|m| {
            let slug = m["slug"].as_str().unwrap_or_default();
            ModelInfo {
                id: slug.into(),
                label: str_of(&m["display_name"]).unwrap_or_else(|| slug.into()),
                note: str_of(&m["description"]),
                efforts: m["supported_reasoning_levels"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|l| str_of(&l["effort"])).collect())
                    .unwrap_or_default(),
                default_effort: str_of(&m["default_reasoning_level"]),
            }
        })
        .collect();
    (!models.is_empty()).then_some(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_matches_the_tauri_catalog() {
        let claude = fallback(Agent::Claude);
        assert_eq!(claude.models[0].id, "claude-opus-5-5");
        assert!(claude.models.iter().all(|m| !m.id.is_empty()));
        assert_eq!(
            claude.default_effort("claude-opus-5-5").as_deref(),
            Some("medium")
        );
        assert_eq!(
            claude.default_effort("claude-sonnet-5-5").as_deref(),
            Some("high")
        );
        assert_eq!(claude.efforts_for("claude-opus-5-5"), CLAUDE_EFFORTS);
        let codex = fallback(Agent::Codex);
        assert_eq!(codex.efforts_for("gpt-6-astra").last().unwrap(), "ultra");
        assert_eq!(codex.efforts_for("gpt-5.5").len(), 4);
        assert_eq!(codex.efforts_for("custom-model"), CODEX_EFFORTS);
    }

    #[test]
    fn codex_reads_its_own_cache_and_hides_hidden_models() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join(".codex");
        std::fs::create_dir_all(&dir).unwrap();
        // shape from codex-cli 0.159.3
        std::fs::write(
            dir.join("models_cache.json"),
            r#"{"fetched_at":"x","models":[
                {"slug":"gpt-5.5","display_name":"GPT-5.5","priority":13,"visibility":"list",
                 "supported_reasoning_levels":[{"effort":"low"},{"effort":"high"}]},
                {"slug":"gpt-reserve","priority":4,"visibility":"hide"},
                {"slug":"gpt-6-luna","display_name":"GPT-6-Luna","priority":4,"visibility":"list",
                 "default_reasoning_level":"medium","description":"Fast",
                 "supported_reasoning_levels":[{"effort":"medium"}]}]}"#,
        )
        .unwrap();
        let c = catalog(Agent::Codex, home.path());
        let ids =
            |c: &AgentCatalog| -> Vec<String> { c.models.iter().map(|m| m.id.clone()).collect() };
        // no preferred model listed, so the cache's own order stands
        assert_eq!(ids(&c), ["gpt-6-luna", "gpt-5.5"]);
        assert_eq!(c.models[0].default_effort.as_deref(), Some("medium"));
        assert_eq!(c.default_effort("gpt-6-luna").as_deref(), Some("medium"));
        assert_eq!(c.efforts_for("gpt-5.5"), ["low", "high"]);
        assert_eq!(c.default_effort("gpt-5.5").as_deref(), Some("low"));
        // the config's model leads, listed or not; a model under a table is not the default
        std::fs::write(
            dir.join("config.toml"),
            "model = \"gpt-5.5\" # mine
[profiles.x]
model = \"other\"
",
        )
        .unwrap();
        let c = catalog(Agent::Codex, home.path());
        assert_eq!(ids(&c), ["gpt-5.5", "gpt-6-luna"]);
        std::fs::write(dir.join("config.toml"), "model = \"gpt-custom\"\n").unwrap();
        let c = catalog(Agent::Codex, home.path());
        assert_eq!(ids(&c), ["gpt-custom", "gpt-6-luna", "gpt-5.5"]);
        assert_eq!(c.models[0].note.as_deref(), Some("From your Codex config"));
        std::fs::write(
            dir.join("config.toml"),
            "[profiles.x]
model = \"other\"
",
        )
        .unwrap();
        assert_eq!(codex_config_model(home.path()), None);
        // a missing or broken cache falls back
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(catalog(Agent::Codex, empty.path()), fallback(Agent::Codex));
        assert_eq!(catalog(Agent::Claude, home.path()), fallback(Agent::Claude));
    }

    #[test]
    fn claude_starts_on_the_model_and_effort_its_settings_name() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        let first = |settings: &str| {
            std::fs::write(dir.join("settings.json"), settings).unwrap();
            let c = catalog(Agent::Claude, home.path());
            let id = c.models[0].id.clone();
            let effort = c.default_effort(&id);
            (id, effort)
        };
        assert_eq!(first(r#"{"model":"sonnet"}"#).0, "claude-sonnet-5-5");
        assert_eq!(first(r#"{"model":"opus[1m]"}"#).0, "claude-opus-5-5");
        assert_eq!(
            first(r#"{"model":"claude-haiku-5-5"}"#).0,
            "claude-haiku-5-5"
        );
        assert_eq!(first(r#"{"model":"claude-next-6"}"#).0, "claude-next-6");
        assert_eq!(
            first(r#"{"model":"default","effortLevel":"medium"}"#),
            ("claude-opus-5-5".into(), Some("medium".into()))
        );
        assert_eq!(first("{}").1.as_deref(), Some("medium"));
        assert_eq!(first(r#"{"model":"sonnet"}"#).1.as_deref(), Some("high"));
    }
}
