// A repo's recent pull requests through the user's own `gh`, for each thread's branch to show its
// pull request and its checks.

use std::path::Path;

use hyprspace_proto::git::{Checks, PrState, PullRequest};
use serde_json::Value;

/// The newest first. Empty when gh is missing, signed out, or the repo isn't on GitHub.
pub fn pull_requests(cwd: &Path) -> Vec<PullRequest> {
    let fields = "number,title,state,isDraft,headRefName,url,statusCheckRollup";
    let args = [
        "pr", "list", "--state", "all", "--limit", "100", "--json", fields,
    ];
    let Ok(out) = super::gh(cwd).args(args).output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    serde_json::from_slice::<Value>(&out.stdout)
        .ok()
        .and_then(|v| {
            v.as_array()
                .map(|all| all.iter().filter_map(parse).collect())
        })
        .unwrap_or_default()
}

fn parse(p: &Value) -> Option<PullRequest> {
    let state = match (p["state"].as_str()?, p["isDraft"].as_bool() == Some(true)) {
        ("MERGED", _) => PrState::Merged,
        ("CLOSED", _) => PrState::Closed,
        (_, true) => PrState::Draft,
        _ => PrState::Open,
    };
    Some(PullRequest {
        number: p["number"].as_u64()?,
        title: p["title"].as_str().unwrap_or_default().to_string(),
        branch: p["headRefName"].as_str()?.to_string(),
        url: p["url"].as_str()?.to_string(),
        state,
        checks: checks(&p["statusCheckRollup"]),
    })
}

/// Check runs report a status and then a conclusion; older status contexts report one state.
fn checks(rollup: &Value) -> Checks {
    let all = rollup.as_array().map(Vec::as_slice).unwrap_or_default();
    let said = |c: &Value| {
        c["conclusion"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or(c["state"].as_str())
            .unwrap_or_default()
            .to_uppercase()
    };
    if all.is_empty() {
        Checks::None
    } else if all.iter().any(|c| {
        matches!(
            said(c).as_str(),
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE"
        )
    }) {
        Checks::Failing
    } else if all
        .iter()
        .any(|c| c["status"].as_str().is_some_and(|s| s != "COMPLETED") || said(c) == "PENDING")
    {
        Checks::Pending
    } else {
        Checks::Passing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_pull_request_reads_its_state_and_what_its_checks_add_up_to() {
        let pr = |state, draft, rollup| {
            parse(
                &json!({ "number": 7, "title": "Fix it", "state": state, "isDraft": draft,
                "headRefName": "hs/fix", "url": "https://github.com/a/b/pull/7",
                "statusCheckRollup": rollup }),
            )
            .unwrap()
        };
        let done = json!({ "status": "COMPLETED", "conclusion": "SUCCESS" });
        let failed = json!({ "status": "COMPLETED", "conclusion": "FAILURE" });
        let running = json!({ "status": "IN_PROGRESS", "conclusion": "" });
        let old = json!({ "state": "PENDING" });

        let open = pr("OPEN", false, json!([done]));
        assert_eq!((open.state, open.checks), (PrState::Open, Checks::Passing));
        assert_eq!(pr("OPEN", true, json!([])).state, PrState::Draft);
        assert_eq!(pr("MERGED", false, json!([])).state, PrState::Merged);
        assert_eq!(
            pr("OPEN", false, json!([done, failed])).checks,
            Checks::Failing
        );
        assert_eq!(
            pr("OPEN", false, json!([done, running])).checks,
            Checks::Pending
        );
        assert_eq!(pr("OPEN", false, json!([old])).checks, Checks::Pending);
        assert_eq!(pr("OPEN", false, json!(null)).checks, Checks::None);
    }
}
