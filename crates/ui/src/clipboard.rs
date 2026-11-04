// GPUI opens the Windows clipboard once and gives up when another program has it open, which
// Windows' own clipboard history does for a moment after every copy. A copy that lands then is
// dropped without a word, and the next paste brings back the old text, so both ways try again
// for a moment instead.

use std::time::Duration;

use gpui::{App, ClipboardItem};

const TRIES: u32 = 8;
const WAIT: Duration = Duration::from_millis(12);

pub fn read(cx: &mut App) -> Option<ClipboardItem> {
    for _ in 1..TRIES {
        if let Some(item) = cx.read_from_clipboard() {
            return Some(item);
        }
        if !busy() {
            return None;
        }
        std::thread::sleep(WAIT);
    }
    cx.read_from_clipboard()
}

pub fn write(cx: &mut App, item: ClipboardItem) {
    for _ in 1..TRIES {
        let before = serial();
        cx.write_to_clipboard(item.clone());
        if before.is_none() || serial() != before {
            return;
        }
        std::thread::sleep(WAIT);
    }
    cx.write_to_clipboard(item);
}

/// Counts every change to the clipboard, so a write that didn't land shows.
#[cfg(windows)]
fn serial() -> Option<u32> {
    Some(unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() })
}

#[cfg(not(windows))]
fn serial() -> Option<u32> {
    None
}

#[cfg(windows)]
fn busy() -> bool {
    use windows::Win32::System::DataExchange::{CloseClipboard, OpenClipboard};
    unsafe {
        if OpenClipboard(None).is_ok() {
            let _ = CloseClipboard();
            false
        } else {
            true
        }
    }
}

#[cfg(not(windows))]
fn busy() -> bool {
    false
}
