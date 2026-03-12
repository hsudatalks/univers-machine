use super::{
    monitor::ConnectivityMonitorState,
    status::{checking_snapshot, error_snapshot, queue_target_snapshot, ready_snapshot},
    CONNECTIVITY_ACTIVE_RECHECK_INTERVAL, CONNECTIVITY_CHECKING_RETRY_INTERVAL,
    CONNECTIVITY_ERROR_BACKOFF_BASE, CONNECTIVITY_ERROR_BACKOFF_MAX, CONNECTIVITY_PROBE_COMMAND,
    CONNECTIVITY_READY_RECHECK_INTERVAL,
};
use crate::{
    infra::russh::execute_chain_blocking,
    machine::{resolve_raw_target, resolve_target_ssh_chain},
    models::{
        ConnectivitySnapshot, ConnectivityState, ConnectivityStatusEvent, MachineTransport,
        ManagedServer, TerminalState, TunnelState,
    },
    runtime::tunnel::tunnel_session_is_alive,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, Runtime};
use univers_ark_russh::ClientOptions as RusshClientOptions;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProbeMode {
    ActiveSignal,
    DirectLocal,
    Russh,
}

#[derive(Clone)]
pub(super) struct ProbeOutcome {
    pub(super) snapshot: ConnectivitySnapshot,
    pub(super) mode: ProbeMode,
}

#[derive(Clone)]
pub(super) struct ProbeSchedule {
    pub(super) next_due_at: Instant,
    pub(super) consecutive_failures: u32,
}

impl ProbeSchedule {
    pub(super) fn due_now(now: Instant) -> Self {
        Self {
            next_due_at: now,
            consecutive_failures: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ProbeTargetKind {
    Host,
    Container,
}

#[derive(Clone)]
pub(super) struct ProbeRequest {
    pub(super) machine_id: String,
    pub(super) target_id: String,
    pub(super) ready_message: String,
    pub(super) kind: ProbeTargetKind,
}

fn probe_options() -> RusshClientOptions {
    RusshClientOptions {
        connect_timeout: Duration::from_secs(2),
        inactivity_timeout: Some(Duration::from_secs(3)),
        keepalive_interval: Some(Duration::from_secs(10)),
        keepalive_max: 1,
    }
}

fn error_backoff_duration(consecutive_failures: u32) -> Duration {
    let shift = consecutive_failures.saturating_sub(1).min(5);
    let seconds = CONNECTIVITY_ERROR_BACKOFF_BASE.as_secs() << shift;
    Duration::from_secs(seconds.min(CONNECTIVITY_ERROR_BACKOFF_MAX.as_secs()))
}

fn active_terminal_target<R: Runtime>(app: &AppHandle<R>, target_id: &str) -> bool {
    let Some(terminal_state) = app.try_state::<TerminalState>() else {
        return false;
    };

    let Ok(sessions) = terminal_state.sessions.lock() else {
        return false;
    };

    sessions
        .get(target_id)
        .map(|session| session.russh.session.is_running())
        .unwrap_or(false)
}

fn active_tunnel_target<R: Runtime>(app: &AppHandle<R>, target_id: &str) -> bool {
    let Some(tunnel_state) = app.try_state::<TunnelState>() else {
        return false;
    };

    let Ok(sessions) = tunnel_state.sessions.lock() else {
        return false;
    };

    let prefix = format!("{target_id}::");
    sessions
        .iter()
        .filter(|(key, _)| key.starts_with(&prefix))
        .any(|(_, session)| tunnel_session_is_alive(session).unwrap_or(false))
}

pub(super) fn probe_target_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    target_id: &str,
    ready_message: &str,
) -> ProbeOutcome {
    if active_terminal_target(app, target_id) {
        return ProbeOutcome {
            snapshot: ready_snapshot(format!(
                "Active terminal session is connected. {ready_message}"
            )),
            mode: ProbeMode::ActiveSignal,
        };
    }

    if active_tunnel_target(app, target_id) {
        return ProbeOutcome {
            snapshot: ready_snapshot(format!(
                "Active managed tunnel is connected. {ready_message}"
            )),
            mode: ProbeMode::ActiveSignal,
        };
    }

    let target = match resolve_raw_target(target_id) {
        Ok(target) => target,
        Err(error) => {
            return ProbeOutcome {
                snapshot: error_snapshot(error),
                mode: ProbeMode::Russh,
            };
        }
    };

    if matches!(target.transport, MachineTransport::Local) {
        return ProbeOutcome {
            snapshot: ready_snapshot(ready_message),
            mode: ProbeMode::DirectLocal,
        };
    }

    let chain = match resolve_target_ssh_chain(target_id) {
        Ok(chain) => chain,
        Err(error) => {
            return ProbeOutcome {
                snapshot: error_snapshot(error),
                mode: ProbeMode::Russh,
            };
        }
    };

    let snapshot =
        match execute_chain_blocking(&chain, CONNECTIVITY_PROBE_COMMAND, &probe_options()) {
            Ok(output) if output.exit_status == 0 => ready_snapshot(ready_message),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let detail = if !stderr.is_empty() {
                    stderr
                } else if !stdout.is_empty() {
                    stdout
                } else {
                    format!("Probe command exited with {}", output.exit_status)
                };

                error_snapshot(detail)
            }
            Err(error) => error_snapshot(format!("russh probe failed: {}", error)),
        };

    ProbeOutcome {
        snapshot,
        mode: ProbeMode::Russh,
    }
}

pub(super) fn prune_probe_schedules(
    monitor_state: &mut ConnectivityMonitorState,
    machines: &[ManagedServer],
) {
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
        .collect::<std::collections::HashSet<_>>();

    monitor_state
        .probe_schedules
        .retain(|target_id, _| active_target_ids.contains(target_id));
}

pub(super) fn schedule_for_target<'a>(
    monitor_state: &'a mut ConnectivityMonitorState,
    target_id: &str,
    now: Instant,
) -> &'a mut ProbeSchedule {
    monitor_state
        .probe_schedules
        .entry(target_id.to_string())
        .or_insert_with(|| ProbeSchedule::due_now(now))
}

fn target_priority(snapshot: Option<&ConnectivitySnapshot>) -> u8 {
    match snapshot.map(|snapshot| snapshot.state.as_str()) {
        None => 0,
        Some("checking") => 0,
        Some("error") => 1,
        Some("ready") => 2,
        _ => 2,
    }
}

pub(super) fn sort_probe_requests(
    requests: &mut Vec<ProbeRequest>,
    target_snapshots: &HashMap<String, ConnectivitySnapshot>,
    prioritized_machine_id: Option<&str>,
    prioritized_target_id: Option<&str>,
) {
    requests.sort_by_key(|request| {
        let focus_priority = if prioritized_target_id == Some(request.target_id.as_str()) {
            0
        } else if prioritized_machine_id == Some(request.machine_id.as_str()) {
            1
        } else {
            2
        };

        (
            focus_priority,
            request.kind,
            target_priority(target_snapshots.get(&request.target_id)),
            request.target_id.clone(),
        )
    });
}

pub(super) fn apply_probe_outcome(
    connectivity_state: &ConnectivityState,
    monitor_state: &mut ConnectivityMonitorState,
    target_snapshots: &mut HashMap<String, ConnectivitySnapshot>,
    machine_id: &str,
    target_id: &str,
    outcome: ProbeOutcome,
    now: Instant,
    pending_events: &mut Vec<ConnectivityStatusEvent>,
) {
    queue_target_snapshot(
        connectivity_state,
        machine_id,
        target_id,
        &outcome.snapshot,
        pending_events,
    );
    target_snapshots.insert(target_id.to_string(), outcome.snapshot.clone());

    let schedule = schedule_for_target(monitor_state, target_id, now);
    if outcome.snapshot.reachable {
        schedule.consecutive_failures = 0;
        schedule.next_due_at = now
            + match outcome.mode {
                ProbeMode::ActiveSignal | ProbeMode::DirectLocal => {
                    CONNECTIVITY_ACTIVE_RECHECK_INTERVAL
                }
                ProbeMode::Russh => CONNECTIVITY_READY_RECHECK_INTERVAL,
            };
    } else {
        schedule.consecutive_failures = schedule.consecutive_failures.saturating_add(1);
        schedule.next_due_at = now + error_backoff_duration(schedule.consecutive_failures);
    }
}

pub(super) fn defer_container_until_host(
    connectivity_state: &ConnectivityState,
    monitor_state: &mut ConnectivityMonitorState,
    target_snapshots: &mut HashMap<String, ConnectivitySnapshot>,
    machine_id: &str,
    machine_host: &str,
    container_target_id: &str,
    host_snapshot: &ConnectivitySnapshot,
    host_next_due_at: Instant,
    now: Instant,
    pending_events: &mut Vec<ConnectivityStatusEvent>,
) {
    let waiting_snapshot = if host_snapshot.state == "checking" {
        checking_snapshot(format!(
            "Waiting for machine host {} connectivity.",
            machine_host
        ))
    } else {
        error_snapshot(format!(
            "Machine host {} is unavailable. {}",
            machine_host, host_snapshot.message
        ))
    };

    queue_target_snapshot(
        connectivity_state,
        machine_id,
        container_target_id,
        &waiting_snapshot,
        pending_events,
    );
    target_snapshots.insert(container_target_id.to_string(), waiting_snapshot);

    let schedule = schedule_for_target(monitor_state, container_target_id, now);
    schedule.next_due_at = if host_snapshot.state == "checking" {
        host_next_due_at.min(now + CONNECTIVITY_CHECKING_RETRY_INTERVAL)
    } else {
        host_next_due_at
    };
}
