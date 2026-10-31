// What HyprSpace and the processes it started use right now, for Settings, Resources. Read on
// this computer only; threads running on another computer aren't in it.

use serde::{Deserialize, Serialize};

use crate::wire::SessionId;

/// What a group of processes uses. CPU is a share of the whole machine, so every core busy is 100.
/// Read and write are bytes per second since the last reading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Load {
    pub cpu: f32,
    pub memory: u64,
    pub read: u64,
    pub write: u64,
    pub processes: u32,
}

impl Load {
    pub fn add(&mut self, other: &Load) {
        self.cpu += other.cpu;
        self.memory += other.memory;
        self.read += other.read;
        self.write += other.write;
        self.processes += other.processes;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resources {
    /// HyprSpace's own process and the ones it started that belong to no session: the terminal
    /// hosts, and short helpers like the one that names threads.
    pub app: Load,
    /// Each session's shell or CLI, with everything under it.
    pub sessions: Vec<(SessionId, Load)>,
    /// The whole machine.
    pub machine_cpu: f32,
    pub machine_memory: u64,
    pub machine_free: u64,
}

impl Resources {
    pub fn total(&self) -> Load {
        let mut total = self.app;
        for (_, load) in &self.sessions {
            total.add(load);
        }
        total
    }
}
