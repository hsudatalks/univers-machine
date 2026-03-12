use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerProjectInfo {
    pub(crate) project_path: String,
    pub(crate) repo_found: bool,
    pub(crate) branch: Option<String>,
    pub(crate) is_dirty: bool,
    pub(crate) changed_files: u64,
    pub(crate) head_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerRuntimeInfo {
    pub(crate) hostname: String,
    pub(crate) uptime_seconds: u64,
    pub(crate) process_count: u64,
    pub(crate) load_average_1m: f64,
    pub(crate) load_average_5m: f64,
    pub(crate) load_average_15m: f64,
    pub(crate) memory_total_bytes: u64,
    pub(crate) memory_used_bytes: u64,
    pub(crate) disk_total_bytes: u64,
    pub(crate) disk_used_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerServiceInfo {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) detail: String,
    pub(crate) url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerAgentInfo {
    pub(crate) active_agent: String,
    pub(crate) source: String,
    pub(crate) last_activity: Option<String>,
    pub(crate) latest_report: Option<String>,
    pub(crate) latest_report_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerTmuxSessionInfo {
    pub(crate) server: String,
    pub(crate) name: String,
    pub(crate) windows: u64,
    pub(crate) attached: bool,
    pub(crate) active_command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerTmuxInfo {
    pub(crate) installed: bool,
    pub(crate) server_running: bool,
    pub(crate) session_count: u64,
    pub(crate) attached_count: u64,
    pub(crate) active_session: Option<String>,
    pub(crate) active_command: Option<String>,
    pub(crate) sessions: Vec<ContainerTmuxSessionInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerDashboard {
    pub(crate) target_id: String,
    pub(crate) project: ContainerProjectInfo,
    pub(crate) runtime: ContainerRuntimeInfo,
    pub(crate) services: Vec<ContainerServiceInfo>,
    pub(crate) agent: ContainerAgentInfo,
    pub(crate) tmux: ContainerTmuxInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerDashboardUpdate {
    pub(crate) target_id: String,
    pub(crate) dashboard: Option<ContainerDashboard>,
    pub(crate) error: Option<String>,
    pub(crate) refreshed_at_ms: u64,
    pub(crate) refresh_seconds: u64,
}
