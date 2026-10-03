//! Types shared across the engine boundary. The UI and the engine only ever exchange what is
//! defined here, so neither this crate nor the engine may depend on GPUI.
//!
//! `wire` holds the commands and events that cross the channel, `run` what a structured session
//! starts with and reports, `state` what the app keeps between runs; the other modules hold the
//! domain records those messages (and the engine's library calls) carry. Words follow docs/CONTEXT.md.

pub mod agents;
pub mod channel;
pub mod folder;
pub mod git;
pub mod grid;
pub mod run;
pub mod skills;
pub mod state;
pub mod update;
pub mod usage;
pub mod wire;

pub use agents::{Agent, AgentState, SubAgent};
pub use channel::{Client, Events};
pub use folder::{FolderCommand, FolderEvent, Opener};
pub use grid::{Grid, Pane};
pub use run::{Answer, Launch, Permission, Prompt, RunEvent, RunStatus, Tool};
pub use skills::{SkillCommand, SkillEvent};
pub use state::{AppState, Entry, Space, Thread, ThreadKind};
pub use update::{UpdateCommand, UpdateEvent};
pub use usage::{UsageCommand, UsageEvent};
pub use wire::{Command, Event, SessionId};
