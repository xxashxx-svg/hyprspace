// Other computers: what the UI and the engine say about the computers this one is paired with.
// The engine keeps a connection to each host's bridge and forwards the sessions the UI binds to
// a host's threads. Shape and reasons: docs/internals/machines.md.

use serde::{Deserialize, Serialize};

use crate::folder::{FolderCommand, FolderEvent};
use std::path::PathBuf;

use crate::phone::{Ask, Board, Down, Up};
use crate::wire::SessionId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PeerCommand {
    /// Pair with a host from its `hyprspace://pair?...` link. Answered with `Peers`, or
    /// `PairFailed`.
    Pair {
        link: String,
    },
    /// Forget a host here and on the host.
    Forget {
        peer: String,
    },
    /// Look for hosts on the local network while Settings shows them. Answered with `Found`.
    Look {
        on: bool,
    },
    /// Route session `id` to `thread` on `peer`: what the UI sends for it goes to the host.
    Bind {
        id: SessionId,
        peer: String,
        thread: u64,
    },
    Unbind {
        id: SessionId,
    },
    /// Anything else for a host's bridge.
    Up {
        peer: String,
        up: Up,
    },
    /// A folder request for a host.
    Folder {
        peer: String,
        cmd: FolderCommand,
    },
    /// A new thread on a host, its `images` uploaded first. `ask` is an `Ask::New`.
    New {
        peer: String,
        ask: Ask,
        images: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PeerEvent {
    /// The hosts this computer is paired with.
    Peers {
        peers: Vec<Peer>,
    },
    /// User-facing.
    PairFailed {
        message: String,
    },
    /// Hosts announcing themselves on the local network.
    Found {
        hosts: Vec<Found>,
    },
    Board {
        peer: String,
        board: Box<Board>,
    },
    /// What a host said that the UI handles.
    Down {
        peer: String,
        down: Down,
    },
    Folder {
        peer: String,
        event: FolderEvent,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub online: bool,
    /// Why it isn't connected. User-facing.
    pub error: Option<String>,
    /// The host's app version.
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Found {
    pub name: String,
    pub hosts: Vec<String>,
    pub port: u16,
    pub fingerprint: String,
}
