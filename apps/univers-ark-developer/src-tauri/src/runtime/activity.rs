use crate::models::RuntimeActivityState;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) const RUNTIME_SUSPEND_GAP_THRESHOLD: Duration = Duration::from_secs(15);
pub(crate) const RUNTIME_RECOVERY_GRACE_PERIOD: Duration = Duration::from_secs(12);
pub(crate) const RUNTIME_BACKGROUND_MONITOR_INTERVAL: Duration = Duration::from_secs(8);
pub(crate) const RUNTIME_BACKGROUND_SUPERVISOR_FLOOR: Duration = Duration::from_secs(5);
pub(crate) const RUNTIME_BACKGROUND_DASHBOARD_REFRESH_SECS: u64 = 60;

#[derive(Clone, Debug)]
pub(crate) struct RuntimeActivitySnapshot {
    pub(crate) visible: bool,
    pub(crate) focused: bool,
    pub(crate) online: bool,
    pub(crate) recovering: bool,
    pub(crate) recovery_generation: u64,
    pub(crate) active_machine_id: Option<String>,
    pub(crate) active_target_id: Option<String>,
}

impl RuntimeActivitySnapshot {
    pub(crate) fn is_foreground(&self) -> bool {
        self.visible || self.focused
    }
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn current_runtime_activity(
    activity_state: &RuntimeActivityState,
) -> RuntimeActivitySnapshot {
    let recovering_until_ms = activity_state.recovering_until_ms.load(Ordering::Acquire);

    RuntimeActivitySnapshot {
        visible: activity_state.visible.load(Ordering::Acquire),
        focused: activity_state.focused.load(Ordering::Acquire),
        online: activity_state.online.load(Ordering::Acquire),
        recovering: recovering_until_ms > now_epoch_ms(),
        recovery_generation: activity_state.recovery_generation.load(Ordering::Acquire),
        active_machine_id: activity_state
            .active_machine_id
            .lock()
            .ok()
            .and_then(|value| value.clone()),
        active_target_id: activity_state
            .active_target_id
            .lock()
            .ok()
            .and_then(|value| value.clone()),
    }
}

pub(crate) fn mark_runtime_recovery(
    activity_state: &RuntimeActivityState,
    duration: Duration,
) -> u64 {
    let now_ms = now_epoch_ms();
    let recovering_until_ms = now_ms.saturating_add(duration.as_millis() as u64);
    activity_state
        .recovering_until_ms
        .store(recovering_until_ms, Ordering::Release);
    activity_state
        .last_recovery_started_at_ms
        .store(now_ms, Ordering::Release);
    activity_state
        .recovery_generation
        .fetch_add(1, Ordering::AcqRel)
        .saturating_add(1)
}

pub(crate) fn update_runtime_activity(
    activity_state: &RuntimeActivityState,
    visible: bool,
    focused: bool,
    online: bool,
    active_machine_id: Option<String>,
    active_target_id: Option<String>,
) {
    let previous_visible = activity_state.visible.swap(visible, Ordering::AcqRel);
    let previous_focused = activity_state.focused.swap(focused, Ordering::AcqRel);
    let previous_online = activity_state.online.swap(online, Ordering::AcqRel);
    if let Ok(mut value) = activity_state.active_machine_id.lock() {
        *value = active_machine_id;
    }
    if let Ok(mut value) = activity_state.active_target_id.lock() {
        *value = active_target_id;
    }

    if (!previous_visible && visible)
        || (!previous_focused && focused)
        || (!previous_online && online)
    {
        mark_runtime_recovery(activity_state, RUNTIME_RECOVERY_GRACE_PERIOD);
    }
}

pub(crate) fn detect_runtime_suspend_gap(
    activity_state: &RuntimeActivityState,
    last_tick_at: &mut Instant,
    now: Instant,
) -> bool {
    let elapsed = now.saturating_duration_since(*last_tick_at);
    *last_tick_at = now;

    if elapsed >= RUNTIME_SUSPEND_GAP_THRESHOLD {
        mark_runtime_recovery(activity_state, RUNTIME_RECOVERY_GRACE_PERIOD);
        return true;
    }

    false
}
