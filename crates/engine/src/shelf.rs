// Worktree folders removed while their threads sat settled, kept so each comes back at the same
// path on the same branch before anything opens there again. The CLIs keep conversations by
// folder, so the same path is what lets a thread resume.

use std::path::Path;
use std::sync::Mutex;

use hyprspace_proto::git::Worktree;

use crate::persist::Store;

const FILE: &str = "shelved";
static LOCK: Mutex<()> = Mutex::new(());

fn load(store: &Store) -> Vec<Worktree> {
    store
        .load(FILE)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save(store: &Store, all: &[Worktree]) {
    if let Ok(json) = serde_json::to_string(all) {
        let _ = store.save(FILE, &json);
    }
}

pub fn record(dir: &Path, wt: &Worktree) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(store) = Store::open(dir) else {
        return;
    };
    let mut all = load(&store);
    all.retain(|w| w.path != wt.path);
    all.push(wt.clone());
    save(&store, &all);
}

/// Puts back the shelved worktree holding `cwd` when its folder is gone. Any other folder is left
/// as it is.
pub fn restore(dir: &Path, cwd: &Path) {
    if cwd.as_os_str().is_empty() || cwd.exists() {
        return;
    }
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(store) = Store::open(dir) else {
        return;
    };
    let mut all = load(&store);
    let Some(at) = all.iter().position(|w| cwd.starts_with(&w.path)) else {
        return;
    };
    if crate::git::unshelve(&all[at]).is_ok() {
        all.remove(at);
        save(&store, &all);
    }
}
