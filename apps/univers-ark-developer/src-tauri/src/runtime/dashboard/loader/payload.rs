use crate::models::{
    ContainerAgentInfo, ContainerProjectInfo, ContainerRuntimeInfo, ContainerTmuxInfo,
    ContainerTmuxSessionInfo,
};

use super::{
    DashboardAgentPayload, DashboardPayload, DashboardProjectPayload, DashboardRuntimePayload,
    DashboardTmuxPayload,
};

pub(super) fn parse_dashboard_payload(
    target_id: &str,
    stdout: &[u8],
) -> Result<DashboardPayload, String> {
    serde_json::from_slice::<DashboardPayload>(stdout)
        .map_err(|error| format!("Failed to parse dashboard for {}: {}", target_id, error))
}

pub(super) fn project_info(payload: DashboardProjectPayload) -> ContainerProjectInfo {
    ContainerProjectInfo {
        project_path: payload.project_path,
        repo_found: payload.repo_found,
        branch: payload.branch,
        is_dirty: payload.is_dirty,
        changed_files: payload.changed_files,
        head_summary: payload.head_summary,
    }
}

pub(super) fn runtime_info(payload: DashboardRuntimePayload) -> ContainerRuntimeInfo {
    ContainerRuntimeInfo {
        hostname: payload.hostname,
        uptime_seconds: payload.uptime_seconds,
        process_count: payload.process_count,
        load_average_1m: payload.load_average_1m,
        load_average_5m: payload.load_average_5m,
        load_average_15m: payload.load_average_15m,
        memory_total_bytes: payload.memory_total_bytes,
        memory_used_bytes: payload.memory_used_bytes,
        disk_total_bytes: payload.disk_total_bytes,
        disk_used_bytes: payload.disk_used_bytes,
    }
}

pub(super) fn agent_info(payload: DashboardAgentPayload) -> ContainerAgentInfo {
    ContainerAgentInfo {
        active_agent: payload.active_agent,
        source: payload.source,
        last_activity: payload.last_activity,
        latest_report: payload.latest_report,
        latest_report_updated_at: payload.latest_report_updated_at,
    }
}

pub(super) fn tmux_info(payload: DashboardTmuxPayload) -> ContainerTmuxInfo {
    ContainerTmuxInfo {
        installed: payload.installed,
        server_running: payload.server_running,
        session_count: payload.session_count,
        attached_count: payload.attached_count,
        active_session: payload.active_session,
        active_command: payload.active_command,
        sessions: payload
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
    }
}
