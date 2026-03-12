use super::activity::{
    RUNTIME_BACKGROUND_SUPERVISOR_FLOOR, current_runtime_activity, detect_runtime_suspend_gap,
};
mod cleanup;
mod forwarding;
mod proxy;
mod session;
mod status;

use crate::{
    constants::{TUNNEL_PROBE_INTERVAL, TUNNEL_PROBE_MESSAGE_DELAY, TUNNEL_PROBE_TIMEOUT},
    models::{RuntimeActivityState, TunnelRegistration, TunnelState, TunnelStatus},
    services::runtime::{resolve_runtime_web_surface, service_key},
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Runtime};

use self::forwarding::should_manage_runtime_surface_tunnel;
pub(crate) use self::{
    cleanup::cleanup_stale_ssh_tunnels,
    forwarding::start_tunnel,
    session::{remove_tunnel_session_if_current, stop_tunnel_session, tunnel_session_is_alive},
    status::{
        active_tunnel_status, direct_tunnel_status, emit_tunnel_status_updates,
        starting_tunnel_status, tunnel_status,
    },
};

pub(super) const TUNNEL_STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const TUNNEL_SUPERVISOR_ACTIVE_SLEEP: Duration = Duration::from_millis(200);
const TUNNEL_SUPERVISOR_MAX_SLEEP: Duration = Duration::from_secs(2);
const TUNNEL_RETRY_INTERVAL: Duration = Duration::from_secs(2);
pub(super) const TUNNEL_READY_PROBE_INTERVAL: Duration = Duration::from_millis(1500);
const TUNNEL_RECOVERY_STAGGER_STEP: Duration = Duration::from_millis(250);

pub(crate) struct TunnelSupervisorState {
    last_tick_at: Instant,
    last_recovery_generation: u64,
}

impl Default for TunnelSupervisorState {
    fn default() -> Self {
        Self {
            last_tick_at: Instant::now(),
            last_recovery_generation: 0,
        }
    }
}

pub(crate) fn register_desired_tunnel(
    tunnel_state: &TunnelState,
    target_id: &str,
    service_id: &str,
) {
    let key = service_key(target_id, service_id);
    if let Ok(mut desired) = tunnel_state.desired_tunnels.lock() {
        desired.insert(
            key,
            TunnelRegistration {
                target_id: target_id.to_string(),
                service_id: service_id.to_string(),
                next_attempt_at: Instant::now(),
            },
        );
    }
}

pub(crate) fn sync_desired_tunnels<R: Runtime>(
    app: &AppHandle<R>,
    tunnel_state: &TunnelState,
    requests: &[(String, String)],
) -> Result<Vec<TunnelStatus>, String> {
    let now = Instant::now();
    let mut desired = tunnel_state
        .desired_tunnels
        .lock()
        .map_err(|_| String::from("Tunnel registration state is unavailable"))?;

    let mut next = HashMap::with_capacity(requests.len());

    for (target_id, service_id) in requests {
        let key = service_key(target_id, service_id);
        let registration = desired.remove(&key).unwrap_or(TunnelRegistration {
            target_id: target_id.clone(),
            service_id: service_id.clone(),
            next_attempt_at: now,
        });
        next.insert(key, registration);
    }

    let removed_keys = desired.keys().cloned().collect::<Vec<_>>();
    *desired = next;
    drop(desired);

    if let Ok(mut status_snapshots) = tunnel_state.status_snapshots.lock() {
        for key in &removed_keys {
            status_snapshots.remove(key);
        }
    }

    let removed_sessions = {
        let mut sessions = tunnel_state
            .sessions
            .lock()
            .map_err(|_| String::from("Tunnel session state is unavailable"))?;

        removed_keys
            .into_iter()
            .filter_map(|key| sessions.remove(&key))
            .collect::<Vec<_>>()
    };

    for session in removed_sessions {
        stop_tunnel_session(&session);
    }

    let mut statuses = Vec::with_capacity(requests.len());

    for (target_id, service_id) in requests {
        let status = reconcile_registered_tunnel(app, tunnel_state, target_id, service_id, false)
            .unwrap_or_else(|error| {
                let local_url = resolve_runtime_web_surface(target_id, service_id, tunnel_state)
                    .ok()
                    .map(|surface| surface.local_url);

                tunnel_status(target_id, service_id, local_url, "error", error)
            });

        statuses.push(status);
    }

    Ok(statuses)
}

fn schedule_tunnel_retry(tunnel_state: &TunnelState, target_id: &str, service_id: &str) {
    let key = service_key(target_id, service_id);
    if let Ok(mut desired) = tunnel_state.desired_tunnels.lock() {
        if let Some(registration) = desired.get_mut(&key) {
            registration.next_attempt_at = Instant::now() + TUNNEL_RETRY_INTERVAL;
        }
    }
}

fn stagger_desired_tunnels_for_recovery(tunnel_state: &TunnelState, now: Instant) {
    let Ok(mut desired) = tunnel_state.desired_tunnels.lock() else {
        return;
    };

    let mut keys = desired.keys().cloned().collect::<Vec<_>>();
    keys.sort();

    for (index, key) in keys.into_iter().enumerate() {
        if let Some(registration) = desired.get_mut(&key) {
            registration.next_attempt_at =
                now + TUNNEL_RECOVERY_STAGGER_STEP.saturating_mul(index as u32);
        }
    }
}

pub(crate) fn reconcile_registered_tunnel<R: Runtime>(
    app: &AppHandle<R>,
    tunnel_state: &TunnelState,
    target_id: &str,
    service_id: &str,
    emit_status_event: bool,
) -> Result<TunnelStatus, String> {
    let surface = resolve_runtime_web_surface(target_id, service_id, tunnel_state)?;

    if !should_manage_runtime_surface_tunnel(target_id, &surface)? {
        let status = direct_tunnel_status(target_id, &surface);
        if emit_status_event {
            emit_tunnel_status_updates(
                app,
                &tunnel_state.status_snapshots,
                &tunnel_state.telemetry,
                [status.clone()],
            );
        }
        return Ok(status);
    }

    let key = service_key(target_id, service_id);
    let stale_session = {
        let mut sessions = tunnel_state
            .sessions
            .lock()
            .map_err(|_| String::from("Tunnel session state is unavailable"))?;

        match sessions.get(&key).cloned() {
            Some(session) => {
                if tunnel_session_is_alive(&session)? {
                    let status = active_tunnel_status(target_id, &surface, &session);
                    if emit_status_event {
                        emit_tunnel_status_updates(
                            app,
                            &tunnel_state.status_snapshots,
                            &tunnel_state.telemetry,
                            [status.clone()],
                        );
                    }
                    return Ok(status);
                }

                sessions.remove(&key);
                Some(session)
            }
            None => None,
        }
    };

    if let Some(session) = stale_session {
        stop_tunnel_session(&session);
    }

    match start_tunnel(app, tunnel_state, target_id, &surface) {
        Ok(status) => {
            if emit_status_event {
                emit_tunnel_status_updates(
                    app,
                    &tunnel_state.status_snapshots,
                    &tunnel_state.telemetry,
                    [status.clone()],
                );
            }
            Ok(status)
        }
        Err(error) => {
            schedule_tunnel_retry(tunnel_state, target_id, service_id);
            let status = tunnel_status(
                target_id,
                service_id,
                Some(surface.local_url.clone()),
                "error",
                error.clone(),
            );
            if emit_status_event {
                emit_tunnel_status_updates(
                    app,
                    &tunnel_state.status_snapshots,
                    &tunnel_state.telemetry,
                    [status],
                );
            }
            Err(error)
        }
    }
}

pub(crate) fn run_tunnel_supervisor_cycle<R: Runtime>(
    app: &AppHandle<R>,
    tunnel_state: &TunnelState,
    activity_state: &RuntimeActivityState,
    scheduler_state: &mut TunnelSupervisorState,
    max_reconciles: usize,
    prioritized_target_id: Option<&str>,
) -> Duration {
    let now = Instant::now();
    let gap_detected =
        detect_runtime_suspend_gap(activity_state, &mut scheduler_state.last_tick_at, now);
    let activity = current_runtime_activity(activity_state);
    let recovery_generation_changed =
        activity.recovery_generation != scheduler_state.last_recovery_generation;
    scheduler_state.last_recovery_generation = activity.recovery_generation;

    if gap_detected || recovery_generation_changed {
        stagger_desired_tunnels_for_recovery(tunnel_state, now);
    }

    let (mut due_registrations, next_due_at, desired_count) = tunnel_state
        .desired_tunnels
        .lock()
        .map(|desired| {
            let mut next_due_at = None;
            let due_registrations = desired
                .values()
                .filter_map(|registration| {
                    if registration.next_attempt_at <= now {
                        Some(registration.clone())
                    } else {
                        next_due_at = Some(
                            next_due_at
                                .map(|current: Instant| current.min(registration.next_attempt_at))
                                .unwrap_or(registration.next_attempt_at),
                        );
                        None
                    }
                })
                .collect::<Vec<_>>();

            (due_registrations, next_due_at, desired.len())
        })
        .unwrap_or_default();
    let due_now_count = due_registrations.len();

    due_registrations.sort_by_key(|registration| {
        let priority = if prioritized_target_id == Some(registration.target_id.as_str()) {
            0
        } else {
            1
        };

        (
            priority,
            registration.target_id.clone(),
            registration.service_id.clone(),
        )
    });
    due_registrations.truncate(max_reconciles.max(1));
    let reconciled_count = due_registrations.len();
    if let Ok(mut telemetry) = tunnel_state.telemetry.lock() {
        telemetry.next_due_in_ms = if due_now_count > 0 {
            0
        } else {
            next_due_at
                .map(|due| due.saturating_duration_since(now).as_millis() as u64)
                .unwrap_or(0)
        };
        telemetry.due_now_count = due_now_count;
        telemetry.waiting_count = desired_count.saturating_sub(due_now_count);
        telemetry.reconciles.record(now, reconciled_count);
    }

    for registration in due_registrations {
        let _ = reconcile_registered_tunnel(
            app,
            tunnel_state,
            &registration.target_id,
            &registration.service_id,
            true,
        );
    }

    let mut sleep_duration = next_due_at
        .map(|due| {
            let remaining = due.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                TUNNEL_SUPERVISOR_ACTIVE_SLEEP
            } else {
                remaining.min(TUNNEL_SUPERVISOR_MAX_SLEEP)
            }
        })
        .unwrap_or(TUNNEL_SUPERVISOR_MAX_SLEEP);

    if !activity.recovering && (!activity.is_foreground() || !activity.online) {
        sleep_duration = sleep_duration.max(RUNTIME_BACKGROUND_SUPERVISOR_FLOOR);
    }

    sleep_duration
}

pub(crate) fn stop_all_tunnels(tunnel_state: &TunnelState) {
    let sessions = tunnel_state
        .sessions
        .lock()
        .map(|mut active| {
            active
                .drain()
                .map(|(_, session)| session)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for session in sessions {
        stop_tunnel_session(&session);
    }
}
