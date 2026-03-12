use super::super::{
    forwarding::{should_manage_runtime_surface_tunnel, start_tunnel},
    registry::schedule_tunnel_retry,
    session::{stop_tunnel_session, tunnel_session_is_alive},
    status::{
        active_tunnel_status, direct_tunnel_status, emit_tunnel_status_updates, tunnel_status,
    },
};
use crate::{
    models::{TunnelState, TunnelStatus},
    services::runtime::{resolve_runtime_web_surface, service_key},
};
use tauri::{AppHandle, Runtime};

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
