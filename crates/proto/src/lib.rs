//! Types shared across the engine boundary. The UI and the engine only ever exchange what is
//! defined here, so neither this crate nor the engine may depend on GPUI.
//!
//! `wire` holds the commands and events that cross the channel, `run` what a structured session
//! starts with and reports; the other modules hold the domain records those messages (and the
//! engine's library calls) carry. Words follow docs/CONTEXT.md.

pub mod agents;
pub mod channel;
pub mod git;
pub mod run;
pub mod usage;
pub mod wire;

pub use agents::Agent;
pub use channel::{Client, Events};
pub use run::{Launch, Permission, Prompt, RunEvent, RunStatus, Tool};
pub use wire::{Command, Event, SessionId};
