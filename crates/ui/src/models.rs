// What a model is called on screen. Every chip, header and picker goes through `name`, so one
// model reads the same everywhere, and nobody sees a raw id like `claude-opus-5-5[1m]`.

use hyprspace_proto::Agent;
use hyprspace_proto::agents::AgentCatalog;

/// A model's name from the agent's catalog. Ids arrive in three shapes: the catalog's own, the
/// CLI's (`claude-opus-5-5[1m]`, a dated Haiku), and an alias the user typed (`haiku`, `opus`).
/// Context tags and dates are dropped before comparing, an alias takes the catalog's newest
/// model of that family, and an unlisted Claude id is still turned into a name.
pub(crate) fn name(catalog: Option<&AgentCatalog>, id: &str) -> String {
    if id.is_empty() {
        return "Default".into();
    }
    let key = bare(id);
    let listed = || {
        catalog
            .into_iter()
            .flat_map(|c| &c.models)
            .filter(|m| !m.id.is_empty())
    };
    if let Some(m) = listed().find(|m| bare(&m.id) == key) {
        return m.label.clone();
    }
    // an alias names a family; the catalog lists newest first
    if !key.contains('-') {
        let family = format!("claude-{key}-");
        if let Some(m) = listed().find(|m| bare(&m.id).starts_with(&family)) {
            return m.label.clone();
        }
        return capitalized(key);
    }
    let Some(rest) = key.strip_prefix("claude-") else {
        return id.to_string();
    };
    let mut parts = rest.split('-');
    let mut name = capitalized(parts.next().unwrap_or_default());
    let version: Vec<&str> = parts.collect();
    if !version.is_empty() {
        name = format!("{name} {}", version.join("."));
    }
    name
}

/// The tag that puts a Claude model on its 1M context window.
const LONG: &str = "[1m]";

/// Whether `id` can run on the 1M window: Claude's Opus and Sonnet can, Haiku and the CLI's own
/// default (an empty id) can't.
pub(crate) fn takes_long(agent: Agent, id: &str) -> bool {
    let b = bare(id);
    agent == Agent::Claude && (b.starts_with("claude-opus") || b.starts_with("claude-sonnet"))
}

/// Whether `id` is on the 1M window.
pub(crate) fn is_long(id: &str) -> bool {
    id.ends_with(LONG)
}

/// `id` on the 1M window when `long`, else on the standard one.
pub(crate) fn windowed(id: &str, long: bool) -> String {
    let id = id.strip_suffix(LONG).unwrap_or(id);
    if long {
        format!("{id}{LONG}")
    } else {
        id.to_string()
    }
}

fn capitalized(word: &str) -> String {
    word.chars()
        .take(1)
        .flat_map(char::to_uppercase)
        .chain(word.chars().skip(1))
        .collect()
}

/// An id without a context tag like `[1m]` or a trailing `-20251001` date.
fn bare(id: &str) -> &str {
    let id = id.split('[').next().unwrap_or(id);
    match id.rsplit_once('-') {
        Some((head, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprspace_proto::agents::{Agent, ModelInfo};

    fn catalog() -> AgentCatalog {
        let m = |id: &str, label: &str| ModelInfo {
            id: id.into(),
            label: label.into(),
            note: None,
            efforts: Vec::new(),
            default_effort: None,
        };
        AgentCatalog {
            agent: Agent::Claude,
            models: vec![
                m("", "Default"),
                m("claude-opus-5-5", "Opus 5.5"),
                m("claude-opus-5", "Opus 5"),
                m("claude-haiku-5-5", "Haiku 5.5"),
                m("claude-haiku-4-5-20251001", "Haiku 4.5"),
            ],
            efforts: Vec::new(),
        }
    }

    #[test]
    fn models_show_by_name_never_by_id() {
        let c = catalog();
        assert_eq!(name(Some(&c), ""), "Default");
        assert_eq!(name(Some(&c), "claude-opus-5-5"), "Opus 5.5");
        assert_eq!(name(Some(&c), "claude-opus-5-5[1m]"), "Opus 5.5");
        assert_eq!(name(Some(&c), "claude-opus-5"), "Opus 5");
        assert_eq!(name(Some(&c), "claude-haiku-4-5"), "Haiku 4.5");
        assert_eq!(name(None, "claude-sonnet-4-6-20260101"), "Sonnet 4.6");
        assert_eq!(name(None, "gpt-5.5"), "gpt-5.5");
    }

    #[test]
    fn aliases_take_the_newest_of_their_family() {
        let c = catalog();
        assert_eq!(name(Some(&c), "haiku"), "Haiku 5.5");
        assert_eq!(name(Some(&c), "opus"), "Opus 5.5");
        // a family the catalog doesn't list still reads as a word, not an id
        assert_eq!(name(Some(&c), "sonnet"), "Sonnet");
    }

    #[test]
    fn only_opus_and_sonnet_take_the_long_window() {
        assert!(takes_long(Agent::Claude, "claude-opus-5-5"));
        assert!(takes_long(Agent::Claude, "claude-sonnet-5-5[1m]"));
        assert!(!takes_long(Agent::Claude, "claude-haiku-4-5"));
        assert!(!takes_long(Agent::Claude, "claude-haiku-5-5"));
        assert!(!takes_long(Agent::Claude, ""));
        assert!(!takes_long(Agent::Codex, "gpt-5.5"));
        assert_eq!(windowed("claude-opus-5-5", true), "claude-opus-5-5[1m]");
        assert_eq!(windowed("claude-opus-5-5[1m]", false), "claude-opus-5-5");
        assert!(is_long("claude-opus-5-5[1m]"));
    }
}
