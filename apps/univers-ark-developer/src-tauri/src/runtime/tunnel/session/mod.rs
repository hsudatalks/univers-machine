mod lifecycle;
mod probe;

use super::{
    status::{emit_tunnel_status_updates, tunnel_status},
    TUNNEL_PROBE_INTERVAL, TUNNEL_PROBE_MESSAGE_DELAY, TUNNEL_READY_PROBE_INTERVAL,
};
use crate::{
    models::{
        BrowserSurface, LocalProxyHandle, RusshTunnelForward, TunnelProcess, TunnelSession,
    },
    services::runtime::service_key,
};
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc, Mutex},
    time::Instant,
};
use tauri::{AppHandle, Runtime};

pub(crate) use self::lifecycle::{
    remove_tunnel_session_if_current, stop_tunnel_session, tunnel_session_is_alive,
};
use self::{
    lifecycle::{russh_forward_excerpt, tunnel_process_excerpt, tunnel_session_is_current},
    probe::probe_targets_ready,
};
use super::proxy::proxy_error_message;

pub(super) struct ManagedTunnelSessionSpec {
    pub(super) session_id: u64,
    pub(super) target_id: String,
    pub(super) surface: BrowserSurface,
    pub(super) processes: Vec<TunnelProcess>,
    pub(super) russh_forwards: Vec<RusshTunnelForward>,
    pub(super) proxy: Option<LocalProxyHandle>,
    pub(super) probe_urls: Vec<String>,
}

pub(super) fn spawn_managed_tunnel_session<R: Runtime>(
    app: &AppHandle<R>,
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    status_snapshots: Arc<Mutex<HashMap<String, crate::models::TunnelStatus>>>,
    telemetry: Arc<Mutex<crate::models::TunnelTelemetry>>,
    spec: ManagedTunnelSessionSpec,
) -> TunnelSession {
    let started_at = Instant::now();
    let session = TunnelSession {
        session_id: spec.session_id,
        started_at,
        processes: spec.processes.clone(),
        russh_forwards: spec.russh_forwards.clone(),
        proxy: spec.proxy.clone(),
        ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    let app_handle = app.clone();
    let ready_flag = session.ready.clone();
    let monitor_sessions = sessions.clone();
    let target_id = spec.target_id;
    let surface_id = spec.surface.id;
    let surface_label = spec.surface.label;
    let local_url = spec.surface.local_url;
    let session_key = service_key(&target_id, &surface_id);
    let monitored_processes = spec.processes;
    let monitored_forwards = spec.russh_forwards;
    let monitored_proxy = spec.proxy;
    let probe_targets = spec.probe_urls;
    let session_id = session.session_id;

    std::thread::spawn(move || {
        let mut waiting_message_emitted = false;

        loop {
            if tunnel_session_is_current(&monitor_sessions, &session_key, session_id)
                && !ready_flag.load(Ordering::Acquire)
            {
                if probe_targets_ready(&probe_targets) {
                    ready_flag.store(true, Ordering::Release);
                    emit_tunnel_status_updates(
                        &app_handle,
                        &status_snapshots,
                        &telemetry,
                        [tunnel_status(
                            &target_id,
                            &surface_id,
                            Some(local_url.clone()),
                            "running",
                            format!(
                                "{surface_label} is forwarding browser traffic to {local_url}."
                            ),
                        )],
                    );
                } else if !waiting_message_emitted
                    && started_at.elapsed() >= TUNNEL_PROBE_MESSAGE_DELAY
                {
                    emit_tunnel_status_updates(
                        &app_handle,
                        &status_snapshots,
                        &telemetry,
                        [tunnel_status(
                            &target_id,
                            &surface_id,
                            Some(local_url.clone()),
                            "starting",
                            format!(
                                "{surface_label} tunnel is up, waiting for {local_url} to accept connections."
                            ),
                        )],
                    );
                    waiting_message_emitted = true;
                }
            }

            if let Some(proxy) = &monitored_proxy {
                if !proxy.running.load(Ordering::Acquire) {
                    if remove_tunnel_session_if_current(&monitor_sessions, &session_key, session_id)
                    {
                        emit_tunnel_status_updates(
                            &app_handle,
                            &status_snapshots,
                            &telemetry,
                            [tunnel_status(
                                &target_id,
                                &surface_id,
                                Some(local_url.clone()),
                                "error",
                                proxy_error_message(proxy).unwrap_or_else(|| {
                                    format!("{surface_label} proxy exited unexpectedly.")
                                }),
                            )],
                        );
                    }
                    break;
                }
            }

            let mut exited_process = None;
            let mut monitor_error = None;

            for process in &monitored_processes {
                let try_wait_result = {
                    let Ok(mut child) = process.child.lock() else {
                        monitor_error = Some(format!(
                            "{} process lock was lost before startup completed.",
                            process.label
                        ));
                        break;
                    };

                    child.try_wait()
                };

                match try_wait_result {
                    Ok(Some(status)) => {
                        exited_process = Some((process.label.clone(), status.success(), status));
                        break;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        monitor_error = Some(format!(
                            "Failed to monitor {} process: {}",
                            process.label, error
                        ));
                        break;
                    }
                }
            }

            if monitor_error.is_none() {
                for forward in &monitored_forwards {
                    if !forward.forward.is_running() {
                        monitor_error =
                            Some(forward.forward.last_error().unwrap_or_else(|| {
                                format!("{} stopped unexpectedly.", forward.label)
                            }));
                        break;
                    }
                }
            }

            if let Some(error) = monitor_error {
                if remove_tunnel_session_if_current(&monitor_sessions, &session_key, session_id) {
                    emit_tunnel_status_updates(
                        &app_handle,
                        &status_snapshots,
                        &telemetry,
                        [tunnel_status(
                            &target_id,
                            &surface_id,
                            Some(local_url.clone()),
                            "error",
                            error,
                        )],
                    );
                }
                break;
            }

            if let Some((label, success, status)) = exited_process {
                if remove_tunnel_session_if_current(&monitor_sessions, &session_key, session_id) {
                    let message = if success {
                        format!("{label} exited.")
                    } else if let Some(excerpt) = tunnel_process_excerpt(&monitored_processes)
                        .or_else(|| russh_forward_excerpt(&monitored_forwards))
                    {
                        format!("{label} exited with {status}. {excerpt}")
                    } else {
                        format!("{label} exited with {status}.")
                    };

                    let state = if success { "stopped" } else { "error" };
                    emit_tunnel_status_updates(
                        &app_handle,
                        &status_snapshots,
                        &telemetry,
                        [tunnel_status(
                            &target_id,
                            &surface_id,
                            Some(local_url.clone()),
                            state,
                            message,
                        )],
                    );
                }
                break;
            }

            let sleep_interval = if ready_flag.load(Ordering::Acquire) {
                TUNNEL_READY_PROBE_INTERVAL
            } else {
                TUNNEL_PROBE_INTERVAL
            };
            std::thread::sleep(sleep_interval);
        }
    });

    session
}
