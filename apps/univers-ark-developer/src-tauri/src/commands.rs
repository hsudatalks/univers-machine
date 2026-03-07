use crate::{
    config::{read_bootstrap_data, read_server_inventory, resolve_raw_target, targets_file_path},
    files::{
        list_remote_directory as load_remote_directory,
        read_remote_file_preview as load_remote_file_preview,
    },
    models::{
        AppBootstrap, ManagedServer, RemoteDirectoryListing, RemoteFilePreview, TerminalSnapshot,
        TerminalState, TunnelState, TunnelStatus,
    },
    runtime::{read_runtime_targets_file, resolve_runtime_surface, surface_key},
    terminal::{snapshot_for, spawn_terminal_session},
    tunnel::{
        active_tunnel_status, direct_tunnel_status, remove_tunnel_session_if_current, start_tunnel,
        stop_tunnel_session, tunnel_session_is_alive,
    },
};
use portable_pty::PtySize;
use std::io::Write;
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) fn load_bootstrap(tunnel_state: State<TunnelState>) -> Result<AppBootstrap, String> {
    let (targets_file, servers) = read_bootstrap_data(false)?;
    let hydrated_targets_file = read_runtime_targets_file(tunnel_state.inner())?;
    let config_path = targets_file_path();

    Ok(AppBootstrap {
        app_name: "Univers Ark Developer".into(),
        config_path: config_path.display().to_string(),
        selected_target_id: targets_file.selected_target_id,
        targets: hydrated_targets_file.targets,
        servers,
    })
}

#[tauri::command]
pub(crate) fn refresh_bootstrap(tunnel_state: State<TunnelState>) -> Result<AppBootstrap, String> {
    let (targets_file, servers) = read_bootstrap_data(true)?;
    let hydrated_targets_file = read_runtime_targets_file(tunnel_state.inner())?;
    let config_path = targets_file_path();

    Ok(AppBootstrap {
        app_name: "Univers Ark Developer".into(),
        config_path: config_path.display().to_string(),
        selected_target_id: targets_file.selected_target_id,
        targets: hydrated_targets_file.targets,
        servers,
    })
}

#[tauri::command]
pub(crate) fn load_server_inventory() -> Result<Vec<ManagedServer>, String> {
    read_server_inventory(false)
}

#[tauri::command]
pub(crate) fn refresh_server_inventory() -> Result<Vec<ManagedServer>, String> {
    read_server_inventory(true)
}

#[tauri::command]
pub(crate) fn attach_terminal(
    app: AppHandle,
    terminal_state: State<TerminalState>,
    target_id: String,
) -> Result<TerminalSnapshot, String> {
    let mut sessions = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Terminal session state is unavailable"))?;

    if let Some(session) = sessions.get(&target_id) {
        return Ok(snapshot_for(&target_id, session));
    }

    let target = resolve_raw_target(&target_id)?;
    let session = spawn_terminal_session(&app, terminal_state.sessions.clone(), &target)?;
    let snapshot = snapshot_for(&target_id, &session);
    sessions.insert(target_id.clone(), session);

    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn ensure_tunnel(
    app: AppHandle,
    tunnel_state: State<TunnelState>,
    target_id: String,
    surface_id: String,
) -> Result<TunnelStatus, String> {
    let surface = resolve_runtime_surface(&target_id, &surface_id, tunnel_state.inner())?;

    if surface.tunnel_command.trim().is_empty() {
        return Ok(direct_tunnel_status(&target_id, &surface));
    }

    let key = surface_key(&target_id, &surface_id);
    let existing_session = tunnel_state
        .sessions
        .lock()
        .map_err(|_| String::from("Tunnel session state is unavailable"))?
        .get(&key)
        .cloned();

    if let Some(session) = existing_session {
        if tunnel_session_is_alive(&session)? {
            return Ok(active_tunnel_status(&target_id, &surface, &session));
        }

        let _ = remove_tunnel_session_if_current(&tunnel_state.sessions, &key, session.session_id);
    }

    start_tunnel(&app, tunnel_state.inner(), &target_id, &surface)
}

#[tauri::command]
pub(crate) fn restart_tunnel(
    app: AppHandle,
    tunnel_state: State<TunnelState>,
    target_id: String,
    surface_id: String,
) -> Result<TunnelStatus, String> {
    let surface = resolve_runtime_surface(&target_id, &surface_id, tunnel_state.inner())?;

    if surface.tunnel_command.trim().is_empty() {
        return Ok(direct_tunnel_status(&target_id, &surface));
    }

    let key = surface_key(&target_id, &surface_id);
    let previous_session = tunnel_state
        .sessions
        .lock()
        .map_err(|_| String::from("Tunnel session state is unavailable"))?
        .remove(&key);

    if let Some(session) = previous_session {
        stop_tunnel_session(&session);
    }

    start_tunnel(&app, tunnel_state.inner(), &target_id, &surface)
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

    let mut writer = session
        .writer
        .lock()
        .map_err(|_| format!("Terminal writer is locked for {}", target_id))?;

    writer
        .write_all(data.as_bytes())
        .map_err(|error| format!("Failed to write to {}: {}", target_id, error))?;
    writer
        .flush()
        .map_err(|error| format!("Failed to flush {}: {}", target_id, error))?;

    Ok(())
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

    let master = session
        .master
        .lock()
        .map_err(|_| format!("Terminal master is locked for {}", target_id))?;

    master
        .resize(PtySize {
            rows: rows.max(12),
            cols: cols.max(40),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to resize {}: {}", target_id, error))
}

#[tauri::command]
pub(crate) fn list_remote_directory(
    target_id: String,
    path: Option<String>,
) -> Result<RemoteDirectoryListing, String> {
    load_remote_directory(&target_id, path)
}

#[tauri::command]
pub(crate) fn read_remote_file_preview(
    target_id: String,
    path: String,
) -> Result<RemoteFilePreview, String> {
    load_remote_file_preview(&target_id, &path)
}
