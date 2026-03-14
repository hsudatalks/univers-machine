use crate::application::catalog::CatalogQueryService;
use crate::application::workspace::WorkspaceApplicationService;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLaunchRequest {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeView {
    pub agent_id: String,
    pub ready: bool,
    pub workspace_id: String,
    pub window_id: String,
    pub running: bool,
}

pub struct AgentApplicationService {
    catalog_service: Arc<CatalogQueryService>,
    workspace_service: Arc<WorkspaceApplicationService>,
}

impl AgentApplicationService {
    pub fn new(
        catalog_service: Arc<CatalogQueryService>,
        workspace_service: Arc<WorkspaceApplicationService>,
    ) -> Self {
        Self {
            catalog_service,
            workspace_service,
        }
    }

    pub async fn runtime(&self, agent_id: &str) -> Result<AgentRuntimeView> {
        let agent = self.catalog_service.get_agent(agent_id).await?;
        let (workspace_id, window_id, running) = self.resolve_binding(agent_id, None).await?;
        Ok(AgentRuntimeView {
            agent_id: agent.id,
            ready: agent.ready,
            workspace_id,
            window_id,
            running,
        })
    }

    pub async fn launch(
        &self,
        agent_id: &str,
        request: AgentLaunchRequest,
    ) -> Result<AgentRuntimeView> {
        let agent = self.catalog_service.get_agent(agent_id).await?;
        if !agent.ready {
            return Err(anyhow!(
                "Agent '{}' is not ready. Missing apps: {}",
                agent_id,
                agent.missing_apps.join(", ")
            ));
        }

        let (workspace_id, window_id, _) = self.resolve_binding(agent_id, Some(request)).await?;
        self.workspace_service
            .start_window(&workspace_id, &window_id)
            .await?;

        Ok(AgentRuntimeView {
            agent_id: agent.id,
            ready: true,
            workspace_id,
            window_id,
            running: true,
        })
    }

    pub async fn stop(
        &self,
        agent_id: &str,
        request: AgentLaunchRequest,
    ) -> Result<AgentRuntimeView> {
        let agent = self.catalog_service.get_agent(agent_id).await?;
        let (workspace_id, window_id, _) = self.resolve_binding(agent_id, Some(request)).await?;
        self.workspace_service
            .stop_window(&workspace_id, &window_id)
            .await?;

        Ok(AgentRuntimeView {
            agent_id: agent.id,
            ready: agent.ready,
            workspace_id,
            window_id,
            running: false,
        })
    }

    async fn resolve_binding(
        &self,
        agent_id: &str,
        request: Option<AgentLaunchRequest>,
    ) -> Result<(String, String, bool)> {
        let workspaces = self.workspace_service.list_workspaces().await;

        if let Some(request) = request {
            if let Some(workspace_id) = request.workspace_id {
                let workspace = workspaces
                    .iter()
                    .find(|workspace| workspace.id == workspace_id)
                    .ok_or_else(|| anyhow!("Unknown workspace '{}'", workspace_id))?;

                if let Some(window_id) = request.window_id {
                    let window = workspace
                        .windows
                        .iter()
                        .find(|window| window.id == window_id)
                        .ok_or_else(|| {
                            anyhow!(
                                "Unknown window '{}' in workspace '{}'",
                                window_id,
                                workspace.id
                            )
                        })?;
                    if window.agent_id.as_deref() != Some(agent_id) {
                        return Err(anyhow!(
                            "Window '{}' in workspace '{}' is not bound to agent '{}'",
                            window_id,
                            workspace.id,
                            agent_id
                        ));
                    }
                    return Ok((workspace.id.clone(), window.id.clone(), window.running));
                }

                let window = workspace
                    .windows
                    .iter()
                    .find(|window| window.agent_id.as_deref() == Some(agent_id))
                    .ok_or_else(|| {
                        anyhow!(
                            "Workspace '{}' has no window bound to agent '{}'",
                            workspace.id,
                            agent_id
                        )
                    })?;
                return Ok((workspace.id.clone(), window.id.clone(), window.running));
            }
        }

        for workspace in workspaces {
            if let Some(window) = workspace
                .windows
                .iter()
                .find(|window| window.agent_id.as_deref() == Some(agent_id))
            {
                return Ok((workspace.id, window.id.clone(), window.running));
            }
        }

        Err(anyhow!(
            "No workspace window is bound to agent '{}'",
            agent_id
        ))
    }
}
