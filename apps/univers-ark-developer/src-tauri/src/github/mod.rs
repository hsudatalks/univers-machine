mod cli;
mod local_repo;

use crate::models::{
    GithubProjectState, GithubPullRequestDetail, GithubPullRequestFile, GithubPullRequestReview,
    GithubPullRequestSummary, GithubRepositoryStatus, GithubStatusCheck,
};
use std::process::Command;

use self::{
    cli::{
        merge_pull_request, pr_list, pull_request_detail, repo_view, viewer_login, GhPullRequest,
        REPO_FULL_NAME,
    },
    local_repo::{hvac_workbench_repo_path, local_branch, local_status_summary},
};

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
        &format!("author:{viewer_login}"),
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
    let detail = pull_request_detail(number)?;

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
    merge_pull_request(number, method)
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
        .map_err(|error| format!("Failed to open {url}: {error}"))?;

    Ok(())
}
