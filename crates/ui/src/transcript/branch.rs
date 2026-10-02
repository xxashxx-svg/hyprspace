// The git branch a thread's folder is on, for the line under its composer. It reads `.git/HEAD`
// straight off disk: one tiny file, read when the session starts and after each run, so the
// line needs no round trip to the engine. A worktree's `.git` is a file that points at its
// real git folder.

use std::path::Path;

/// The branch checked out in `cwd` or the repository above it, or a short commit when HEAD is
/// detached. None outside a repository.
pub fn of(cwd: &Path) -> Option<String> {
    let dot = cwd
        .ancestors()
        .map(|d| d.join(".git"))
        .find(|g| g.exists())?;
    let dir = if dot.is_file() {
        let link = std::fs::read_to_string(&dot).ok()?;
        let target = link.trim().strip_prefix("gitdir:")?.trim().to_string();
        dot.parent()?.join(target)
    } else {
        dot
    };
    parse(&std::fs::read_to_string(dir.join("HEAD")).ok()?)
}

fn parse(head: &str) -> Option<String> {
    let head = head.trim();
    match head.strip_prefix("ref:") {
        Some(r) => Some(r.trim().trim_start_matches("refs/heads/").to_string()),
        None => (head.len() >= 7).then(|| head[..7].to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_branch_or_a_detached_commit() {
        assert_eq!(parse("ref: refs/heads/main\n").as_deref(), Some("main"));
        assert_eq!(parse("ref: refs/heads/feat/x").as_deref(), Some("feat/x"));
        assert_eq!(parse("6b8dadf0123456789").as_deref(), Some("6b8dadf"));
        assert_eq!(parse(""), None);
    }

    #[test]
    fn follows_a_worktree_link() {
        let root = std::env::temp_dir().join(format!("hs-branch-{}", std::process::id()));
        let real = root.join("real");
        let tree = root.join("tree");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::create_dir_all(tree.join("sub")).unwrap();
        std::fs::write(real.join("HEAD"), "ref: refs/heads/rewrite-ui\n").unwrap();
        std::fs::write(tree.join(".git"), format!("gitdir: {}\n", real.display())).unwrap();
        assert_eq!(of(&tree.join("sub")).as_deref(), Some("rewrite-ui"));
        let _ = std::fs::remove_dir_all(root);
    }
}
