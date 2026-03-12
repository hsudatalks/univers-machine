use crate::{
    machine::targets_file_path,
    models::{
        AppDiagnostics, ConnectivityDiagnostics, ConnectivityState, DashboardDiagnostics,
        DashboardState, PortRangeDiagnostics, RuntimeActivityDiagnostics, RuntimeActivityState,
        SchedulerState, TerminalDiagnostics, TerminalState, TunnelDiagnostics, TunnelState,
    },
    secrets::SecretManagementState,
};
use std::{collections::BTreeMap, sync::atomic::Ordering, time::Instant};

use super::{activity::current_runtime_activity, scheduler::scheduler_budget_diagnostics};

fn count_states<'a>(states: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for state in states {
        *counts.entry(state.to_string()).or_insert(0) += 1;
    }
    counts
}

pub(crate) fn collect_app_diagnostics(
    terminal_state: &TerminalState,
    tunnel_state: &TunnelState,
    connectivity_state: &ConnectivityState,
    dashboard_state: &DashboardState,
    activity_state: &RuntimeActivityState,
    scheduler_state: &SchedulerState,
    secret_management_state: &SecretManagementState,
) -> Result<AppDiagnostics, String> {
    let activity = current_runtime_activity(activity_state);
    let scheduler = scheduler_budget_diagnostics(&activity, scheduler_state);
    let now = Instant::now();

    let terminal_sessions = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Failed to inspect terminal sessions"))?;

    let desired_tunnels = tunnel_state
        .desired_tunnels
        .lock()
        .map_err(|_| String::from("Failed to inspect desired tunnel registrations"))?;
    let tunnel_sessions = tunnel_state
        .sessions
        .lock()
        .map_err(|_| String::from("Failed to inspect active tunnel sessions"))?;
    let tunnel_ports = tunnel_state
        .local_ports
        .lock()
        .map_err(|_| String::from("Failed to inspect tunnel local ports"))?;
    let tunnel_statuses = tunnel_state
        .status_snapshots
        .lock()
        .map_err(|_| String::from("Failed to inspect tunnel status snapshots"))?;

    let machine_snapshots = connectivity_state
        .machine_snapshots
        .lock()
        .map_err(|_| String::from("Failed to inspect machine connectivity snapshots"))?;
    let container_snapshots = connectivity_state
        .target_snapshots
        .lock()
        .map_err(|_| String::from("Failed to inspect container connectivity snapshots"))?;

    let dashboard_sessions = dashboard_state
        .sessions
        .lock()
        .map_err(|_| String::from("Failed to inspect dashboard registrations"))?;
    let tunnel_telemetry = tunnel_state
        .telemetry
        .lock()
        .map(|value| value.clone())
        .map_err(|_| String::from("Failed to inspect tunnel telemetry"))?;
    let connectivity_telemetry = connectivity_state
        .telemetry
        .lock()
        .map(|value| value.clone())
        .map_err(|_| String::from("Failed to inspect connectivity telemetry"))?;
    let dashboard_telemetry = dashboard_state
        .telemetry
        .lock()
        .map(|value| value.clone())
        .map_err(|_| String::from("Failed to inspect dashboard telemetry"))?;
    let secret_management = secret_management_state.diagnostics()?;

    Ok(AppDiagnostics {
        process_id: std::process::id(),
        channel: if cfg!(debug_assertions) {
            String::from("dev")
        } else {
            String::from("prod")
        },
        config_path: targets_file_path().display().to_string(),
        surface_ports: PortRangeDiagnostics {
            start: crate::constants::SURFACE_PORT_START,
            end: crate::constants::SURFACE_PORT_END,
        },
        internal_tunnel_ports: PortRangeDiagnostics {
            start: crate::constants::INTERNAL_TUNNEL_PORT_START,
            end: crate::constants::INTERNAL_TUNNEL_PORT_END,
        },
        activity: RuntimeActivityDiagnostics {
            visible: activity.visible,
            focused: activity.focused,
            online: activity.online,
            recovering: activity.recovering,
            recovery_generation: activity.recovery_generation,
            last_recovery_started_at_ms: activity_state
                .last_recovery_started_at_ms
                .load(Ordering::Acquire),
            active_machine_id: activity.active_machine_id.clone(),
            active_target_id: activity.active_target_id.clone(),
        },
        scheduler,
        terminals: TerminalDiagnostics {
            session_count: terminal_sessions.len(),
        },
        tunnels: TunnelDiagnostics {
            desired_count: desired_tunnels.len(),
            session_count: tunnel_sessions.len(),
            ready_session_count: tunnel_sessions
                .values()
                .filter(|session| session.ready.load(Ordering::Acquire))
                .count(),
            local_port_count: tunnel_ports.len(),
            status_counts: count_states(
                tunnel_statuses.values().map(|status| status.state.as_str()),
            ),
            status_events_per_minute: tunnel_telemetry.status_events.per_minute(now),
            status_items_per_minute: tunnel_telemetry.status_items.per_minute(now),
            reconciles_per_minute: tunnel_telemetry.reconciles.per_minute(now),
            next_due_in_ms: tunnel_telemetry.next_due_in_ms,
            due_now_count: tunnel_telemetry.due_now_count,
            waiting_count: tunnel_telemetry.waiting_count,
        },
        connectivity: ConnectivityDiagnostics {
            machine_snapshot_count: machine_snapshots.len(),
            container_snapshot_count: container_snapshots.len(),
            machine_state_counts: count_states(
                machine_snapshots
                    .values()
                    .map(|snapshot| snapshot.state.as_str()),
            ),
            container_state_counts: count_states(
                container_snapshots
                    .values()
                    .map(|snapshot| snapshot.state.as_str()),
            ),
            status_events_per_minute: connectivity_telemetry.status_events.per_minute(now),
            status_items_per_minute: connectivity_telemetry.status_items.per_minute(now),
            probes_per_minute: connectivity_telemetry.probes.per_minute(now),
            next_due_in_ms: connectivity_telemetry.next_due_in_ms,
            due_now_count: connectivity_telemetry.due_now_count,
            backoff_target_count: connectivity_telemetry.backoff_target_count,
            max_consecutive_failures: connectivity_telemetry.max_consecutive_failures,
        },
        dashboards: DashboardDiagnostics {
            registered_count: dashboard_sessions.len(),
            updates_per_minute: dashboard_telemetry.updates.per_minute(now),
            next_due_in_ms: dashboard_telemetry.next_due_in_ms,
            due_now_count: dashboard_telemetry.due_now_count,
        },
        secret_management,
    })
}
