#![allow(unused_imports)]
use super::*;
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

// Absolute path of the repo containing `cwd`, or "" when it isn't a repo. The Files tree needs this
// because `git status --porcelain` reports paths relative to the REPO ROOT, while the tree can be
// rooted at any subfolder (it follows the focused pane) — the root lets us line the two up.
#[tauri::command]
pub async fn git_root(cwd: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Ok(String::new());
        }
        match git(&cwd, &["rev-parse", "--show-toplevel"]) {
            Ok(out) => Ok(out.trim().to_string()),
            Err(_) => Ok(String::new()), // not a repo — no decoration, no error
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

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

    // -z gives NUL-separated records with NO quoting/escaping, so odd paths survive intact. Each
    // record is "XY <path>"; a rename/copy (R or C in either status column) is followed by an EXTRA
    // NUL-terminated field holding the original path — we take the new path and skip that field.
    let status = git(&cwd, &["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
    let mut files = vec![];
    let mut parts = status.split('\0');
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue; // trailing empty field, or a malformed short record
        }
        let code = entry[..2].to_string(); // raw XY porcelain code (X=staged, Y=unstaged)
        let path = entry[3..].to_string(); // new path, unquoted in -z mode
        let b = code.as_bytes();
        if b[0] == b'R' || b[0] == b'C' || b[1] == b'R' || b[1] == b'C' {
            let _ = parts.next(); // consume the original-path field so it isn't read as a record
        }
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

// suggested defaults to pre-fill the "Create PR" dialog (branches, a title + body from the commits)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDefaults {
    head: String,     // current branch
    base: String,     // default target branch
    title: String,    // suggested title
    body: String,     // suggested body
    branches: Vec<String>, // choices for the base picker
    pushed: bool,     // does the current branch have an upstream?
    on_default: bool, // head == base → no PR possible
}

#[tauri::command]
pub async fn git_pr_defaults(cwd: String) -> Result<PrDefaults, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        let head = git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default().trim().to_string();
        // default base = the remote's HEAD branch, else main, else master
        let base = git(&cwd, &["rev-parse", "--abbrev-ref", "origin/HEAD"])
            .ok()
            .map(|s| s.trim().trim_start_matches("origin/").to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if git(&cwd, &["rev-parse", "--verify", "main"]).is_ok() {
                    "main".to_string()
                } else if git(&cwd, &["rev-parse", "--verify", "master"]).is_ok() {
                    "master".to_string()
                } else {
                    "main".to_string()
                }
            });
        // title: the latest commit subject, else the branch name made readable
        let subj = git(&cwd, &["log", "-1", "--pretty=%s"]).unwrap_or_default().trim().to_string();
        let title = if !subj.is_empty() { subj } else { head.replace(['-', '_'], " ") };
        // body: the commit subjects on head but not base, as a bullet list (best-effort)
        let body = git(&cwd, &["log", "--pretty=- %s", &format!("{base}..{head}")])
            .unwrap_or_default()
            .trim()
            .to_string();
        // branch list for the base picker (local + remote, deduped, no HEAD)
        let mut branches: Vec<String> = vec![];
        if let Ok(out) = git(&cwd, &["branch", "--format=%(refname:short)"]) {
            for l in out.lines() {
                let b = l.trim();
                if !b.is_empty() && !branches.iter().any(|x| x == b) {
                    branches.push(b.to_string());
                }
            }
        }
        if let Ok(out) = git(&cwd, &["branch", "-r", "--format=%(refname:short)"]) {
            for l in out.lines() {
                let b = l.trim().trim_start_matches("origin/");
                if !b.is_empty() && b != "HEAD" && !branches.iter().any(|x| x == b) {
                    branches.push(b.to_string());
                }
            }
        }
        let pushed = git(&cwd, &["rev-parse", "--abbrev-ref", &format!("{head}@{{upstream}}")]).is_ok();
        let on_default = head == base;
        Ok(PrDefaults { head, base, title, body, branches, pushed, on_default })
    })
    .await
    .map_err(|e| e.to_string())?
}

// open a GitHub PR via the gh CLI with explicit title/body/base; pushes the branch first if asked
// (so gh never tries to prompt about where to push). returns the PR URL.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn git_create_pr(
    cwd: String,
    title: String,
    body: String,
    base: String,
    draft: bool,
    push: bool,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        if push {
            let head = git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default().trim().to_string();
            if !head.is_empty() {
                let mut p = Command::new("git");
                p.current_dir(&cwd).args(["push", "-u", "origin", &head]);
                #[cfg(windows)]
                p.creation_flags(0x08000000);
                let _ = p.output(); // best-effort; gh reports a clear error if the branch still isn't there
            }
        }
        let mut cmd = Command::new("gh");
        cmd.current_dir(&cwd).args(["pr", "create"]);
        if !title.trim().is_empty() {
            cmd.args(["--title", &title]);
        }
        cmd.args(["--body", &body]);
        if !base.trim().is_empty() {
            cmd.args(["--base", &base]);
        }
        if draft {
            cmd.arg("--draft");
        }
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

// Full "initialize repository" flow driven by the dialog: init + default branch, optional
// .gitignore / README (never clobbers an existing file), optional first commit, and optionally
// create the repo on GitHub via `gh` and push. Returns a summary, or the repo URL when on GitHub.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn git_init_repo(
    cwd: String,
    name: String,
    branch: String,
    gitignore: String, // file contents; empty = don't add
    readme: bool,
    commit: bool,
    commit_msg: String,
    github: bool,
    private: bool,
    description: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cwd.is_empty() {
            return Err("No folder.".to_string());
        }
        // 1. init + name the (unborn) default branch — symbolic-ref works before any commit exists
        git(&cwd, &["init"])?;
        let branch = branch.trim();
        if !branch.is_empty() {
            let _ = git(&cwd, &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")]);
        }
        // 2. .gitignore + README — only write if absent, so we never overwrite the user's files
        if !gitignore.trim().is_empty() {
            let p = Path::new(&cwd).join(".gitignore");
            if !p.exists() {
                let _ = std::fs::write(&p, gitignore);
            }
        }
        if readme {
            let p = Path::new(&cwd).join("README.md");
            if !p.exists() {
                let title = if name.trim().is_empty() { "Project" } else { name.trim() };
                let _ = std::fs::write(&p, format!("# {title}\n"));
            }
        }
        // 3. initial commit — forced when creating on GitHub, since `gh ... --push` needs a commit
        if commit || github {
            git(&cwd, &["add", "-A"])?;
            let msg = if commit_msg.trim().is_empty() { "Initial commit" } else { commit_msg.trim() };
            let mut c = Command::new("git");
            c.current_dir(&cwd).args(["commit", "-m", msg]);
            #[cfg(windows)]
            c.creation_flags(0x08000000);
            let out = c.output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                // an empty folder has nothing to commit — that's fine; anything else bubbles up
                if !err.contains("nothing to commit") {
                    return Err(if err.is_empty() {
                        "Couldn't create the initial commit (is git user.name/email configured?).".to_string()
                    } else {
                        err
                    });
                }
            }
        }
        // 4. create the repo on GitHub + push
        if github {
            let repo = if name.trim().is_empty() {
                Path::new(&cwd).file_name().and_then(|s| s.to_str()).unwrap_or("repo").to_string()
            } else {
                name.trim().to_string()
            };
            let mut g = Command::new("gh");
            g.current_dir(&cwd).args(["repo", "create", &repo]);
            g.arg(if private { "--private" } else { "--public" });
            if !description.trim().is_empty() {
                g.args(["--description", description.trim()]);
            }
            g.arg("--source=.").args(["--remote", "origin"]).arg("--push");
            #[cfg(windows)]
            g.creation_flags(0x08000000);
            let out = g
                .output()
                .map_err(|_| "GitHub CLI (gh) not found — install it from cli.github.com.".to_string())?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                return Err(if err.is_empty() { "Couldn't create the GitHub repo.".to_string() } else { err });
            }
            let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return Ok(if url.is_empty() {
                String::from_utf8_lossy(&out.stderr).trim().to_string()
            } else {
                url
            });
        }
        Ok("Initialized the repository.".to_string())
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
