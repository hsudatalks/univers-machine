use crate::{
    machine::initialize_targets_file_path,
    models::{
        ConnectivityState, DashboardState, RuntimeActivityState, SchedulerState, ServiceState,
        TerminalState, TunnelState, VncState,
    },
    runtime::{
        dashboard::stop_all_dashboard_monitors,
        scheduler::{start_background_scheduler, stop_background_scheduler},
        tunnel::{cleanup_stale_ssh_tunnels, stop_all_tunnels},
    },
    secrets::SecretManagementState,
};
use tauri::{App, Manager, Runtime};

pub(super) fn manage_app_state<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .manage(TerminalState::default())
        .manage(TunnelState::default())
        .manage(ServiceState::default())
        .manage(DashboardState::default())
        .manage(ConnectivityState::default())
        .manage(RuntimeActivityState::default())
        .manage(SchedulerState::default())
        .manage(SecretManagementState::new().expect("failed to initialize secret management"))
        .manage(VncState::default())
}

pub(super) fn setup_app<R: Runtime>(app: &mut App<R>) -> Result<(), Box<dyn std::error::Error>> {
    initialize_targets_file_path(app.handle())?;
    start_background_scheduler(
        app.handle().clone(),
        app.state::<SchedulerState>().inner().clone(),
        app.state::<TunnelState>().inner().clone(),
        app.state::<ConnectivityState>().inner().clone(),
        app.state::<DashboardState>().inner().clone(),
        app.state::<RuntimeActivityState>().inner().clone(),
    );

    std::thread::spawn(|| match cleanup_stale_ssh_tunnels() {
        Ok(cleaned) if cleaned > 0 => {
            eprintln!(
                "Reaped {cleaned} stale managed SSH tunnel process(es) before startup."
            );
        }
        Ok(_) => {}
        Err(error) => {
            eprintln!("Failed to reap stale managed SSH tunnels: {error}");
        }
    });

    Ok(())
}

pub(super) fn handle_run_event<R: Runtime>(
    app_handle: &tauri::AppHandle<R>,
    event: tauri::RunEvent,
) {
    if matches!(
        event,
        tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
    ) {
        let tunnel_state = app_handle.state::<TunnelState>();
        let dashboard_state = app_handle.state::<DashboardState>();
        let scheduler_state = app_handle.state::<SchedulerState>();
        stop_background_scheduler(scheduler_state.inner());
        stop_all_tunnels(tunnel_state.inner());
        stop_all_dashboard_monitors(dashboard_state.inner());
    }
}
