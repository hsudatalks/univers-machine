use crate::{
    config::{
        read_bootstrap_data, read_server_inventory, resolve_raw_target,
        restart_container as restart_remote_container, targets_file_path,
    },
    files::{
        list_remote_directory as load_remote_directory,
        read_remote_file_preview as load_remote_file_preview,
    },
    github::{
        load_github_project_state as read_github_project_state,
        load_github_pull_request_detail as read_github_pull_request_detail,
        merge_github_pull_request as execute_github_pull_request_merge,
        open_external_url,
    },
    models::{
        AppBootstrap, GithubProjectState, GithubPullRequestDetail, ManagedServer,
        RemoteDirectoryListing, RemoteFilePreview, TerminalSnapshot, TerminalState, TunnelState,
        TunnelStatus,
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
use tauri::{async_runtime, AppHandle, State};

#[tauri::command]
pub(crate) async fn load_bootstrap(
    tunnel_state: State<'_, TunnelState>,
) -> Result<AppBootstrap, String> {
    let tunnel_state_inner = tunnel_state.inner().clone();

    async_runtime::spawn_blocking(move || {
        let (targets_file, servers) = read_bootstrap_data(false)?;
        let hydrated_targets_file = read_runtime_targets_file(&tunnel_state_inner)?;
        let config_path = targets_file_path();

        Ok(AppBootstrap {
            app_name: "Univers Ark Developer".into(),
            config_path: config_path.display().to_string(),
            selected_target_id: targets_file.selected_target_id,
            targets: hydrated_targets_file.targets,
            servers,
        })
    })
    .await
    .map_err(|error| format!("Failed to join bootstrap task: {}", error))?
}

#[tauri::command]
pub(crate) async fn refresh_bootstrap(
    tunnel_state: State<'_, TunnelState>,
) -> Result<AppBootstrap, String> {
    let tunnel_state_inner = tunnel_state.inner().clone();

    async_runtime::spawn_blocking(move || {
        let (targets_file, servers) = read_bootstrap_data(true)?;
        let hydrated_targets_file = read_runtime_targets_file(&tunnel_state_inner)?;
        let config_path = targets_file_path();

        Ok(AppBootstrap {
            app_name: "Univers Ark Developer".into(),
            config_path: config_path.display().to_string(),
            selected_target_id: targets_file.selected_target_id,
            targets: hydrated_targets_file.targets,
            servers,
        })
    })
    .await
    .map_err(|error| format!("Failed to join refresh bootstrap task: {}", error))?
}

#[tauri::command]
pub(crate) async fn load_server_inventory() -> Result<Vec<ManagedServer>, String> {
    async_runtime::spawn_blocking(|| read_server_inventory(false))
        .await
        .map_err(|error| format!("Failed to join server inventory task: {}", error))?
}

#[tauri::command]
pub(crate) async fn refresh_server_inventory() -> Result<Vec<ManagedServer>, String> {
    async_runtime::spawn_blocking(|| read_server_inventory(true))
        .await
        .map_err(|error| format!("Failed to join refresh server inventory task: {}", error))?
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
    surface_id: String,
) -> Result<TunnelStatus, String> {
    let tunnel_inner = tunnel_state.inner().clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        let surface = resolve_runtime_surface(&target_id, &surface_id, &tunnel_inner)?;

        if surface.tunnel_command.trim().is_empty() {
            return Ok(direct_tunnel_status(&target_id, &surface));
        }

        let key = surface_key(&target_id, &surface_id);
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

        start_tunnel(&app_clone, &tunnel_inner, &target_id, &surface)
    })
    .await
    .map_err(|error| format!("Failed to join ensure tunnel task: {}", error))?
}

#[tauri::command]
pub(crate) async fn restart_tunnel(
    app: AppHandle,
    tunnel_state: State<'_, TunnelState>,
    target_id: String,
    surface_id: String,
) -> Result<TunnelStatus, String> {
    let tunnel_inner = tunnel_state.inner().clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        let surface = resolve_runtime_surface(&target_id, &surface_id, &tunnel_inner)?;

        if surface.tunnel_command.trim().is_empty() {
            return Ok(direct_tunnel_status(&target_id, &surface));
        }

        let key = surface_key(&target_id, &surface_id);
        let previous_session = tunnel_inner
            .sessions
            .lock()
            .map_err(|_| String::from("Tunnel session state is unavailable"))?
            .remove(&key);

        if let Some(session) = previous_session {
            stop_tunnel_session(&session);
        }

        start_tunnel(&app_clone, &tunnel_inner, &target_id, &surface)
    })
    .await
    .map_err(|error| format!("Failed to join restart tunnel task: {}", error))?
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
pub(crate) async fn load_github_project_state() -> Result<GithubProjectState, String> {
    async_runtime::spawn_blocking(read_github_project_state)
        .await
        .map_err(|error| format!("Failed to join GitHub project state task: {}", error))?
}

#[tauri::command]
pub(crate) async fn open_external_link(url: String) -> Result<(), String> {
    async_runtime::spawn_blocking(move || open_external_url(&url))
        .await
        .map_err(|error| format!("Failed to join external link task: {}", error))?
}

#[tauri::command]
pub(crate) async fn load_github_pull_request_detail(
    number: u64,
) -> Result<GithubPullRequestDetail, String> {
    async_runtime::spawn_blocking(move || read_github_pull_request_detail(number))
        .await
        .map_err(|error| format!("Failed to join pull request detail task: {}", error))?
}

#[tauri::command]
pub(crate) async fn merge_github_pull_request(number: u64, method: String) -> Result<(), String> {
    async_runtime::spawn_blocking(move || execute_github_pull_request_merge(number, &method))
        .await
        .map_err(|error| format!("Failed to join pull request merge task: {}", error))?
}

#[tauri::command]
pub(crate) async fn restart_container(
    server_id: String,
    container_name: String,
) -> Result<(), String> {
    async_runtime::spawn_blocking(move || restart_remote_container(&server_id, &container_name))
        .await
        .map_err(|error| format!("Failed to join pull request merge task: {}", error))?
}
