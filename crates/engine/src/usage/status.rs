// Claude's status-line payload, read for the meter. Claude hands its status-line command a JSON
// blob every turn with the account's rate-limit windows and the model in use (hooks.rs tees it
// to us). No token is read and nothing is fetched: this is what Claude already told its own UI.
// Follows the Tauri app's src/stores/usage.ts.

use hyprspace_proto::usage::{LiveBar, StatusReport};
use serde_json::Value;

const HOUR_MS: i64 = 3_600_000;

/// Claude's window keys in the order the meter shows them, with the words its own UI uses.
const WINDOWS: &[(&str, &str, i64)] = &[
    ("five_hour", "Session · 5h", 5 * HOUR_MS),
    ("seven_day", "This week", 168 * HOUR_MS),
    ("seven_day_opus", "Opus this week", 168 * HOUR_MS),
    ("seven_day_sonnet", "Sonnet this week", 168 * HOUR_MS),
    ("seven_day_overage_included", "Fable 5", 168 * HOUR_MS),
    ("overage", "Usage credits", 0),
];

/// "seven_day_fable" becomes "Seven day fable" for a key this build has no words for.
fn label(key: &str) -> String {
    if let Some((_, l, _)) = WINDOWS.iter().find(|(k, ..)| *k == key) {
        return (*l).to_string();
    }
    let words = key.replace('_', " ");
    let mut chars = words.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

/// Claude writes `resets_at` as unix seconds or an ISO string depending on the window.
fn reset_ms(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_f64() {
        let n = n as i64;
        return Some(if n > 100_000_000_000 { n } else { n * 1000 });
    }
    v.as_str().and_then(super::live::iso_ms)
}

fn window(key: &str, raw: &Value) -> Option<LiveBar> {
    let pct = raw["used_percentage"]
        .as_f64()
        .or_else(|| raw["utilization"].as_f64())?;
    // utilization comes as a 0-1 fraction, used_percentage as 0-100
    let pct = if pct <= 1.0 { pct * 100.0 } else { pct };
    let resets = raw.get("resets_at").or_else(|| raw.get("resetsAt"));
    Some(LiveBar {
        id: key.to_string(),
        label: label(key),
        percent: pct.clamp(0.0, 100.0),
        resets_at: resets.and_then(reset_ms),
        window_ms: WINDOWS
            .iter()
            .find(|(k, ..)| *k == key)
            .map(|(.., ms)| *ms)
            .filter(|ms| *ms > 0),
        severity: None,
    })
}

/// The windows this plan reports, known ones first in the meter's order, then the rest.
pub fn report(status_line: &Value) -> StatusReport {
    let model = &status_line["model"];
    let mut windows: Vec<LiveBar> = status_line["rate_limits"]
        .as_object()
        .map(|o| o.iter().filter_map(|(k, v)| window(k, v)).collect())
        .unwrap_or_default();
    let rank = |id: &str| {
        WINDOWS
            .iter()
            .position(|(k, ..)| *k == id)
            .unwrap_or(WINDOWS.len())
    };
    windows.sort_by_key(|w| rank(&w.id));
    StatusReport {
        model: model
            .as_str()
            .or_else(|| model["display_name"].as_str())
            .map(String::from),
        model_id: model["id"].as_str().map(String::from),
        windows,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_windows_in_order_whatever_their_shape() {
        let r = report(&json!({
            "model": { "id": "claude-opus-5-5", "display_name": "Opus 5.5" },
            "rate_limits": {
                "seven_day": { "utilization": 0.25, "resets_at": "2026-10-05T10:00:00Z" },
                "five_hour": { "used_percentage": 42, "resets_at": 1_790_000_000 },
                "seven_day_fable": { "used_percentage": 7 },
                "nothing": { "resets_at": 1 }
            }
        }));
        assert_eq!(r.model.as_deref(), Some("Opus 5.5"));
        assert_eq!(r.model_id.as_deref(), Some("claude-opus-5-5"));
        let ids: Vec<_> = r.windows.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["five_hour", "seven_day", "seven_day_fable"]);
        let five = &r.windows[0];
        assert_eq!(five.label, "Session · 5h");
        assert_eq!(five.percent, 42.0);
        assert_eq!(five.resets_at, Some(1_790_000_000_000));
        assert_eq!(five.window_ms, Some(5 * HOUR_MS));
        assert_eq!(r.windows[1].percent, 25.0);
        assert!(r.windows[1].resets_at.is_some());
        assert_eq!(r.windows[2].label, "Seven day fable");
        assert_eq!(r.windows[2].window_ms, None);
    }

    #[test]
    fn a_status_line_without_limits_still_names_the_model() {
        let r = report(&json!({ "model": "Haiku 4.5" }));
        assert_eq!(r.model.as_deref(), Some("Haiku 4.5"));
        assert!(r.windows.is_empty());
    }
}
