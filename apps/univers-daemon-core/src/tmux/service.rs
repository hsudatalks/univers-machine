use crate::tmux::spec::{
    DefaultWorkspaceSpecRepository, WindowDefinition, WorkspaceDefinition, WorkspaceProfile,
    WorkspaceSpecRepository,
};
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use univers_infra_tmux::TmuxGateway;
pub use univers_ark_kernel::workspace::{WindowStatus, WorkspaceStatus};

#[derive(Clone)]
pub struct WorkspaceManager {
    profile: WorkspaceProfile,
    gateway: TmuxGateway,
    repository: Arc<dyn WorkspaceSpecRepository>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self::for_machine()
    }

    pub fn for_machine() -> Self {
        Self::with_repository(
            WorkspaceProfile::Machine,
            Arc::new(DefaultWorkspaceSpecRepository::new()),
        )
    }

    pub fn for_container() -> Self {
        Self::with_repository(
            WorkspaceProfile::Container,
            Arc::new(DefaultWorkspaceSpecRepository::new()),
        )
    }

    pub(crate) fn with_repository(
        profile: WorkspaceProfile,
        repository: Arc<dyn WorkspaceSpecRepository>,
    ) -> Self {
        Self {
            profile,
            gateway: TmuxGateway,
            repository,
        }
    }

    pub async fn list_workspaces(&self) -> Vec<WorkspaceStatus> {
        self.repository
            .list(self.profile)
            .into_iter()
            .map(|workspace| self.build_workspace_status(workspace))
            .collect()
    }

    pub async fn list_windows(&self, workspace_id: &str) -> Result<Vec<WindowStatus>> {
        let workspace = self.find_workspace(workspace_id)?;
        Ok(self.build_window_statuses(&workspace))
    }

    pub async fn start_workspace(&self, workspace_id: &str) -> Result<()> {
        let workspace = self.find_workspace(workspace_id)?;
        self.ensure_workspace(&workspace)
    }

    pub async fn stop_workspace(&self, workspace_id: &str) -> Result<()> {
        let workspace = self.find_workspace(workspace_id)?;
        self.stop_workspace_impl(&workspace)
    }

    pub async fn restart_workspace(&self, workspace_id: &str) -> Result<()> {
        let workspace = self.find_workspace(workspace_id)?;
        self.stop_workspace_impl(&workspace)?;
        self.ensure_workspace(&workspace)
    }

    pub async fn capture_workspace_logs(&self, workspace_id: &str) -> Result<String> {
        let workspace = self.find_workspace(workspace_id)?;
        let target_window = workspace.windows.first().map(|window| window.id.as_str());
        self.gateway.capture_logs(
            workspace.tmux_server.as_deref(),
            &workspace.id,
            target_window,
        )
    }

    pub async fn start_window(&self, workspace_id: &str, window_id: &str) -> Result<()> {
        let workspace = self.find_workspace(workspace_id)?;
        self.ensure_workspace(&workspace)?;
        let window = find_window(&workspace, window_id)?;
        self.ensure_window(&workspace, window)
    }

    pub async fn stop_window(&self, workspace_id: &str, window_id: &str) -> Result<()> {
        let workspace = self.find_workspace(workspace_id)?;
        let window = find_window(&workspace, window_id)?;
        self.stop_window_impl(&workspace, window)
    }

    pub async fn restart_window(&self, workspace_id: &str, window_id: &str) -> Result<()> {
        let workspace = self.find_workspace(workspace_id)?;
        let window = find_window(&workspace, window_id)?;
        self.stop_window_impl(&workspace, window)?;
        self.ensure_workspace(&workspace)?;
        self.ensure_window(&workspace, window)
    }

    pub async fn capture_window_logs(&self, workspace_id: &str, window_id: &str) -> Result<String> {
        let workspace = self.find_workspace(workspace_id)?;
        let window = find_window(&workspace, window_id)?;
        self.gateway.capture_logs(
            workspace.tmux_server.as_deref(),
            &workspace.id,
            Some(window.id.as_str()),
        )
    }

    fn find_workspace(&self, workspace_id: &str) -> Result<WorkspaceDefinition> {
        self.repository
            .list(self.profile)
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .ok_or_else(|| anyhow!("Unknown workspace '{workspace_id}'"))
    }

    fn build_workspace_status(&self, workspace: WorkspaceDefinition) -> WorkspaceStatus {
        let running = self
            .gateway
            .session_exists(workspace.tmux_server.as_deref(), &workspace.id);
        let windows = self.build_window_statuses(&workspace);
        let healthy = running && windows.iter().all(|window| window.running);
        let attached = if running {
            self.gateway
                .session_attached(workspace.tmux_server.as_deref(), &workspace.id)
        } else {
            false
        };
        let active_command = if running {
            self.gateway
                .session_active_command(workspace.tmux_server.as_deref(), &workspace.id)
        } else {
            None
        };

        WorkspaceStatus {
            id: workspace.id,
            title: workspace.title,
            category: workspace.category,
            source: workspace.source,
            tmux_server: workspace.tmux_server,
            working_directory: workspace.working_directory.display().to_string(),
            running,
            healthy,
            attached,
            active_command,
            windows,
        }
    }

    fn build_window_statuses(&self, workspace: &WorkspaceDefinition) -> Vec<WindowStatus> {
        workspace
            .windows
            .iter()
            .map(|window| WindowStatus {
                id: window.id.clone(),
                title: window.title.clone(),
                kind: window.kind.clone(),
                agent_id: window.agent_id.clone(),
                app_id: window.app_id.clone(),
                skills: window.skills.clone(),
                running: self.gateway.window_exists(
                    workspace.tmux_server.as_deref(),
                    &workspace.id,
                    &window.id,
                ),
            })
            .collect()
    }

    fn ensure_workspace(&self, workspace: &WorkspaceDefinition) -> Result<()> {
        if workspace.windows.is_empty() {
            return Err(anyhow!("Workspace '{}' has no windows", workspace.id));
        }

        if !self
            .gateway
            .session_exists(workspace.tmux_server.as_deref(), &workspace.id)
        {
            let first_window = &workspace.windows[0];
            let output = self
                .gateway
                .new_session(
                    workspace.tmux_server.as_deref(),
                    &workspace.id,
                    &first_window.id,
                    &workspace.working_directory,
                    first_window.command.as_deref(),
                )
                .with_context(|| format!("Failed to create workspace '{}'", workspace.id))?;
            if !output.status.success() {
                return Err(anyhow!(stderr_or_default(
                    &output,
                    &format!("tmux new-session failed for '{}'", workspace.id),
                )));
            }
        }

        for window in &workspace.windows {
            self.ensure_window(workspace, window)?;
        }

        Ok(())
    }

    fn stop_workspace_impl(&self, workspace: &WorkspaceDefinition) -> Result<()> {
        if !self
            .gateway
            .session_exists(workspace.tmux_server.as_deref(), &workspace.id)
        {
            return Ok(());
        }

        self.gateway
            .kill_session(workspace.tmux_server.as_deref(), &workspace.id)
    }

    fn ensure_window(
        &self,
        workspace: &WorkspaceDefinition,
        window: &WindowDefinition,
    ) -> Result<()> {
        if self
            .gateway
            .window_exists(workspace.tmux_server.as_deref(), &workspace.id, &window.id)
        {
            return Ok(());
        }

        if !self
            .gateway
            .session_exists(workspace.tmux_server.as_deref(), &workspace.id)
        {
            let output = self
                .gateway
                .new_session(
                    workspace.tmux_server.as_deref(),
                    &workspace.id,
                    &window.id,
                    &workspace.working_directory,
                    window.command.as_deref(),
                )
                .with_context(|| {
                    format!(
                        "Failed to create workspace '{}' for window '{}'",
                        workspace.id, window.id
                    )
                })?;
            if !output.status.success() {
                return Err(anyhow!(stderr_or_default(
                    &output,
                    &format!("tmux new-session failed for '{}'", workspace.id),
                )));
            }
            return Ok(());
        }

        let output = self
            .gateway
            .new_window(
                workspace.tmux_server.as_deref(),
                &workspace.id,
                &window.id,
                &workspace.working_directory,
                window.command.as_deref(),
            )
            .with_context(|| {
                format!(
                    "Failed to create window '{}' in workspace '{}'",
                    window.id, workspace.id
                )
            })?;
        if output.status.success() {
            return Ok(());
        }

        Err(anyhow!(stderr_or_default(
            &output,
            &format!(
                "tmux new-window failed for '{}:{}'",
                workspace.id, window.id
            ),
        )))
    }

    fn stop_window_impl(
        &self,
        workspace: &WorkspaceDefinition,
        window: &WindowDefinition,
    ) -> Result<()> {
        if !self
            .gateway
            .window_exists(workspace.tmux_server.as_deref(), &workspace.id, &window.id)
        {
            return Ok(());
        }

        self.gateway
            .kill_window(workspace.tmux_server.as_deref(), &workspace.id, &window.id)
    }
}

fn find_window<'a>(
    workspace: &'a WorkspaceDefinition,
    window_id: &str,
) -> Result<&'a WindowDefinition> {
    workspace
        .windows
        .iter()
        .find(|window| window.id == window_id)
        .ok_or_else(|| {
            anyhow!(
                "Unknown window '{window_id}' in workspace '{}'",
                workspace.id
            )
        })
}

fn stderr_or_default(output: &std::process::Output, default_message: &str) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        default_message.to_string()
    } else {
        stderr
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkspaceManager, WorkspaceProfile};
    use crate::tmux::spec::{
        container_native_workspaces, container_tmux_server_name, first_existing_directory,
        machine_tmux_server_name,
    };
    use std::path::PathBuf;

    #[test]
    fn defaults_to_machine_profile() {
        assert!(matches!(
            WorkspaceManager::new().profile,
            WorkspaceProfile::Machine
        ));
        assert!(matches!(
            WorkspaceManager::for_container().profile,
            WorkspaceProfile::Container
        ));
    }

    #[test]
    fn container_profile_uses_native_workspaces() {
        let workspaces = container_native_workspaces();
        assert_eq!(workspaces.len(), 2);
        assert!(workspaces
            .iter()
            .any(|workspace| workspace.id == "container-desktop-view"));
        assert!(workspaces
            .iter()
            .all(|workspace| workspace.source == "native::container-daemon"));
        assert!(workspaces
            .iter()
            .all(|workspace| workspace.tmux_server.as_deref() == Some("ark-container")));
    }

    #[test]
    fn uses_clean_named_tmux_servers() {
        assert_eq!(container_tmux_server_name(), "ark-container");
        assert_eq!(machine_tmux_server_name(), "ark-machine");
    }

    #[test]
    fn picks_first_existing_working_directory() {
        let existing = std::env::temp_dir();
        let selected = first_existing_directory([
            PathBuf::from("/definitely-not-a-real-directory"),
            existing.clone(),
            PathBuf::from("/tmp"),
        ]);

        assert_eq!(selected, Some(existing));
    }
}
