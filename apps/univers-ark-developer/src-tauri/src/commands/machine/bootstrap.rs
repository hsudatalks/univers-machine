use crate::{
    machine::{
        read_bootstrap_data, read_server_inventory, scan_and_store_server_inventory,
        targets_file_path,
    },
    models::{AppBootstrap, ConnectivityState, ManagedServer},
    runtime::connectivity::apply_connectivity_snapshots,
    services::runtime::read_runtime_targets_file,
};
use tauri::{async_runtime, State};

#[tauri::command]
pub(crate) async fn load_bootstrap(
    tunnel_state: State<'_, crate::models::TunnelState>,
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
            app_name: "Ark Console".into(),
            config_path: config_path.display().to_string(),
            selected_target_id: targets_file.selected_target_id,
            targets: hydrated_targets_file.targets,
            machines: servers,
        })
    })
    .await
    .map_err(|error| format!("Failed to join bootstrap task: {error}"))?
}

#[tauri::command]
pub(crate) async fn refresh_bootstrap(
    tunnel_state: State<'_, crate::models::TunnelState>,
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
            app_name: "Ark Console".into(),
            config_path: config_path.display().to_string(),
            selected_target_id: targets_file.selected_target_id,
            targets: hydrated_targets_file.targets,
            machines: servers,
        })
    })
    .await
    .map_err(|error| format!("Failed to join refresh bootstrap task: {error}"))?
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
    .map_err(|error| format!("Failed to join machine inventory task: {error}"))?
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
    .map_err(|error| format!("Failed to join refresh machine inventory task: {error}"))?
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
    .map_err(|error| format!("Failed to join machine scan task: {error}"))?
}
