use std::fs;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// Single-writer, crash-safe JSON blob store at ~/.hyprspace/v2/<name>.json.
// The frontend owns the schema; we persist the stringified state atomically
// (temp file + fsync + rename) under one lock so concurrent writes can't tear a file.
pub struct Store {
    lock: Mutex<()>,
    dir: PathBuf,
}

impl Store {
    pub fn new() -> Self {
        let dir = state_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("hyprspace: could not create state dir {dir:?}: {e}");
        }
        Store { lock: Mutex::new(()), dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        // keep the name a single safe token so it can't traverse out of the state dir
        // (a "../x" or absolute name would otherwise escape via Path::join)
        let safe: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect();
        let safe = if safe.is_empty() { "_".to_string() } else { safe };
        self.dir.join(format!("{safe}.json"))
    }

    // the protected data is just (), so recovering a poisoned lock is always safe —
    // one panic elsewhere must not brick persistence for the rest of the session.
    fn guard(&self) -> std::sync::MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn save(&self, name: &str, data: &str) -> Result<(), String> {
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
            let _ = fs::remove_file(&tmp); // don't leave a turd that blocks the next write
            return Err(e.to_string());
        }
        // atomic replace — readers see the old or new file, never a partial one
        fs::rename(&tmp, &path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            e.to_string()
        })
    }

    // Ok(None) = file genuinely absent (a real first run). Err = a real IO error —
    // the caller MUST NOT treat that as first run (doing so would clobber saved data).
    pub fn load(&self, name: &str) -> Result<Option<String>, String> {
        let _g = self.guard();
        match fs::read_to_string(self.path(name)) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    // move a present-but-corrupt file aside so a fresh seed won't destroy recoverable data
    pub fn backup(&self, name: &str) -> Result<(), String> {
        let _g = self.guard();
        let path = self.path(name);
        if !path.exists() {
            return Ok(());
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dest = self.dir.join(format!("{name}.corrupt-{ts}.json"));
        fs::rename(&path, &dest).map_err(|e| e.to_string())
    }
}

fn state_dir() -> PathBuf {
    // dev escape hatch: point a dev instance at its own state dir so it can't clobber the
    // installed app's data. unset in release builds, so it never affects real users.
    if let Ok(d) = std::env::var("HYPRSPACE_STATE_DIR") {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".hyprspace").join("v2")
}
