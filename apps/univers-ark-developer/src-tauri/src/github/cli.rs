use serde::Deserialize;
use std::{path::Path, process::Command};

pub(super) const REPO_FULL_NAME: &str = "hsudatalks/hvac-workbench";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhUser {
    pub(super) login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhDefaultBranchRef {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhRepoView {
    pub(super) name_with_owner: String,
    pub(super) description: Option<String>,
    pub(super) url: String,
    pub(super) default_branch_ref: GhDefaultBranchRef,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhPullRequest {
    pub(super) number: u64,
    pub(super) title: String,
    pub(super) url: String,
    pub(super) author: GhUser,
    pub(super) head_ref_name: String,
    pub(super) is_draft: bool,
    pub(super) state: String,
    pub(super) review_decision: Option<String>,
    pub(super) updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhPullRequestFile {
    pub(super) path: String,
    pub(super) additions: u64,
    pub(super) deletions: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhReviewAuthor {
    pub(super) login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhPullRequestReview {
    pub(super) author: GhReviewAuthor,
    pub(super) state: String,
    pub(super) body: String,
    pub(super) submitted_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhStatusCheck {
    #[serde(rename = "__typename")]
    pub(super) typename: String,
    pub(super) name: String,
    pub(super) status: String,
    pub(super) conclusion: Option<String>,
    pub(super) workflow_name: Option<String>,
    pub(super) details_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GhPullRequestDetailJson {
    pub(super) number: u64,
    pub(super) title: String,
    pub(super) body: String,
    pub(super) url: String,
    pub(super) author: GhUser,
    pub(super) head_ref_name: String,
    pub(super) base_ref_name: String,
    pub(super) is_draft: bool,
    pub(super) state: String,
    pub(super) review_decision: Option<String>,
    pub(super) updated_at: String,
    pub(super) merge_state_status: String,
    pub(super) mergeable: String,
    pub(super) changed_files: u64,
    pub(super) additions: u64,
    pub(super) deletions: u64,
    pub(super) files: Vec<GhPullRequestFile>,
    pub(super) latest_reviews: Vec<GhPullRequestReview>,
    pub(super) status_check_rollup: Vec<GhStatusCheck>,
}

fn resolve_gh_path() -> String {
    if cfg!(target_os = "windows") {
        return String::from("gh");
    }

    for candidate in [
        "/opt/homebrew/bin/gh",
        "/usr/local/bin/gh",
        "/usr/bin/gh",
        "gh",
    ] {
        if candidate == "gh" || Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }

    String::from("gh")
}

pub(super) fn run_gh(args: &[&str], current_dir: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut command = Command::new(resolve_gh_path());
    command.args(args);

    if let Some(path) = current_dir {
        command.current_dir(path);
    }

    let output = command
        .output()
        .map_err(|error| format!("Failed to launch gh: {error}"))?;

    if output.status.success() {
        return Ok(output.stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("gh exited with status {}", output.status)
    };

    Err(detail)
}

pub(super) fn parse_gh_json<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    context: &str,
) -> Result<T, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("Failed to parse {context} from gh output: {error}"))
}

pub(super) fn viewer_login() -> Result<String, String> {
    let output = run_gh(&["api", "user"], None)?;
    let user: GhUser = parse_gh_json(&output, "GitHub viewer")?;
    Ok(user.login)
}

pub(super) fn repo_view() -> Result<GhRepoView, String> {
    let output = run_gh(
        &[
            "repo",
            "view",
            REPO_FULL_NAME,
            "--json",
            "nameWithOwner,description,url,defaultBranchRef",
        ],
        None,
    )?;

    parse_gh_json(&output, "repository view")
}

pub(super) fn pr_list(args: &[&str]) -> Result<Vec<GhPullRequest>, String> {
    let output = run_gh(args, None)?;
    parse_gh_json(&output, "pull request list")
}

pub(super) fn pull_request_detail(number: u64) -> Result<GhPullRequestDetailJson, String> {
    let output = run_gh(
        &[
            "pr",
            "view",
            &number.to_string(),
            "--repo",
            REPO_FULL_NAME,
            "--json",
            "number,title,body,url,author,headRefName,baseRefName,isDraft,state,reviewDecision,updatedAt,mergeStateStatus,mergeable,changedFiles,additions,deletions,files,latestReviews,statusCheckRollup",
        ],
        None,
    )?;

    parse_gh_json(&output, "pull request detail")
}

pub(super) fn merge_pull_request(number: u64, method: &str) -> Result<(), String> {
    let strategy_flag = match method {
        "merge" => "--merge",
        "squash" => "--squash",
        "rebase" => "--rebase",
        _ => return Err(format!("Unsupported merge method: {method}")),
    };

    let _ = run_gh(
        &[
            "pr",
            "merge",
            &number.to_string(),
            "--repo",
            REPO_FULL_NAME,
            strategy_flag,
        ],
        None,
    )?;

    Ok(())
}
