mod command;
mod payload;

use crate::{
    models::ContainerDashboard,
    services::health::{into_container_service_infos, DashboardServicePayload},
};
use serde::Deserialize;

use self::command::load_container_dashboard_stdout;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardPayload {
    project: DashboardProjectPayload,
    runtime: DashboardRuntimePayload,
    services: Vec<DashboardServicePayload>,
    agent: DashboardAgentPayload,
    tmux: DashboardTmuxPayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardProjectPayload {
    project_path: String,
    repo_found: bool,
    branch: Option<String>,
    is_dirty: bool,
    changed_files: u64,
    head_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardRuntimePayload {
    hostname: String,
    uptime_seconds: u64,
    process_count: u64,
    load_average_1m: f64,
    load_average_5m: f64,
    load_average_15m: f64,
    memory_total_bytes: u64,
    memory_used_bytes: u64,
    disk_total_bytes: u64,
    disk_used_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardAgentPayload {
    active_agent: String,
    source: String,
    last_activity: Option<String>,
    latest_report: Option<String>,
    latest_report_updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardTmuxSessionPayload {
    server: String,
    name: String,
    windows: u64,
    attached: bool,
    active_command: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardTmuxPayload {
    installed: bool,
    server_running: bool,
    session_count: u64,
    attached_count: u64,
    active_session: Option<String>,
    active_command: Option<String>,
    sessions: Vec<DashboardTmuxSessionPayload>,
}

pub(crate) fn load_container_dashboard(target_id: &str) -> Result<ContainerDashboard, String> {
    let stdout = load_container_dashboard_stdout(target_id)?;
    let payload = payload::parse_dashboard_payload(target_id, &stdout)?;

    Ok(ContainerDashboard {
        target_id: target_id.to_string(),
        project: payload::project_info(payload.project),
        runtime: payload::runtime_info(payload.runtime),
        services: into_container_service_infos(payload.services),
        agent: payload::agent_info(payload.agent),
        tmux: payload::tmux_info(payload.tmux),
    })
}
