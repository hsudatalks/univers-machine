use super::{
    monitor::{load_inventory, ConnectivityMonitorState, ConnectivitySchedulerState},
    probe::{
        apply_probe_outcome, defer_container_until_host, probe_target_snapshot,
        schedule_for_target, sort_probe_requests, ProbeMutationContext, ProbeOutcome,
        ProbeRequest, ProbeTargetKind,
    },
    status::{
        aggregate_machine_snapshot, checking_snapshot, clone_target_snapshots,
        emit_connectivity_statuses, queue_machine_snapshot,
    },
    CONNECTIVITY_CHECKING_RETRY_INTERVAL, CONNECTIVITY_MONITOR_TICK,
    CONNECTIVITY_READY_RECHECK_INTERVAL,
};
use crate::{
    models::{ConnectivityState, ManagedServer, RuntimeActivityState},
    runtime::activity::{
        current_runtime_activity, detect_runtime_suspend_gap, RUNTIME_BACKGROUND_MONITOR_INTERVAL,
    },
};
use std::{
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Runtime};

fn run_probe_batch<R: Runtime>(
    app: &AppHandle<R>,
    requests: Vec<ProbeRequest>,
) -> Vec<(ProbeRequest, ProbeOutcome)> {
    thread::scope(|scope| {
        let handles = requests
            .into_iter()
            .map(|request| {
                let app_handle = app.clone();
                scope.spawn(move || {
                    let outcome = probe_target_snapshot(
                        &app_handle,
                        &request.target_id,
                        &request.ready_message,
                    );
                    (request, outcome)
                })
            })
            .collect::<Vec<_>>();

        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    })
}

pub(crate) fn apply_connectivity_snapshots(
    machines: &mut [ManagedServer],
    connectivity_state: &ConnectivityState,
) {
    let machine_snapshots = connectivity_state
        .machine_snapshots
        .lock()
        .map(|snapshots| snapshots.clone())
        .unwrap_or_default();
    let target_snapshots = connectivity_state
        .target_snapshots
        .lock()
        .map(|snapshots| snapshots.clone())
        .unwrap_or_default();

    for machine in machines {
        if let Some(snapshot) = machine_snapshots.get(&machine.id) {
            machine.state = snapshot.state.clone();
            machine.message = snapshot.message.clone();
        }

        for container in &mut machine.containers {
            if let Some(snapshot) = target_snapshots.get(&container.target_id) {
                container.ssh_state = snapshot.state.clone();
                container.ssh_message = snapshot.message.clone();
                container.ssh_reachable = snapshot.reachable;
            }
        }
    }
}

fn run_connectivity_probe_cycle<R: Runtime>(
    app: &AppHandle<R>,
    connectivity_state: &ConnectivityState,
    monitor_state: &mut ConnectivityMonitorState,
    force_recovery_refresh: bool,
    max_probes_per_tick: usize,
    prioritized_machine_id: Option<&str>,
    prioritized_target_id: Option<&str>,
) {
    let now = Instant::now();
    if force_recovery_refresh {
        monitor_state.inventory_cache = None;
        for schedule in monitor_state.probe_schedules.values_mut() {
            schedule.next_due_at = now;
        }
    }
    let mut pending_events = Vec::new();
    let Some(machines) =
        load_inventory(connectivity_state, monitor_state, now, &mut pending_events)
    else {
        return;
    };

    let mut target_snapshots = clone_target_snapshots(connectivity_state);

    for machine in &machines {
        schedule_for_target(monitor_state, &machine.host_target_id, now);
        for container in &machine.containers {
            schedule_for_target(monitor_state, &container.target_id, now);
        }
    }

    let mut host_requests = machines
        .iter()
        .filter_map(|machine| {
            let schedule = schedule_for_target(monitor_state, &machine.host_target_id, now);
            if schedule.next_due_at > now {
                return None;
            }

            Some(ProbeRequest {
                machine_id: machine.id.clone(),
                target_id: machine.host_target_id.clone(),
                ready_message: format!("Machine host {} is ready.", machine.host),
                kind: ProbeTargetKind::Host,
            })
        })
        .collect::<Vec<_>>();
    sort_probe_requests(
        &mut host_requests,
        &target_snapshots,
        prioritized_machine_id,
        prioritized_target_id,
    );
    host_requests.truncate(max_probes_per_tick.max(1));

    let host_probe_count = host_requests.len();
    let mut executed_probe_count = host_probe_count;
    for (request, outcome) in run_probe_batch(app, host_requests) {
        let mut mutation_context = ProbeMutationContext {
            connectivity_state,
            monitor_state,
            target_snapshots: &mut target_snapshots,
            pending_events: &mut pending_events,
        };
        apply_probe_outcome(
            &mut mutation_context,
            &request.machine_id,
            &request.target_id,
            outcome,
            now,
        );
    }

    let mut container_requests = Vec::new();
    let remaining_probe_budget = max_probes_per_tick.saturating_sub(host_probe_count);

    for machine in &machines {
        let host_snapshot = target_snapshots
            .get(&machine.host_target_id)
            .cloned()
            .unwrap_or_else(|| {
                checking_snapshot(format!("Checking {} host connectivity.", machine.label))
            });
        let host_next_due_at = monitor_state
            .probe_schedules
            .get(&machine.host_target_id)
            .map(|schedule| schedule.next_due_at)
            .unwrap_or(now + CONNECTIVITY_CHECKING_RETRY_INTERVAL);

        for container in &machine.containers {
            if host_snapshot.state != "ready" || !host_snapshot.reachable {
                let mut mutation_context = ProbeMutationContext {
                    connectivity_state,
                    monitor_state,
                    target_snapshots: &mut target_snapshots,
                    pending_events: &mut pending_events,
                };
                defer_container_until_host(
                    &mut mutation_context,
                    &machine.id,
                    &machine.host,
                    &container.target_id,
                    &host_snapshot,
                    host_next_due_at,
                    now,
                );
                continue;
            }

            let schedule = schedule_for_target(monitor_state, &container.target_id, now);
            if schedule.next_due_at > now {
                continue;
            }

            container_requests.push(ProbeRequest {
                machine_id: machine.id.clone(),
                target_id: container.target_id.clone(),
                ready_message: format!("{} is ready for SSH.", container.label),
                kind: ProbeTargetKind::Container,
            });
        }
    }

    sort_probe_requests(
        &mut container_requests,
        &target_snapshots,
        prioritized_machine_id,
        prioritized_target_id,
    );
    container_requests.truncate(remaining_probe_budget);
    executed_probe_count += container_requests.len();

    for (request, outcome) in run_probe_batch(app, container_requests) {
        let mut mutation_context = ProbeMutationContext {
            connectivity_state,
            monitor_state,
            target_snapshots: &mut target_snapshots,
            pending_events: &mut pending_events,
        };
        apply_probe_outcome(
            &mut mutation_context,
            &request.machine_id,
            &request.target_id,
            outcome,
            now,
        );
    }

    for machine in &machines {
        let host_snapshot = target_snapshots
            .get(&machine.host_target_id)
            .cloned()
            .unwrap_or_else(|| {
                checking_snapshot(format!("Checking {} host connectivity.", machine.label))
            });
        let container_snapshots = machine
            .containers
            .iter()
            .map(|container| {
                (
                    container.target_id.clone(),
                    target_snapshots
                        .get(&container.target_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            checking_snapshot(format!("Checking {} connectivity.", container.label))
                        }),
                )
            })
            .collect::<Vec<_>>();

        let machine_snapshot = aggregate_machine_snapshot(&host_snapshot, &container_snapshots);
        queue_machine_snapshot(
            connectivity_state,
            &machine.id,
            &machine_snapshot,
            &mut pending_events,
        );
    }
    let (next_due_in_ms, due_now_count, backoff_target_count, max_consecutive_failures) =
        monitor_state.probe_schedules.values().fold(
            (u64::MAX, 0usize, 0usize, 0u32),
            |acc, schedule| {
                let next_due_in_ms = acc.0.min(
                    schedule
                        .next_due_at
                        .saturating_duration_since(now)
                        .as_millis() as u64,
                );
                let due_now_count = acc.1 + usize::from(schedule.next_due_at <= now);
                let backoff_target_count = acc.2 + usize::from(schedule.consecutive_failures > 0);
                let max_consecutive_failures = acc.3.max(schedule.consecutive_failures);
                (
                    next_due_in_ms,
                    due_now_count,
                    backoff_target_count,
                    max_consecutive_failures,
                )
            },
        );
    if let Ok(mut telemetry) = connectivity_state.telemetry.lock() {
        telemetry.probes.record(now, executed_probe_count);
        telemetry.next_due_in_ms = if monitor_state.probe_schedules.is_empty() {
            0
        } else {
            next_due_in_ms.min(u64::MAX - 1)
        };
        telemetry.due_now_count = due_now_count;
        telemetry.backoff_target_count = backoff_target_count;
        telemetry.max_consecutive_failures = max_consecutive_failures;
    }

    emit_connectivity_statuses(app, connectivity_state, pending_events);
}

pub(crate) fn run_connectivity_scheduler_cycle<R: Runtime>(
    app: AppHandle<R>,
    connectivity_state: ConnectivityState,
    activity_state: RuntimeActivityState,
    scheduler_state: &mut ConnectivitySchedulerState,
    max_probes_per_tick: usize,
    prioritized_machine_id: Option<&str>,
    prioritized_target_id: Option<&str>,
) -> Duration {
    let now = Instant::now();
    let gap_detected =
        detect_runtime_suspend_gap(&activity_state, &mut scheduler_state.last_tick_at, now);
    let activity = current_runtime_activity(&activity_state);
    let recovery_generation_changed =
        activity.recovery_generation != scheduler_state.last_recovery_generation;
    scheduler_state.last_recovery_generation = activity.recovery_generation;

    run_connectivity_probe_cycle(
        &app,
        &connectivity_state,
        &mut scheduler_state.monitor_state,
        gap_detected || recovery_generation_changed,
        max_probes_per_tick,
        prioritized_machine_id,
        prioritized_target_id,
    );

    if activity.recovering || activity.is_foreground() {
        CONNECTIVITY_MONITOR_TICK
    } else if !activity.online {
        RUNTIME_BACKGROUND_MONITOR_INTERVAL.max(CONNECTIVITY_READY_RECHECK_INTERVAL)
    } else {
        RUNTIME_BACKGROUND_MONITOR_INTERVAL
    }
}
