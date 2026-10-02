//! The GPUI app. It talks to the engine only through `hyprspace_proto::Client` and the event
//! stream, never by calling engine code (docs/adr/0002-channel-boundary.md).

mod assets;
mod attach;
mod colors;
mod composer;
mod dock;
mod input;
mod intro;
mod markdown;
mod models;
mod palette;
mod panes;
mod root;
mod settings;
mod sidebar;
mod skills;
mod terminal;
mod time;
mod transcript;
mod usage;
mod viewer;
mod widgets;

pub use assets::Assets;
pub use root::Root;

/// Binds the app's keys. Call once before opening the window.
pub fn init(cx: &mut gpui::App) {
    assets::load_fonts(cx);
    input::bind_keys(cx);
    panes::bind_keys(cx);
    palette::bind_keys(cx);
}
