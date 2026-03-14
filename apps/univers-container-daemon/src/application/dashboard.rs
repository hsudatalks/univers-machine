use crate::dashboard::{
    collect_agent, collect_dashboard, collect_project, collect_services, collect_tmux,
    ContainerDashboard, DashboardAgentInfo, DashboardProjectInfo, DashboardRequest,
    DashboardServiceInfo, DashboardTmuxInfo,
};
use anyhow::Result;
use std::sync::Arc;
use univers_daemon_shared::{
    application::{
        agent_session::AgentSessionApplicationService, workspace::WorkspaceApplicationService,
    },
};

#[derive(Clone)]
pub(crate) struct ContainerDashboardApplicationService {
    agent_sessions: Arc<AgentSessionApplicationService>,
    workspace_service: Arc<WorkspaceApplicationService>,
}

impl ContainerDashboardApplicationService {
    pub(crate) fn new(
        agent_sessions: Arc<AgentSessionApplicationService>,
        workspace_service: Arc<WorkspaceApplicationService>,
    ) -> Self {
        Self {
            agent_sessions,
            workspace_service,
        }
    }

    pub(crate) async fn dashboard(&self, request: DashboardRequest) -> Result<ContainerDashboard> {
        collect_dashboard(
            request,
            self.agent_sessions.clone(),
            self.workspace_service.clone(),
        )
        .await
    }

    pub(crate) async fn project(&self, request: DashboardRequest) -> Result<DashboardProjectInfo> {
        collect_project(request).await
    }

    pub(crate) async fn services(
        &self,
        request: DashboardRequest,
    ) -> Result<Vec<DashboardServiceInfo>> {
        collect_services(request).await
    }

    pub(crate) async fn agent(&self, request: DashboardRequest) -> Result<DashboardAgentInfo> {
        collect_agent(
            request,
            self.agent_sessions.clone(),
            self.workspace_service.clone(),
        )
        .await
    }

    pub(crate) async fn tmux(&self, request: DashboardRequest) -> Result<DashboardTmuxInfo> {
        collect_tmux(request, self.workspace_service.clone()).await
    }
}
