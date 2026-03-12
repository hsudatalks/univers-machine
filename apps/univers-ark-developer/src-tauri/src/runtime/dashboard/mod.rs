use super::activity::{RUNTIME_BACKGROUND_DASHBOARD_REFRESH_SECS, current_runtime_activity};
mod loader;

use crate::{
    models::{
        ContainerDashboard, ContainerDashboardUpdate, DashboardMonitor, DashboardState,
        RuntimeActivityState,
    },
    services::registry::emit_dashboard_service_statuses,
};
use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Runtime, State};

pub(crate) use self::loader::load_container_dashboard;

pub(crate) const DASHBOARD_UPDATED_EVENT: &str = "container-dashboard-updated";

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_dashboard_update<R: Runtime>(
    app: &AppHandle<R>,
    dashboard_state: &DashboardState,
    target_id: &str,
    refresh_seconds: u64,
    result: Result<ContainerDashboard, String>,
) {
    if let Ok(mut telemetry) = dashboard_state.telemetry.lock() {
        telemetry.updates.record(Instant::now(), 1);
    }

    if let Ok(dashboard) = result.as_ref() {
        emit_dashboard_service_statuses(app, target_id, dashboard);
    }

    let payload = match result {
        Ok(dashboard) => ContainerDashboardUpdate {
            target_id: target_id.to_string(),
            dashboard: Some(dashboard),
            error: None,
            refreshed_at_ms: now_ms(),
            refresh_seconds,
        },
        Err(error) => ContainerDashboardUpdate {
            target_id: target_id.to_string(),
            dashboard: None,
            error: Some(error),
            refreshed_at_ms: now_ms(),
            refresh_seconds,
        },
    };

    let _ = app.emit(DASHBOARD_UPDATED_EVENT, payload);
}

fn stop_dashboard_monitor_inner(
    dashboard_state: &DashboardState,
    target_id: &str,
) -> Result<(), String> {
    dashboard_state
        .sessions
        .lock()
        .map_err(|_| String::from("Dashboard monitor state is unavailable"))?
        .remove(target_id);

    Ok(())
}

pub(crate) fn start_dashboard_monitor(
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
    refresh_seconds: u64,
) -> Result<(), String> {
    dashboard_state
        .sessions
        .lock()
        .map_err(|_| String::from("Dashboard monitor state is unavailable"))?
        .insert(target_id, DashboardMonitor { refresh_seconds });

    Ok(())
}

pub(crate) fn stop_dashboard_monitor(
    dashboard_state: State<'_, DashboardState>,
    target_id: String,
) -> Result<(), String> {
    stop_dashboard_monitor_inner(dashboard_state.inner(), &target_id)
}

pub(crate) fn refresh_dashboard_once<R: Runtime>(
    app: AppHandle<R>,
    dashboard_state: DashboardState,
    target_id: String,
) {
    thread::spawn(move || {
        emit_dashboard_update(
            &app,
            &dashboard_state,
            &target_id,
            0,
            load_container_dashboard(&target_id),
        );
    });
}

fn effective_dashboard_refresh_seconds(
    activity_state: &RuntimeActivityState,
    refresh_seconds: u64,
) -> u64 {
    let activity = current_runtime_activity(activity_state);

    if activity.is_foreground() {
        return refresh_seconds.max(1);
    }

    if !activity.online {
        return RUNTIME_BACKGROUND_DASHBOARD_REFRESH_SECS.max(refresh_seconds.max(1) * 2);
    }

    refresh_seconds.max(RUNTIME_BACKGROUND_DASHBOARD_REFRESH_SECS)
}

pub(crate) fn run_dashboard_scheduler_cycle<R: Runtime>(
    app: &AppHandle<R>,
    dashboard_state: &DashboardState,
    activity_state: &RuntimeActivityState,
    next_due_at: &mut HashMap<String, Instant>,
    max_refreshes: usize,
    prioritized_target_id: Option<&str>,
) -> Duration {
    let now = Instant::now();

    let monitors = dashboard_state
        .sessions
        .lock()
        .map(|sessions| sessions.clone())
        .unwrap_or_default();

    next_due_at.retain(|target_id, _| monitors.contains_key(target_id));

    for target_id in monitors.keys() {
        next_due_at.entry(target_id.clone()).or_insert(now);
    }

    let mut due_targets = monitors
        .iter()
        .filter_map(|(target_id, monitor)| {
            let next_due = next_due_at.get(target_id).copied().unwrap_or(now);
            if next_due <= now {
                Some((target_id.clone(), monitor.refresh_seconds))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    due_targets.sort_by_key(|(target_id, _)| {
        let priority = if prioritized_target_id == Some(target_id.as_str()) {
            0
        } else {
            1
        };

        (priority, target_id.clone())
    });
    let due_now_count = due_targets.len();
    due_targets.truncate(max_refreshes.max(1));

    for (target_id, refresh_seconds) in due_targets {
        emit_dashboard_update(
            app,
            dashboard_state,
            &target_id,
            refresh_seconds,
            load_container_dashboard(&target_id),
        );
        next_due_at.insert(
            target_id,
            now + Duration::from_secs(effective_dashboard_refresh_seconds(
                activity_state,
                refresh_seconds,
            )),
        );
    }
    let next_due = next_due_at
        .values()
        .min()
        .copied()
        .map(|due| due.saturating_duration_since(Instant::now()))
        .unwrap_or(Duration::from_secs(2));
    if let Ok(mut telemetry) = dashboard_state.telemetry.lock() {
        telemetry.next_due_in_ms = next_due.as_millis() as u64;
        telemetry.due_now_count = due_now_count;
    }
    next_due
}

pub(crate) fn stop_all_dashboard_monitors(dashboard_state: &DashboardState) {
    if let Ok(mut sessions) = dashboard_state.sessions.lock() {
        sessions.clear();
    }
}
