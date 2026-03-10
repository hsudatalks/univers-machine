use crate::{
    config::{
        read_bootstrap_data, read_server_inventory, read_targets_config,
        execute_target_command_via_russh,
        resolve_raw_target, restart_container as restart_remote_container,
        run_target_shell_command, scan_and_store_server_inventory,
        save_targets_config, targets_file_path,
    },
    dashboard::{
        load_container_dashboard as read_container_dashboard,
        refresh_dashboard_once, start_dashboard_monitor as spawn_dashboard_monitor,
        stop_dashboard_monitor as halt_dashboard_monitor,
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
        AppBootstrap, AppSettings, ContainerDashboard, DashboardState,
        GithubProjectState, GithubPullRequestDetail, ManagedServer, RemoteDirectoryListing,
        RemoteFilePreview, TerminalSnapshot, TerminalState, TunnelState, TunnelStatus,
        command_service, tmux_command_service,
    },
    runtime::{read_runtime_targets_file, resolve_runtime_web_surface, surface_key},
    service_registry::{
        emit_command_service_status, emit_dashboard_service_statuses,
        emit_tunnel_service_status, sync_registered_web_services,
    },
    settings::{load_app_settings as read_app_settings, save_app_settings as write_app_settings},
    terminal::{
        resize_terminal_session, snapshot_for, spawn_terminal_session, stop_terminal_session,
        write_to_terminal_session,
    },
    tunnel::{
        active_tunnel_status, direct_tunnel_status, reconcile_registered_tunnel,
        register_desired_tunnel, remove_tunnel_session_if_current, start_tunnel,
        stop_tunnel_session, sync_desired_tunnels, tunnel_session_is_alive,
    },
};
use serde::Deserialize;
use tauri::{async_runtime, AppHandle, Emitter, State};

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

fn execute_command_service_inner(
    app: Option<&AppHandle>,
    target_id: &str,
    service_id: &str,
    action: &str,
) -> Result<(), String> {
    let target = resolve_raw_target(target_id)?;
    let service = command_service(&target, service_id)
        .ok_or_else(|| format!("Unknown command service {} for target {}", service_id, target_id))?;

    let command = match action {
        "restart" => service
            .command
            .as_ref()
            .map(|command| command.restart.trim())
            .filter(|command| !command.is_empty())
            .ok_or_else(|| {
                format!(
                    "Command service {} does not define a restart action",
                    service_id
                )
            })?,
        other => {
            return Err(format!(
                "Unsupported command service action {} for {}",
                other, service_id
            ));
        }
    };

    let is_local_target = matches!(
        target.host.trim(),
        "" | "localhost" | "127.0.0.1" | "::1"
    );

    if let Some(app) = app {
        emit_command_service_status(
            app,
            target_id,
            service_id,
            "running",
            format!("Executing {} action.", action),
        );
    }

    let (exit_status, stdout, stderr) = if is_local_target {
        let output = run_target_shell_command(target_id, command)?;
        (
            output.status.code().unwrap_or(if output.status.success() { 0 } else { 1 }) as u32,
            output.stdout,
            output.stderr,
        )
    } else {
        let output = execute_target_command_via_russh(target_id, command)?;
        (output.exit_status, output.stdout, output.stderr)
    };

    if exit_status == 0 {
        if let Some(app) = app {
            emit_command_service_status(
                app,
                target_id,
                service_id,
                "ready",
                format!("{} action finished successfully.", action),
            );
        }
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&stdout).trim().to_string();

    let error = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("Failed to execute {} action for {}", action, service_id)
    };

    if let Some(app) = app {
        emit_command_service_status(app, target_id, service_id, "error", error.clone());
    }

    Err(error)
}

fn restart_tunnel_inner(
    app: &AppHandle,
    tunnel_inner: &TunnelState,
    target_id: &str,
    service_id: &str,
) -> Result<TunnelStatus, String> {
    register_desired_tunnel(tunnel_inner, target_id, service_id);
    let surface = resolve_runtime_web_surface(target_id, service_id, tunnel_inner)?;

    if surface.tunnel_command.trim().is_empty() {
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
    async_runtime::spawn_blocking(|| read_server_inventory(false))
        .await
        .map_err(|error| format!("Failed to join refresh server inventory task: {}", error))?
}

#[tauri::command]
pub(crate) async fn scan_server_inventory(server_id: String) -> Result<ManagedServer, String> {
    async_runtime::spawn_blocking(move || scan_and_store_server_inventory(&server_id))
        .await
        .map_err(|error| format!("Failed to join server scan task: {}", error))?
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
        // Remove the old session from the map first, then drop the lock
        // BEFORE dropping the old session. The old session's reader thread
        // (terminal.rs) also locks `sessions` on exit — dropping the old
        // session while holding the lock causes a deadlock.
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

        if surface.tunnel_command.trim().is_empty() {
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

        for status in &statuses {
            emit_tunnel_service_status(&app_clone, status);
            let _ = app_clone.emit("tunnel-status", status.clone());
        }

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
    app: AppHandle,
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
    refresh_seconds: u64,
) -> Result<(), String> {
    spawn_dashboard_monitor(app, dashboard_state, target_id, refresh_seconds)
}

#[tauri::command]
pub(crate) fn stop_dashboard_monitor(
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
) -> Result<(), String> {
    halt_dashboard_monitor(dashboard_state, target_id)
}

#[tauri::command]
pub(crate) fn refresh_container_dashboard(
    app: AppHandle,
    target_id: String,
) -> Result<(), String> {
    refresh_dashboard_once(app, target_id);
    Ok(())
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
        .map_err(|error| format!("Failed to join restart container task: {}", error))?
}

#[tauri::command]
pub(crate) async fn restart_tmux(app: AppHandle, target_id: String) -> Result<(), String> {
    async_runtime::spawn_blocking(move || {
        let target = resolve_raw_target(&target_id)?;
        let service_id = tmux_command_service(&target)
            .map(|service| service.id.clone())
            .unwrap_or_else(|| String::from("tmux-developer"));

        execute_command_service_inner(Some(&app), &target_id, &service_id, "restart")
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
        execute_command_service_inner(
            Some(&app),
            &spec.target_id,
            &spec.service_id,
            &spec.action,
        )
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
pub(crate) fn load_targets_config() -> Result<String, String> {
    read_targets_config()
}

#[tauri::command]
pub(crate) fn update_targets_config(content: String) -> Result<(), String> {
    save_targets_config(&content)
}

#[tauri::command]
pub(crate) fn load_app_settings(app_handle: AppHandle) -> Result<AppSettings, String> {
    read_app_settings(&app_handle)
}

#[tauri::command]
pub(crate) fn save_app_settings(
    app_handle: AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    write_app_settings(&app_handle, settings)
}
