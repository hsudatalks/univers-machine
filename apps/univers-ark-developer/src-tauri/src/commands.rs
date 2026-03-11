use crate::{
    config::{
        execute_target_command_via_russh, read_bootstrap_data, read_server_inventory,
        read_targets_config, resolve_raw_target, restart_container as restart_remote_container,
        save_targets_config, scan_and_store_server_inventory, targets_file_path,
    },
    connectivity::apply_connectivity_snapshots,
    dashboard::{
        load_container_dashboard as read_container_dashboard, refresh_dashboard_once,
        start_dashboard_monitor as spawn_dashboard_monitor,
        stop_dashboard_monitor as halt_dashboard_monitor,
    },
    files::{
        list_remote_directory as load_remote_directory,
        read_remote_file_preview as load_remote_file_preview,
    },
    github::{
        load_github_project_state as read_github_project_state,
        load_github_pull_request_detail as read_github_pull_request_detail,
        merge_github_pull_request as execute_github_pull_request_merge, open_external_url,
    },
    models::{
        command_service, tmux_command_service, AppBootstrap, AppSettings, ConnectivityState,
        ContainerDashboard, DashboardState, GithubProjectState, GithubPullRequestDetail,
        MachineImportCandidate, ManagedServer, RemoteDirectoryListing, RemoteFilePreview,
        TerminalSnapshot, TerminalState, TunnelState, TunnelStatus,
    },
    runtime::{read_runtime_targets_file, resolve_runtime_web_surface, surface_key},
    service_registry::{
        emit_command_service_status, emit_dashboard_service_statuses, sync_registered_web_services,
    },
    settings::{load_app_settings as read_app_settings, save_app_settings as write_app_settings},
    shell::shell_command,
    terminal::{
        resize_terminal_session, snapshot_for, spawn_terminal_session, stop_terminal_session,
        write_to_terminal_session,
    },
    tunnel::{
        active_tunnel_status, direct_tunnel_status, emit_tunnel_status_updates,
        reconcile_registered_tunnel, register_desired_tunnel,
        remove_tunnel_session_if_current, start_tunnel,
        stop_tunnel_session, sync_desired_tunnels, tunnel_session_is_alive,
    },
};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};
use tauri::{async_runtime, AppHandle, State};
use univers_ark_russh::SshConfigResolver;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct TailscaleStatusResponse {
    #[serde(default)]
    peer: HashMap<String, TailscalePeer>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct TailscalePeer {
    #[serde(default)]
    host_name: String,
    #[serde(default)]
    dns_name: String,
    #[serde(default)]
    tailscale_ips: Vec<String>,
    #[serde(default)]
    online: bool,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    os: String,
}

fn sanitize_machine_id(value: &str) -> String {
    let mut machine_id = String::new();
    let mut previous_was_separator = false;

    for character in value.chars() {
        let normalized = if character.is_ascii_alphanumeric() {
            previous_was_separator = false;
            Some(character.to_ascii_lowercase())
        } else if !previous_was_separator {
            previous_was_separator = true;
            Some('-')
        } else {
            None
        };

        if let Some(character) = normalized {
            machine_id.push(character);
        }
    }

    machine_id.trim_matches('-').to_string()
}

fn display_label(value: &str) -> String {
    value
        .split(['-', '_', '.', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };

            let mut label = String::new();
            label.extend(first.to_uppercase());
            label.push_str(chars.as_str());
            label
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}

fn default_import_ssh_user() -> String {
    std::env::var("USER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("ubuntu"))
}

fn scan_ssh_config_machine_candidates_inner() -> Result<Vec<MachineImportCandidate>, String> {
    let resolver = SshConfigResolver::from_default_path()
        .map_err(|error| format!("Failed to read ~/.ssh/config: {}", error))?;

    let mut candidates = resolver
        .aliases()
        .into_iter()
        .filter_map(|alias| {
            let chain = resolver.resolve(&alias).ok()?;
            let final_hop = chain.hops().last()?;
            let jump_chain = chain
                .hops()
                .iter()
                .take(chain.hops().len().saturating_sub(1))
                .map(|hop| crate::models::ImportedMachineJump {
                    host: hop.host.clone(),
                    port: hop.port,
                    user: hop.user.clone(),
                    identity_files: hop.identity_files().iter().map(path_to_string).collect(),
                })
                .collect::<Vec<_>>();

            let detail = if jump_chain.is_empty() {
                format!("{}@{}:{}", final_hop.user, final_hop.host, final_hop.port)
            } else {
                let route = jump_chain
                    .iter()
                    .map(|jump| format!("{}@{}", jump.user, jump.host))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                format!(
                    "{}@{}:{} via {}",
                    final_hop.user, final_hop.host, final_hop.port, route
                )
            };

            Some(MachineImportCandidate {
                import_id: format!("ssh-config:{}", alias),
                machine_id: sanitize_machine_id(&alias),
                label: display_label(&alias),
                host: final_hop.host.clone(),
                port: final_hop.port,
                ssh_user: final_hop.user.clone(),
                identity_files: final_hop
                    .identity_files()
                    .iter()
                    .map(path_to_string)
                    .collect(),
                jump_chain,
                description: format!("Imported from SSH config alias {}.", alias),
                detail,
            })
        })
        .filter(|candidate| !candidate.machine_id.is_empty())
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(candidates)
}

fn scan_tailscale_machine_candidates_inner() -> Result<Vec<MachineImportCandidate>, String> {
    let output = shell_command("tailscale status --json")
        .output()
        .map_err(|error| format!("Failed to execute tailscale status --json: {}", error))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            String::from("tailscale status --json failed")
        } else {
            stderr
        });
    }

    let parsed: TailscaleStatusResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Failed to parse tailscale status output: {}", error))?;

    let mut candidates = parsed
        .peer
        .into_values()
        .filter_map(|peer| {
            let dns_name = peer.dns_name.trim_end_matches('.').to_string();
            let host = if !dns_name.is_empty() {
                dns_name.clone()
            } else {
                peer.tailscale_ips.first().cloned().unwrap_or_default()
            };

            if host.trim().is_empty() {
                return None;
            }

            let label_seed = if !peer.host_name.trim().is_empty() {
                peer.host_name.clone()
            } else if !dns_name.is_empty() {
                dns_name.split('.').next().unwrap_or_default().to_string()
            } else {
                host.clone()
            };

            let status = if peer.online {
                "online"
            } else if peer.active {
                "active"
            } else {
                "offline"
            };

            let detail = [
                Some(host.clone()),
                (!peer.os.trim().is_empty()).then(|| peer.os.clone()),
                Some(status.to_string()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");

            Some(MachineImportCandidate {
                import_id: format!("tailscale:{}", host),
                machine_id: sanitize_machine_id(&label_seed),
                label: display_label(&label_seed),
                host,
                port: 22,
                ssh_user: default_import_ssh_user(),
                identity_files: vec![],
                jump_chain: vec![],
                description: String::from(
                    "Imported from Tailscale peer discovery. Verify SSH user before connecting.",
                ),
                detail,
            })
        })
        .filter(|candidate| !candidate.machine_id.is_empty())
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(candidates)
}

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
    let service = command_service(&target, service_id).ok_or_else(|| {
        format!(
            "Unknown command service {} for target {}",
            service_id, target_id
        )
    })?;

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

    if let Some(app) = app {
        emit_command_service_status(
            app,
            target_id,
            service_id,
            "running",
            format!("Executing {} action.", action),
        );
    }

    let output = execute_target_command_via_russh(target_id, command)?;
    let (exit_status, stdout, stderr) = (output.exit_status, output.stdout, output.stderr);

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
pub(crate) async fn load_bootstrap(
    tunnel_state: State<'_, TunnelState>,
    connectivity_state: State<'_, ConnectivityState>,
) -> Result<AppBootstrap, String> {
    let tunnel_state_inner = tunnel_state.inner().clone();
    let connectivity_state_inner = connectivity_state.inner().clone();

    async_runtime::spawn_blocking(move || {
        let (targets_file, mut servers) = read_bootstrap_data(false)?;
        apply_connectivity_snapshots(&mut servers, &connectivity_state_inner);
        let hydrated_targets_file = read_runtime_targets_file(&tunnel_state_inner)?;
        let config_path = targets_file_path();

        Ok(AppBootstrap {
            app_name: "Univers Ark Developer".into(),
            config_path: config_path.display().to_string(),
            selected_target_id: targets_file.selected_target_id,
            targets: hydrated_targets_file.targets,
            machines: servers,
        })
    })
    .await
    .map_err(|error| format!("Failed to join bootstrap task: {}", error))?
}

#[tauri::command]
pub(crate) async fn refresh_bootstrap(
    tunnel_state: State<'_, TunnelState>,
    connectivity_state: State<'_, ConnectivityState>,
) -> Result<AppBootstrap, String> {
    let tunnel_state_inner = tunnel_state.inner().clone();
    let connectivity_state_inner = connectivity_state.inner().clone();

    async_runtime::spawn_blocking(move || {
        let (targets_file, mut servers) = read_bootstrap_data(false)?;
        apply_connectivity_snapshots(&mut servers, &connectivity_state_inner);
        let hydrated_targets_file = read_runtime_targets_file(&tunnel_state_inner)?;
        let config_path = targets_file_path();

        Ok(AppBootstrap {
            app_name: "Univers Ark Developer".into(),
            config_path: config_path.display().to_string(),
            selected_target_id: targets_file.selected_target_id,
            targets: hydrated_targets_file.targets,
            machines: servers,
        })
    })
    .await
    .map_err(|error| format!("Failed to join refresh bootstrap task: {}", error))?
}

#[tauri::command]
pub(crate) async fn load_machine_inventory(
    connectivity_state: State<'_, ConnectivityState>,
) -> Result<Vec<ManagedServer>, String> {
    let connectivity_state_inner = connectivity_state.inner().clone();

    async_runtime::spawn_blocking(move || {
        let mut servers = read_server_inventory(false)?;
        apply_connectivity_snapshots(&mut servers, &connectivity_state_inner);
        Ok(servers)
    })
    .await
    .map_err(|error| format!("Failed to join machine inventory task: {}", error))?
}

#[tauri::command]
pub(crate) async fn refresh_machine_inventory(
    connectivity_state: State<'_, ConnectivityState>,
) -> Result<Vec<ManagedServer>, String> {
    let connectivity_state_inner = connectivity_state.inner().clone();

    async_runtime::spawn_blocking(move || {
        let mut servers = read_server_inventory(false)?;
        apply_connectivity_snapshots(&mut servers, &connectivity_state_inner);
        Ok(servers)
    })
    .await
    .map_err(|error| format!("Failed to join refresh machine inventory task: {}", error))?
}

#[tauri::command]
pub(crate) async fn scan_machine_inventory(
    machine_id: String,
    connectivity_state: State<'_, ConnectivityState>,
) -> Result<ManagedServer, String> {
    let connectivity_state_inner = connectivity_state.inner().clone();

    async_runtime::spawn_blocking(move || {
        let mut server = scan_and_store_server_inventory(&machine_id)?;
        let mut servers = vec![server.clone()];
        apply_connectivity_snapshots(&mut servers, &connectivity_state_inner);
        server = servers.into_iter().next().unwrap_or(server);
        Ok(server)
    })
    .await
    .map_err(|error| format!("Failed to join machine scan task: {}", error))?
}

#[tauri::command]
pub(crate) async fn scan_ssh_config_machine_candidates(
) -> Result<Vec<MachineImportCandidate>, String> {
    async_runtime::spawn_blocking(scan_ssh_config_machine_candidates_inner)
        .await
        .map_err(|error| format!("Failed to join SSH config scan task: {}", error))?
}

#[tauri::command]
pub(crate) async fn scan_tailscale_machine_candidates(
) -> Result<Vec<MachineImportCandidate>, String> {
    async_runtime::spawn_blocking(scan_tailscale_machine_candidates_inner)
        .await
        .map_err(|error| format!("Failed to join Tailscale scan task: {}", error))?
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

        emit_tunnel_status_updates(&app_clone, &tunnel_inner.status_snapshots, statuses.clone());

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
pub(crate) fn refresh_container_dashboard(app: AppHandle, target_id: String) -> Result<(), String> {
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
        execute_command_service_inner(Some(&app), &spec.target_id, &spec.service_id, &spec.action)
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
