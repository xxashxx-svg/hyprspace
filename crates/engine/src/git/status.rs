// Reading a working tree: what changed, the diff of one file, and the branch.

use std::collections::HashMap;
use std::path::Path;

use hyprspace_proto::git::{BranchInfo, FileChange};

use super::{Result, empty, git, git_cmd};

/// Whether `cwd` is inside a git work tree.
pub fn is_repo(cwd: &Path) -> bool {
    !empty(cwd) && git(cwd, &["rev-parse", "--is-inside-work-tree"]).is_ok()
}

/// Changed files in the repo containing `cwd`, with +/- line counts where git has them. Not a
/// repo means nothing to show, quietly.
pub fn changes(cwd: &Path) -> Result<Vec<FileChange>> {
    if !is_repo(cwd) {
        return Ok(vec![]);
    }

    // line counts from staged + unstaged numstat, summed per path
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    for args in [
        ["diff", "--numstat"].as_slice(),
        ["diff", "--numstat", "--cached"].as_slice(),
    ] {
        if let Ok(s) = git(cwd, args) {
            for (path, a, r) in numstat(&s) {
                let e = counts.entry(path).or_insert((0, 0));
                e.0 += a;
                e.1 += r;
            }
        }
    }

    let status = git(
        cwd,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    Ok(porcelain(&status)
        .into_iter()
        .map(|(status, path)| {
            let (added, removed) = counts.get(&path).copied().unwrap_or((0, 0));
            FileChange {
                path,
                status,
                added,
                removed,
            }
        })
        .collect())
}

// "<added>\t<removed>\t<path>" per line; binary files report "-" and count as 0
fn numstat(s: &str) -> Vec<(String, u32, u32)> {
    s.lines()
        .filter_map(|line| {
            let mut p = line.split('\t');
            let a = p.next()?.parse().unwrap_or(0);
            let r = p.next()?.parse().unwrap_or(0);
            Some((p.next()?.to_string(), a, r))
        })
        .collect()
}

// -z gives NUL-separated records with NO quoting or escaping, so odd paths survive intact. Each
// record is "XY <path>"; a rename or copy (R or C in either status column) is followed by an EXTRA
// NUL-terminated field holding the original path: take the new path and skip that field.
fn porcelain(s: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut parts = s.split('\0');
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue; // trailing empty field, or a malformed short record
        }
        let code = &entry[..2];
        let b = code.as_bytes();
        if b[0] == b'R' || b[0] == b'C' || b[1] == b'R' || b[1] == b'C' {
            let _ = parts.next();
        }
        out.push((code.to_string(), entry[3..].to_string()));
    }
    out
}

/// Unified diff for one file. Falls back through HEAD, then staged, then untracked (all added).
pub fn diff(cwd: &Path, path: &str) -> Result<String> {
    if empty(cwd) {
        return Ok(String::new());
    }
    let d = git(cwd, &["diff", "HEAD", "--", path]).unwrap_or_default();
    if !d.trim().is_empty() {
        return Ok(d);
    }
    let staged = git(cwd, &["diff", "--cached", "--", path]).unwrap_or_default();
    if !staged.trim().is_empty() {
        return Ok(staged);
    }
    // untracked file: diff against nothing so it shows as all-added. --no-index exits 1 when the
    // files differ, which is expected here, so read stdout regardless of status.
    let out = git_cmd()
        .arg("-C")
        .arg(cwd)
        .args(["diff", "--no-index", "--", "/dev/null", path])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The current branch and how far ahead or behind its upstream it is.
pub fn branch_info(cwd: &Path) -> BranchInfo {
    let mut bi = BranchInfo::default();
    if !is_repo(cwd) {
        return bi;
    }
    bi.is_repo = true;
    bi.branch = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string();
    // "<behind>\t<ahead>" relative to the upstream, if one is set
    if let Ok(s) = git(
        cwd,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    ) {
        let mut it = s.split_whitespace();
        bi.behind = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        bi.ahead = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        bi.upstream = true;
    }
    bi
}

#[cfg(test)]
mod tests {
    use super::super::testing;
    use super::*;

    #[test]
    fn porcelain_skips_the_rename_source() {
        let s = "R  new.txt\0old.txt\0 M a b.txt\0?? x\0";
        assert_eq!(
            porcelain(s),
            [
                ("R ".to_string(), "new.txt".to_string()),
                (" M".to_string(), "a b.txt".to_string()),
                ("??".to_string(), "x".to_string()),
            ]
        );
    }

    #[test]
    fn numstat_counts_binary_files_as_zero() {
        assert_eq!(
            numstat("3\t1\ta.rs\n-\t-\timg.png\n"),
            [("a.rs".to_string(), 3, 1), ("img.png".to_string(), 0, 0)]
        );
    }

    #[test]
    fn reads_changes_diff_and_branch_from_a_real_repo() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        assert!(!is_repo(cwd));
        assert_eq!(changes(cwd).unwrap(), []);

        testing::repo(cwd);
        std::fs::write(cwd.join("a.txt"), "one\n").unwrap();
        testing::commit_all(cwd, "first");
        std::fs::write(cwd.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(cwd.join("new.txt"), "hi\n").unwrap();

        let mut got = changes(cwd).unwrap();
        got.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(got.len(), 2);
        assert_eq!(
            (got[0].path.as_str(), got[0].status.as_str()),
            ("a.txt", " M")
        );
        assert_eq!((got[0].added, got[0].removed), (1, 0));
        assert_eq!(
            (got[1].path.as_str(), got[1].status.as_str()),
            ("new.txt", "??")
        );

        assert!(diff(cwd, "a.txt").unwrap().contains("+two"));
        assert!(diff(cwd, "new.txt").unwrap().contains("+hi"));

        let bi = branch_info(cwd);
        assert!(bi.is_repo);
        assert_eq!(bi.branch, "main");
        assert!(!bi.upstream);
    }
}
