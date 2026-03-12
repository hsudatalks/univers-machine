use crate::{
    activity::{current_runtime_activity, RUNTIME_BACKGROUND_DASHBOARD_REFRESH_SECS},
    machine::{resolve_raw_target, resolve_target_ssh_chain, run_target_shell_command},
    models::{
        ContainerAgentInfo, ContainerDashboard, ContainerDashboardUpdate, ContainerProjectInfo,
        ContainerRuntimeInfo, ContainerTmuxInfo, ContainerTmuxSessionInfo, DashboardMonitor,
        DashboardState, DeveloperTarget, RuntimeActivityState,
    },
    services::{
        health::{
            dashboard_probe_command, into_container_service_infos, DashboardServicePayload,
        },
        registry::emit_dashboard_service_statuses,
    },
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Runtime, State};
use univers_ark_russh::{execute_chain, ClientOptions as RusshClientOptions};

const DEFAULT_PROJECT_PATH: &str = "~/repos";
pub(crate) const DASHBOARD_UPDATED_EVENT: &str = "container-dashboard-updated";

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

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Failed to build russh runtime: {}", error))?;
    let output = runtime
        .block_on(execute_chain(
            &chain,
            command,
            &RusshClientOptions::default(),
        ))
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_dashboard_update<R: Runtime>(
    app: &AppHandle<R>,
    dashboard_state: &DashboardState,
    target_id: &str,
    refresh_seconds: u64,
    result: Result<ContainerDashboard, String>,
) {
    if let Ok(mut telemetry) = dashboard_state.telemetry.lock() {
        telemetry.updates.record(Instant::now(), 1);
    }

    if let Ok(dashboard) = result.as_ref() {
        emit_dashboard_service_statuses(app, target_id, dashboard);
    }

    let payload = match result {
        Ok(dashboard) => ContainerDashboardUpdate {
            target_id: target_id.to_string(),
            dashboard: Some(dashboard),
            error: None,
            refreshed_at_ms: now_ms(),
            refresh_seconds,
        },
        Err(error) => ContainerDashboardUpdate {
            target_id: target_id.to_string(),
            dashboard: None,
            error: Some(error),
            refreshed_at_ms: now_ms(),
            refresh_seconds,
        },
    };

    let _ = app.emit(DASHBOARD_UPDATED_EVENT, payload);
}

fn stop_dashboard_monitor_inner(
    dashboard_state: &DashboardState,
    target_id: &str,
) -> Result<(), String> {
    dashboard_state
        .sessions
        .lock()
        .map_err(|_| String::from("Dashboard monitor state is unavailable"))?
        .remove(target_id);

    Ok(())
}

pub(crate) fn start_dashboard_monitor(
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
    refresh_seconds: u64,
) -> Result<(), String> {
    dashboard_state
        .sessions
        .lock()
        .map_err(|_| String::from("Dashboard monitor state is unavailable"))?
        .insert(target_id, DashboardMonitor { refresh_seconds });

    Ok(())
}

pub(crate) fn stop_dashboard_monitor(
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
) -> Result<(), String> {
    stop_dashboard_monitor_inner(dashboard_state.inner(), &target_id)
}

pub(crate) fn refresh_dashboard_once<R: Runtime>(
    app: AppHandle<R>,
    dashboard_state: DashboardState,
    target_id: String,
) {
    thread::spawn(move || {
        emit_dashboard_update(
            &app,
            &dashboard_state,
            &target_id,
            0,
            load_container_dashboard(&target_id),
        );
    });
}

fn effective_dashboard_refresh_seconds(
    activity_state: &RuntimeActivityState,
    refresh_seconds: u64,
) -> u64 {
    let activity = current_runtime_activity(activity_state);

    if activity.is_foreground() {
        return refresh_seconds.max(1);
    }

    if !activity.online {
        return RUNTIME_BACKGROUND_DASHBOARD_REFRESH_SECS.max(refresh_seconds.max(1) * 2);
    }

    refresh_seconds.max(RUNTIME_BACKGROUND_DASHBOARD_REFRESH_SECS)
}

pub(crate) fn run_dashboard_scheduler_cycle<R: Runtime>(
    app: &AppHandle<R>,
    dashboard_state: &DashboardState,
    activity_state: &RuntimeActivityState,
    next_due_at: &mut HashMap<String, Instant>,
    max_refreshes: usize,
    prioritized_target_id: Option<&str>,
) -> Duration {
    let now = Instant::now();

    let monitors = dashboard_state
        .sessions
        .lock()
        .map(|sessions| sessions.clone())
        .unwrap_or_default();

    next_due_at.retain(|target_id, _| monitors.contains_key(target_id));

    for target_id in monitors.keys() {
        next_due_at.entry(target_id.clone()).or_insert(now);
    }

    let mut due_targets = monitors
        .iter()
        .filter_map(|(target_id, monitor)| {
            let next_due = next_due_at.get(target_id).copied().unwrap_or(now);
            if next_due <= now {
                Some((target_id.clone(), monitor.refresh_seconds))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    due_targets.sort_by_key(|(target_id, _)| {
        let priority = if prioritized_target_id == Some(target_id.as_str()) {
            0
        } else {
            1
        };

        (priority, target_id.clone())
    });
    let due_now_count = due_targets.len();
    due_targets.truncate(max_refreshes.max(1));

    for (target_id, refresh_seconds) in due_targets {
        emit_dashboard_update(
            app,
            dashboard_state,
            &target_id,
            refresh_seconds,
            load_container_dashboard(&target_id),
        );
        next_due_at.insert(
            target_id,
            now + Duration::from_secs(effective_dashboard_refresh_seconds(activity_state, refresh_seconds)),
        );
    }
    let next_due = next_due_at
        .values()
        .min()
        .copied()
        .map(|due| due.saturating_duration_since(Instant::now()))
        .unwrap_or(Duration::from_secs(2));
    if let Ok(mut telemetry) = dashboard_state.telemetry.lock() {
        telemetry.next_due_in_ms = next_due.as_millis() as u64;
        telemetry.due_now_count = due_now_count;
    }
    next_due
}

pub(crate) fn stop_all_dashboard_monitors(dashboard_state: &DashboardState) {
    if let Ok(mut sessions) = dashboard_state.sessions.lock() {
        sessions.clear();
    }
}
