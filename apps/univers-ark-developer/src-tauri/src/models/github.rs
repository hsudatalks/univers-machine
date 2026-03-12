use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequestSummary {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) author_login: String,
    pub(crate) head_ref_name: String,
    pub(crate) is_draft: bool,
    pub(crate) state: String,
    pub(crate) review_decision: Option<String>,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubRepositoryStatus {
    pub(crate) name_with_owner: String,
    pub(crate) description: String,
    pub(crate) url: String,
    pub(crate) default_branch: String,
    pub(crate) viewer_login: String,
    pub(crate) local_repo_path: Option<String>,
    pub(crate) local_branch: Option<String>,
    pub(crate) local_status_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubProjectState {
    pub(crate) repository: GithubRepositoryStatus,
    pub(crate) current_branch_pr: Option<GithubPullRequestSummary>,
    pub(crate) my_open_prs: Vec<GithubPullRequestSummary>,
    pub(crate) open_prs: Vec<GithubPullRequestSummary>,
    pub(crate) closed_prs: Vec<GithubPullRequestSummary>,
    pub(crate) merged_prs: Vec<GithubPullRequestSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequestFile {
    pub(crate) path: String,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequestReview {
    pub(crate) author_login: String,
    pub(crate) state: String,
    pub(crate) body: String,
    pub(crate) submitted_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubStatusCheck {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) conclusion: Option<String>,
    pub(crate) workflow_name: Option<String>,
    pub(crate) details_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequestDetail {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) url: String,
    pub(crate) author_login: String,
    pub(crate) head_ref_name: String,
    pub(crate) base_ref_name: String,
    pub(crate) is_draft: bool,
    pub(crate) state: String,
    pub(crate) review_decision: Option<String>,
    pub(crate) updated_at: String,
    pub(crate) merge_state_status: String,
    pub(crate) mergeable: String,
    pub(crate) changed_files: u64,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
    pub(crate) files: Vec<GithubPullRequestFile>,
    pub(crate) latest_reviews: Vec<GithubPullRequestReview>,
    pub(crate) status_checks: Vec<GithubStatusCheck>,
}
