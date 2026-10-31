#[cfg_attr(windows, path = "win.rs")]
#[cfg_attr(target_os = "macos", path = "mac.rs")]
mod os;

pub use os::Taskbar;

pub fn label(count: usize) -> Option<String> {
    match count {
        0 => None,
        1..=9 => Some(count.to_string()),
        _ => Some("9+".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_badge_is_blank_at_zero_and_stops_at_nine() {
        assert_eq!(label(0), None);
        assert_eq!(label(3).as_deref(), Some("3"));
        assert_eq!(label(9).as_deref(), Some("9"));
        assert_eq!(label(10).as_deref(), Some("9+"));
    }
}
