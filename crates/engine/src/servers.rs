// The TCP ports each session's processes listen on, such as a dev server an agent started, so its
// thread can show and open them. A session's processes are found as for Resources. Only a process
// with a shell between it and the session counts: that's where the agent's commands and a
// terminal's own run, while the MCP servers a CLI starts as its children listen too.

use std::collections::{BTreeMap, HashMap};

use hyprspace_proto::SessionId;
use hyprspace_proto::resources::Server;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

const SHELLS: &[&str] = &[
    "bash",
    "sh",
    "zsh",
    "fish",
    "dash",
    "cmd",
    "powershell",
    "pwsh",
];

fn processes() -> System {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, ProcessRefreshKind::new());
    sys
}

/// Each session that has servers, with them in port order.
pub fn read(roots: &[(SessionId, u32)]) -> Vec<(SessionId, Vec<Server>)> {
    let sys = processes();
    let owner = crate::resources::assign(&crate::resources::rows(&sys), std::process::id(), roots);
    let root: HashMap<SessionId, u32> = roots.iter().copied().collect();
    let mut out: BTreeMap<SessionId, Vec<Server>> = BTreeMap::new();
    for (pid, port) in listening() {
        let Some(&Some(id)) = owner.get(&pid) else {
            continue;
        };
        if !under_shell(&sys, pid, root.get(&id).copied()) {
            continue;
        }
        let list = out.entry(id).or_default();
        if !list.iter().any(|s| s.port == port) {
            list.push(Server { port, pid });
        }
    }
    for list in out.values_mut() {
        list.sort_by_key(|s| s.port);
    }
    out.into_iter().collect()
}

/// Ends the process behind a server, when it is one of `roots`' sessions' own. Any other process
/// is left alone, so a stale id can't stop something else.
pub fn stop(pid: u32, roots: &[(SessionId, u32)]) {
    let sys = processes();
    let owner = crate::resources::assign(&crate::resources::rows(&sys), std::process::id(), roots);
    if matches!(owner.get(&pid), Some(Some(_)))
        && let Some(p) = sys.process(Pid::from_u32(pid))
        && p.kill()
    {
        p.wait();
    }
}

fn under_shell(sys: &System, pid: u32, root: Option<u32>) -> bool {
    let mut at = Some(Pid::from_u32(pid));
    // a parent chain can loop on Windows, where ids are reused
    for _ in 0..64 {
        let Some(p) = at.and_then(|a| sys.process(a)) else {
            return false;
        };
        let name = p.name().to_string_lossy().to_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        if SHELLS.contains(&name) {
            return true;
        }
        if Some(p.pid().as_u32()) == root {
            return false;
        }
        at = p.parent();
    }
    false
}

/// Every listening TCP port on the machine, with the process that owns it.
#[cfg(windows)]
fn listening() -> Vec<(u32, u16)> {
    #[link(name = "iphlpapi")]
    unsafe extern "system" {
        fn GetExtendedTcpTable(
            table: *mut u32,
            size: *mut u32,
            order: i32,
            family: u32,
            class: u32,
            reserved: u32,
        ) -> u32;
    }
    const LISTENERS: u32 = 3;
    let mut out = Vec::new();
    // (family, row size, port offset, pid offset) of MIB_TCPROW_OWNER_PID and MIB_TCP6ROW_OWNER_PID
    for (family, row, port_at, pid_at) in [(2u32, 24, 8, 20), (23, 56, 20, 52)] {
        let mut size = 0u32;
        let mut buf: Vec<u32> = Vec::new();
        for _ in 0..3 {
            buf = vec![0; size as usize / 4 + 1];
            size = (buf.len() * 4) as u32;
            // SAFETY: `buf` holds `size` bytes; the call writes at most that and says what it needs
            let got = unsafe {
                GetExtendedTcpTable(buf.as_mut_ptr(), &mut size, 0, family, LISTENERS, 0)
            };
            if got == 0 {
                break;
            }
            buf.clear();
        }
        let bytes: Vec<u8> = buf.iter().flat_map(|w| w.to_le_bytes()).collect();
        let Some(count) = bytes
            .get(..4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        else {
            continue;
        };
        for i in 0..count as usize {
            let Some(r) = bytes.get(4 + i * row..4 + (i + 1) * row) else {
                break;
            };
            let port = u16::from_be_bytes([r[port_at], r[port_at + 1]]);
            let pid = u32::from_le_bytes([r[pid_at], r[pid_at + 1], r[pid_at + 2], r[pid_at + 3]]);
            out.push((pid, port));
        }
    }
    out
}

#[cfg(not(windows))]
fn listening() -> Vec<(u32, u16)> {
    let Ok(out) = std::process::Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpn"])
        .output()
    else {
        return Vec::new();
    };
    let mut pid = 0;
    let mut found = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.parse().unwrap_or(0);
        } else if let Some(port) = line
            .strip_prefix('n')
            .and_then(|n| n.rsplit(':').next())
            .and_then(|p| p.parse().ok())
        {
            found.push((pid, port));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_started_from_a_shell_is_its_sessions_and_its_port_is_read() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let me = std::process::id();
        assert!(listening().contains(&(me, port)));
        // this test process stands in for the session root; nothing between is a shell
        let roots = [(SessionId(1), me)];
        assert!(
            !read(&roots)
                .iter()
                .any(|(_, s)| s.iter().any(|s| s.port == port))
        );
    }
}
