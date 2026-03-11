use crate::{
    connectivity::{run_connectivity_scheduler_cycle, ConnectivitySchedulerState},
    dashboard::run_dashboard_scheduler_cycle,
    models::{
        ConnectivityState, DashboardState, RuntimeActivityState, SchedulerState, TunnelState,
    },
    tunnel::{run_tunnel_supervisor_cycle, TunnelSupervisorState},
};
use std::{
    collections::HashMap,
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Runtime};

const SCHEDULER_STOP_POLL_INTERVAL: Duration = Duration::from_millis(250);
const DASHBOARD_MAX_REFRESHES_PER_TICK: usize = 4;

pub(crate) fn start_background_scheduler<R: Runtime>(
    app: AppHandle<R>,
    scheduler_state: SchedulerState,
    tunnel_state: TunnelState,
    connectivity_state: ConnectivityState,
    dashboard_state: DashboardState,
    activity_state: RuntimeActivityState,
) {
    thread::spawn(move || {
        let mut tunnel_scheduler = TunnelSupervisorState::default();
        let mut connectivity_scheduler = ConnectivitySchedulerState::default();
        let mut dashboard_due_at = HashMap::<String, Instant>::new();

        while !scheduler_state.stop_requested.load(Ordering::Relaxed) {
            let tunnel_sleep = run_tunnel_supervisor_cycle(
                &app,
                &tunnel_state,
                &activity_state,
                &mut tunnel_scheduler,
            );
            let connectivity_sleep = run_connectivity_scheduler_cycle(
                app.clone(),
                connectivity_state.clone(),
                activity_state.clone(),
                &mut connectivity_scheduler,
            );
            let dashboard_sleep = run_dashboard_scheduler_cycle(
                &app,
                &dashboard_state,
                &activity_state,
                &mut dashboard_due_at,
                DASHBOARD_MAX_REFRESHES_PER_TICK,
            );

            let sleep_duration = tunnel_sleep.min(connectivity_sleep).min(dashboard_sleep);
            let mut remaining = sleep_duration;

            while remaining > Duration::ZERO {
                if scheduler_state.stop_requested.load(Ordering::Relaxed) {
                    return;
                }

                let next_sleep = remaining.min(SCHEDULER_STOP_POLL_INTERVAL);
                thread::sleep(next_sleep);
                remaining = remaining.saturating_sub(next_sleep);
            }
        }
    });
}

pub(crate) fn stop_background_scheduler(scheduler_state: &SchedulerState) {
    scheduler_state.stop_requested.store(true, Ordering::Relaxed);
}
