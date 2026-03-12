use crate::{
    activity::update_runtime_activity as apply_runtime_activity,
    dashboard::{
        load_container_dashboard as read_container_dashboard, refresh_dashboard_once,
        start_dashboard_monitor as register_dashboard_monitor,
        stop_dashboard_monitor as unregister_dashboard_monitor,
    },
    files::{
        list_remote_directory as load_remote_directory,
        read_remote_file_preview as load_remote_file_preview,
    },
    machine::resolve_raw_target,
    models::{
        ContainerDashboard, DashboardState, RemoteDirectoryListing, RemoteFilePreview,
        RuntimeActivityState, TerminalSnapshot, TerminalState, TunnelState, TunnelStatus,
    },
    services::{
        actions::execute_command_service_action,
        catalog::tmux_command_service,
        registry::{emit_dashboard_service_statuses, sync_registered_web_services},
        runtime::{resolve_runtime_web_surface, surface_key},
    },
    terminal::{
        resize_terminal_session, snapshot_for, spawn_terminal_session, stop_terminal_session,
        write_to_terminal_session,
    },
    tunnel::{
        active_tunnel_status, direct_tunnel_status, emit_tunnel_status_updates,
        reconcile_registered_tunnel, register_desired_tunnel, remove_tunnel_session_if_current,
        start_tunnel, stop_tunnel_session, sync_desired_tunnels, tunnel_session_is_alive,
    },
};
use serde::Deserialize;
use tauri::{AppHandle, State, async_runtime};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TunnelRestartSpec {
    pub(crate) target_id: String,
    #[serde(alias = "surfaceId")]
    pub(crate) service_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandServiceActionSpec {
    pub(crate) target_id: String,
    pub(crate) service_id: String,
    pub(crate) action: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeActivityInput {
    visible: bool,
    focused: bool,
    online: bool,
    active_machine_id: Option<String>,
    active_target_id: Option<String>,
}

fn restart_tunnel_inner(
    app: &AppHandle,
    tunnel_inner: &TunnelState,
    target_id: &str,
    service_id: &str,
) -> Result<TunnelStatus, String> {
    register_desired_tunnel(tunnel_inner, target_id, service_id);
    let surface = resolve_runtime_web_surface(target_id, service_id, tunnel_inner)?;

    if !should_manage_runtime_surface_tunnel(target_id, &surface)? {
        return Ok(direct_tunnel_status(target_id, &surface));
    }

    let key = surface_key(target_id, service_id);
    let previous_session = tunnel_inner
        .sessions
        .lock()
        .map_err(|_| String::from("Tunnel session state is unavailable"))?
        .remove(&key);

    if let Some(session) = previous_session {
        stop_tunnel_session(&session);
    }

    start_tunnel(app, tunnel_inner, target_id, &surface)
}

fn should_manage_runtime_surface_tunnel(
    target_id: &str,
    surface: &crate::models::BrowserSurface,
) -> Result<bool, String> {
    if !surface.tunnel_command.trim().is_empty() {
        return Ok(true);
    }

    let target = resolve_raw_target(target_id)?;
    Ok(matches!(
        target.transport,
        crate::models::MachineTransport::Ssh
    ))
}

#[tauri::command]
pub(crate) async fn attach_terminal(
    app: AppHandle,
    terminal_state: State<'_, TerminalState>,
    target_id: String,
) -> Result<TerminalSnapshot, String> {
    let sessions_arc = terminal_state.sessions.clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        let mut sessions = sessions_arc
            .lock()
            .map_err(|_| String::from("Terminal session state is unavailable"))?;

        if let Some(session) = sessions.get(&target_id) {
            return Ok(snapshot_for(&target_id, session));
        }

        let target = resolve_raw_target(&target_id)?;
        let session = spawn_terminal_session(&app_clone, sessions_arc.clone(), &target)?;
        let snapshot = snapshot_for(&target_id, &session);
        sessions.insert(target_id.clone(), session);

        Ok(snapshot)
    })
    .await
    .map_err(|error| format!("Failed to join attach terminal task: {}", error))?
}

#[tauri::command]
pub(crate) async fn restart_terminal(
    app: AppHandle,
    terminal_state: State<'_, TerminalState>,
    target_id: String,
) -> Result<TerminalSnapshot, String> {
    let sessions_arc = terminal_state.sessions.clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        let old_session = sessions_arc
            .lock()
            .map_err(|_| String::from("Terminal session state is unavailable"))?
            .remove(&target_id);

        if let Some(session) = old_session.as_ref() {
            stop_terminal_session(session);
        }
        drop(old_session);

        let target = resolve_raw_target(&target_id)?;
        let session = spawn_terminal_session(&app_clone, sessions_arc.clone(), &target)?;
        let snapshot = snapshot_for(&target_id, &session);

        sessions_arc
            .lock()
            .map_err(|_| String::from("Terminal session state is unavailable"))?
            .insert(target_id.clone(), session);

        Ok(snapshot)
    })
    .await
    .map_err(|error| format!("Failed to join restart terminal task: {}", error))?
}

#[tauri::command]
pub(crate) async fn ensure_tunnel(
    app: AppHandle,
    tunnel_state: State<'_, TunnelState>,
    target_id: String,
    service_id: String,
) -> Result<TunnelStatus, String> {
    let tunnel_inner = tunnel_state.inner().clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        register_desired_tunnel(&tunnel_inner, &target_id, &service_id);
        let surface = resolve_runtime_web_surface(&target_id, &service_id, &tunnel_inner)?;

        if !should_manage_runtime_surface_tunnel(&target_id, &surface)? {
            return Ok(direct_tunnel_status(&target_id, &surface));
        }

        let key = surface_key(&target_id, &service_id);
        let existing_session = tunnel_inner
            .sessions
            .lock()
            .map_err(|_| String::from("Tunnel session state is unavailable"))?
            .get(&key)
            .cloned();

        if let Some(session) = existing_session {
            if tunnel_session_is_alive(&session)? {
                return Ok(active_tunnel_status(&target_id, &surface, &session));
            }

            let _ =
                remove_tunnel_session_if_current(&tunnel_inner.sessions, &key, session.session_id);
        }

        reconcile_registered_tunnel(&app_clone, &tunnel_inner, &target_id, &service_id, false)
    })
    .await
    .map_err(|error| format!("Failed to join ensure tunnel task: {}", error))?
}

#[tauri::command]
pub(crate) async fn sync_tunnel_registrations(
    app: AppHandle,
    tunnel_state: State<'_, TunnelState>,
    requests: Vec<TunnelRestartSpec>,
) -> Result<Vec<TunnelStatus>, String> {
    let tunnel_inner = tunnel_state.inner().clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        let request_pairs = requests
            .iter()
            .map(|request| (request.target_id.clone(), request.service_id.clone()))
            .collect::<Vec<_>>();

        sync_registered_web_services(&app_clone, &request_pairs);
        let statuses = sync_desired_tunnels(&app_clone, &tunnel_inner, &request_pairs)?;

        emit_tunnel_status_updates(
            &app_clone,
            &tunnel_inner.status_snapshots,
            &tunnel_inner.telemetry,
            statuses.clone(),
        );

        Ok(statuses)
    })
    .await
    .map_err(|error| format!("Failed to join sync tunnel registrations task: {}", error))?
}

#[tauri::command]
pub(crate) async fn restart_tunnel(
    app: AppHandle,
    tunnel_state: State<'_, TunnelState>,
    target_id: String,
    service_id: String,
) -> Result<TunnelStatus, String> {
    let tunnel_inner = tunnel_state.inner().clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        restart_tunnel_inner(&app_clone, &tunnel_inner, &target_id, &service_id)
    })
    .await
    .map_err(|error| format!("Failed to join restart tunnel task: {}", error))?
}

#[tauri::command]
pub(crate) async fn restart_all_tunnels(
    app: AppHandle,
    tunnel_state: State<'_, TunnelState>,
    requests: Vec<TunnelRestartSpec>,
) -> Result<Vec<TunnelStatus>, String> {
    let tunnel_inner = tunnel_state.inner().clone();
    let handles = requests
        .into_iter()
        .map(|request| {
            let app_clone = app.clone();
            let tunnel_inner = tunnel_inner.clone();
            async_runtime::spawn_blocking(move || {
                restart_tunnel_inner(
                    &app_clone,
                    &tunnel_inner,
                    &request.target_id,
                    &request.service_id,
                )
            })
        })
        .collect::<Vec<_>>();

    let mut statuses = Vec::with_capacity(handles.len());
    for handle in handles {
        statuses.push(
            handle
                .await
                .map_err(|error| format!("Failed to join restart tunnel task: {}", error))??,
        );
    }

    Ok(statuses)
}

#[tauri::command]
pub(crate) fn write_terminal(
    terminal_state: State<TerminalState>,
    target_id: String,
    data: String,
) -> Result<(), String> {
    let session = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Terminal session state is unavailable"))?
        .get(&target_id)
        .cloned()
        .ok_or_else(|| format!("No active terminal session for {}", target_id))?;

    write_to_terminal_session(&target_id, &session, &data)
}

#[tauri::command]
pub(crate) fn resize_terminal(
    terminal_state: State<TerminalState>,
    target_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let session = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Terminal session state is unavailable"))?
        .get(&target_id)
        .cloned()
        .ok_or_else(|| format!("No active terminal session for {}", target_id))?;

    resize_terminal_session(&target_id, &session, cols, rows)
}

#[tauri::command]
pub(crate) async fn list_remote_directory(
    target_id: String,
    path: Option<String>,
) -> Result<RemoteDirectoryListing, String> {
    async_runtime::spawn_blocking(move || load_remote_directory(&target_id, path))
        .await
        .map_err(|error| format!("Failed to join remote directory task: {}", error))?
}

#[tauri::command]
pub(crate) async fn read_remote_file_preview(
    target_id: String,
    path: String,
) -> Result<RemoteFilePreview, String> {
    async_runtime::spawn_blocking(move || load_remote_file_preview(&target_id, &path))
        .await
        .map_err(|error| format!("Failed to join remote file preview task: {}", error))?
}

#[tauri::command]
pub(crate) async fn load_container_dashboard(
    app: AppHandle,
    target_id: String,
) -> Result<ContainerDashboard, String> {
    async_runtime::spawn_blocking(move || {
        let dashboard = read_container_dashboard(&target_id)?;
        emit_dashboard_service_statuses(&app, &target_id, &dashboard);
        Ok(dashboard)
    })
    .await
    .map_err(|error| format!("Failed to join container dashboard task: {}", error))?
}

#[tauri::command]
pub(crate) fn start_dashboard_monitor(
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
    refresh_seconds: u64,
) -> Result<(), String> {
    register_dashboard_monitor(dashboard_state, target_id, refresh_seconds)
}

#[tauri::command]
pub(crate) fn stop_dashboard_monitor(
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
) -> Result<(), String> {
    unregister_dashboard_monitor(dashboard_state, target_id)
}

#[tauri::command]
pub(crate) fn refresh_container_dashboard(
    app: AppHandle,
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
) -> Result<(), String> {
    refresh_dashboard_once(app, dashboard_state.inner().clone(), target_id);
    Ok(())
}

#[tauri::command]
pub(crate) async fn restart_tmux(app: AppHandle, target_id: String) -> Result<(), String> {
    async_runtime::spawn_blocking(move || {
        let target = resolve_raw_target(&target_id)?;
        let service_id = tmux_command_service(&target)
            .map(|service| service.id.clone())
            .unwrap_or_else(|| String::from("tmux-developer"));

        execute_command_service_action(Some(&app), &target_id, &service_id, "restart")
    })
    .await
    .map_err(|error| format!("Failed to join restart tmux task: {}", error))?
}

#[tauri::command]
pub(crate) async fn execute_command_service(
    app: AppHandle,
    spec: CommandServiceActionSpec,
) -> Result<(), String> {
    async_runtime::spawn_blocking(move || {
        execute_command_service_action(Some(&app), &spec.target_id, &spec.service_id, &spec.action)
    })
    .await
    .map_err(|error| format!("Failed to join command service task: {}", error))?
}

#[tauri::command]
pub(crate) fn clipboard_write(text: String) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("Clipboard unavailable: {}", error))?;
    clipboard
        .set_text(text)
        .map_err(|error| format!("Failed to write to clipboard: {}", error))
}

#[tauri::command]
pub(crate) fn clipboard_read() -> Result<String, String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("Clipboard unavailable: {}", error))?;
    clipboard
        .get_text()
        .map_err(|error| format!("Failed to read clipboard: {}", error))
}

#[tauri::command]
pub(crate) fn update_runtime_activity(
    activity: RuntimeActivityInput,
    activity_state: State<'_, RuntimeActivityState>,
) -> Result<(), String> {
    apply_runtime_activity(
        activity_state.inner(),
        activity.visible,
        activity.focused,
        activity.online,
        activity.active_machine_id,
        activity.active_target_id,
    );
    Ok(())
}
