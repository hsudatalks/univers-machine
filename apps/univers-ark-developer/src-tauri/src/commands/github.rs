use crate::github::{
    load_github_project_state as read_github_project_state,
    load_github_pull_request_detail as read_github_pull_request_detail,
    merge_github_pull_request as execute_github_pull_request_merge, open_external_url,
};
use crate::models::{GithubProjectState, GithubPullRequestDetail};
use tauri::async_runtime;

#[tauri::command]
pub(crate) async fn load_github_project_state() -> Result<GithubProjectState, String> {
    async_runtime::spawn_blocking(read_github_project_state)
        .await
        .map_err(|error| format!("Failed to join GitHub project state task: {error}"))?
}

#[tauri::command]
pub(crate) async fn open_external_link(url: String) -> Result<(), String> {
    async_runtime::spawn_blocking(move || open_external_url(&url))
        .await
        .map_err(|error| format!("Failed to join external link task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_github_pull_request_detail(
    number: u64,
) -> Result<GithubPullRequestDetail, String> {
    async_runtime::spawn_blocking(move || read_github_pull_request_detail(number))
        .await
        .map_err(|error| format!("Failed to join pull request detail task: {error}"))?
}

#[tauri::command]
pub(crate) async fn merge_github_pull_request(number: u64, method: String) -> Result<(), String> {
    async_runtime::spawn_blocking(move || execute_github_pull_request_merge(number, &method))
        .await
        .map_err(|error| format!("Failed to join pull request merge task: {error}"))?
}
