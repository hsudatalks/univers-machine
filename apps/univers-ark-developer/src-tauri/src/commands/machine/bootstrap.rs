use crate::{
    machine::{
        load_bootstrap_view, load_machine_inventory_view, scan_machine_inventory_view,
    },
    models::{AppBootstrap, ConnectivityState, ManagedServer},
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
        load_bootstrap_view(&tunnel_state_inner, &connectivity_state_inner, false)
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
        load_bootstrap_view(&tunnel_state_inner, &connectivity_state_inner, false)
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
        load_machine_inventory_view(&connectivity_state_inner, false)
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
        load_machine_inventory_view(&connectivity_state_inner, false)
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
        scan_machine_inventory_view(&machine_id, &connectivity_state_inner)
    })
    .await
    .map_err(|error| format!("Failed to join machine scan task: {error}"))?
}
