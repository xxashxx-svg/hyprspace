//! The GPUI app. It talks to the engine only through `hyprspace_proto::Client` and the event
//! stream, never by calling engine code (docs/adr/0002-channel-boundary.md).

mod colors;
mod root;
mod terminal;
mod transcript;

pub use root::{Layout, Root};
