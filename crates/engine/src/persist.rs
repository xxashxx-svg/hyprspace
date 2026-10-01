// Single-writer, crash-safe JSON blob store, copied from src-tauri/src/persist.rs. The caller owns
// the schema; this persists the serialized state atomically (temp file + fsync + rename) under one
// lock so concurrent writes can't tear a file. Clones share the lock.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Store {
    lock: Arc<Mutex<()>>,
    dir: PathBuf,
}

impl Store {
    pub fn open(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Store {
            lock: Arc::new(Mutex::new(())),
            dir,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    // keep the name a single safe token so it can't traverse out of the state dir
    // (a "../x" or absolute name would otherwise escape via Path::join)
    fn safe_name(name: &str) -> String {
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if safe.is_empty() {
            "_".to_string()
        } else {
            safe
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.json", Self::safe_name(name)))
    }

    // the protected data is just (), so recovering a poisoned lock is always safe, and one panic
    // elsewhere must not brick persistence for the rest of the session
    fn guard(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn save(&self, name: &str, data: &str) -> std::io::Result<()> {
        let _g = self.guard();
        let path = self.path(name);
        let tmp = path.with_extension("json.tmp");
        let write = || -> std::io::Result<()> {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(data.as_bytes())?;
            f.sync_all()?;
            Ok(())
        };
        if let Err(e) = write() {
            // don't leave a temp file that blocks the next write
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        // atomic replace: readers see the old or new file, never a partial one
        fs::rename(&tmp, &path).inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
    }

    /// Ok(None) means the file is genuinely absent (a real first run). Err is a real IO error,
    /// and the caller MUST NOT treat that as a first run: doing so would clobber saved data.
    pub fn load(&self, name: &str) -> std::io::Result<Option<String>> {
        let _g = self.guard();
        match fs::read_to_string(self.path(name)) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Move a present-but-corrupt file aside so a fresh seed won't destroy recoverable data.
    pub fn backup(&self, name: &str) -> std::io::Result<()> {
        let _g = self.guard();
        let path = self.path(name);
        if !path.exists() {
            return Ok(());
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dest = self
            .dir
            .join(format!("{}.corrupt-{ts}.json", Self::safe_name(name)));
        fs::rename(&path, &dest)
    }
}

/// Where the GPUI app keeps its state: `~/.hyprspace/native`, beside the Tauri app's `v2` rather
/// than inside it. The two apps run side by side during the rewrite and their schemas differ, so
/// sharing a folder would let one clobber the other.
pub fn state_dir() -> PathBuf {
    // dev escape hatch: point a dev instance at its own state dir so it can't clobber the
    // installed app's data. unset in release builds, so it never affects real users.
    if let Ok(d) = std::env::var("HYPRSPACE_STATE_DIR")
        && !d.trim().is_empty()
    {
        return PathBuf::from(d);
    }
    crate::util::home_dir().join(".hyprspace").join("native")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_loads_and_reports_absence() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.load("spaces").unwrap(), None);
        store.save("spaces", "[1]").unwrap();
        store.save("spaces", "[1,2]").unwrap();
        assert_eq!(store.load("spaces").unwrap().as_deref(), Some("[1,2]"));
        assert!(!dir.path().join("spaces.json.tmp").exists());
    }

    #[test]
    fn names_cannot_escape_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("state")).unwrap();
        store.save("../../evil", "x").unwrap();
        assert!(dir.path().join("state").join("______evil.json").exists());
        assert_eq!(store.load("").unwrap(), None);
    }

    #[test]
    fn backup_moves_the_file_aside() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.backup("missing").unwrap();
        store.save("ui", "{broken").unwrap();
        store.backup("ui").unwrap();
        assert_eq!(store.load("ui").unwrap(), None);
        let moved: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            moved.iter().any(|n| n.starts_with("ui.corrupt-")),
            "{moved:?}"
        );
    }
}
