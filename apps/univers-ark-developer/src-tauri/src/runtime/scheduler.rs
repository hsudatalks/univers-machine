use super::tunnel::{run_tunnel_supervisor_cycle, TunnelSupervisorState};
use super::{
    activity::{current_runtime_activity, RuntimeActivitySnapshot},
    connectivity::{run_connectivity_scheduler_cycle, ConnectivitySchedulerState},
    dashboard::run_dashboard_scheduler_cycle,
};
use crate::models::{
    ConnectivityState, DashboardState, RuntimeActivityState, SchedulerBudgetDiagnostics,
    SchedulerState, TunnelState,
};
use std::{
    collections::HashMap,
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Runtime};

const SCHEDULER_STOP_POLL_INTERVAL: Duration = Duration::from_millis(250);

struct SchedulerBudget {
    max_tunnel_reconciles: usize,
    max_connectivity_probes: usize,
    max_dashboard_refreshes: usize,
}

fn scheduler_budget(activity: &RuntimeActivitySnapshot) -> SchedulerBudget {
    if activity.recovering {
        return SchedulerBudget {
            max_tunnel_reconciles: 6,
            max_connectivity_probes: 10,
            max_dashboard_refreshes: 3,
        };
    }

    if activity.is_foreground() && activity.online {
        return SchedulerBudget {
            max_tunnel_reconciles: 4,
            max_connectivity_probes: 8,
            max_dashboard_refreshes: 2,
        };
    }

    if activity.is_foreground() {
        return SchedulerBudget {
            max_tunnel_reconciles: 2,
            max_connectivity_probes: 4,
            max_dashboard_refreshes: 1,
        };
    }

    if activity.online {
        return SchedulerBudget {
            max_tunnel_reconciles: 2,
            max_connectivity_probes: 4,
            max_dashboard_refreshes: 1,
        };
    }

    SchedulerBudget {
        max_tunnel_reconciles: 1,
        max_connectivity_probes: 2,
        max_dashboard_refreshes: 1,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn scheduler_budget_diagnostics(
    activity: &RuntimeActivitySnapshot,
    scheduler_state: &SchedulerState,
) -> SchedulerBudgetDiagnostics {
    let budget = scheduler_budget(activity);
    let telemetry = scheduler_state
        .telemetry
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default();
    SchedulerBudgetDiagnostics {
        max_tunnel_reconciles: budget.max_tunnel_reconciles,
        max_connectivity_probes: budget.max_connectivity_probes,
        max_dashboard_refreshes: budget.max_dashboard_refreshes,
        next_wake_in_ms: telemetry.next_wake_in_ms,
        last_cycle_started_at_ms: telemetry.last_cycle_started_at_ms,
        last_cycle_finished_at_ms: telemetry.last_cycle_finished_at_ms,
    }
}

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
            let cycle_started_at_ms = now_ms();
            if let Ok(mut telemetry) = scheduler_state.telemetry.lock() {
                telemetry.last_cycle_started_at_ms = cycle_started_at_ms;
            }
            let activity = current_runtime_activity(&activity_state);
            let budget = scheduler_budget(&activity);
            let tunnel_sleep = run_tunnel_supervisor_cycle(
                &app,
                &tunnel_state,
                &activity_state,
                &mut tunnel_scheduler,
                budget.max_tunnel_reconciles,
                activity.active_target_id.as_deref(),
            );
            let connectivity_sleep = run_connectivity_scheduler_cycle(
                app.clone(),
                connectivity_state.clone(),
                activity_state.clone(),
                &mut connectivity_scheduler,
                budget.max_connectivity_probes,
                activity.active_machine_id.as_deref(),
                activity.active_target_id.as_deref(),
            );
            let dashboard_sleep = run_dashboard_scheduler_cycle(
                &app,
                &dashboard_state,
                &activity_state,
                &mut dashboard_due_at,
                budget.max_dashboard_refreshes,
                activity.active_target_id.as_deref(),
            );

            let sleep_duration = tunnel_sleep.min(connectivity_sleep).min(dashboard_sleep);
            if let Ok(mut telemetry) = scheduler_state.telemetry.lock() {
                telemetry.last_cycle_finished_at_ms = now_ms();
                telemetry.next_wake_in_ms = sleep_duration.as_millis() as u64;
            }
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
    scheduler_state
        .stop_requested
        .store(true, Ordering::Relaxed);
}
