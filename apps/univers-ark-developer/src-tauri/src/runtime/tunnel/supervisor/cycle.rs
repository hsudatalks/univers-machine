use super::super::{
    TUNNEL_RECOVERY_STAGGER_STEP, TUNNEL_SUPERVISOR_ACTIVE_SLEEP, TUNNEL_SUPERVISOR_MAX_SLEEP,
};
use super::reconcile::reconcile_registered_tunnel;
use crate::{
    models::{RuntimeActivityState, TunnelState},
    runtime::activity::{
        current_runtime_activity, detect_runtime_suspend_gap, RUNTIME_BACKGROUND_SUPERVISOR_FLOOR,
    },
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Runtime};

pub(crate) struct TunnelSupervisorState {
    pub(super) last_tick_at: Instant,
    pub(super) last_recovery_generation: u64,
}

impl Default for TunnelSupervisorState {
    fn default() -> Self {
        Self {
            last_tick_at: Instant::now(),
            last_recovery_generation: 0,
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
