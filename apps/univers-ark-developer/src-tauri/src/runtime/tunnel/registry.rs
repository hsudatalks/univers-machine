use super::{
    session::stop_tunnel_session, status::tunnel_status, supervisor::reconcile_registered_tunnel,
    TUNNEL_RETRY_INTERVAL,
};
use crate::{
    models::{TunnelRegistration, TunnelState, TunnelStatus},
    services::runtime::{resolve_runtime_web_surface, service_key},
};
use std::{collections::HashMap, time::Instant};
use tauri::{AppHandle, Runtime};

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

pub(super) fn schedule_tunnel_retry(tunnel_state: &TunnelState, target_id: &str, service_id: &str) {
    let key = service_key(target_id, service_id);
    if let Ok(mut desired) = tunnel_state.desired_tunnels.lock() {
        if let Some(registration) = desired.get_mut(&key) {
            registration.next_attempt_at = Instant::now() + TUNNEL_RETRY_INTERVAL;
        }
    }
}
