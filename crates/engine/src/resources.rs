// What HyprSpace and everything it started use, for Settings, Resources. Processes are found by
// walking down from each session's shell or CLI, then from our own process for the rest.

use std::collections::HashMap;
use std::time::Instant;

use hyprspace_proto::SessionId;
use hyprspace_proto::resources::{Load, Resources};
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, ProcessRefreshKind, ProcessesToUpdate, System};

/// Kept between readings, since CPU and I/O are measured between two looks.
pub struct Sampler {
    sys: System,
    at: Option<Instant>,
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            sys: System::new(),
            at: None,
        }
    }

    fn refresh(&mut self) {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            ProcessRefreshKind::new()
                .with_cpu()
                .with_memory()
                .with_disk_usage(),
        );
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
    }

    /// One reading. The first waits a moment for a second look to measure against.
    pub fn read(&mut self, roots: &[(SessionId, u32)]) -> Resources {
        let last = match self.at {
            Some(at) => at,
            None => {
                self.refresh();
                Instant::now()
            }
        };
        if let Some(wait) = MINIMUM_CPU_UPDATE_INTERVAL.checked_sub(last.elapsed()) {
            std::thread::sleep(wait);
        }
        self.refresh();
        let secs = last.elapsed().as_secs_f64().max(0.001);
        self.at = Some(Instant::now());

        let procs = self.sys.processes();
        let rows: Vec<(u32, u32, u64)> = procs
            .iter()
            .map(|(pid, p)| {
                let parent = p.parent().map_or(0, |p| p.as_u32());
                (pid.as_u32(), parent, p.start_time())
            })
            .collect();
        let cores = self.sys.cpus().len().max(1) as f32;
        let mut app = Load::default();
        let mut sessions: HashMap<SessionId, Load> = HashMap::new();
        for (pid, owner) in assign(&rows, std::process::id(), roots) {
            let Some(p) = procs.get(&sysinfo::Pid::from_u32(pid)) else {
                continue;
            };
            let io = p.disk_usage();
            let load = Load {
                cpu: p.cpu_usage() / cores,
                memory: p.memory(),
                read: (io.read_bytes as f64 / secs) as u64,
                write: (io.written_bytes as f64 / secs) as u64,
                processes: 1,
            };
            match owner {
                Some(id) => sessions.entry(id).or_default().add(&load),
                None => app.add(&load),
            }
        }
        let mut sessions: Vec<_> = sessions.into_iter().collect();
        sessions.sort_by_key(|(id, _)| *id);
        Resources {
            app,
            sessions,
            machine_cpu: self.sys.global_cpu_usage(),
            machine_memory: self.sys.total_memory(),
            machine_free: self.sys.available_memory(),
        }
    }
}

/// The session each process under `roots` or `me` belongs to, None for HyprSpace's own. `rows` is
/// every process as (pid, parent, start). A child counts only if it started after its parent:
/// Windows leaves a dead parent's id on its children and can hand that id to a new process.
fn assign(
    rows: &[(u32, u32, u64)],
    me: u32,
    roots: &[(SessionId, u32)],
) -> HashMap<u32, Option<SessionId>> {
    let start: HashMap<u32, u64> = rows.iter().map(|&(pid, _, at)| (pid, at)).collect();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(pid, parent, at) in rows {
        if pid != parent && start.get(&parent).is_some_and(|&p| at >= p) {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut owner = HashMap::new();
    let seeds = roots.iter().map(|&(id, pid)| (pid, Some(id)));
    for (seed, id) in seeds.chain([(me, None)]) {
        if !start.contains_key(&seed) {
            continue;
        }
        let mut stack = vec![seed];
        while let Some(pid) = stack.pop() {
            if owner.contains_key(&pid) {
                continue;
            }
            owner.insert(pid, id);
            stack.extend(children.get(&pid).into_iter().flatten().copied());
        }
    }
    owner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_process_goes_to_the_session_above_it_and_the_rest_to_the_app() {
        let rows = [
            (1, 0, 0),
            (10, 1, 100),
            (20, 10, 110),
            (21, 20, 120),
            (30, 10, 130),
            (31, 30, 131),
            (40, 10, 140),
            (50, 1, 150),
        ];
        let owner = assign(&rows, 10, &[(SessionId(1), 20), (SessionId(2), 30)]);
        assert_eq!(owner[&10], None);
        assert_eq!(owner[&40], None);
        assert_eq!(owner[&20], Some(SessionId(1)));
        assert_eq!(owner[&21], Some(SessionId(1)));
        assert_eq!(owner[&31], Some(SessionId(2)));
        assert!(!owner.contains_key(&1));
        assert!(!owner.contains_key(&50));
    }

    #[test]
    fn a_process_that_reused_a_dead_parents_id_is_not_ours() {
        // 20 is newer than 30, so 30's parent id 20 names a process that died
        let rows = [(10, 1, 100), (20, 10, 300), (30, 20, 200)];
        let owner = assign(&rows, 10, &[]);
        assert!(owner.contains_key(&20));
        assert!(!owner.contains_key(&30));
    }

    #[test]
    fn a_session_outside_our_tree_still_counts_and_a_gone_one_is_skipped() {
        let rows = [(10, 1, 100), (60, 2, 50), (61, 60, 60)];
        let owner = assign(&rows, 10, &[(SessionId(5), 60), (SessionId(6), 99)]);
        assert_eq!(owner[&61], Some(SessionId(5)));
        assert_eq!(owner.len(), 3);
    }

    #[test]
    fn ids_that_loop_end() {
        let rows = [(10, 1, 100), (20, 30, 100), (30, 20, 100), (40, 10, 100)];
        let owner = assign(&rows, 20, &[]);
        assert_eq!(owner.len(), 2);
    }
}
