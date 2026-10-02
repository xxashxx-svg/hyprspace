// Times the way the sidebar and the resume list show them.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// "now", "5m", "3h", "2d", "6w" since `then` (unix ms).
pub fn ago(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then) / 1000;
    match secs {
        0..60 => "now".into(),
        60..3600 => format!("{}m", secs / 60),
        3600..86_400 => format!("{}h", secs / 3600),
        86_400..1_209_600 => format!("{}d", secs / 86_400),
        _ => format!("{}w", secs / 604_800),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_down_to_the_largest_unit() {
        let now = 10_000_000_000;
        assert_eq!(ago(now - 5_000, now), "now");
        assert_eq!(ago(now - 125_000, now), "2m");
        assert_eq!(ago(now - 3 * 3_600_000, now), "3h");
        assert_eq!(ago(now - 2 * 86_400_000, now), "2d");
        assert_eq!(ago(now - 21 * 86_400_000, now), "3w");
        assert_eq!(ago(now + 9, now), "now");
    }
}
