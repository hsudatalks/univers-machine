use super::TUNNEL_PROBE_MESSAGE_DELAY;
use crate::{
    models::{BrowserSurface, TunnelSession, TunnelStatus},
    services::{registry::emit_tunnel_service_status, runtime::service_key},
};
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc, Mutex},
    time::Instant,
};
use tauri::{AppHandle, Emitter, Runtime};

pub(crate) const TUNNEL_STATUS_BATCH_EVENT: &str = "tunnel-status-batch";

pub(crate) fn tunnel_status(
    target_id: &str,
    service_id: &str,
    local_url: Option<String>,
    state: &str,
    message: impl Into<String>,
) -> TunnelStatus {
    TunnelStatus {
        target_id: target_id.to_string(),
        service_id: service_id.to_string(),
        surface_id: service_id.to_string(),
        local_url,
        state: state.to_string(),
        message: message.into(),
    }
}

pub(crate) fn direct_tunnel_status(target_id: &str, surface: &BrowserSurface) -> TunnelStatus {
    tunnel_status(
        target_id,
        &surface.id,
        Some(surface.local_url.clone()),
        "direct",
        format!("{} is using the local URL directly.", surface.label),
    )
}

pub(crate) fn starting_tunnel_status(target_id: &str, surface: &BrowserSurface) -> TunnelStatus {
    tunnel_status(
        target_id,
        &surface.id,
        Some(surface.local_url.clone()),
        "starting",
        format!(
            "Starting the {} tunnel and probing {} for readiness.",
            surface.label.to_lowercase(),
            surface.local_url
        ),
    )
}

pub(crate) fn running_tunnel_status(target_id: &str, surface: &BrowserSurface) -> TunnelStatus {
    tunnel_status(
        target_id,
        &surface.id,
        Some(surface.local_url.clone()),
        "running",
        format!(
            "{} is forwarding browser traffic to {}.",
            surface.label, surface.local_url
        ),
    )
}

pub(crate) fn active_tunnel_status(
    target_id: &str,
    surface: &BrowserSurface,
    session: &TunnelSession,
) -> TunnelStatus {
    if session.ready.load(Ordering::Acquire) {
        return running_tunnel_status(target_id, surface);
    }

    if session.started_at.elapsed() >= TUNNEL_PROBE_MESSAGE_DELAY {
        return tunnel_status(
            target_id,
            &surface.id,
            Some(surface.local_url.clone()),
            "starting",
            format!(
                "{} tunnel is up, waiting for {} to accept connections.",
                surface.label, surface.local_url
            ),
        );
    }

    starting_tunnel_status(target_id, surface)
}

fn tunnel_status_changed(current: Option<&TunnelStatus>, next: &TunnelStatus) -> bool {
    current
        .map(|status| {
            status.local_url != next.local_url
                || status.state != next.state
                || status.message != next.message
        })
        .unwrap_or(true)
}

pub(crate) fn emit_tunnel_status_updates<R: Runtime>(
    app: &AppHandle<R>,
    status_snapshots: &Arc<Mutex<HashMap<String, TunnelStatus>>>,
    telemetry: &Arc<Mutex<crate::models::TunnelTelemetry>>,
    statuses: impl IntoIterator<Item = TunnelStatus>,
) {
    let candidates = statuses.into_iter().collect::<Vec<_>>();
    if candidates.is_empty() {
        return;
    }

    let changed = if let Ok(mut snapshots) = status_snapshots.lock() {
        let mut changed = Vec::new();

        for status in candidates {
            let key = service_key(&status.target_id, &status.service_id);
            if tunnel_status_changed(snapshots.get(&key), &status) {
                snapshots.insert(key, status.clone());
                changed.push(status);
            }
        }

        changed
    } else {
        candidates
    };

    if changed.is_empty() {
        return;
    }

    if let Ok(mut value) = telemetry.lock() {
        let now = Instant::now();
        value.status_events.record(now, 1);
        value.status_items.record(now, changed.len());
    }

    for status in &changed {
        emit_tunnel_service_status(app, status);
    }

    let _ = app.emit(TUNNEL_STATUS_BATCH_EVENT, changed);
}
