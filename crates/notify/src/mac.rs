use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSRequestUserAttentionType};
use objc2_foundation::NSString;
use raw_window_handle::HasWindowHandle;

pub struct Taskbar;

impl Taskbar {
    pub fn of(_: &impl HasWindowHandle) -> Option<Self> {
        Some(Self)
    }

    pub fn badge(&self, count: usize) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let label = crate::label(count).map(|l| NSString::from_str(&l));
        NSApplication::sharedApplication(mtm)
            .dockTile()
            .setBadgeLabel(label.as_deref());
    }

    pub fn flash(&self) {
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm)
                .requestUserAttention(NSRequestUserAttentionType::InformationalRequest);
        }
    }
}
