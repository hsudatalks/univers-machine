use super::TunnelRestartSpec;
use crate::{
    machine::resolve_raw_target,
    models::{MachineTransport, TunnelState, TunnelStatus},
    runtime::tunnel::{
        active_tunnel_status, direct_tunnel_status, emit_tunnel_status_updates,
        reconcile_registered_tunnel, register_desired_tunnel, remove_tunnel_session_if_current,
        start_tunnel, stop_tunnel_session, sync_desired_tunnels, tunnel_session_is_alive,
    },
    services::{
        registry::sync_registered_web_services,
        runtime::{resolve_runtime_web_surface, surface_key},
    },
};
use tauri::{async_runtime, AppHandle, State};

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
    Ok(matches!(target.transport, MachineTransport::Ssh))
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
    .map_err(|error| format!("Failed to join ensure tunnel task: {error}"))?
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
    .map_err(|error| format!("Failed to join sync tunnel registrations task: {error}"))?
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
    .map_err(|error| format!("Failed to join restart tunnel task: {error}"))?
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
                .map_err(|error| format!("Failed to join restart tunnel task: {error}"))??,
        );
    }

    Ok(statuses)
}
