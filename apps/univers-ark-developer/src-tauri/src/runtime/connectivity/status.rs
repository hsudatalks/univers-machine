use crate::models::{
    ConnectivitySnapshot, ConnectivityState, ConnectivityStatusEvent, ManagedServer,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Instant,
};
use tauri::{AppHandle, Emitter, Runtime};

pub(crate) const CONNECTIVITY_STATUS_BATCH_EVENT: &str = "connectivity-status-batch";

pub(super) fn checking_snapshot(message: impl Into<String>) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        state: String::from("checking"),
        message: message.into(),
        reachable: false,
    }
}

pub(super) fn ready_snapshot(message: impl Into<String>) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        state: String::from("ready"),
        message: message.into(),
        reachable: true,
    }
}

pub(super) fn error_snapshot(message: impl Into<String>) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        state: String::from("error"),
        message: message.into(),
        reachable: false,
    }
}

pub(super) fn aggregate_machine_snapshot(
    host_snapshot: &ConnectivitySnapshot,
    container_snapshots: &[(String, ConnectivitySnapshot)],
) -> ConnectivitySnapshot {
    if container_snapshots.is_empty() {
        return if host_snapshot.reachable {
            ready_snapshot("Machine host is ready. No managed containers detected.")
        } else if host_snapshot.state == "checking" {
            checking_snapshot("Checking machine host. No managed containers detected.")
        } else {
            error_snapshot(format!(
                "Machine host is unavailable: {}",
                host_snapshot.message
            ))
        };
    }

    if host_snapshot.state == "checking"
        || container_snapshots
            .iter()
            .any(|(_, snapshot)| snapshot.state == "checking")
    {
        return checking_snapshot(format!(
            "Checking machine host and {} managed container(s).",
            container_snapshots.len()
        ));
    }

    if !host_snapshot.reachable {
        return error_snapshot(format!(
            "Machine host is unavailable: {}",
            host_snapshot.message
        ));
    }

    let ready_count = container_snapshots
        .iter()
        .filter(|(_, snapshot)| snapshot.reachable)
        .count();

    if ready_count == container_snapshots.len() {
        return ready_snapshot(format!(
            "Machine host and {} managed container(s) are ready.",
            ready_count
        ));
    }

    let first_error = container_snapshots
        .iter()
        .find(|(_, snapshot)| !snapshot.reachable)
        .map(|(target_id, snapshot)| format!("{}: {}", target_id, snapshot.message))
        .unwrap_or_else(|| String::from("One or more managed containers are unavailable."));

    error_snapshot(format!(
        "Machine host is ready, but only {} of {} managed container(s) are ready. {}",
        ready_count,
        container_snapshots.len(),
        first_error
    ))
}

fn snapshot_changed(current: Option<&ConnectivitySnapshot>, next: &ConnectivitySnapshot) -> bool {
    current
        .map(|snapshot| {
            snapshot.state != next.state
                || snapshot.message != next.message
                || snapshot.reachable != next.reachable
        })
        .unwrap_or(true)
}

fn upsert_snapshot(
    snapshots: &Arc<Mutex<HashMap<String, ConnectivitySnapshot>>>,
    key: &str,
    next: &ConnectivitySnapshot,
) -> bool {
    let Ok(mut snapshots) = snapshots.lock() else {
        return false;
    };

    let changed = snapshot_changed(snapshots.get(key), next);
    if changed {
        snapshots.insert(key.to_string(), next.clone());
    }

    changed
}

fn machine_status_event(
    machine_id: &str,
    snapshot: &ConnectivitySnapshot,
) -> ConnectivityStatusEvent {
    ConnectivityStatusEvent {
        entity: String::from("machine"),
        machine_id: machine_id.to_string(),
        target_id: None,
        state: snapshot.state.clone(),
        message: snapshot.message.clone(),
        reachable: snapshot.reachable,
    }
}

fn target_status_event(
    machine_id: &str,
    target_id: &str,
    snapshot: &ConnectivitySnapshot,
) -> ConnectivityStatusEvent {
    ConnectivityStatusEvent {
        entity: String::from("container"),
        machine_id: machine_id.to_string(),
        target_id: Some(target_id.to_string()),
        state: snapshot.state.clone(),
        message: snapshot.message.clone(),
        reachable: snapshot.reachable,
    }
}

pub(super) fn queue_machine_snapshot(
    connectivity_state: &ConnectivityState,
    machine_id: &str,
    snapshot: &ConnectivitySnapshot,
    pending_events: &mut Vec<ConnectivityStatusEvent>,
) {
    if !upsert_snapshot(&connectivity_state.machine_snapshots, machine_id, snapshot) {
        return;
    }

    pending_events.push(machine_status_event(machine_id, snapshot));
}

pub(super) fn queue_target_snapshot(
    connectivity_state: &ConnectivityState,
    machine_id: &str,
    target_id: &str,
    snapshot: &ConnectivitySnapshot,
    pending_events: &mut Vec<ConnectivityStatusEvent>,
) {
    if !upsert_snapshot(&connectivity_state.target_snapshots, target_id, snapshot) {
        return;
    }

    pending_events.push(target_status_event(machine_id, target_id, snapshot));
}

pub(super) fn emit_connectivity_statuses<R: Runtime>(
    app: &AppHandle<R>,
    connectivity_state: &ConnectivityState,
    statuses: Vec<ConnectivityStatusEvent>,
) {
    if statuses.is_empty() {
        return;
    }

    if let Ok(mut telemetry) = connectivity_state.telemetry.lock() {
        let now = Instant::now();
        telemetry.status_events.record(now, 1);
        telemetry.status_items.record(now, statuses.len());
    }

    let _ = app.emit(CONNECTIVITY_STATUS_BATCH_EVENT, statuses);
}

pub(super) fn seed_missing_snapshots(
    connectivity_state: &ConnectivityState,
    machines: &[ManagedServer],
    pending_events: &mut Vec<ConnectivityStatusEvent>,
) {
    let known_machine_ids = connectivity_state
        .machine_snapshots
        .lock()
        .map(|snapshots| snapshots.keys().cloned().collect::<HashSet<_>>())
        .unwrap_or_default();
    let known_target_ids = connectivity_state
        .target_snapshots
        .lock()
        .map(|snapshots| snapshots.keys().cloned().collect::<HashSet<_>>())
        .unwrap_or_default();

    for machine in machines {
        if !known_machine_ids.contains(&machine.id) {
            queue_machine_snapshot(
                connectivity_state,
                &machine.id,
                &checking_snapshot("Checking machine connectivity."),
                pending_events,
            );
        }

        if !known_target_ids.contains(&machine.host_target_id) {
            queue_target_snapshot(
                connectivity_state,
                &machine.id,
                &machine.host_target_id,
                &checking_snapshot(format!("Checking {} host connectivity.", machine.label)),
                pending_events,
            );
        }

        for container in &machine.containers {
            if !known_target_ids.contains(&container.target_id) {
                queue_target_snapshot(
                    connectivity_state,
                    &machine.id,
                    &container.target_id,
                    &checking_snapshot(format!("Checking {} connectivity.", container.label)),
                    pending_events,
                );
            }
        }
    }
}

pub(super) fn prune_snapshots(connectivity_state: &ConnectivityState, machines: &[ManagedServer]) {
    let active_machine_ids = machines
        .iter()
        .map(|machine| machine.id.clone())
        .collect::<HashSet<_>>();
    let active_target_ids = machines
        .iter()
        .flat_map(|machine| {
            std::iter::once(machine.host_target_id.clone()).chain(
                machine
                    .containers
                    .iter()
                    .map(|container| container.target_id.clone()),
            )
        })
        .collect::<HashSet<_>>();

    if let Ok(mut snapshots) = connectivity_state.machine_snapshots.lock() {
        snapshots.retain(|machine_id, _| active_machine_ids.contains(machine_id));
    }

    if let Ok(mut snapshots) = connectivity_state.target_snapshots.lock() {
        snapshots.retain(|target_id, _| active_target_ids.contains(target_id));
    }
}

pub(super) fn clone_target_snapshots(
    connectivity_state: &ConnectivityState,
) -> HashMap<String, ConnectivitySnapshot> {
    connectivity_state
        .target_snapshots
        .lock()
        .map(|snapshots| snapshots.clone())
        .unwrap_or_default()
}
