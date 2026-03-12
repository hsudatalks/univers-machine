use crate::{
    infra::russh::execute_chain_blocking,
    machine::{resolve_raw_target, resolve_target_ssh_chain, run_target_shell_command},
    models::{
        ContainerAgentInfo, ContainerDashboard, ContainerProjectInfo, ContainerRuntimeInfo,
        ContainerTmuxInfo, ContainerTmuxSessionInfo, DeveloperTarget,
    },
    services::health::{
        DashboardServicePayload, dashboard_probe_command, into_container_service_infos,
    },
};
use serde::Deserialize;
use univers_ark_russh::ClientOptions as RusshClientOptions;

const DEFAULT_PROJECT_PATH: &str = "~/repos";

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

fn target_project_path(target: &DeveloperTarget) -> &str {
    let project_path = target.workspace.project_path.trim();
    if !project_path.is_empty() {
        return project_path;
    }

    let files_root = target.workspace.files_root.trim();
    if !files_root.is_empty() {
        return files_root;
    }

    DEFAULT_PROJECT_PATH
}

fn dashboard_command(target: &DeveloperTarget) -> Result<String, String> {
    dashboard_probe_command(target, target_project_path(target))
}

pub(crate) fn load_container_dashboard(target_id: &str) -> Result<ContainerDashboard, String> {
    let stdout = load_container_dashboard_stdout(target_id)?;

    let payload = serde_json::from_slice::<DashboardPayload>(&stdout)
        .map_err(|error| format!("Failed to parse dashboard for {}: {}", target_id, error))?;

    Ok(ContainerDashboard {
        target_id: target_id.to_string(),
        project: ContainerProjectInfo {
            project_path: payload.project.project_path,
            repo_found: payload.project.repo_found,
            branch: payload.project.branch,
            is_dirty: payload.project.is_dirty,
            changed_files: payload.project.changed_files,
            head_summary: payload.project.head_summary,
        },
        runtime: ContainerRuntimeInfo {
            hostname: payload.runtime.hostname,
            uptime_seconds: payload.runtime.uptime_seconds,
            process_count: payload.runtime.process_count,
            load_average_1m: payload.runtime.load_average_1m,
            load_average_5m: payload.runtime.load_average_5m,
            load_average_15m: payload.runtime.load_average_15m,
            memory_total_bytes: payload.runtime.memory_total_bytes,
            memory_used_bytes: payload.runtime.memory_used_bytes,
            disk_total_bytes: payload.runtime.disk_total_bytes,
            disk_used_bytes: payload.runtime.disk_used_bytes,
        },
        services: into_container_service_infos(payload.services),
        agent: ContainerAgentInfo {
            active_agent: payload.agent.active_agent,
            source: payload.agent.source,
            last_activity: payload.agent.last_activity,
            latest_report: payload.agent.latest_report,
            latest_report_updated_at: payload.agent.latest_report_updated_at,
        },
        tmux: ContainerTmuxInfo {
            installed: payload.tmux.installed,
            server_running: payload.tmux.server_running,
            session_count: payload.tmux.session_count,
            attached_count: payload.tmux.attached_count,
            active_session: payload.tmux.active_session,
            active_command: payload.tmux.active_command,
            sessions: payload
                .tmux
                .sessions
                .into_iter()
                .map(|session| ContainerTmuxSessionInfo {
                    server: session.server,
                    name: session.name,
                    windows: session.windows,
                    attached: session.attached,
                    active_command: session.active_command,
                })
                .collect(),
        },
    })
}

fn load_container_dashboard_stdout(target_id: &str) -> Result<Vec<u8>, String> {
    let target = resolve_raw_target(target_id)?;
    let command = dashboard_command(&target)?;

    if let Ok(stdout) = load_container_dashboard_via_russh(target_id, &command) {
        return Ok(stdout);
    }

    let output = run_target_shell_command(target_id, &command)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("Dashboard command failed for {}", target_id)
        });
    }

    Ok(output.stdout)
}

fn load_container_dashboard_via_russh(target_id: &str, command: &str) -> Result<Vec<u8>, String> {
    let chain = resolve_target_ssh_chain(target_id)?;
    let output = execute_chain_blocking(&chain, command, &RusshClientOptions::default())
        .map_err(|error| format!("russh dashboard exec failed for {}: {}", target_id, error))?;

    if output.exit_status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("russh dashboard command failed for {}", target_id)
        });
    }

    Ok(output.stdout)
}
