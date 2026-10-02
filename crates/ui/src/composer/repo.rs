// Spotting a git repository link at the start of the composer's text, carried over from the
// Tauri app's src/lib/repoUrl.ts: hosted links need owner/name, anything else needs `.git`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub url: String,
    /// "owner/name" for hosted repos, else the repo name.
    pub label: String,
    /// The folder name a clone gets.
    pub name: String,
}

const HOSTS: &[&str] = &[
    "github.com",
    "gitlab.com",
    "bitbucket.org",
    "codeberg.org",
    "gitea.com",
    "sr.ht",
];

/// One token as a repository link.
pub fn parse(token: &str) -> Option<RepoRef> {
    let t = token.trim();
    let lower = t.to_ascii_lowercase();
    let (host, path) = if let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    {
        let rest = rest.strip_prefix("www.").unwrap_or(rest);
        rest.split_once('/')?
    } else {
        let rest = lower.strip_prefix("ssh://").unwrap_or(&lower);
        let rest = rest.strip_prefix("git@")?;
        rest.split_once([':', '/'])?
    };
    if host.is_empty() || t.contains(char::is_whitespace) || path.contains(['?', '#']) {
        return None;
    }
    // keep the original casing for the name, matching the lowered path's position
    let path_start = t.len() - path.len();
    let path = t[path_start..].trim_end_matches('/');
    let git = path.to_ascii_lowercase().ends_with(".git");
    let path = if git { &path[..path.len() - 4] } else { path };
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let hosted = HOSTS.contains(&host);
    if (hosted && parts.len() != 2) || (!hosted && !git) {
        return None;
    }
    let name = parts.last()?.to_string();
    let label = if hosted {
        format!("{}/{name}", parts[0])
    } else {
        name.clone()
    };
    Some(RepoRef {
        url: t.to_string(),
        label,
        name,
    })
}

/// The repository link the text starts with, and the task after it.
pub fn split(text: &str) -> Option<(RepoRef, String)> {
    let trimmed = text.trim();
    let first = trimmed.split_whitespace().next()?;
    let repo = parse(first)?;
    Some((repo, trimmed[first.len()..].trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_links_need_owner_and_name() {
        let r = parse("https://github.com/zeronsh/Zeron").unwrap();
        assert_eq!(
            (r.label.as_str(), r.name.as_str()),
            ("zeronsh/Zeron", "Zeron")
        );
        assert!(parse("https://github.com/zeronsh").is_none());
        assert!(parse("https://github.com/a/b/tree/main").is_none());
        assert_eq!(parse("git@github.com:a/b.git").unwrap().name, "b");
        assert_eq!(parse("https://www.gitlab.com/a/b/").unwrap().label, "a/b");
    }

    #[test]
    fn other_hosts_need_dot_git() {
        assert_eq!(
            parse("https://git.example.com/x/y/repo.git").unwrap().name,
            "repo"
        );
        assert!(parse("https://example.com/x/y").is_none());
        assert!(parse("not a link").is_none());
        assert!(parse("https://github.com/a/b?tab=readme").is_none());
    }

    #[test]
    fn splits_the_task_off() {
        let (r, rest) = split("  https://github.com/a/b  fix the build ").unwrap();
        assert_eq!(r.url, "https://github.com/a/b");
        assert_eq!(rest, "fix the build");
        assert!(split("fix https://github.com/a/b").is_none());
    }
}
