use super::{
    status::{emit_tunnel_status_updates, tunnel_status},
    TUNNEL_PROBE_INTERVAL, TUNNEL_PROBE_MESSAGE_DELAY, TUNNEL_PROBE_TIMEOUT,
    TUNNEL_READY_PROBE_INTERVAL, TUNNEL_STOP_WAIT_TIMEOUT,
};
use crate::{
    models::{BrowserSurface, RusshTunnelForward, TunnelProcess, TunnelSession},
    services::runtime::service_key,
};
use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::{atomic::Ordering, Arc, Mutex},
    time::Instant,
};
use tauri::{AppHandle, Runtime};
use url::Url;

use super::proxy::proxy_error_message;

fn tunnel_output_excerpt(output: &Arc<Mutex<String>>) -> Option<String> {
    let current_output = output.lock().ok()?;
    let line = current_output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)?;

    if line.is_empty() {
        return None;
    }

    Some(line.chars().take(220).collect())
}

fn tunnel_process_excerpt(processes: &[TunnelProcess]) -> Option<String> {
    processes.iter().find_map(|process| {
        tunnel_output_excerpt(&process.output)
            .map(|excerpt| format!("{}: {}", process.label, excerpt))
    })
}

fn russh_forward_excerpt(forwards: &[RusshTunnelForward]) -> Option<String> {
    forwards.iter().find_map(|forward| {
        forward
            .forward
            .last_error()
            .map(|error| format!("{}: {}", forward.label, error))
    })
}

fn tunnel_process_is_alive(process: &TunnelProcess) -> Result<bool, String> {
    let mut child = process
        .child
        .lock()
        .map_err(|_| format!("{} process is unavailable", process.label))?;

    child
        .try_wait()
        .map(|status| status.is_none())
        .map_err(|error| format!("Failed to inspect {} process: {}", process.label, error))
}

pub(crate) fn tunnel_session_is_alive(session: &TunnelSession) -> Result<bool, String> {
    for process in &session.processes {
        if !tunnel_process_is_alive(process)? {
            return Ok(false);
        }
    }

    for forward in &session.russh_forwards {
        if !forward.forward.is_running() {
            return Ok(false);
        }
    }

    if let Some(proxy) = &session.proxy {
        if !proxy.running.load(Ordering::Acquire) {
            return Ok(false);
        }
    }

    Ok(true)
}

fn browser_probe_path(url: &Url) -> String {
    let mut path = url.path().to_string();

    if path.is_empty() {
        path.push('/');
    }

    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }

    path
}

fn browser_probe_host_header(url: &Url) -> Option<String> {
    let host = url.host_str()?;

    match url.port() {
        Some(port) => Some(format!("{}:{}", host, port)),
        None => Some(host.to_string()),
    }
}

fn browser_probe_addrs(url: &Url) -> Vec<std::net::SocketAddr> {
    let Some(host) = url.host_str() else {
        return Vec::new();
    };

    let Some(port) = url.port_or_known_default() else {
        return Vec::new();
    };

    (host, port)
        .to_socket_addrs()
        .map(|addrs| addrs.collect())
        .unwrap_or_default()
}

fn probe_browser_http(url: &Url) -> bool {
    if url.scheme() != "http" {
        return false;
    }

    let host_header = match browser_probe_host_header(url) {
        Some(value) => value,
        None => return false,
    };

    let request = format!(
        "HEAD {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        browser_probe_path(url),
        host_header
    );

    for addr in browser_probe_addrs(url) {
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, TUNNEL_PROBE_TIMEOUT) else {
            continue;
        };

        let _ = stream.set_read_timeout(Some(TUNNEL_PROBE_TIMEOUT));
        let _ = stream.set_write_timeout(Some(TUNNEL_PROBE_TIMEOUT));

        if stream.write_all(request.as_bytes()).is_err() {
            continue;
        }

        let mut buffer = [0u8; 64];

        match stream.read(&mut buffer) {
            Ok(read_count) if read_count > 0 => {
                let response = String::from_utf8_lossy(&buffer[..read_count]);
                if response.starts_with("HTTP/") {
                    return true;
                }
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::UnexpectedEof
                ) => {}
            Err(_) => {}
        }
    }

    false
}

fn probe_browser_tcp(url: &Url) -> bool {
    browser_probe_addrs(url)
        .into_iter()
        .any(|addr| TcpStream::connect_timeout(&addr, TUNNEL_PROBE_TIMEOUT).is_ok())
}

fn probe_browser_ready(local_url: &str) -> bool {
    let Ok(url) = Url::parse(local_url) else {
        return false;
    };

    if probe_browser_http(&url) {
        return true;
    }

    probe_browser_tcp(&url)
}

fn probe_targets_ready(probe_urls: &[String]) -> bool {
    !probe_urls.is_empty() && probe_urls.iter().all(|url| probe_browser_ready(url))
}

fn tunnel_session_is_current(
    sessions: &Arc<Mutex<HashMap<String, TunnelSession>>>,
    key: &str,
    session_id: u64,
) -> bool {
    sessions
        .lock()
        .ok()
        .and_then(|active| {
            active
                .get(key)
                .map(|session| session.session_id == session_id)
        })
        .unwrap_or(false)
}

pub(crate) fn remove_tunnel_session_if_current(
    sessions: &Arc<Mutex<HashMap<String, TunnelSession>>>,
    key: &str,
    session_id: u64,
) -> bool {
    let Ok(mut active_sessions) = sessions.lock() else {
        return false;
    };

    match active_sessions.get(key) {
        Some(session) if session.session_id == session_id => {
            active_sessions.remove(key);
            true
        }
        _ => false,
    }
}

pub(crate) fn stop_tunnel_session(session: &TunnelSession) {
    if let Some(proxy) = &session.proxy {
        proxy.stop_requested.store(true, Ordering::Release);
    }

    for forward in &session.russh_forwards {
        forward.forward.request_stop();
    }

    for forward in &session.russh_forwards {
        let _ = forward.forward.wait_stopped(TUNNEL_STOP_WAIT_TIMEOUT);
    }

    for process in &session.processes {
        if let Ok(mut child) = process.child.lock() {
            let _ = child.kill();
        }
    }
}

pub(super) fn spawn_managed_tunnel_session<R: Runtime>(
    app: &AppHandle<R>,
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    status_snapshots: Arc<Mutex<HashMap<String, crate::models::TunnelStatus>>>,
    telemetry: Arc<Mutex<crate::models::TunnelTelemetry>>,
    session_id: u64,
    target_id: &str,
    surface: &BrowserSurface,
    processes: Vec<TunnelProcess>,
    russh_forwards: Vec<RusshTunnelForward>,
    proxy: Option<crate::models::LocalProxyHandle>,
    probe_urls: Vec<String>,
) -> TunnelSession {
    let started_at = Instant::now();
    let session = TunnelSession {
        session_id,
        started_at,
        processes: processes.clone(),
        russh_forwards: russh_forwards.clone(),
        proxy: proxy.clone(),
        ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    let app_handle = app.clone();
    let ready_flag = session.ready.clone();
    let monitor_sessions = sessions.clone();
    let target_id = target_id.to_string();
    let surface_id = surface.id.clone();
    let surface_label = surface.label.clone();
    let local_url = surface.local_url.clone();
    let session_key = service_key(&target_id, &surface_id);
    let monitored_processes = processes;
    let monitored_forwards = russh_forwards;
    let monitored_proxy = proxy;
    let probe_targets = probe_urls;

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
                                "{} is forwarding browser traffic to {}.",
                                surface_label, local_url
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
                                "{} tunnel is up, waiting for {} to accept connections.",
                                surface_label, local_url
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
                                    format!("{} proxy exited unexpectedly.", surface_label)
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
                        format!("{} exited.", label)
                    } else if let Some(excerpt) = tunnel_process_excerpt(&monitored_processes)
                        .or_else(|| russh_forward_excerpt(&monitored_forwards))
                    {
                        format!("{} exited with {}. {}", label, status, excerpt)
                    } else {
                        format!("{} exited with {}.", label, status)
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
