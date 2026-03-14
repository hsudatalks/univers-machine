use crate::agents::{AgentCatalog, AgentStatus};
use crate::app::{AppCatalog, AppSpec, AppStatus};
use crate::application::agent_session::AgentSessionApplicationService;
use crate::installer::InstallerRegistry;
use anyhow::Result;
use std::sync::Arc;

pub struct CatalogQueryService {
    app_catalog: Arc<AppCatalog>,
    agent_catalog: Arc<AgentCatalog>,
    installer_registry: Arc<InstallerRegistry>,
    agent_sessions: Arc<AgentSessionApplicationService>,
}

impl CatalogQueryService {
    pub fn new(
        app_catalog: Arc<AppCatalog>,
        agent_catalog: Arc<AgentCatalog>,
        installer_registry: Arc<InstallerRegistry>,
        agent_sessions: Arc<AgentSessionApplicationService>,
    ) -> Self {
        Self {
            app_catalog,
            agent_catalog,
            installer_registry,
            agent_sessions,
        }
    }

    pub fn list_app_specs(&self) -> Vec<AppSpec> {
        self.app_catalog.list_specs()
    }

    pub async fn list_apps(&self) -> Vec<AppStatus> {
        self.app_catalog
            .list_statuses(&self.installer_registry)
            .await
    }

    pub async fn get_app(&self, id: &str) -> Result<AppStatus> {
        self.app_catalog
            .status_for(id, &self.installer_registry)
            .await
    }

    pub async fn list_agents(&self) -> Vec<AgentStatus> {
        let sessions = self.agent_sessions.list_sessions(true).await;
        self.agent_catalog
            .list_statuses(&self.app_catalog, &self.installer_registry, &sessions)
            .await
    }

    pub async fn get_agent(&self, id: &str) -> Result<AgentStatus> {
        let sessions = self.agent_sessions.list_sessions(true).await;
        self.agent_catalog
            .status_for(id, &self.app_catalog, &self.installer_registry, &sessions)
            .await
    }
}
