use crate::{
    config::{read_server_inventory, resolve_raw_target, resolve_target_ssh_chain},
    models::{
        ConnectivitySnapshot, ConnectivityState, ConnectivityStatusEvent, MachineTransport,
        ManagedServer, TerminalState, TunnelState,
    },
    tunnel::tunnel_session_is_alive,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::Ordering, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use univers_ark_russh::{execute_chain, ClientOptions as RusshClientOptions};

pub(crate) const CONNECTIVITY_STATUS_EVENT: &str = "connectivity-status";
const CONNECTIVITY_MONITOR_TICK: Duration = Duration::from_secs(2);
const CONNECTIVITY_INVENTORY_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
const CONNECTIVITY_ACTIVE_RECHECK_INTERVAL: Duration = Duration::from_secs(15);
const CONNECTIVITY_READY_RECHECK_INTERVAL: Duration = Duration::from_secs(60);
const CONNECTIVITY_CHECKING_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const CONNECTIVITY_ERROR_BACKOFF_BASE: Duration = Duration::from_secs(10);
const CONNECTIVITY_ERROR_BACKOFF_MAX: Duration = Duration::from_secs(300);
const CONNECTIVITY_MAX_PROBES_PER_TICK: usize = 8;
const CONNECTIVITY_PROBE_COMMAND: &str = "true";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeMode {
    ActiveSignal,
    DirectLocal,
    Russh,
}

#[derive(Clone)]
struct ProbeOutcome {
    snapshot: ConnectivitySnapshot,
    mode: ProbeMode,
}

#[derive(Clone)]
struct ProbeSchedule {
    next_due_at: Instant,
    consecutive_failures: u32,
}

impl ProbeSchedule {
    fn due_now(now: Instant) -> Self {
        Self {
            next_due_at: now,
            consecutive_failures: 0,
        }
    }
}

#[derive(Clone)]
struct InventoryCache {
    machines: Vec<ManagedServer>,
    refreshed_at: Instant,
}

#[derive(Default)]
struct ConnectivityMonitorState {
    inventory_cache: Option<InventoryCache>,
    probe_schedules: HashMap<String, ProbeSchedule>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ProbeTargetKind {
    Host,
    Container,
}

#[derive(Clone)]
struct ProbeRequest {
    machine_id: String,
    target_id: String,
    ready_message: String,
    kind: ProbeTargetKind,
}

fn checking_snapshot(message: impl Into<String>) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        state: String::from("checking"),
        message: message.into(),
        reachable: false,
    }
}

fn ready_snapshot(message: impl Into<String>) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        state: String::from("ready"),
        message: message.into(),
        reachable: true,
    }
}

fn error_snapshot(message: impl Into<String>) -> ConnectivitySnapshot {
    ConnectivitySnapshot {
        state: String::from("error"),
        message: message.into(),
        reachable: false,
    }
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

fn probe_target_snapshot<R: Runtime>(
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
            }
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
            }
        }
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return ProbeOutcome {
                snapshot: error_snapshot(format!("Failed to build russh runtime: {}", error)),
                mode: ProbeMode::Russh,
            }
        }
    };

    let snapshot = match runtime.block_on(execute_chain(
        &chain,
        CONNECTIVITY_PROBE_COMMAND,
        &probe_options(),
    )) {
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

fn aggregate_machine_snapshot(
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

fn emit_machine_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    connectivity_state: &ConnectivityState,
    machine_id: &str,
    snapshot: &ConnectivitySnapshot,
) {
    if !upsert_snapshot(&connectivity_state.machine_snapshots, machine_id, snapshot) {
        return;
    }

    let _ = app.emit(
        CONNECTIVITY_STATUS_EVENT,
        ConnectivityStatusEvent {
            entity: String::from("machine"),
            machine_id: machine_id.to_string(),
            target_id: None,
            state: snapshot.state.clone(),
            message: snapshot.message.clone(),
            reachable: snapshot.reachable,
        },
    );
}

fn emit_target_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    connectivity_state: &ConnectivityState,
    machine_id: &str,
    target_id: &str,
    snapshot: &ConnectivitySnapshot,
) {
    if !upsert_snapshot(&connectivity_state.target_snapshots, target_id, snapshot) {
        return;
    }

    let _ = app.emit(
        CONNECTIVITY_STATUS_EVENT,
        ConnectivityStatusEvent {
            entity: String::from("container"),
            machine_id: machine_id.to_string(),
            target_id: Some(target_id.to_string()),
            state: snapshot.state.clone(),
            message: snapshot.message.clone(),
            reachable: snapshot.reachable,
        },
    );
}

fn seed_missing_snapshots<R: Runtime>(
    app: &AppHandle<R>,
    connectivity_state: &ConnectivityState,
    machines: &[ManagedServer],
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
            emit_machine_snapshot(
                app,
                connectivity_state,
                &machine.id,
                &checking_snapshot("Checking machine connectivity."),
            );
        }

        if !known_target_ids.contains(&machine.host_target_id) {
            emit_target_snapshot(
                app,
                connectivity_state,
                &machine.id,
                &machine.host_target_id,
                &checking_snapshot(format!("Checking {} host connectivity.", machine.label)),
            );
        }

        for container in &machine.containers {
            if !known_target_ids.contains(&container.target_id) {
                emit_target_snapshot(
                    app,
                    connectivity_state,
                    &machine.id,
                    &container.target_id,
                    &checking_snapshot(format!("Checking {} connectivity.", container.label)),
                );
            }
        }
    }
}

fn prune_snapshots(connectivity_state: &ConnectivityState, machines: &[ManagedServer]) {
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

fn prune_probe_schedules(monitor_state: &mut ConnectivityMonitorState, machines: &[ManagedServer]) {
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

    monitor_state
        .probe_schedules
        .retain(|target_id, _| active_target_ids.contains(target_id));
}

fn clone_target_snapshots(
    connectivity_state: &ConnectivityState,
) -> HashMap<String, ConnectivitySnapshot> {
    connectivity_state
        .target_snapshots
        .lock()
        .map(|snapshots| snapshots.clone())
        .unwrap_or_default()
}

fn schedule_for_target<'a>(
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

fn sort_probe_requests(
    requests: &mut Vec<ProbeRequest>,
    target_snapshots: &HashMap<String, ConnectivitySnapshot>,
) {
    requests.sort_by_key(|request| {
        (
            request.kind,
            target_priority(target_snapshots.get(&request.target_id)),
            request.target_id.clone(),
        )
    });
}

fn apply_probe_outcome<R: Runtime>(
    app: &AppHandle<R>,
    connectivity_state: &ConnectivityState,
    monitor_state: &mut ConnectivityMonitorState,
    target_snapshots: &mut HashMap<String, ConnectivitySnapshot>,
    machine_id: &str,
    target_id: &str,
    outcome: ProbeOutcome,
    now: Instant,
) {
    emit_target_snapshot(
        app,
        connectivity_state,
        machine_id,
        target_id,
        &outcome.snapshot,
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

fn defer_container_until_host<R: Runtime>(
    app: &AppHandle<R>,
    connectivity_state: &ConnectivityState,
    monitor_state: &mut ConnectivityMonitorState,
    target_snapshots: &mut HashMap<String, ConnectivitySnapshot>,
    machine_id: &str,
    machine_host: &str,
    container_target_id: &str,
    host_snapshot: &ConnectivitySnapshot,
    host_next_due_at: Instant,
    now: Instant,
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

    emit_target_snapshot(
        app,
        connectivity_state,
        machine_id,
        container_target_id,
        &waiting_snapshot,
    );
    target_snapshots.insert(container_target_id.to_string(), waiting_snapshot);

    let schedule = schedule_for_target(monitor_state, container_target_id, now);
    schedule.next_due_at = if host_snapshot.state == "checking" {
        host_next_due_at.min(now + CONNECTIVITY_CHECKING_RETRY_INTERVAL)
    } else {
        host_next_due_at
    };
}

fn load_inventory<R: Runtime>(
    app: &AppHandle<R>,
    connectivity_state: &ConnectivityState,
    monitor_state: &mut ConnectivityMonitorState,
    now: Instant,
) -> Option<Vec<ManagedServer>> {
    let should_refresh = monitor_state
        .inventory_cache
        .as_ref()
        .map(|cache| {
            now.duration_since(cache.refreshed_at) >= CONNECTIVITY_INVENTORY_REFRESH_INTERVAL
        })
        .unwrap_or(true);

    if should_refresh {
        match read_server_inventory(false) {
            Ok(machines) => {
                seed_missing_snapshots(app, connectivity_state, &machines);
                prune_snapshots(connectivity_state, &machines);
                prune_probe_schedules(monitor_state, &machines);
                monitor_state.inventory_cache = Some(InventoryCache {
                    machines: machines.clone(),
                    refreshed_at: now,
                });
            }
            Err(error) => {
                eprintln!(
                    "Failed to load machine inventory for connectivity monitor: {}",
                    error
                );
            }
        }
    }

    monitor_state
        .inventory_cache
        .as_ref()
        .map(|cache| cache.machines.clone())
}

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
) {
    let now = Instant::now();
    let Some(machines) = load_inventory(app, connectivity_state, monitor_state, now) else {
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
    sort_probe_requests(&mut host_requests, &target_snapshots);
    host_requests.truncate(CONNECTIVITY_MAX_PROBES_PER_TICK);

    let host_probe_count = host_requests.len();
    for (request, outcome) in run_probe_batch(app, host_requests) {
        apply_probe_outcome(
            app,
            connectivity_state,
            monitor_state,
            &mut target_snapshots,
            &request.machine_id,
            &request.target_id,
            outcome,
            now,
        );
    }

    let mut container_requests = Vec::new();
    let remaining_probe_budget = CONNECTIVITY_MAX_PROBES_PER_TICK.saturating_sub(host_probe_count);

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
                defer_container_until_host(
                    app,
                    connectivity_state,
                    monitor_state,
                    &mut target_snapshots,
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

    sort_probe_requests(&mut container_requests, &target_snapshots);
    container_requests.truncate(remaining_probe_budget);

    for (request, outcome) in run_probe_batch(app, container_requests) {
        apply_probe_outcome(
            app,
            connectivity_state,
            monitor_state,
            &mut target_snapshots,
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
        emit_machine_snapshot(app, connectivity_state, &machine.id, &machine_snapshot);
    }
}

pub(crate) fn start_connectivity_monitor<R: Runtime>(
    app: AppHandle<R>,
    connectivity_state: ConnectivityState,
) {
    let stop_requested = connectivity_state.stop_requested.clone();

    thread::spawn(move || {
        let mut monitor_state = ConnectivityMonitorState::default();

        while !stop_requested.load(Ordering::Relaxed) {
            run_connectivity_probe_cycle(&app, &connectivity_state, &mut monitor_state);

            let sleep_chunks = CONNECTIVITY_MONITOR_TICK.as_millis() / 250;
            for _ in 0..sleep_chunks.max(1) {
                if stop_requested.load(Ordering::Relaxed) {
                    return;
                }

                thread::sleep(Duration::from_millis(250));
            }
        }
    });
}
