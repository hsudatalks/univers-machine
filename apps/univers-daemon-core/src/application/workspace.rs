use crate::tmux::workspace::{WindowStatus, WorkspaceManager, WorkspaceStatus};
use anyhow::Result;
use std::sync::Arc;

pub struct WorkspaceApplicationService {
    workspace_manager: Arc<WorkspaceManager>,
}

impl WorkspaceApplicationService {
    pub fn new(workspace_manager: Arc<WorkspaceManager>) -> Self {
        Self { workspace_manager }
    }

    pub async fn list_workspaces(&self) -> Vec<WorkspaceStatus> {
        self.workspace_manager.list_workspaces().await
    }

    pub async fn list_windows(&self, workspace_id: &str) -> Result<Vec<WindowStatus>> {
        self.workspace_manager.list_windows(workspace_id).await
    }

    pub async fn start_workspace(&self, workspace_id: &str) -> Result<()> {
        self.workspace_manager.start_workspace(workspace_id).await
    }

    pub async fn stop_workspace(&self, workspace_id: &str) -> Result<()> {
        self.workspace_manager.stop_workspace(workspace_id).await
    }

    pub async fn restart_workspace(&self, workspace_id: &str) -> Result<()> {
        self.workspace_manager.restart_workspace(workspace_id).await
    }

    pub async fn capture_workspace_logs(&self, workspace_id: &str) -> Result<String> {
        self.workspace_manager
            .capture_workspace_logs(workspace_id)
            .await
    }

    pub async fn start_window(&self, workspace_id: &str, window_id: &str) -> Result<()> {
        self.workspace_manager
            .start_window(workspace_id, window_id)
            .await
    }

    pub async fn stop_window(&self, workspace_id: &str, window_id: &str) -> Result<()> {
        self.workspace_manager
            .stop_window(workspace_id, window_id)
            .await
    }

    pub async fn restart_window(&self, workspace_id: &str, window_id: &str) -> Result<()> {
        self.workspace_manager
            .restart_window(workspace_id, window_id)
            .await
    }

    pub async fn capture_window_logs(&self, workspace_id: &str, window_id: &str) -> Result<String> {
        self.workspace_manager
            .capture_window_logs(workspace_id, window_id)
            .await
    }
}
