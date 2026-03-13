use crate::{
    models::{ContainerDashboard, DashboardState},
    runtime::dashboard::{
        load_container_dashboard as read_container_dashboard, refresh_dashboard_once,
        start_dashboard_monitor as register_dashboard_monitor,
        stop_dashboard_monitor as unregister_dashboard_monitor,
    },
    services::registry::emit_dashboard_service_statuses,
};
use tauri::{async_runtime, AppHandle, State};

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
    .map_err(|error| format!("Failed to join container dashboard task: {error}"))?
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
