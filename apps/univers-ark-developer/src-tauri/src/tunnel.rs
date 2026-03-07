use crate::{
    constants::{TUNNEL_PROBE_INTERVAL, TUNNEL_PROBE_MESSAGE_DELAY, TUNNEL_PROBE_TIMEOUT},
    models::{BrowserSurface, TunnelProcess, TunnelSession, TunnelState, TunnelStatus},
    proxy::{proxy_error_message, start_vite_proxy},
    runtime::{
        allocate_internal_tunnel_port, internal_probe_url, resolve_runtime_vite_hmr_tunnel_command,
        rewrite_tunnel_forward_port, surface_key, surface_local_port,
    },
    terminal::append_output,
};
use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    process::{Command, Stdio},
    sync::{atomic::Ordering, Arc, Mutex},
    time::Instant,
};
use tauri::{AppHandle, Emitter};
use url::Url;

pub(crate) fn tunnel_status(
    target_id: &str,
    surface_id: &str,
    state: &str,
    message: impl Into<String>,
) -> TunnelStatus {
    TunnelStatus {
        target_id: target_id.to_string(),
        surface_id: surface_id.to_string(),
        state: state.to_string(),
        message: message.into(),
    }
}

pub(crate) fn direct_tunnel_status(target_id: &str, surface: &BrowserSurface) -> TunnelStatus {
    tunnel_status(
        target_id,
        &surface.id,
        "direct",
        format!("{} is using the local URL directly.", surface.label),
    )
}

pub(crate) fn starting_tunnel_status(target_id: &str, surface: &BrowserSurface) -> TunnelStatus {
    tunnel_status(
        target_id,
        &surface.id,
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

    for process in &session.processes {
        if let Ok(mut child) = process.child.lock() {
            let _ = child.kill();
        }
    }
}

fn spawn_output_reader<R>(mut reader: R, output: Arc<Mutex<String>>)
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read_count) => {
                    let chunk = String::from_utf8_lossy(&buffer[..read_count]).to_string();
                    append_output(&output, &chunk);
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_tunnel_process(
    command_line: &str,
    label: impl Into<String>,
) -> Result<TunnelProcess, String> {
    let label = label.into();
    let mut command = Command::new("/bin/zsh");
    command.arg("-lc");
    command.arg(format!("exec {}", command_line));
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to start {}: {}", label, error))?;

    let output = Arc::new(Mutex::new(String::new()));

    if let Some(stdout) = child.stdout.take() {
        spawn_output_reader(stdout, output.clone());
    }

    if let Some(stderr) = child.stderr.take() {
        spawn_output_reader(stderr, output.clone());
    }

    Ok(TunnelProcess {
        label,
        child: Arc::new(Mutex::new(child)),
        output,
    })
}

fn stop_tunnel_processes(processes: &[TunnelProcess]) {
    for process in processes {
        if let Ok(mut child) = process.child.lock() {
            let _ = child.kill();
        }
    }
}

fn spawn_managed_tunnel_session(
    app: &AppHandle,
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    session_id: u64,
    target_id: &str,
    surface: &BrowserSurface,
    processes: Vec<TunnelProcess>,
    proxy: Option<crate::models::LocalProxyHandle>,
    probe_urls: Vec<String>,
) -> TunnelSession {
    let started_at = Instant::now();
    let session = TunnelSession {
        session_id,
        started_at,
        processes: processes.clone(),
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
    let session_key = surface_key(&target_id, &surface_id);
    let monitored_processes = processes;
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

            if let Some(error) = monitor_error {
                if remove_tunnel_session_if_current(&monitor_sessions, &session_key, session_id) {
                    let _ = app_handle.emit(
                        "tunnel-status",
                        tunnel_status(&target_id, &surface_id, "error", error),
                    );
                }
                break;
            }

            if let Some((label, success, status)) = exited_process {
                if remove_tunnel_session_if_current(&monitor_sessions, &session_key, session_id) {
                    let message = if success {
                        format!("{} exited.", label)
                    } else if let Some(excerpt) = tunnel_process_excerpt(&monitored_processes) {
                        format!("{} exited with {}. {}", label, status, excerpt)
                    } else {
                        format!("{} exited with {}.", label, status)
                    };

                    let state = if success { "stopped" } else { "error" };
                    let _ = app_handle.emit(
                        "tunnel-status",
                        tunnel_status(&target_id, &surface_id, state, message),
                    );
                }
                break;
            }

            std::thread::sleep(TUNNEL_PROBE_INTERVAL);
        }
    });

    session
}

fn spawn_tunnel_session(
    app: &AppHandle,
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    session_id: u64,
    target_id: &str,
    surface: &BrowserSurface,
) -> Result<TunnelSession, String> {
    let process =
        spawn_tunnel_process(&surface.tunnel_command, format!("{} tunnel", surface.label))?;

    Ok(spawn_managed_tunnel_session(
        app,
        sessions,
        session_id,
        target_id,
        surface,
        vec![process],
        None,
        vec![surface.local_url.clone()],
    ))
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

    let http_tunnel_command =
        rewrite_tunnel_forward_port(&surface.tunnel_command, http_forward_port);
    let hmr_tunnel_command =
        resolve_runtime_vite_hmr_tunnel_command(&surface.vite_hmr_tunnel_command, hmr_forward_port);

    let http_process = spawn_tunnel_process(
        &http_tunnel_command,
        format!("{} HTTP tunnel", surface.label),
    )?;
    let hmr_process =
        match spawn_tunnel_process(&hmr_tunnel_command, format!("{} HMR tunnel", surface.label)) {
            Ok(process) => process,
            Err(error) => {
                stop_tunnel_processes(std::slice::from_ref(&http_process));
                return Err(error);
            }
        };

    let processes = vec![http_process, hmr_process];
    let proxy = match start_vite_proxy(public_port, http_forward_port, hmr_forward_port) {
        Ok(proxy) => proxy,
        Err(error) => {
            stop_tunnel_processes(&processes);
            return Err(error);
        }
    };

    Ok(spawn_managed_tunnel_session(
        app,
        sessions,
        session_id,
        target_id,
        surface,
        processes,
        Some(proxy),
        vec![
            internal_probe_url(http_forward_port),
            internal_probe_url(hmr_forward_port),
        ],
    ))
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
    let session = if !surface.vite_hmr_tunnel_command.trim().is_empty() {
        spawn_vite_proxy_session(
            app,
            tunnel_state.sessions.clone(),
            tunnel_state,
            session_id,
            target_id,
            surface,
        )?
    } else {
        spawn_tunnel_session(
            app,
            tunnel_state.sessions.clone(),
            session_id,
            target_id,
            surface,
        )?
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
