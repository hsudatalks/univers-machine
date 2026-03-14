use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceProfile {
    Machine,
    Container,
}

#[derive(Debug, Clone)]
pub struct WorkspaceDefinition {
    pub id: String,
    pub title: String,
    pub category: String,
    pub source: String,
    pub tmux_server: Option<String>,
    pub working_directory: PathBuf,
    pub windows: Vec<WindowDefinition>,
}

#[derive(Debug, Clone)]
pub struct WindowDefinition {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub agent_id: Option<String>,
    pub app_id: Option<String>,
    pub skills: Vec<String>,
    pub command: Option<String>,
}

impl WindowDefinition {
    pub fn with_agent(mut self, agent_id: &str) -> Self {
        self.agent_id = Some(agent_id.to_string());
        self
    }
}

pub trait WorkspaceSpecRepository: Send + Sync {
    fn list(&self, profile: WorkspaceProfile) -> Vec<WorkspaceDefinition>;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub id: String,
    pub title: String,
    pub category: String,
    pub source: String,
    pub tmux_server: Option<String>,
    pub working_directory: String,
    pub running: bool,
    pub healthy: bool,
    pub attached: bool,
    pub active_command: Option<String>,
    pub windows: Vec<WindowStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowStatus {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub agent_id: Option<String>,
    pub app_id: Option<String>,
    pub skills: Vec<String>,
    pub running: bool,
}
