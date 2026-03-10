use crate::{
    config::resolve_target_ssh_chain,
    constants::{TUNNEL_PROBE_INTERVAL, TUNNEL_PROBE_MESSAGE_DELAY, TUNNEL_PROBE_TIMEOUT},
    models::{
        BrowserServiceType, BrowserSurface, RusshTunnelForward, TunnelProcess, TunnelRegistration,
        TunnelSession, TunnelState, TunnelStatus,
    },
    proxy::{proxy_error_message, start_vite_proxy},
    runtime::{
        allocate_internal_tunnel_port, internal_probe_url, resolve_runtime_vite_hmr_tunnel_command,
        resolve_runtime_web_surface, service_key, surface_key, surface_local_port,
    },
    service_registry::emit_tunnel_service_status,
};
use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use univers_ark_russh::{
    start_local_forward_chain, ClientOptions as RusshClientOptions, ResolvedEndpointChain,
};
use url::Url;

const TUNNEL_STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const TUNNEL_SUPERVISOR_INTERVAL: Duration = Duration::from_millis(500);
const TUNNEL_RETRY_INTERVAL: Duration = Duration::from_secs(2);

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

pub(crate) fn sync_desired_tunnels(
    app: &AppHandle,
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

pub(crate) fn reconcile_registered_tunnel(
    app: &AppHandle,
    tunnel_state: &TunnelState,
    target_id: &str,
    service_id: &str,
    emit_status_event: bool,
) -> Result<TunnelStatus, String> {
    let surface = resolve_runtime_web_surface(target_id, service_id, tunnel_state)?;

    if surface.tunnel_command.trim().is_empty() {
        let status = direct_tunnel_status(target_id, &surface);
        if emit_status_event {
            emit_tunnel_service_status(app, &status);
            let _ = app.emit("tunnel-status", status.clone());
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
                        emit_tunnel_service_status(app, &status);
                        let _ = app.emit("tunnel-status", status.clone());
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
                emit_tunnel_service_status(app, &status);
                let _ = app.emit("tunnel-status", status.clone());
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
                emit_tunnel_service_status(app, &status);
                let _ = app.emit("tunnel-status", status);
            }
            Err(error)
        }
    }
}

pub(crate) fn start_tunnel_supervisor(app: AppHandle, tunnel_state: TunnelState) {
    std::thread::spawn(move || loop {
        let due_registrations = tunnel_state
            .desired_tunnels
            .lock()
            .map(|desired| {
                desired
                    .values()
                    .filter(|registration| registration.next_attempt_at <= Instant::now())
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for registration in due_registrations {
            let _ = reconcile_registered_tunnel(
                &app,
                &tunnel_state,
                &registration.target_id,
                &registration.service_id,
                true,
            );
        }

        std::thread::sleep(TUNNEL_SUPERVISOR_INTERVAL);
    });
}

fn resolve_container_chain(target_id: &str) -> Result<ResolvedEndpointChain, String> {
    resolve_target_ssh_chain(target_id)
}

fn parse_forward_target(command_line: &str) -> Result<(String, u16), String> {
    let tokens = command_line.split_whitespace().collect::<Vec<_>>();

    for index in 0..tokens.len() {
        let forward_spec = if tokens[index] == "-L" {
            tokens.get(index + 1).copied()
        } else {
            tokens[index].strip_prefix("-L")
        };

        let Some(forward_spec) = forward_spec else {
            continue;
        };

        let Some((before_port, remote_port)) = forward_spec.rsplit_once(':') else {
            continue;
        };
        let remote_port = remote_port.parse::<u16>().map_err(|error| {
            format!("Invalid remote forward port in {}: {}", forward_spec, error)
        })?;
        let Some(remote_host) = before_port.rsplit(':').next() else {
            continue;
        };

        return Ok((remote_host.to_string(), remote_port));
    }

    Err(format!(
        "Failed to parse -L forward target from tunnel command: {}",
        command_line
    ))
}

fn spawn_russh_forward(
    target_id: &str,
    local_bind_addr: &str,
    remote_host: &str,
    remote_port: u16,
    label: impl Into<String>,
) -> Result<RusshTunnelForward, String> {
    let chain = resolve_container_chain(target_id)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Failed to build russh runtime: {}", error))?;
    let forward = runtime
        .block_on(start_local_forward_chain(
            &chain,
            local_bind_addr,
            remote_host,
            remote_port,
            &RusshClientOptions::default(),
        ))
        .map_err(|error| format!("Failed to start russh forward: {}", error))?;

    Ok(RusshTunnelForward {
        label: label.into(),
        forward,
    })
}

fn spawn_managed_tunnel_session(
    app: &AppHandle,
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
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
                    let _ = app_handle.emit(
                        "tunnel-status",
                        tunnel_status(
                            &target_id,
                            &surface_id,
                            Some(local_url.clone()),
                            "running",
                            format!(
                                "{} is forwarding browser traffic to {}.",
                                surface_label, local_url
                            ),
                        ),
                    );
                } else if !waiting_message_emitted
                    && started_at.elapsed() >= TUNNEL_PROBE_MESSAGE_DELAY
                {
                    let _ = app_handle.emit(
                        "tunnel-status",
                        tunnel_status(
                            &target_id,
                            &surface_id,
                            Some(local_url.clone()),
                            "starting",
                            format!(
                                "{} tunnel is up, waiting for {} to accept connections.",
                                surface_label, local_url
                            ),
                        ),
                    );
                    waiting_message_emitted = true;
                }
            }

            if let Some(proxy) = &monitored_proxy {
                if !proxy.running.load(Ordering::Acquire) {
                    if remove_tunnel_session_if_current(&monitor_sessions, &session_key, session_id)
                    {
                        let _ = app_handle.emit(
                            "tunnel-status",
                            tunnel_status(
                                &target_id,
                                &surface_id,
                                Some(local_url.clone()),
                                "error",
                                proxy_error_message(proxy).unwrap_or_else(|| {
                                    format!("{} proxy exited unexpectedly.", surface_label)
                                }),
                            ),
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
                    let _ = app_handle.emit(
                        "tunnel-status",
                        tunnel_status(
                            &target_id,
                            &surface_id,
                            Some(local_url.clone()),
                            "error",
                            error,
                        ),
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
                    let _ = app_handle.emit(
                        "tunnel-status",
                        tunnel_status(
                            &target_id,
                            &surface_id,
                            Some(local_url.clone()),
                            state,
                            message,
                        ),
                    );
                }
                break;
            }

            std::thread::sleep(TUNNEL_PROBE_INTERVAL);
        }
    });

    session
}

fn spawn_vite_proxy_session(
    app: &AppHandle,
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    tunnel_state: &TunnelState,
    session_id: u64,
    target_id: &str,
    surface: &BrowserSurface,
) -> Result<TunnelSession, String> {
    let public_port = surface_local_port(surface)?;
    let http_forward_port =
        allocate_internal_tunnel_port(tunnel_state, target_id, &surface.id, "http-forward")?;
    let hmr_forward_port =
        allocate_internal_tunnel_port(tunnel_state, target_id, &surface.id, "vite-hmr")?;

    let remote_url = Url::parse(&surface.remote_url).map_err(|error| {
        format!(
            "Failed to parse remote URL for {} surface: {}",
            surface.id, error
        )
    })?;
    let remote_host = remote_url
        .host_str()
        .ok_or_else(|| format!("Remote URL for {} surface is missing a host", surface.id))?;
    let remote_port = remote_url
        .port_or_known_default()
        .ok_or_else(|| format!("Remote URL for {} surface is missing a port", surface.id))?;
    let (_, hmr_remote_port) = parse_forward_target(&resolve_runtime_vite_hmr_tunnel_command(
        &surface.vite_hmr_tunnel_command,
        hmr_forward_port,
    ))?;

    let local_http_bind = format!("127.0.0.1:{}", http_forward_port);
    let http_forward = spawn_russh_forward(
        target_id,
        &local_http_bind,
        remote_host,
        remote_port,
        format!("{} HTTP tunnel", surface.label),
    )?;
    let local_hmr_bind = format!("127.0.0.1:{}", hmr_forward_port);
    let hmr_forward = match spawn_russh_forward(
        target_id,
        &local_hmr_bind,
        "127.0.0.1",
        hmr_remote_port,
        format!("{} HMR tunnel", surface.label),
    ) {
        Ok(forward) => forward,
        Err(error) => {
            http_forward.forward.request_stop();
            return Err(error);
        }
    };

    let russh_forwards = vec![http_forward, hmr_forward];
    let proxy = match start_vite_proxy(public_port, http_forward_port, hmr_forward_port) {
        Ok(proxy) => proxy,
        Err(error) => {
            for forward in &russh_forwards {
                forward.forward.request_stop();
            }
            return Err(error);
        }
    };

    Ok(spawn_managed_tunnel_session(
        app,
        sessions,
        session_id,
        target_id,
        surface,
        Vec::new(),
        russh_forwards,
        Some(proxy),
        vec![
            internal_probe_url(http_forward_port),
            internal_probe_url(hmr_forward_port),
        ],
    ))
}

fn uses_vite_forwarding(surface: &BrowserSurface) -> bool {
    matches!(surface.service_type, BrowserServiceType::Vite)
        || !surface.vite_hmr_tunnel_command.trim().is_empty()
}

pub(crate) fn start_tunnel(
    app: &AppHandle,
    tunnel_state: &TunnelState,
    target_id: &str,
    surface: &BrowserSurface,
) -> Result<TunnelStatus, String> {
    let session_id = tunnel_state
        .next_session_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let session = if uses_vite_forwarding(surface) {
        spawn_vite_proxy_session(
            app,
            tunnel_state.sessions.clone(),
            tunnel_state,
            session_id,
            target_id,
            surface,
        )?
    } else {
        let local_port = surface_local_port(surface)?;
        let remote_url = Url::parse(&surface.remote_url).map_err(|error| {
            format!(
                "Failed to parse remote URL for {} surface: {}",
                surface.id, error
            )
        })?;
        let remote_host = remote_url
            .host_str()
            .ok_or_else(|| format!("Remote URL for {} surface is missing a host", surface.id))?;
        let remote_port = remote_url
            .port_or_known_default()
            .ok_or_else(|| format!("Remote URL for {} surface is missing a port", surface.id))?;
        let local_bind_addr = format!("127.0.0.1:{}", local_port);
        let forward = spawn_russh_forward(
            target_id,
            &local_bind_addr,
            remote_host,
            remote_port,
            format!("{} tunnel", surface.label),
        )?;

        spawn_managed_tunnel_session(
            app,
            tunnel_state.sessions.clone(),
            session_id,
            target_id,
            surface,
            Vec::new(),
            vec![forward],
            None,
            vec![surface.local_url.clone()],
        )
    };

    tunnel_state
        .sessions
        .lock()
        .map_err(|_| String::from("Tunnel session state is unavailable"))?
        .insert(surface_key(target_id, &surface.id), session);

    Ok(starting_tunnel_status(target_id, surface))
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
