// The changelog, built into the app so What's new needs no network. deploy.ps1 writes the
// version's section before the release is built, so the shipped app always carries its own notes
// (the Tauri app's src/lib/changelog.ts does the same).

const CHANGELOG: &str = include_str!("../../../../docs/CHANGELOG.md");

/// The bullets under `## <version>` (e.g. `## 0.21.1 — 2026-09-28`), up to the next heading.
pub fn notes(version: &str) -> Vec<String> {
    notes_in(CHANGELOG, version)
}

fn notes_in(log: &str, version: &str) -> Vec<String> {
    let version = version.trim().trim_start_matches('v');
    let is_head = |line: &str| {
        let Some(rest) = line.trim().strip_prefix("## ") else {
            return false;
        };
        let rest = rest.trim_start_matches('v');
        rest.strip_prefix(version).is_some_and(|after| {
            after.is_empty() || !after.starts_with(|c: char| c == '.' || c.is_ascii_digit())
        })
    };
    let mut lines = log.lines().skip_while(|l| !is_head(l));
    if lines.next().is_none() {
        return Vec::new();
    }
    lines
        .take_while(|l| !l.starts_with("## "))
        .filter_map(|l| {
            let item = l.trim().trim_start_matches(['-', '*']).trim();
            (!item.is_empty()).then(|| item.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "# Changelog\n\nIntro.\n\n## 0.21.10 — 2026-10-01\n\n- Ten\n\n## 0.21.1 — 2026-09-28\n\n- One\n* Two\n\n## 0.21.0 — 2026-09-24\n\n- Zero\n";

    #[test]
    fn reads_one_versions_bullets() {
        assert_eq!(notes_in(LOG, "0.21.1"), vec!["One", "Two"]);
        assert_eq!(notes_in(LOG, "v0.21.10"), vec!["Ten"]);
        assert!(notes_in(LOG, "9.9.9").is_empty());
    }

    #[test]
    fn the_bundled_changelog_has_the_last_tauri_release() {
        assert!(!notes("0.21.1").is_empty());
    }
}
