// Dev cockpit backend: git changes/diff for the Review dock, and dev-command detection
// for the Run panel. All best-effort — a non-repo or missing git returns empty, never errors.
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::Serialize;
use serde_json::Value;
use base64::Engine;

#[derive(Serialize)]
pub struct FileChange {
    path: String,
    status: String, // porcelain code: M, A, D, R, ??, etc.
    added: u32,
    removed: u32,
}

fn git(cwd: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(cwd).args(args);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW — don't flash a console on each git call
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// Changed files in the active workspace's repo, with +/- line counts where git has them.
// git on a large repo can take seconds, and sync Tauri commands run on the UI thread — so the
// dock's 4s poll would freeze the window. Push the work onto a blocking thread.
#[tauri::command]
pub async fn git_changes(cwd: String) -> Result<Vec<FileChange>, String> {
    tauri::async_runtime::spawn_blocking(move || git_changes_blocking(cwd))
        .await
        .map_err(|e| e.to_string())?
}

fn git_changes_blocking(cwd: String) -> Result<Vec<FileChange>, String> {
    if cwd.is_empty() || git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        return Ok(vec![]); // not a repo → nothing to show, quietly
    }

    // line counts from staged + unstaged numstat, summed per path
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    for args in [
        ["diff", "--numstat"].as_slice(),
        ["diff", "--numstat", "--cached"].as_slice(),
    ] {
        if let Ok(s) = git(&cwd, args) {
            for line in s.lines() {
                let mut p = line.split('\t');
                let a = p.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
                let r = p.next().unwrap_or("0").parse::<u32>().unwrap_or(0);
                if let Some(path) = p.next() {
                    let e = counts.entry(path.to_string()).or_insert((0, 0));
                    e.0 += a;
                    e.1 += r;
                }
            }
        }
    }

    let status = git(&cwd, &["status", "--porcelain=v1", "--untracked-files=all"])?;
    let mut files = vec![];
    for line in status.lines() {
        if line.len() < 4 {
            continue;
        }
        let code = line[..2].to_string(); // raw XY porcelain code (X=staged, Y=unstaged)
        let raw = &line[3..];
        // renames look like "old -> new"; key off the new path
        let path = raw.split(" -> ").last().unwrap_or(raw).trim_matches('"').to_string();
        let (added, removed) = counts.get(&path).copied().unwrap_or((0, 0));
        files.push(FileChange { path, status: code, added, removed });
    }
    Ok(files)
}

// Unified diff for one file. Falls back through HEAD → staged → untracked (all-added).
#[tauri::command]
pub async fn git_diff(cwd: String, path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || git_diff_blocking(cwd, path))
        .await
        .map_err(|e| e.to_string())?
}

fn git_diff_blocking(cwd: String, path: String) -> Result<String, String> {
    if cwd.is_empty() {
        return Ok(String::new());
    }
    let d = git(&cwd, &["diff", "HEAD", "--", &path]).unwrap_or_default();
    if !d.trim().is_empty() {
        return Ok(d);
    }
    let staged = git(&cwd, &["diff", "--cached", "--", &path]).unwrap_or_default();
    if !staged.trim().is_empty() {
        return Ok(staged);
    }
    // untracked file: diff against nothing so it shows as all-added. --no-index exits 1
    // when the files differ, which is expected here, so read stdout regardless of status.
    let out = Command::new("git")
        .arg("-C")
        .arg(&cwd)
        .args(["diff", "--no-index", "--", "/dev/null", &path])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[derive(Serialize)]
pub struct Worktree {
    path: String,
    branch: String,
}

// Create an isolated worktree off the workspace repo so an agent can work without
// colliding with others. Worktrees live under ~/.hyprspace/worktrees/<repo>-<branch>
// (outside the repo, so they don't pollute its own git status).
#[tauri::command]
pub async fn worktree_create(cwd: String, name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || worktree_create_blocking(cwd, name))
        .await
        .map_err(|e| e.to_string())?
}

fn worktree_create_blocking(cwd: String, name: String) -> Result<String, String> {
    if cwd.is_empty() {
        return Err("no workspace folder".into());
    }
    let root = git(&cwd, &["rev-parse", "--show-toplevel"]).map_err(|_| "not a git repo")?;
    let root = root.trim();
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let branch = format!("hs/{}", safe.trim_matches('-'));

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let repo_name = Path::new(root)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".into());
    let wt = Path::new(&home)
        .join(".hyprspace")
        .join("worktrees")
        .join(format!("{repo_name}-{}", branch.replace('/', "-")));
    let wt = wt.to_string_lossy().to_string();

    git(&cwd, &["worktree", "add", "-b", &branch, &wt, "HEAD"])?;
    Ok(wt)
}

#[tauri::command]
pub async fn worktree_remove(cwd: String, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        git(&cwd, &["worktree", "remove", "--force", &path]).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn worktree_list(cwd: String) -> Result<Vec<Worktree>, String> {
    tauri::async_runtime::spawn_blocking(move || worktree_list_blocking(cwd))
        .await
        .map_err(|e| e.to_string())
}

fn worktree_list_blocking(cwd: String) -> Vec<Worktree> {
    if cwd.is_empty() {
        return vec![];
    }
    let out = git(&cwd, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    let mut res = vec![];
    let mut path = String::new();
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = p.to_string();
        } else if let Some(b) = line.strip_prefix("branch ") {
            res.push(Worktree {
                path: path.clone(),
                branch: b.replace("refs/heads/", ""),
            });
        }
    }
    res
}

// Best-guess command to run the project (for the Run panel).
#[tauri::command]
pub async fn detect_run_cmd(cwd: String) -> String {
    tauri::async_runtime::spawn_blocking(move || detect_run_cmd_blocking(cwd))
        .await
        .unwrap_or_default()
}

fn detect_run_cmd_blocking(cwd: String) -> String {
    let p = Path::new(&cwd);
    if let Ok(s) = std::fs::read_to_string(p.join("package.json")) {
        if s.contains("\"dev\"") {
            return "npm run dev".into();
        }
        if s.contains("\"start\"") {
            return "npm start".into();
        }
    }
    if p.join("Cargo.toml").exists() {
        return "cargo run".into();
    }
    String::new()
}

// ---- git write ops for the topbar "Commit & push" menu ----

// push the current branch; if it has no upstream yet, set one to origin on first push
fn push_current(cwd: &str) -> Result<(), String> {
    match git(cwd, &["push"]) {
        Ok(_) => Ok(()),
        Err(e)
            if e.contains("no upstream")
                || e.contains("has no upstream")
                || e.contains("set-upstream") =>
        {
            git(cwd, &["push", "-u", "origin", "HEAD"]).map(|_| ())
        }
        Err(e) => Err(e),
    }
}

// commit with the given message and optionally push. stage_all=true stages everything first
// (topbar "Commit & push"); false commits only what's already staged (Source Control panel).
#[tauri::command]
pub async fn git_commit(cwd: String, message: String, push: bool, stage_all: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() || git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_err() {
            return Err("Not a git repository.".to_string());
        }
        if stage_all {
            git(&cwd, &["add", "-A"])?;
        }
        if let Err(e) = git(&cwd, &["commit", "-m", &message]) {
            return Err(if e.contains("nothing to commit") {
                "Nothing staged to commit.".to_string()
            } else {
                e
            });
        }
        if push {
            push_current(&cwd)?;
            return Ok("Changes committed and pushed.".to_string());
        }
        Ok("Changes committed.".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn git_push(cwd: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        push_current(&cwd)?;
        Ok("Pushed to remote.".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// open a GitHub PR via the gh CLI (fills title/body from commits); returns the PR URL
#[tauri::command]
pub async fn git_create_pr(cwd: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        let mut cmd = Command::new("gh");
        cmd.current_dir(&cwd).args(["pr", "create", "--fill"]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        let out = cmd
            .output()
            .map_err(|_| "GitHub CLI (gh) not found — install it from cli.github.com.".to_string())?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() { "Couldn't create the PR.".to_string() } else { err });
        }
        let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Ok(if url.is_empty() {
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        } else {
            url
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// whether a folder is inside a git work tree — gates the topbar "Commit & push" button
#[tauri::command]
pub async fn git_is_repo(cwd: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        !cwd.is_empty() && git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_ok()
    })
    .await
    .unwrap_or(false)
}

// `git init` for a folder that isn't a repo yet
#[tauri::command]
pub async fn git_init(cwd: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        git(&cwd, &["init"])?;
        Ok("Initialized a git repository.".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Default)]
pub struct BranchInfo {
    branch: String,
    ahead: u32,
    behind: u32,
    upstream: bool,
    is_repo: bool,
}

// current branch + how far ahead/behind its upstream — for the Source Control header
#[tauri::command]
pub async fn git_branch_info(cwd: String) -> BranchInfo {
    tauri::async_runtime::spawn_blocking(move || {
        let mut bi = BranchInfo::default();
        if cwd.is_empty() || git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_err() {
            return bi;
        }
        bi.is_repo = true;
        bi.branch = git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string();
        // "<behind>\t<ahead>" relative to the upstream, if one is set
        if let Ok(s) = git(&cwd, &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"]) {
            let mut it = s.split_whitespace();
            bi.behind = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            bi.ahead = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            bi.upstream = true;
        }
        bi
    })
    .await
    .unwrap_or_default()
}

// stage / unstage / discard a file (empty path = all, except discard which refuses all)
#[tauri::command]
pub async fn git_file_op(cwd: String, op: String, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        let all = path.is_empty();
        match op.as_str() {
            "stage" => {
                if all {
                    git(&cwd, &["add", "-A"])?;
                } else {
                    git(&cwd, &["add", "--", &path])?;
                }
            }
            "unstage" => {
                if all {
                    git(&cwd, &["reset", "-q"])?;
                } else {
                    git(&cwd, &["reset", "-q", "--", &path])?;
                }
            }
            "discard" => {
                if all {
                    return Err("Refusing to discard everything at once.".to_string());
                }
                // restore a tracked file to HEAD; if it's untracked (not in HEAD), delete it
                if git(&cwd, &["checkout", "HEAD", "--", &path]).is_err() {
                    let _ = std::fs::remove_file(Path::new(&cwd).join(&path));
                }
            }
            _ => return Err("unknown op".to_string()),
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// Create (or reuse) a folder for a new project, optionally seeding README.md / .gitignore.
// create_dir_all is idempotent, so this is safe to call on an existing folder too; we only
// write the seed files when they're absent, never clobbering something already there.
#[tauri::command]
pub async fn create_project_dir(
    path: String,
    readme: Option<String>,
    gitignore: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if path.trim().is_empty() {
            return Err("No folder path.".to_string());
        }
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        if let Some(body) = readme {
            let p = Path::new(&path).join("README.md");
            if !p.exists() {
                std::fs::write(&p, body).map_err(|e| e.to_string())?;
            }
        }
        if let Some(body) = gitignore {
            let p = Path::new(&path).join(".gitignore");
            if !p.exists() {
                std::fs::write(&p, body).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// open a folder in the OS file manager (Explorer / Finder / xdg-open)
#[tauri::command]
pub async fn reveal_path(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if path.trim().is_empty() {
            return Err("No folder.".to_string());
        }
        // resolve to a real existing dir before launching anything. this is the security guard:
        // a "vscode://" / "http://" scheme or a "-flag" string won't canonicalize to a folder,
        // so the launcher never sees a URI to hand off or an option it could misread as a flag.
        let canon = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
        if !canon.is_dir() {
            return Err("Not a folder.".to_string());
        }
        // strip Windows' \\?\ verbatim prefix so explorer opens the folder normally
        let s = canon.to_string_lossy();
        #[cfg(windows)]
        let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
        let s = s.to_string();
        if s.starts_with('-') {
            return Err("Invalid path.".to_string());
        }
        #[cfg(windows)]
        let mut cmd = Command::new("explorer");
        #[cfg(target_os = "macos")]
        let mut cmd = Command::new("open");
        #[cfg(all(unix, not(target_os = "macos")))]
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&s);
        // explorer returns a nonzero exit even on success, so just launch it and don't wait
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- file tree: list one directory level for the Files panel ----

#[derive(Serialize)]
pub struct DirEntry {
    name: String,
    dir: bool,
}

// list a single directory level (folders first, then files; case-insensitive alphabetical).
// lazy — the UI calls this per folder on expand, so a huge tree is never read all at once.
#[tauri::command]
pub async fn list_dir(path: String) -> Result<Vec<DirEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let rd = std::fs::read_dir(&path).map_err(|e| e.to_string())?;
        let mut out: Vec<DirEntry> = vec![];
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            out.push(DirEntry { name, dir });
        }
        out.sort_by(|a, b| {
            (b.dir as u8)
                .cmp(&(a.dir as u8))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- provider status (version + signed-in account/plan) for Settings → Providers ----

#[derive(Serialize, Default)]
pub struct ProviderStatus {
    id: String,
    installed: bool,
    version: Option<String>,
    account: Option<String>,
    plan: Option<String>,
    detail: Option<String>,
}

fn home_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default(),
    )
}

// run "<cli> --version". On Windows go through `cmd /c` so a .cmd/.ps1 shim on PATH resolves —
// but pass the args separately (never build a shell string) so `cli` can't be misread as syntax.
fn cli_version(cli: &str) -> Option<String> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/c", cli, "--version"]);
        c
    } else {
        let mut c = Command::new(cli);
        c.arg("--version");
        c
    };
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    // first token that looks like a version number, e.g. "2.1.183 (Claude Code)" → 2.1.183
    for tok in s.split_whitespace() {
        let t = tok
            .trim_start_matches('v')
            .trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        let head = t.split('.').next().unwrap_or("");
        if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) && t.contains('.') {
            return Some(t.to_string());
        }
    }
    s.lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn read_json(path: std::path::PathBuf) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

// pull the (unverified) payload out of a JWT — we only read display claims, never trust it
fn decode_jwt(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[tauri::command]
pub async fn provider_status(id: String) -> ProviderStatus {
    tauri::async_runtime::spawn_blocking(move || provider_status_blocking(&id))
        .await
        .unwrap_or_default()
}

fn provider_status_blocking(id: &str) -> ProviderStatus {
    let mut st = ProviderStatus {
        id: id.to_string(),
        ..Default::default()
    };
    let cli = match id {
        "claude" | "gemini" | "codex" => id,
        _ => return st,
    };
    st.version = cli_version(cli);
    st.installed = st.version.is_some();
    if !st.installed {
        st.detail = Some(format!("`{cli}` is not installed or not on PATH."));
        return st;
    }

    let home = home_dir();
    match id {
        "claude" => {
            if let Some(v) = read_json(home.join(".claude.json")) {
                st.account = v["oauthAccount"]["emailAddress"].as_str().map(String::from);
            }
            if let Some(v) = read_json(home.join(".claude").join(".credentials.json")) {
                if let Some(sub) = v["claudeAiOauth"]["subscriptionType"].as_str() {
                    st.plan = Some(format!("Claude {} Subscription", title_case(sub)));
                }
            }
            if st.account.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        "codex" => {
            if let Some(v) = read_json(home.join(".codex").join("auth.json")) {
                if let Some(tok) = v["tokens"]["id_token"].as_str() {
                    if let Some(p) = decode_jwt(tok) {
                        st.account = p["email"].as_str().map(String::from);
                        if let Some(plan) =
                            p["https://api.openai.com/auth"]["chatgpt_plan_type"].as_str()
                        {
                            st.plan = Some(format!("ChatGPT {} Subscription", title_case(plan)));
                        }
                    }
                }
                if st.account.is_none() && v["OPENAI_API_KEY"].as_str().is_some() {
                    st.detail = Some("Authenticated via API key".into());
                }
            }
            if st.account.is_none() && st.detail.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        "gemini" => {
            if let Some(v) = read_json(home.join(".gemini").join("google_accounts.json")) {
                st.account = v["active"].as_str().map(String::from);
            }
            if st.account.is_none() {
                st.detail = Some("Not signed in".into());
            }
        }
        _ => {}
    }
    st
}

// ---- MCP servers: read/write Claude's ~/.claude.json top-level "mcpServers" ----

#[derive(Serialize)]
pub struct McpEntry {
    name: String,
    config: Value,
}

fn claude_json_path() -> std::path::PathBuf {
    home_dir().join(".claude.json")
}

#[tauri::command]
pub async fn mcp_list() -> Vec<McpEntry> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut out = vec![];
        if let Some(v) = read_json(claude_json_path()) {
            if let Some(map) = v.get("mcpServers").and_then(|m| m.as_object()) {
                for (name, config) in map {
                    out.push(McpEntry {
                        name: name.clone(),
                        config: config.clone(),
                    });
                }
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    })
    .await
    .unwrap_or_default()
}

// upsert a server (preserving every other key in the file). prev_name handles a rename.
#[tauri::command]
pub async fn mcp_set(name: String, config: Value, prev_name: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if name.trim().is_empty() {
            return Err("Server needs a name.".to_string());
        }
        let path = claude_json_path();
        let mut root = read_json(path.clone()).unwrap_or_else(|| serde_json::json!({}));
        if !root.is_object() {
            root = serde_json::json!({});
        }
        let obj = root.as_object_mut().unwrap();
        let servers = obj.entry("mcpServers").or_insert_with(|| serde_json::json!({}));
        if !servers.is_object() {
            *servers = serde_json::json!({});
        }
        let smap = servers.as_object_mut().unwrap();
        if let Some(prev) = prev_name {
            if prev != name {
                smap.remove(&prev);
            }
        }
        smap.insert(name, config);
        let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---- skills: discover Claude skills/commands so the UI can list + drag them ----

#[derive(Serialize)]
pub struct SkillItem {
    name: String,
    command: String, // "/name"
    description: String,
    body: String, // markdown after the frontmatter — the instructions, for non-Claude agents
    scope: String, // "project" | "user"
    kind: String,  // "skill" | "command"
}

// everything after the YAML frontmatter block (or the whole file if there's none)
fn strip_frontmatter(content: &str) -> String {
    let mut lines = content.lines();
    if lines.next().map(|l| l.trim()) == Some("---") {
        let mut body = String::new();
        let mut in_fm = true;
        for line in lines {
            if in_fm {
                if line.trim() == "---" {
                    in_fm = false;
                }
                continue;
            }
            body.push_str(line);
            body.push('\n');
        }
        return body.trim().to_string();
    }
    content.trim().to_string()
}

// pull a single `key: value` line out of a markdown YAML frontmatter block (best-effort)
fn frontmatter_field(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next().map(|l| l.trim()) != Some("---") {
        return None;
    }
    let pfx = format!("{key}:");
    for line in lines {
        let t = line.trim();
        if t == "---" {
            break;
        }
        if let Some(rest) = t.strip_prefix(&pfx) {
            return Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

fn scan_skills(base: &Path, scope: &str, out: &mut Vec<SkillItem>, seen: &mut std::collections::HashSet<String>) {
    // .claude/skills/<name>/SKILL.md
    if let Ok(rd) = std::fs::read_dir(base.join(".claude").join("skills")) {
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            let cmd = format!("/{name}");
            if seen.contains(&cmd) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(e.path().join("SKILL.md")) {
                seen.insert(cmd.clone());
                out.push(SkillItem {
                    description: frontmatter_field(&content, "description").unwrap_or_default(),
                    body: strip_frontmatter(&content),
                    command: cmd,
                    name,
                    scope: scope.to_string(),
                    kind: "skill".to_string(),
                });
            }
        }
    }
    // .claude/commands/<name>.md
    if let Ok(rd) = std::fs::read_dir(base.join(".claude").join("commands")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map_or(false, |x| x == "md") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let cmd = format!("/{stem}");
                    if seen.contains(&cmd) {
                        continue;
                    }
                    seen.insert(cmd.clone());
                    let content = std::fs::read_to_string(&p).unwrap_or_default();
                    out.push(SkillItem {
                        description: frontmatter_field(&content, "description").unwrap_or_default(),
                        body: strip_frontmatter(&content),
                        command: cmd,
                        name: stem.to_string(),
                        scope: scope.to_string(),
                        kind: "command".to_string(),
                    });
                }
            }
        }
    }
}

// project skills (active folder) take priority over user-global ones of the same name
#[tauri::command]
pub async fn list_skills(cwd: String) -> Vec<SkillItem> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = vec![];
        let mut seen = std::collections::HashSet::new();
        let home = home_dir();
        let c = Path::new(&cwd);
        if !cwd.trim().is_empty() && c != home {
            scan_skills(c, "project", &mut out, &mut seen);
        }
        scan_skills(&home, "user", &mut out, &mut seen);
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    })
    .await
    .unwrap_or_default()
}

// keep a skill name safe as a folder name (no path traversal, no separators)
fn safe_skill_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn skill_base(scope: &str, cwd: &str) -> Option<std::path::PathBuf> {
    match scope {
        "user" => Some(home_dir()),
        "project" if !cwd.trim().is_empty() => Some(std::path::PathBuf::from(cwd)),
        _ => None,
    }
}

// the file backing an item: skills/<name>/SKILL.md, or commands/<name>.md for legacy commands
fn skill_file(base: &std::path::Path, kind: &str, name: &str) -> std::path::PathBuf {
    let safe = safe_skill_name(name);
    if kind == "command" {
        base.join(".claude").join("commands").join(format!("{safe}.md"))
    } else {
        base.join(".claude").join("skills").join(safe).join("SKILL.md")
    }
}

#[tauri::command]
pub async fn skill_read(scope: String, cwd: String, name: String, kind: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let base = skill_base(&scope, &cwd).ok_or("No folder for that scope.")?;
        std::fs::read_to_string(skill_file(&base, &kind, &name)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skill_write(
    scope: String,
    cwd: String,
    name: String,
    content: String,
    kind: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if safe_skill_name(&name).is_empty() {
            return Err("Needs a name (letters, numbers, hyphens).".to_string());
        }
        let base = skill_base(&scope, &cwd).ok_or("No folder for that scope.")?;
        let file = skill_file(&base, &kind, &name);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&file, content).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skill_delete(scope: String, cwd: String, name: String, kind: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let safe = safe_skill_name(&name);
        if safe.is_empty() {
            return Ok(());
        }
        let base = skill_base(&scope, &cwd).ok_or("No folder for that scope.")?;
        if kind == "command" {
            let p = base.join(".claude").join("commands").join(format!("{safe}.md"));
            if p.exists() {
                std::fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
        } else {
            let dir = base.join(".claude").join("skills").join(&safe);
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn mcp_remove(name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = claude_json_path();
        let Some(mut root) = read_json(path.clone()) else {
            return Ok(());
        };
        if let Some(smap) = root.get_mut("mcpServers").and_then(|m| m.as_object_mut()) {
            smap.remove(&name);
        }
        let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
