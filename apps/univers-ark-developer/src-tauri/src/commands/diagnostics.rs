use crate::{
    models::{
        AppDiagnostics, ConnectivityState, DashboardState, RuntimeActivityState, SchedulerState,
        TerminalState, TunnelState,
    },
    runtime::diagnostics::collect_app_diagnostics,
    secrets::SecretManagementState,
};
use tauri::State;

#[tauri::command]
pub(crate) fn load_app_diagnostics(
    terminal_state: State<'_, TerminalState>,
    tunnel_state: State<'_, TunnelState>,
    connectivity_state: State<'_, ConnectivityState>,
    dashboard_state: State<'_, DashboardState>,
    activity_state: State<'_, RuntimeActivityState>,
    scheduler_state: State<'_, SchedulerState>,
    secret_management_state: State<'_, SecretManagementState>,
) -> Result<AppDiagnostics, String> {
    collect_app_diagnostics(
        terminal_state.inner(),
        tunnel_state.inner(),
        connectivity_state.inner(),
        dashboard_state.inner(),
        activity_state.inner(),
        scheduler_state.inner(),
        secret_management_state.inner(),
    )
}
