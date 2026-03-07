#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use url::Url;

const OUTPUT_BUFFER_LIMIT: usize = 128 * 1024;
const SURFACE_PORT_START: u16 = 43000;
const SURFACE_PORT_END: u16 = 43999;
const INTERNAL_TUNNEL_PORT_START: u16 = 44000;
const INTERNAL_TUNNEL_PORT_END: u16 = 44999;
const SURFACE_HOST: &str = "127.0.0.1";
const TUNNEL_PROBE_INTERVAL: Duration = Duration::from_millis(180);
const TUNNEL_PROBE_TIMEOUT: Duration = Duration::from_millis(700);
const TUNNEL_PROBE_MESSAGE_DELAY: Duration = Duration::from_secs(2);
const PROXY_ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(60);
const PROXY_CONNECT_TIMEOUT: Duration = Duration::from_millis(900);
const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserSurface {
    id: String,
    label: String,
    tunnel_command: String,
    local_url: String,
    remote_url: String,
    #[serde(default)]
    vite_hmr_tunnel_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeveloperTarget {
    id: String,
    label: String,
    host: String,
    description: String,
    terminal_command: String,
    #[serde(default)]
    notes: Vec<String>,
    surfaces: Vec<BrowserSurface>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetsFile {
    selected_target_id: Option<String>,
    targets: Vec<DeveloperTarget>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppBootstrap {
    app_name: String,
    config_path: String,
    selected_target_id: Option<String>,
    targets: Vec<DeveloperTarget>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalSnapshot {
    target_id: String,
    output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalOutputEvent {
    target_id: String,
    data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalExitEvent {
    target_id: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelStatus {
    target_id: String,
    surface_id: String,
    state: String,
    message: String,
}

#[derive(Clone)]
struct TerminalSession {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    output: Arc<Mutex<String>>,
}

#[derive(Clone)]
struct TunnelProcess {
    label: String,
    child: Arc<Mutex<Child>>,
    output: Arc<Mutex<String>>,
}

#[derive(Clone)]
struct LocalProxyHandle {
    stop_requested: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
struct TunnelSession {
    session_id: u64,
    started_at: Instant,
    processes: Vec<TunnelProcess>,
    proxy: Option<LocalProxyHandle>,
    ready: Arc<AtomicBool>,
}

#[derive(Clone, Default)]
struct TerminalState {
    sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
}

struct TunnelState {
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    local_ports: Arc<Mutex<HashMap<String, u16>>>,
    next_session_id: AtomicU64,
}

impl Default for TunnelState {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            local_ports: Arc::new(Mutex::new(HashMap::new())),
            next_session_id: AtomicU64::new(1),
        }
    }
}

fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn targets_file_path() -> PathBuf {
    app_root().join("developer-targets.json")
}

fn read_targets_file() -> Result<TargetsFile, String> {
    let config_path = targets_file_path();
    let content = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;

    serde_json::from_str::<TargetsFile>(&content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))
}

fn resolve_raw_target(target_id: &str) -> Result<DeveloperTarget, String> {
    let targets_file = read_targets_file()?;

    targets_file
        .targets
        .into_iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| format!("Unknown target: {}", target_id))
}

fn surface_key(target_id: &str, surface_id: &str) -> String {
    format!("{}::{}", target_id, surface_id)
}

fn tunnel_port_key(target_id: &str, surface_id: &str, suffix: &str) -> String {
    format!("{}::{}::{}", target_id, surface_id, suffix)
}

fn port_span(start: u16, end: u16) -> usize {
    usize::from(end - start) + 1
}

fn stable_port_offset(key: &str, start: u16, end: u16) -> usize {
    let mut hash = 2_166_136_261_u32;

    for byte in key.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }

    usize::try_from(hash).unwrap_or(0) % port_span(start, end)
}

fn port_is_available(port: u16) -> bool {
    TcpListener::bind((SURFACE_HOST, port)).is_ok()
}

fn allocate_stable_port(
    tunnel_state: &TunnelState,
    key: &str,
    start: u16,
    end: u16,
) -> Result<u16, String> {
    let mut local_ports = tunnel_state
        .local_ports
        .lock()
        .map_err(|_| String::from("Surface port state is unavailable"))?;

    if let Some(port) = local_ports.get(key).copied() {
        return Ok(port);
    }

    let span = port_span(start, end);
    let start_offset = stable_port_offset(key, start, end);

    for step in 0..span {
        let candidate = start + ((start_offset + step) % span) as u16;

        if local_ports.values().any(|assigned| *assigned == candidate) {
            continue;
        }

        if port_is_available(candidate) {
            local_ports.insert(key.to_string(), candidate);
            return Ok(candidate);
        }
    }

    Err(format!(
        "No free browser surface ports available in {}-{}.",
        start, end
    ))
}

fn allocate_surface_port(
    tunnel_state: &TunnelState,
    target_id: &str,
    surface_id: &str,
) -> Result<u16, String> {
    allocate_stable_port(
        tunnel_state,
        &surface_key(target_id, surface_id),
        SURFACE_PORT_START,
        SURFACE_PORT_END,
    )
}

fn allocate_internal_tunnel_port(
    tunnel_state: &TunnelState,
    target_id: &str,
    surface_id: &str,
    suffix: &str,
) -> Result<u16, String> {
    allocate_stable_port(
        tunnel_state,
        &tunnel_port_key(target_id, surface_id, suffix),
        INTERNAL_TUNNEL_PORT_START,
        INTERNAL_TUNNEL_PORT_END,
    )
}

fn replace_known_tunnel_placeholders(
    tunnel_command: &str,
    remote_url: &str,
    local_port: u16,
) -> String {
    let mut resolved = tunnel_command.replace("{localPort}", &local_port.to_string());

    if let Ok(remote_url) = Url::parse(remote_url) {
        if let Some(host) = remote_url.host_str() {
            resolved = resolved.replace("{remoteHost}", host);
            resolved = resolved.replace("{previewRemoteHost}", host);
        }

        if let Some(port) = remote_url.port_or_known_default() {
            resolved = resolved.replace("{remotePort}", &port.to_string());
            resolved = resolved.replace("{previewRemotePort}", &port.to_string());
        }
    }

    resolved
}

fn rewrite_forward_spec_local_port(forward_spec: &str, local_port: u16) -> String {
    match forward_spec.split_once(':') {
        Some((_, rest)) => format!("{}:{}", local_port, rest),
        None => forward_spec.to_string(),
    }
}

fn rewrite_tunnel_forward_port(tunnel_command: &str, local_port: u16) -> String {
    let mut tokens = tunnel_command
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();

    for index in 0..tokens.len() {
        if tokens[index] == "-L" {
            if let Some(forward_spec) = tokens.get_mut(index + 1) {
                *forward_spec = rewrite_forward_spec_local_port(forward_spec, local_port);
                return tokens.join(" ");
            }
        }

        if let Some(forward_spec) = tokens[index].strip_prefix("-L") {
            tokens[index] = format!(
                "-L{}",
                rewrite_forward_spec_local_port(forward_spec, local_port)
            );
            return tokens.join(" ");
        }
    }

    tunnel_command.to_string()
}

fn resolve_runtime_tunnel_command(
    tunnel_command: &str,
    remote_url: &str,
    local_port: u16,
) -> String {
    let placeholder_resolved =
        replace_known_tunnel_placeholders(tunnel_command, remote_url, local_port);

    if placeholder_resolved != tunnel_command {
        return placeholder_resolved;
    }

    rewrite_tunnel_forward_port(&placeholder_resolved, local_port)
}

fn resolve_runtime_vite_hmr_tunnel_command(tunnel_command: &str, local_port: u16) -> String {
    let placeholder_resolved = tunnel_command.replace("{localPort}", &local_port.to_string());

    if placeholder_resolved != tunnel_command {
        return placeholder_resolved;
    }

    rewrite_tunnel_forward_port(&placeholder_resolved, local_port)
}

fn resolve_runtime_local_url(local_url: &str, remote_url: &str, local_port: u16) -> String {
    let template = if local_url.trim().is_empty() {
        remote_url
    } else {
        local_url
    }
    .replace("{localPort}", &local_port.to_string());

    if let Ok(mut url) = Url::parse(&template) {
        let _ = url.set_host(Some(SURFACE_HOST));
        let _ = url.set_port(Some(local_port));
        return url.to_string();
    }

    if let Ok(mut remote_url) = Url::parse(remote_url) {
        let _ = remote_url.set_host(Some(SURFACE_HOST));
        let _ = remote_url.set_port(Some(local_port));
        return remote_url.to_string();
    }

    format!("http://{}:{}/", SURFACE_HOST, local_port)
}

fn surface_local_port(surface: &BrowserSurface) -> Result<u16, String> {
    let url = Url::parse(&surface.local_url).map_err(|error| {
        format!(
            "Failed to parse local URL for {} surface: {}",
            surface.id, error
        )
    })?;

    url.port_or_known_default().ok_or_else(|| {
        format!(
            "Local URL for {} surface is missing a port: {}",
            surface.id, surface.local_url
        )
    })
}

fn hydrate_surface(
    target_id: &str,
    surface: &BrowserSurface,
    tunnel_state: &TunnelState,
) -> Result<BrowserSurface, String> {
    if surface.tunnel_command.trim().is_empty() {
        return Ok(surface.clone());
    }

    let local_port = allocate_surface_port(tunnel_state, target_id, &surface.id)?;
    let mut runtime_surface = surface.clone();

    runtime_surface.tunnel_command = resolve_runtime_tunnel_command(
        &runtime_surface.tunnel_command,
        &runtime_surface.remote_url,
        local_port,
    );
    runtime_surface.local_url = resolve_runtime_local_url(
        &runtime_surface.local_url,
        &runtime_surface.remote_url,
        local_port,
    );

    Ok(runtime_surface)
}

fn hydrate_target(
    target: &DeveloperTarget,
    tunnel_state: &TunnelState,
) -> Result<DeveloperTarget, String> {
    let surfaces = target
        .surfaces
        .iter()
        .map(|surface| hydrate_surface(&target.id, surface, tunnel_state))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DeveloperTarget {
        surfaces,
        ..target.clone()
    })
}

fn read_runtime_targets_file(tunnel_state: &TunnelState) -> Result<TargetsFile, String> {
    let targets_file = read_targets_file()?;
    let targets = targets_file
        .targets
        .into_iter()
        .map(|target| hydrate_target(&target, tunnel_state))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TargetsFile {
        selected_target_id: targets_file.selected_target_id,
        targets,
    })
}

fn resolve_runtime_target(
    target_id: &str,
    tunnel_state: &TunnelState,
) -> Result<DeveloperTarget, String> {
    let target = resolve_raw_target(target_id)?;
    hydrate_target(&target, tunnel_state)
}

fn resolve_runtime_surface(
    target_id: &str,
    surface_id: &str,
    tunnel_state: &TunnelState,
) -> Result<BrowserSurface, String> {
    let target = resolve_runtime_target(target_id, tunnel_state)?;

    target
        .surfaces
        .into_iter()
        .find(|surface| surface.id == surface_id)
        .ok_or_else(|| {
            format!(
                "Unknown browser surface {} for target {}",
                surface_id, target_id
            )
        })
}

fn append_output(output: &Arc<Mutex<String>>, chunk: &str) {
    if let Ok(mut current_output) = output.lock() {
        current_output.push_str(chunk);

        if current_output.len() > OUTPUT_BUFFER_LIMIT {
            let mut drain_until = current_output.len() - OUTPUT_BUFFER_LIMIT;

            while drain_until < current_output.len()
                && !current_output.is_char_boundary(drain_until)
            {
                drain_until += 1;
            }

            current_output.drain(..drain_until);
        }
    }
}

fn snapshot_for(target_id: &str, session: &TerminalSession) -> TerminalSnapshot {
    let output = session
        .output
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default();

    TerminalSnapshot {
        target_id: target_id.to_string(),
        output,
    }
}

fn tunnel_status(
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

fn direct_tunnel_status(target_id: &str, surface: &BrowserSurface) -> TunnelStatus {
    tunnel_status(
        target_id,
        &surface.id,
        "direct",
        format!("{} is using the local URL directly.", surface.label),
    )
}

fn starting_tunnel_status(target_id: &str, surface: &BrowserSurface) -> TunnelStatus {
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

fn running_tunnel_status(target_id: &str, surface: &BrowserSurface) -> TunnelStatus {
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

fn active_tunnel_status(
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
        tunnel_output_excerpt(&process.output).map(|excerpt| format!("{}: {}", process.label, excerpt))
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

fn tunnel_session_is_alive(session: &TunnelSession) -> Result<bool, String> {
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

fn remove_tunnel_session_if_current(
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

fn stop_tunnel_session(session: &TunnelSession) {
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

fn socket_addr_for_local_port(port: u16) -> std::net::SocketAddr {
    ([127, 0, 0, 1], port).into()
}

fn find_header_terminator(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn read_http_request_head(
    stream: &mut TcpStream,
) -> Result<(Vec<u8>, usize, String, String, String, Vec<(String, String)>), String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];

    let _ = stream.set_read_timeout(Some(PROXY_CONNECT_TIMEOUT));

    loop {
        let read_count = stream
            .read(&mut chunk)
            .map_err(|error| format!("Failed to read proxy request: {}", error))?;

        if read_count == 0 {
            return Err(String::from(
                "Proxy request closed before the HTTP headers were complete.",
            ));
        }

        buffer.extend_from_slice(&chunk[..read_count]);

        if buffer.len() > MAX_HTTP_HEADER_BYTES {
            return Err(String::from("Proxy request headers exceeded the configured limit."));
        }

        if let Some(header_end) = find_header_terminator(&buffer) {
            let head = String::from_utf8(buffer[..header_end].to_vec())
                .map_err(|_| String::from("Proxy request headers were not valid UTF-8."))?;
            let mut lines = head.split("\r\n");
            let request_line = lines
                .next()
                .ok_or_else(|| String::from("Proxy request line was missing."))?;
            let mut parts = request_line.split_whitespace();
            let method = parts
                .next()
                .ok_or_else(|| String::from("Proxy request method was missing."))?;
            let path = parts
                .next()
                .ok_or_else(|| String::from("Proxy request path was missing."))?;
            let version = parts
                .next()
                .ok_or_else(|| String::from("Proxy request version was missing."))?;
            let headers = lines
                .filter(|line| !line.is_empty())
                .filter_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    Some((name.trim().to_string(), value.trim().to_string()))
                })
                .collect::<Vec<_>>();

            return Ok((
                buffer,
                header_end + 4,
                method.to_string(),
                path.to_string(),
                version.to_string(),
                headers,
            ));
        }
    }
}

fn is_websocket_request(headers: &[(String, String)]) -> bool {
    headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("upgrade") && value.eq_ignore_ascii_case("websocket")
    })
}

fn rebuild_http_request(
    method: &str,
    path: &str,
    version: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Vec<u8> {
    let mut request = format!("{} {} {}\r\n", method, path, version);

    for (name, value) in headers {
        if name.eq_ignore_ascii_case("accept-encoding")
            || name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("proxy-connection")
            || name.eq_ignore_ascii_case("if-none-match")
        {
            continue;
        }

        request.push_str(name);
        request.push_str(": ");
        request.push_str(value);
        request.push_str("\r\n");
    }

    request.push_str("Connection: close\r\n");
    request.push_str("Accept-Encoding: identity\r\n");
    request.push_str("\r\n");

    let mut request_bytes = request.into_bytes();
    request_bytes.extend_from_slice(body);
    request_bytes
}

fn parse_http_response_head(
    response: &[u8],
) -> Result<(String, Vec<(String, String)>, usize), String> {
    let header_end = find_header_terminator(response)
        .ok_or_else(|| String::from("Proxy response was missing an HTTP header terminator."))?;
    let head = String::from_utf8(response[..header_end].to_vec())
        .map_err(|_| String::from("Proxy response headers were not valid UTF-8."))?;
    let mut lines = head.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| String::from("Proxy response status line was missing."))?
        .to_string();
    let headers = lines
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect::<Vec<_>>();

    Ok((status_line, headers, header_end + 4))
}

fn response_header_value<'a>(
    headers: &'a [(String, String)],
    name: &str,
) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn decode_chunked_body(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoded = Vec::new();
    let mut index = 0usize;

    loop {
        let size_line_end = body[index..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|offset| index + offset)
            .ok_or_else(|| String::from("Invalid chunked response framing."))?;
        let size_line = std::str::from_utf8(&body[index..size_line_end])
            .map_err(|_| String::from("Chunked response size line was not valid UTF-8."))?;
        let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| String::from("Chunked response size could not be parsed."))?;
        index = size_line_end + 2;

        if size == 0 {
            break;
        }

        let chunk_end = index + size;
        if chunk_end + 2 > body.len() {
            return Err(String::from("Chunked response ended unexpectedly."));
        }

        decoded.extend_from_slice(&body[index..chunk_end]);
        index = chunk_end + 2;
    }

    Ok(decoded)
}

fn replace_js_statement(script: &str, prefix: &str, replacement: &str) -> String {
    let Some(start) = script.find(prefix) else {
        return script.to_string();
    };
    let Some(relative_end) = script[start..].find(';') else {
        return script.to_string();
    };
    let end = start + relative_end + 1;

    let mut updated = String::with_capacity(script.len() + replacement.len());
    updated.push_str(&script[..start]);
    updated.push_str(replacement);
    updated.push_str(&script[end..]);
    updated
}

fn rewrite_vite_client_script(script: &str, public_port: u16) -> String {
    let script = replace_js_statement(
        script,
        "const hmrPort = ",
        &format!("const hmrPort = {};", public_port),
    );

    replace_js_statement(
        &script,
        "const directSocketHost = ",
        &format!(
            "const directSocketHost = \"{}:{}/\";",
            SURFACE_HOST, public_port
        ),
    )
}

fn build_rewritten_http_response(
    status_line: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Vec<u8> {
    let mut response = String::new();
    response.push_str(status_line);
    response.push_str("\r\n");

    for (name, value) in headers {
        if name.eq_ignore_ascii_case("content-length")
            || name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("etag")
            || name.eq_ignore_ascii_case("content-encoding")
        {
            continue;
        }

        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }

    response.push_str(&format!("Content-Length: {}\r\n", body.len()));
    response.push_str("Connection: close\r\n");
    response.push_str("\r\n");

    let mut response_bytes = response.into_bytes();
    response_bytes.extend_from_slice(body);
    response_bytes
}

fn rewrite_vite_client_response(response: &[u8], public_port: u16) -> Result<Vec<u8>, String> {
    let (status_line, headers, body_offset) = parse_http_response_head(response)?;
    let body = if response_header_value(&headers, "transfer-encoding")
        .map(|value| value.eq_ignore_ascii_case("chunked"))
        .unwrap_or(false)
    {
        decode_chunked_body(&response[body_offset..])?
    } else {
        response[body_offset..].to_vec()
    };

    let script = String::from_utf8(body)
        .map_err(|_| String::from("The Vite client response body was not valid UTF-8."))?;
    let rewritten = rewrite_vite_client_script(&script, public_port);

    Ok(build_rewritten_http_response(
        &status_line,
        &headers,
        rewritten.as_bytes(),
    ))
}

fn write_proxy_error_response(stream: &mut TcpStream, status_line: &str, message: &str) {
    let body = message.as_bytes();
    let response = format!(
        "{}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_line,
        body.len(),
        message
    );
    let _ = stream.write_all(response.as_bytes());
}

fn proxy_websocket_connection(
    mut client_stream: TcpStream,
    request_bytes: &[u8],
    upstream_port: u16,
) {
    let Ok(mut upstream_stream) = TcpStream::connect(socket_addr_for_local_port(upstream_port))
    else {
        return;
    };

    let _ = upstream_stream.write_all(request_bytes);

    let Ok(mut client_reader) = client_stream.try_clone() else {
        return;
    };
    let Ok(mut upstream_writer) = upstream_stream.try_clone() else {
        return;
    };

    let forward = std::thread::spawn(move || {
        let _ = std::io::copy(&mut client_reader, &mut upstream_writer);
    });

    let _ = std::io::copy(&mut upstream_stream, &mut client_stream);
    let _ = forward.join();
}

fn proxy_http_connection(
    client_stream: &mut TcpStream,
    request_bytes: &[u8],
    request_body_offset: usize,
    method: &str,
    path: &str,
    version: &str,
    headers: &[(String, String)],
    upstream_port: u16,
    public_port: u16,
) -> Result<(), String> {
    let mut upstream_stream = TcpStream::connect(socket_addr_for_local_port(upstream_port))
        .map_err(|error| format!("Failed to connect to the upstream dev server: {}", error))?;
    let _ = upstream_stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = upstream_stream.set_write_timeout(Some(Duration::from_secs(10)));

    let request = rebuild_http_request(
        method,
        path,
        version,
        headers,
        &request_bytes[request_body_offset..],
    );
    upstream_stream
        .write_all(&request)
        .map_err(|error| format!("Failed to forward the proxy request: {}", error))?;

    let mut response = Vec::new();
    upstream_stream
        .read_to_end(&mut response)
        .map_err(|error| format!("Failed to read the upstream response: {}", error))?;

    let response_bytes = if path == "/@vite/client" {
        rewrite_vite_client_response(&response, public_port).unwrap_or(response)
    } else {
        response
    };

    client_stream
        .write_all(&response_bytes)
        .map_err(|error| format!("Failed to write the proxy response: {}", error))
}

fn handle_vite_proxy_connection(
    mut client_stream: TcpStream,
    public_port: u16,
    upstream_http_port: u16,
    upstream_hmr_port: u16,
) {
    let _ = client_stream.set_nonblocking(false);
    let request = read_http_request_head(&mut client_stream);
    let Ok((request_bytes, request_body_offset, method, path, version, headers)) = request else {
        if let Err(error) = request {
            write_proxy_error_response(
                &mut client_stream,
                "HTTP/1.1 400 Bad Request",
                &error,
            );
        }
        return;
    };

    if is_websocket_request(&headers) {
        proxy_websocket_connection(client_stream, &request_bytes, upstream_hmr_port);
        return;
    }

    if let Err(error) = proxy_http_connection(
        &mut client_stream,
        &request_bytes,
        request_body_offset,
        &method,
        &path,
        &version,
        &headers,
        upstream_http_port,
        public_port,
    ) {
        write_proxy_error_response(
            &mut client_stream,
            "HTTP/1.1 502 Bad Gateway",
            &error,
        );
    }
}

fn start_vite_proxy(
    public_port: u16,
    upstream_http_port: u16,
    upstream_hmr_port: u16,
) -> Result<LocalProxyHandle, String> {
    let listener = TcpListener::bind(socket_addr_for_local_port(public_port)).map_err(|error| {
        format!(
            "Failed to bind the local development proxy on {}: {}",
            public_port, error
        )
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure the local development proxy: {}", error))?;

    let stop_requested = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    let error = Arc::new(Mutex::new(None));
    let stop_flag = stop_requested.clone();
    let running_flag = running.clone();
    let error_state = error.clone();

    std::thread::spawn(move || {
        loop {
            if stop_flag.load(Ordering::Acquire) {
                break;
            }

            match listener.accept() {
                Ok((stream, _)) => {
                    std::thread::spawn(move || {
                        handle_vite_proxy_connection(
                            stream,
                            public_port,
                            upstream_http_port,
                            upstream_hmr_port,
                        );
                    });
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(PROXY_ACCEPT_POLL_INTERVAL);
                }
                Err(error) => {
                    if let Ok(mut last_error) = error_state.lock() {
                        *last_error = Some(format!("The local development proxy stopped: {}", error));
                    }
                    break;
                }
            }
        }

        running_flag.store(false, Ordering::Release);
    });

    Ok(LocalProxyHandle {
        stop_requested,
        running,
        error,
    })
}

fn proxy_error_message(proxy: &LocalProxyHandle) -> Option<String> {
    proxy
        .error
        .lock()
        .ok()
        .and_then(|message| message.clone())
}

fn internal_probe_url(port: u16) -> String {
    format!("http://{}:{}/", SURFACE_HOST, port)
}

fn spawn_managed_tunnel_session(
    app: &AppHandle,
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    session_id: u64,
    target_id: &str,
    surface: &BrowserSurface,
    processes: Vec<TunnelProcess>,
    proxy: Option<LocalProxyHandle>,
    probe_urls: Vec<String>,
) -> TunnelSession {
    let started_at = Instant::now();
    let session = TunnelSession {
        session_id,
        started_at,
        processes: processes.clone(),
        proxy: proxy.clone(),
        ready: Arc::new(AtomicBool::new(false)),
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
                        monitor_error =
                            Some(format!("{} process lock was lost before startup completed.", process.label));
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
    let process = spawn_tunnel_process(
        &surface.tunnel_command,
        format!("{} tunnel", surface.label),
    )?;

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

    let http_tunnel_command = rewrite_tunnel_forward_port(&surface.tunnel_command, http_forward_port);
    let hmr_tunnel_command =
        resolve_runtime_vite_hmr_tunnel_command(&surface.vite_hmr_tunnel_command, hmr_forward_port);

    let http_process = spawn_tunnel_process(
        &http_tunnel_command,
        format!("{} HTTP tunnel", surface.label),
    )?;
    let hmr_process = match spawn_tunnel_process(
        &hmr_tunnel_command,
        format!("{} HMR tunnel", surface.label),
    ) {
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

fn start_tunnel(
    app: &AppHandle,
    tunnel_state: &TunnelState,
    target_id: &str,
    surface: &BrowserSurface,
) -> Result<TunnelStatus, String> {
    let session_id = tunnel_state.next_session_id.fetch_add(1, Ordering::Relaxed);
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

fn stop_all_tunnels(tunnel_state: &TunnelState) {
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

fn spawn_terminal_session(
    app: &AppHandle,
    sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
    target: &DeveloperTarget,
) -> Result<TerminalSession, String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 32,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to allocate PTY for {}: {}", target.id, error))?;

    let mut command = CommandBuilder::new("/bin/zsh");
    command.arg("-lc");
    command.arg(target.terminal_command.clone());
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");

    pair.slave
        .spawn_command(command)
        .map_err(|error| format!("Failed to start terminal for {}: {}", target.id, error))?;

    let mut reader = pair.master.try_clone_reader().map_err(|error| {
        format!(
            "Failed to open terminal reader for {}: {}",
            target.id, error
        )
    })?;

    let writer = pair.master.take_writer().map_err(|error| {
        format!(
            "Failed to open terminal writer for {}: {}",
            target.id, error
        )
    })?;

    let session = TerminalSession {
        master: Arc::new(Mutex::new(pair.master)),
        writer: Arc::new(Mutex::new(writer)),
        output: Arc::new(Mutex::new(String::new())),
    };

    let output = session.output.clone();
    let app_handle = app.clone();
    let target_id = target.id.clone();

    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        let mut exit_reason = String::from("session closed");

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read_count) => {
                    let chunk = String::from_utf8_lossy(&buffer[..read_count]).to_string();
                    append_output(&output, &chunk);
                    let _ = app_handle.emit(
                        "terminal-output",
                        TerminalOutputEvent {
                            target_id: target_id.clone(),
                            data: chunk,
                        },
                    );
                }
                Err(error) => {
                    exit_reason = format!("terminal read failed: {}", error);
                    break;
                }
            }
        }

        let _ = app_handle.emit(
            "terminal-exit",
            TerminalExitEvent {
                target_id: target_id.clone(),
                reason: exit_reason,
            },
        );

        if let Ok(mut active_sessions) = sessions.lock() {
            active_sessions.remove(&target_id);
        }
    });

    Ok(session)
}

#[tauri::command]
fn load_bootstrap(tunnel_state: State<TunnelState>) -> Result<AppBootstrap, String> {
    let targets_file = read_runtime_targets_file(tunnel_state.inner())?;
    let config_path = targets_file_path();

    Ok(AppBootstrap {
        app_name: "Univers Ark Developer".into(),
        config_path: config_path.display().to_string(),
        selected_target_id: targets_file.selected_target_id,
        targets: targets_file.targets,
    })
}

#[tauri::command]
fn attach_terminal(
    app: AppHandle,
    terminal_state: State<TerminalState>,
    target_id: String,
) -> Result<TerminalSnapshot, String> {
    let mut sessions = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Terminal session state is unavailable"))?;

    if let Some(session) = sessions.get(&target_id) {
        return Ok(snapshot_for(&target_id, session));
    }

    let target = resolve_raw_target(&target_id)?;
    let session = spawn_terminal_session(&app, terminal_state.sessions.clone(), &target)?;
    let snapshot = snapshot_for(&target_id, &session);
    sessions.insert(target_id.clone(), session);

    Ok(snapshot)
}

#[tauri::command]
fn ensure_tunnel(
    app: AppHandle,
    tunnel_state: State<TunnelState>,
    target_id: String,
    surface_id: String,
) -> Result<TunnelStatus, String> {
    let surface = resolve_runtime_surface(&target_id, &surface_id, tunnel_state.inner())?;

    if surface.tunnel_command.trim().is_empty() {
        return Ok(direct_tunnel_status(&target_id, &surface));
    }

    let key = surface_key(&target_id, &surface_id);
    let existing_session = tunnel_state
        .sessions
        .lock()
        .map_err(|_| String::from("Tunnel session state is unavailable"))?
        .get(&key)
        .cloned();

    if let Some(session) = existing_session {
        if tunnel_session_is_alive(&session)? {
            return Ok(active_tunnel_status(&target_id, &surface, &session));
        }

        let _ = remove_tunnel_session_if_current(&tunnel_state.sessions, &key, session.session_id);
    }

    start_tunnel(&app, tunnel_state.inner(), &target_id, &surface)
}

#[tauri::command]
fn restart_tunnel(
    app: AppHandle,
    tunnel_state: State<TunnelState>,
    target_id: String,
    surface_id: String,
) -> Result<TunnelStatus, String> {
    let surface = resolve_runtime_surface(&target_id, &surface_id, tunnel_state.inner())?;

    if surface.tunnel_command.trim().is_empty() {
        return Ok(direct_tunnel_status(&target_id, &surface));
    }

    let key = surface_key(&target_id, &surface_id);
    let previous_session = tunnel_state
        .sessions
        .lock()
        .map_err(|_| String::from("Tunnel session state is unavailable"))?
        .remove(&key);

    if let Some(session) = previous_session {
        stop_tunnel_session(&session);
    }

    start_tunnel(&app, tunnel_state.inner(), &target_id, &surface)
}

#[tauri::command]
fn write_terminal(
    terminal_state: State<TerminalState>,
    target_id: String,
    data: String,
) -> Result<(), String> {
    let session = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Terminal session state is unavailable"))?
        .get(&target_id)
        .cloned()
        .ok_or_else(|| format!("No active terminal session for {}", target_id))?;

    let mut writer = session
        .writer
        .lock()
        .map_err(|_| format!("Terminal writer is locked for {}", target_id))?;

    writer
        .write_all(data.as_bytes())
        .map_err(|error| format!("Failed to write to {}: {}", target_id, error))?;
    writer
        .flush()
        .map_err(|error| format!("Failed to flush {}: {}", target_id, error))?;

    Ok(())
}

#[tauri::command]
fn resize_terminal(
    terminal_state: State<TerminalState>,
    target_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let session = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Terminal session state is unavailable"))?
        .get(&target_id)
        .cloned()
        .ok_or_else(|| format!("No active terminal session for {}", target_id))?;

    let master = session
        .master
        .lock()
        .map_err(|_| format!("Terminal master is locked for {}", target_id))?;

    master
        .resize(PtySize {
            rows: rows.max(12),
            cols: cols.max(40),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to resize {}: {}", target_id, error))
}

fn main() {
    tauri::Builder::default()
        .manage(TerminalState::default())
        .manage(TunnelState::default())
        .invoke_handler(tauri::generate_handler![
            load_bootstrap,
            attach_terminal,
            ensure_tunnel,
            restart_tunnel,
            write_terminal,
            resize_terminal
        ])
        .build(tauri::generate_context!())
        .expect("error while building univers-ark-developer")
        .run(|app_handle, event| {
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                let tunnel_state = app_handle.state::<TunnelState>();
                stop_all_tunnels(tunnel_state.inner());
            }
        });
}
