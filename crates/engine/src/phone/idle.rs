// Whether someone is at this computer: how long since its keyboard or mouse was last used, from
// the OS. A phone holds back its notifications while the answer is "just now", since the user
// sees the desktop app anyway.

use std::time::Duration;

/// Input within this long counts as someone at the computer.
pub const PRESENT: Duration = Duration::from_secs(120);

pub fn present() -> bool {
    idle().is_some_and(|d| d < PRESENT)
}

#[cfg(windows)]
fn idle() -> Option<Duration> {
    #[repr(C)]
    struct LastInput {
        size: u32,
        time: u32,
    }
    unsafe extern "system" {
        fn GetLastInputInfo(info: *mut LastInput) -> i32;
        fn GetTickCount() -> u32;
    }
    let mut info = LastInput {
        size: std::mem::size_of::<LastInput>() as u32,
        time: 0,
    };
    // SAFETY: `info` is a LASTINPUTINFO with its size set, as the call requires
    let ok = unsafe { GetLastInputInfo(&mut info) } != 0;
    // SAFETY: no arguments; both ticks wrap together every 49 days
    let now = unsafe { GetTickCount() };
    ok.then(|| Duration::from_millis(now.wrapping_sub(info.time) as u64))
}

#[cfg(target_os = "macos")]
fn idle() -> Option<Duration> {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, kind: u32) -> f64;
    }
    // the combined session state, and any input event
    // SAFETY: plain values in, a number of seconds out
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
    (secs.is_finite() && secs >= 0.0).then(|| Duration::from_secs_f64(secs))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn idle() -> Option<Duration> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_os_answers() {
        // a CI runner may never have seen input, so only the call itself is checked
        #[cfg(any(windows, target_os = "macos"))]
        assert!(super::idle().is_some());
    }
}
