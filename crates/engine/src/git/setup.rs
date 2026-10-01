// Making a folder or a repository: new project folders, `git init`, and clones.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use hyprspace_proto::git::InitRepo;

use super::{NO_GH, Result, empty, gh, git, git_cmd, not_found};

/// Create (or reuse) a folder for a new project, seeding README.md and .gitignore when given.
/// Seed files are only written when absent, never clobbering something already there.
pub fn create_project_dir(
    path: &Path,
    readme: Option<&str>,
    gitignore: Option<&str>,
) -> Result<()> {
    if empty(path) {
        return Err("No folder path.".to_string());
    }
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    for (name, body) in [("README.md", readme), (".gitignore", gitignore)] {
        let p = path.join(name);
        if let Some(body) = body
            && !p.exists()
        {
            std::fs::write(&p, body).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// `git init` for a folder that isn't a repo yet.
pub fn init(cwd: &Path) -> Result<String> {
    if empty(cwd) {
        return Err("No folder.".to_string());
    }
    git(cwd, &["init"])?;
    Ok("Initialized a git repository.".to_string())
}

/// The full "initialize repository" flow: init with a default branch, optional .gitignore and
/// README (never clobbering an existing file), an optional first commit, and optionally a new
/// GitHub repo through `gh`, pushed. Returns a summary, or the repo URL when on GitHub.
pub fn init_repo(cwd: &Path, o: &InitRepo) -> Result<String> {
    if empty(cwd) {
        return Err("No folder.".to_string());
    }
    // 1. init + name the (unborn) default branch; symbolic-ref works before any commit exists
    git(cwd, &["init"])?;
    let branch = o.branch.trim();
    if !branch.is_empty() {
        let _ = git(
            cwd,
            &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")],
        );
    }
    // 2. .gitignore + README, only if absent
    if !o.gitignore.trim().is_empty() {
        let p = cwd.join(".gitignore");
        if !p.exists() {
            let _ = std::fs::write(&p, &o.gitignore);
        }
    }
    if o.readme {
        let p = cwd.join("README.md");
        if !p.exists() {
            let title = if o.name.trim().is_empty() {
                "Project"
            } else {
                o.name.trim()
            };
            let _ = std::fs::write(&p, format!("# {title}\n"));
        }
    }
    // 3. initial commit, forced when creating on GitHub since `gh ... --push` needs a commit
    if o.commit || o.github {
        git(cwd, &["add", "-A"])?;
        let msg = if o.commit_msg.trim().is_empty() {
            "Initial commit"
        } else {
            o.commit_msg.trim()
        };
        let out = git_cmd()
            .current_dir(cwd)
            .args(["commit", "-m", msg])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            // an empty folder has nothing to commit, which is fine; anything else bubbles up
            if !err.contains("nothing to commit") {
                return Err(if err.is_empty() {
                    "Couldn't create the initial commit. Is git's user.name and user.email set?"
                        .to_string()
                } else {
                    err
                });
            }
        }
    }
    // 4. create the repo on GitHub and push
    if o.github {
        let repo = if o.name.trim().is_empty() {
            cwd.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("repo")
                .to_string()
        } else {
            o.name.trim().to_string()
        };
        let mut g = gh(cwd);
        g.args(["repo", "create", &repo]);
        g.arg(if o.private { "--private" } else { "--public" });
        if !o.description.trim().is_empty() {
            g.args(["--description", o.description.trim()]);
        }
        g.arg("--source=.")
            .args(["--remote", "origin"])
            .arg("--push");
        let out = g.output().map_err(|_| NO_GH.to_string())?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() {
                "Couldn't create the GitHub repo.".to_string()
            } else {
                err
            });
        }
        let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return Ok(if url.is_empty() {
            String::from_utf8_lossy(&out.stderr).trim().to_string()
        } else {
            url
        });
    }
    Ok("Initialized the repository.".to_string())
}

/// Where a clone goes: `<parent>/<name>`, or with `here` straight into `parent` when it's empty.
/// Refuses an existing folder so a typo cannot merge two checkouts.
fn clone_dest(url: &str, parent: &Path, name: &str, here: bool) -> Result<PathBuf> {
    if url.is_empty() || empty(parent) {
        return Err("Need a repository and a folder.".to_string());
    }
    // anything that starts with a dash would be read as a git option
    if url.starts_with('-') {
        return Err("That does not look like a repository URL.".to_string());
    }
    if here {
        // git only allows cloning into an empty folder
        if parent
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
        {
            return Err(format!(
                "{} isn't empty, so the repository can't go straight into it.",
                parent.display()
            ));
        }
        return Ok(parent.to_path_buf());
    }
    if name.is_empty() {
        return Err("Give the new folder a name.".to_string());
    }
    if name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err("The folder name cannot contain slashes.".to_string());
    }
    let d = parent.join(name);
    if d.exists() {
        return Err(format!("{} already exists.", d.display()));
    }
    Ok(d)
}

/// `git clone <url>` per `clone_dest`, streaming git's progress lines to `on_progress` as they
/// arrive. Returns the new folder.
pub fn clone(
    url: &str,
    parent: &Path,
    name: &str,
    here: bool,
    mut on_progress: impl FnMut(String),
) -> Result<PathBuf> {
    let url = url.trim();
    let dest = clone_dest(url, parent, name.trim(), here)?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;

    // --progress makes git report even though nobody's attached; it writes the counters to
    // stderr, redrawing each with \r, so every \r or \n ends a line worth passing on
    let mut child = git_cmd()
        .arg("-C")
        .arg(parent)
        .args(["clone", "--progress", "--", url])
        .arg(&dest)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        // there's no console to type a password into, so a prompt would hang the clone forever
        .env("GIT_TERMINAL_PROMPT", "0")
        .spawn()
        .map_err(not_found)?;
    let mut err = child.stderr.take().ok_or("git gave no output")?;
    let mut buf = [0u8; 4096];
    let mut line: Vec<u8> = Vec::new();
    let mut tail: Vec<String> = Vec::new();
    loop {
        let n = err.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if b != b'\r' && b != b'\n' {
                line.push(b);
                continue;
            }
            let text = String::from_utf8_lossy(&line).trim().to_string();
            line.clear();
            if text.is_empty() {
                continue;
            }
            on_progress(text.clone());
            tail.push(text);
            if tail.len() > 8 {
                tail.remove(0);
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        // git's own reason is on its fatal:/error: lines; the counters around them are noise
        let why: Vec<&str> = tail
            .iter()
            .map(String::as_str)
            .filter(|l| {
                l.starts_with("fatal") || l.starts_with("error") || l.starts_with("remote: ")
            })
            .collect();
        return Err(if why.is_empty() {
            tail.join("\n")
        } else {
            why.join("\n")
        });
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::super::{is_repo, testing};
    use super::*;

    #[test]
    fn project_dir_never_clobbers_seed_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("proj");
        create_project_dir(&p, Some("# A\n"), None).unwrap();
        create_project_dir(&p, Some("# B\n"), Some("target\n")).unwrap();
        assert_eq!(
            std::fs::read_to_string(p.join("README.md")).unwrap(),
            "# A\n"
        );
        assert_eq!(
            std::fs::read_to_string(p.join(".gitignore")).unwrap(),
            "target\n"
        );
    }

    #[test]
    fn clone_refuses_bad_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path();
        std::fs::create_dir(parent.join("taken")).unwrap();
        assert!(clone_dest("-oops", parent, "x", false).is_err());
        assert!(clone_dest("u", parent, "", false).is_err());
        assert!(clone_dest("u", parent, "a/b", false).is_err());
        assert!(clone_dest("u", parent, "taken", false).is_err());
        assert!(clone_dest("u", parent, "", true).is_err());
        assert_eq!(
            clone_dest("u", parent, "fresh", false).unwrap(),
            parent.join("fresh")
        );
    }

    #[test]
    fn clones_a_local_repo_and_reports_failures() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        testing::repo(&src);
        std::fs::write(src.join("a.txt"), "hi\n").unwrap();
        testing::commit_all(&src, "first");

        let out = dir.path().join("out");
        let url = src.to_string_lossy();
        let dest = clone(&url, &out, "copy", false, |_| {}).unwrap();
        assert!(dest.join("a.txt").exists());
        assert!(is_repo(&dest));

        let missing = dir.path().join("nope").to_string_lossy().to_string();
        assert!(clone(&missing, &out, "bad", false, |_| {}).is_err());
    }

    #[test]
    fn init_repo_names_the_branch_and_commits() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        testing::repo(cwd);
        let o = InitRepo {
            name: "Demo".into(),
            branch: "trunk".into(),
            readme: true,
            commit: true,
            ..Default::default()
        };
        assert_eq!(init_repo(cwd, &o).unwrap(), "Initialized the repository.");
        assert_eq!(
            std::fs::read_to_string(cwd.join("README.md")).unwrap(),
            "# Demo\n"
        );
        let head = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
        assert_eq!(head.trim(), "trunk");
    }
}
