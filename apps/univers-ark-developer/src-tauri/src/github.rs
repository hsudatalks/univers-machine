use crate::models::{
    GithubProjectState, GithubPullRequestDetail, GithubPullRequestFile, GithubPullRequestReview,
    GithubPullRequestSummary, GithubRepositoryStatus, GithubStatusCheck,
};
use serde::Deserialize;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

const REPO_NAME: &str = "hvac-workbench";
const REPO_FULL_NAME: &str = "hsudatalks/hvac-workbench";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhUser {
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhDefaultBranchRef {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhRepoView {
    name_with_owner: String,
    description: Option<String>,
    url: String,
    default_branch_ref: GhDefaultBranchRef,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequest {
    number: u64,
    title: String,
    url: String,
    author: GhUser,
    head_ref_name: String,
    is_draft: bool,
    state: String,
    review_decision: Option<String>,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequestFile {
    path: String,
    additions: u64,
    deletions: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewAuthor {
    login: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequestReview {
    author: GhReviewAuthor,
    state: String,
    body: String,
    submitted_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequestDetailJson {
    number: u64,
    title: String,
    body: String,
    url: String,
    author: GhUser,
    head_ref_name: String,
    base_ref_name: String,
    is_draft: bool,
    state: String,
    review_decision: Option<String>,
    updated_at: String,
    merge_state_status: String,
    mergeable: String,
    changed_files: u64,
    additions: u64,
    deletions: u64,
    files: Vec<GhPullRequestFile>,
    latest_reviews: Vec<GhPullRequestReview>,
    status_check_rollup: Vec<GhStatusCheck>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhStatusCheck {
    #[serde(rename = "__typename")]
    typename: String,
    name: String,
    status: String,
    conclusion: Option<String>,
    workflow_name: Option<String>,
    details_url: Option<String>,
}

fn resolve_gh_path() -> String {
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

fn run_gh(args: &[&str], current_dir: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut command = Command::new(resolve_gh_path());
    command.args(args);

    if let Some(path) = current_dir {
        command.current_dir(path);
    }

    let output = command
        .output()
        .map_err(|error| format!("Failed to launch gh: {}", error))?;

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

fn parse_gh_json<T: for<'de> Deserialize<'de>>(bytes: &[u8], context: &str) -> Result<T, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("Failed to parse {} from gh output: {}", context, error))
}

fn viewer_login() -> Result<String, String> {
    let output = run_gh(&["api", "user"], None)?;
    let user: GhUser = parse_gh_json(&output, "GitHub viewer")?;
    Ok(user.login)
}

fn repo_view() -> Result<GhRepoView, String> {
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

fn pr_list(args: &[&str]) -> Result<Vec<GhPullRequest>, String> {
    let output = run_gh(args, None)?;
    parse_gh_json(&output, "pull request list")
}

fn summarize_pr(pr: GhPullRequest) -> GithubPullRequestSummary {
    GithubPullRequestSummary {
        number: pr.number,
        title: pr.title,
        url: pr.url,
        author_login: pr.author.login,
        head_ref_name: pr.head_ref_name,
        is_draft: pr.is_draft,
        state: pr.state,
        review_decision: pr.review_decision,
        updated_at: pr.updated_at,
    }
}

fn hvac_workbench_repo_path() -> Option<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from)?;
    let candidate = home.join("repos").join(REPO_NAME);
    candidate.exists().then_some(candidate)
}

fn local_branch(repo_path: &Path) -> Result<Option<String>, String> {
    let output = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(repo_path)
        .arg("branch")
        .arg("--show-current")
        .output()
        .map_err(|error| format!("Failed to inspect {} branch: {}", REPO_NAME, error))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("Failed to inspect {} branch", REPO_NAME)
        } else {
            stderr
        });
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!branch.is_empty()).then_some(branch))
}

fn local_status_summary(repo_path: &Path) -> Result<Option<String>, String> {
    let output = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(repo_path)
        .arg("status")
        .arg("--short")
        .arg("--branch")
        .output()
        .map_err(|error| format!("Failed to inspect {} status: {}", REPO_NAME, error))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("Failed to inspect {} status", REPO_NAME)
        } else {
            stderr
        });
    }

    let summary = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from);

    Ok(summary)
}

fn current_branch_pr(branch: Option<&str>) -> Result<Option<GithubPullRequestSummary>, String> {
    let Some(branch_name) = branch else {
        return Ok(None);
    };

    let prs = pr_list(&[
        "pr",
        "list",
        "--repo",
        REPO_FULL_NAME,
        "--head",
        branch_name,
        "--state",
        "all",
        "--limit",
        "1",
        "--json",
        "number,title,url,author,headRefName,isDraft,updatedAt,reviewDecision,state",
    ])?;

    Ok(prs.into_iter().next().map(summarize_pr))
}

pub(crate) fn load_github_project_state() -> Result<GithubProjectState, String> {
    let viewer_login = viewer_login()?;
    let repo = repo_view()?;
    let local_repo_path = hvac_workbench_repo_path();
    let local_branch = if let Some(path) = local_repo_path.as_deref() {
        local_branch(path)?
    } else {
        None
    };
    let local_status_summary = if let Some(path) = local_repo_path.as_deref() {
        local_status_summary(path)?
    } else {
        None
    };

    let current_branch_pr = current_branch_pr(local_branch.as_deref())?;

    let my_open_prs = pr_list(&[
        "pr",
        "list",
        "--repo",
        REPO_FULL_NAME,
        "--state",
        "open",
        "--search",
        &format!("author:{}", viewer_login),
        "--limit",
        "8",
        "--json",
        "number,title,url,author,headRefName,isDraft,updatedAt,reviewDecision,state",
    ])?
    .into_iter()
    .map(summarize_pr)
    .collect();

    let open_prs = pr_list(&[
        "pr",
        "list",
        "--repo",
        REPO_FULL_NAME,
        "--state",
        "open",
        "--limit",
        "8",
        "--json",
        "number,title,url,author,headRefName,isDraft,updatedAt,reviewDecision,state",
    ])?
    .into_iter()
    .map(summarize_pr)
    .collect();

    let closed_prs = pr_list(&[
        "pr",
        "list",
        "--repo",
        REPO_FULL_NAME,
        "--state",
        "closed",
        "--limit",
        "8",
        "--json",
        "number,title,url,author,headRefName,isDraft,updatedAt,reviewDecision,state",
    ])?
    .into_iter()
    .map(summarize_pr)
    .collect();

    let merged_prs = pr_list(&[
        "pr",
        "list",
        "--repo",
        REPO_FULL_NAME,
        "--state",
        "merged",
        "--limit",
        "8",
        "--json",
        "number,title,url,author,headRefName,isDraft,updatedAt,reviewDecision,state",
    ])?
    .into_iter()
    .map(summarize_pr)
    .collect();

    Ok(GithubProjectState {
        repository: GithubRepositoryStatus {
            name_with_owner: repo.name_with_owner,
            description: repo.description.unwrap_or_default(),
            url: repo.url,
            default_branch: repo.default_branch_ref.name,
            viewer_login,
            local_repo_path: local_repo_path.map(|path| path.display().to_string()),
            local_branch,
            local_status_summary,
        },
        current_branch_pr,
        my_open_prs,
        open_prs,
        closed_prs,
        merged_prs,
    })
}

pub(crate) fn load_github_pull_request_detail(
    number: u64,
) -> Result<GithubPullRequestDetail, String> {
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

    let detail: GhPullRequestDetailJson = parse_gh_json(&output, "pull request detail")?;

    Ok(GithubPullRequestDetail {
        number: detail.number,
        title: detail.title,
        body: detail.body,
        url: detail.url,
        author_login: detail.author.login,
        head_ref_name: detail.head_ref_name,
        base_ref_name: detail.base_ref_name,
        is_draft: detail.is_draft,
        state: detail.state,
        review_decision: detail.review_decision,
        updated_at: detail.updated_at,
        merge_state_status: detail.merge_state_status,
        mergeable: detail.mergeable,
        changed_files: detail.changed_files,
        additions: detail.additions,
        deletions: detail.deletions,
        files: detail
            .files
            .into_iter()
            .map(|file| GithubPullRequestFile {
                path: file.path,
                additions: file.additions,
                deletions: file.deletions,
            })
            .collect(),
        latest_reviews: detail
            .latest_reviews
            .into_iter()
            .map(|review| GithubPullRequestReview {
                author_login: review.author.login,
                state: review.state,
                body: review.body,
                submitted_at: review.submitted_at,
            })
            .collect(),
        status_checks: detail
            .status_check_rollup
            .into_iter()
            .map(|check| GithubStatusCheck {
                kind: check.typename,
                name: check.name,
                status: check.status,
                conclusion: check.conclusion,
                workflow_name: check.workflow_name,
                details_url: check.details_url.unwrap_or_default(),
            })
            .collect(),
    })
}

pub(crate) fn merge_github_pull_request(number: u64, method: &str) -> Result<(), String> {
    let strategy_flag = match method {
        "merge" => "--merge",
        "squash" => "--squash",
        "rebase" => "--rebase",
        _ => return Err(format!("Unsupported merge method: {}", method)),
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

pub(crate) fn open_external_url(url: &str) -> Result<(), String> {
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("/usr/bin/open", &[url])
    } else if cfg!(target_os = "windows") {
        ("cmd", &["/C", "start", "", url])
    } else {
        ("xdg-open", &[url])
    };

    Command::new(program)
        .args(args)
        .spawn()
        .map_err(|error| format!("Failed to open {}: {}", url, error))?;

    Ok(())
}
