mod ssh_config;

use std::{
    env, fs,
    net::SocketAddr,
    io::ErrorKind,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

use russh::{
    client, client::Handle, keys::PrivateKeyWithHashAlg, ChannelMsg, Disconnect, Preferred,
};
use serde::Deserialize;
use thiserror::Error;
use tokio::{
    io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

pub use ssh_config::{ResolvedEndpoint, ResolvedEndpointChain, SshConfigResolver};

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub connect_timeout: Duration,
    pub inactivity_timeout: Option<Duration>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(8),
            inactivity_timeout: None,
        }
    }
}

#[derive(Debug)]
pub struct ExecOutput {
    pub exit_status: u32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub struct HttpProbeOutput {
    pub status_line: String,
    pub body_preview: String,
}

#[derive(Debug)]
pub struct PtyShellProbeOutput {
    pub marker_found: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub struct RemoteDirectoryListing {
    pub path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<RemoteFileEntry>,
}

#[derive(Debug)]
pub struct RemoteFileEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub is_hidden: bool,
}

#[derive(Debug)]
pub struct RemoteFilePreview {
    pub path: String,
    pub content: String,
    pub is_binary: bool,
    pub truncated: bool,
}

#[derive(Debug, Error)]
pub enum RusshError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to resolve ssh destination {0}")]
    ResolveDestination(String),
    #[error("russh error: {0}")]
    Russh(#[from] russh::Error),
    #[error("ssh key error: {0}")]
    Key(#[from] russh::keys::Error),
    #[error("authentication failed for {0}@{1}")]
    Auth(String, String),
    #[error("missing identity file for {0}")]
    MissingIdentity(String),
    #[error("forward task failed: {0}")]
    ForwardTask(String),
}

#[derive(Clone)]
pub struct LocalForward {
    inner: Arc<LocalForwardInner>,
}

impl LocalForward {
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr
    }

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }

    pub fn last_error(&self) -> Option<String> {
        self.inner.error.lock().ok().and_then(|error| error.clone())
    }

    pub fn request_stop(&self) {
        if let Ok(mut shutdown) = self.inner.shutdown.lock() {
            if let Some(sender) = shutdown.take() {
                let _ = sender.send(());
            }
        }
    }
}

struct LocalForwardInner {
    local_addr: SocketAddr,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    running: AtomicBool,
    error: Mutex<Option<String>>,
}

pub async fn execute_alias(
    resolver: &SshConfigResolver,
    destination: &str,
    command: &str,
    options: &ClientOptions,
) -> Result<ExecOutput, RusshError> {
    let chain = resolver
        .resolve(destination)
        .map_err(|error| RusshError::ResolveDestination(error.to_string()))?;
    execute_chain(&chain, command, options).await
}

pub async fn probe_http_alias(
    resolver: &SshConfigResolver,
    destination: &str,
    remote_host: &str,
    remote_port: u16,
    path: &str,
    options: &ClientOptions,
) -> Result<HttpProbeOutput, RusshError> {
    let chain = resolver
        .resolve(destination)
        .map_err(|error| RusshError::ResolveDestination(error.to_string()))?;
    probe_http_chain(&chain, remote_host, remote_port, path, options).await
}

pub async fn probe_pty_shell_alias(
    resolver: &SshConfigResolver,
    destination: &str,
    command: &str,
    options: &ClientOptions,
) -> Result<PtyShellProbeOutput, RusshError> {
    let chain = resolver
        .resolve(destination)
        .map_err(|error| RusshError::ResolveDestination(error.to_string()))?;
    probe_pty_shell_chain(&chain, command, options).await
}

pub async fn list_directory_alias(
    resolver: &SshConfigResolver,
    destination: &str,
    path: Option<&str>,
    options: &ClientOptions,
) -> Result<RemoteDirectoryListing, RusshError> {
    let chain = resolver
        .resolve(destination)
        .map_err(|error| RusshError::ResolveDestination(error.to_string()))?;
    list_directory_chain(&chain, path, options).await
}

pub async fn read_file_preview_alias(
    resolver: &SshConfigResolver,
    destination: &str,
    path: &str,
    options: &ClientOptions,
) -> Result<RemoteFilePreview, RusshError> {
    let chain = resolver
        .resolve(destination)
        .map_err(|error| RusshError::ResolveDestination(error.to_string()))?;
    read_file_preview_chain(&chain, path, options).await
}

pub async fn execute_chain(
    chain: &ResolvedEndpointChain,
    command: &str,
    options: &ClientOptions,
) -> Result<ExecOutput, RusshError> {
    let client = connect_chain(chain, options).await?;
    let mut channel = client.handle.channel_open_session().await?;
    channel.exec(true, command).await?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_status = None;

    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
            ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => exit_status = Some(status),
            ChannelMsg::Eof => break,
            _ => {}
        }
    }

    let _ = client
        .handle
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;

    Ok(ExecOutput {
        exit_status: exit_status.unwrap_or_default(),
        stdout,
        stderr,
    })
}

pub async fn probe_http_chain(
    chain: &ResolvedEndpointChain,
    remote_host: &str,
    remote_port: u16,
    path: &str,
    options: &ClientOptions,
) -> Result<HttpProbeOutput, RusshError> {
    let client = connect_chain(chain, options).await?;
    let channel = client
        .handle
        .channel_open_direct_tcpip(
            remote_host.to_string(),
            remote_port.into(),
            String::from("127.0.0.1"),
            0,
        )
        .await?;

    let mut stream = channel.into_stream();
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, remote_host
    );
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;

    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        response.extend_from_slice(&buffer[..bytes_read]);
        let header_complete = response.windows(4).any(|window| window == b"\r\n\r\n");
        if header_complete && response.len() >= 512 {
            break;
        }
    }

    let _ = client
        .handle
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;

    let response = String::from_utf8_lossy(&response).to_string();
    let mut lines = response.lines();
    let status_line = lines.next().unwrap_or_default().to_string();
    let body_preview = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.chars().take(240).collect::<String>())
        .unwrap_or_default();

    Ok(HttpProbeOutput {
        status_line,
        body_preview,
    })
}

pub async fn probe_pty_shell_chain(
    chain: &ResolvedEndpointChain,
    command: &str,
    options: &ClientOptions,
) -> Result<PtyShellProbeOutput, RusshError> {
    let client = connect_chain(chain, options).await?;
    let mut channel = client.handle.channel_open_session().await?;
    channel
        .request_pty(true, "xterm-256color", 120, 32, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;
    channel.window_change(132, 36, 0, 0).await?;

    let marker = "__UA_RUSSH_DONE__";
    let shell_command = format!("{command}\nprintf '\\n{marker}:%s\\n' $? \nexit\n");
    let mut writer = channel.make_writer();
    writer.write_all(shell_command.as_bytes()).await?;
    writer.flush().await?;
    drop(writer);

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut marker_found = false;

    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => {
                stdout.extend_from_slice(&data);
                if !marker_found
                    && stdout
                        .windows(marker.len())
                        .any(|window| window == marker.as_bytes())
                {
                    marker_found = true;
                }
            }
            ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
            ChannelMsg::Close | ChannelMsg::Eof => break,
            _ => {}
        }
    }

    let _ = client
        .handle
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;

    Ok(PtyShellProbeOutput {
        marker_found,
        stdout,
        stderr,
    })
}

pub async fn start_local_forward_alias(
    resolver: &SshConfigResolver,
    destination: &str,
    local_bind_addr: &str,
    remote_host: &str,
    remote_port: u16,
    options: &ClientOptions,
) -> Result<LocalForward, RusshError> {
    let chain = resolver
        .resolve(destination)
        .map_err(|error| RusshError::ResolveDestination(error.to_string()))?;
    start_local_forward_chain(&chain, local_bind_addr, remote_host, remote_port, options).await
}

pub async fn start_local_forward_chain(
    chain: &ResolvedEndpointChain,
    local_bind_addr: &str,
    remote_host: &str,
    remote_port: u16,
    options: &ClientOptions,
) -> Result<LocalForward, RusshError> {
    let chain = chain.clone();
    let remote_host = remote_host.to_string();
    let options = options.clone();
    let local_bind_addr = local_bind_addr.to_string();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<LocalForward, RusshError>>(1);

    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = ready_tx.send(Err(RusshError::ForwardTask(format!(
                    "failed to build forward runtime: {error}"
                ))));
                return;
            }
        };

        runtime.block_on(async move {
            let listener = match TcpListener::bind(&local_bind_addr).await {
                Ok(listener) => listener,
                Err(error) => {
                    let _ = ready_tx.send(Err(RusshError::Io(error)));
                    return;
                }
            };
            let local_addr = match listener.local_addr() {
                Ok(addr) => addr,
                Err(error) => {
                    let _ = ready_tx.send(Err(RusshError::Io(error)));
                    return;
                }
            };

            let client = match connect_chain(&chain, &options).await {
                Ok(client) => client,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            let handle = Arc::new(client.handle);

            let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
            let inner = Arc::new(LocalForwardInner {
                local_addr,
                shutdown: Mutex::new(Some(shutdown_tx)),
                running: AtomicBool::new(true),
                error: Mutex::new(None),
            });

            let _ = ready_tx.send(Ok(LocalForward {
                inner: inner.clone(),
            }));

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => {
                        match accepted {
                            Ok((socket, _)) => {
                                let remote_host = remote_host.clone();
                                let handle = handle.clone();
                                let inner = inner.clone();
                                tokio::spawn(async move {
                                    if let Err(error) =
                                        forward_connection(socket, &handle, &remote_host, remote_port).await
                                    {
                                        if is_benign_forward_error(&error) {
                                            return;
                                        }
                                        if let Ok(mut stored) = inner.error.lock() {
                                            *stored = Some(format!(
                                                "forward connection failed: {error}"
                                            ));
                                        }
                                        if let Ok(mut shutdown) = inner.shutdown.lock() {
                                            if let Some(sender) = shutdown.take() {
                                                let _ = sender.send(());
                                            }
                                        }
                                    }
                                });
                            }
                            Err(error) => {
                                if let Ok(mut stored) = inner.error.lock() {
                                    *stored = Some(format!("failed to accept local forward connection: {error}"));
                                }
                                break;
                            }
                        }
                    }
                }
            }

            inner.running.store(false, Ordering::Release);
            let _ = handle
                .disconnect(Disconnect::ByApplication, "", "English")
                .await;
        });
    });

    ready_rx
        .recv()
        .map_err(|error| RusshError::ForwardTask(format!("failed to receive local forward startup: {error}")))?
}

pub async fn list_directory_chain(
    chain: &ResolvedEndpointChain,
    path: Option<&str>,
    options: &ClientOptions,
) -> Result<RemoteDirectoryListing, RusshError> {
    let command = list_directory_command(path);
    let output = execute_chain(chain, &command, options).await?;
    if output.exit_status != 0 {
        return Err(RusshError::ForwardTask(format!(
            "remote listing failed with exit status {}",
            output.exit_status
        )));
    }

    let payload: RemoteDirectoryListingPayload =
        serde_json::from_slice(&output.stdout).map_err(|error| {
            RusshError::ForwardTask(format!(
                "failed to parse directory listing: {error}; stdout={}; stderr={}",
                String::from_utf8_lossy(&output.stdout)
                    .chars()
                    .take(240)
                    .collect::<String>(),
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(240)
                    .collect::<String>(),
            ))
        })?;

    Ok(RemoteDirectoryListing {
        path: payload.path,
        parent_path: payload.parent_path,
        entries: payload
            .entries
            .into_iter()
            .map(|entry| RemoteFileEntry {
                name: entry.name,
                path: entry.path,
                kind: entry.kind,
                size: entry.size,
                is_hidden: entry.is_hidden,
            })
            .collect(),
    })
}

pub async fn read_file_preview_chain(
    chain: &ResolvedEndpointChain,
    path: &str,
    options: &ClientOptions,
) -> Result<RemoteFilePreview, RusshError> {
    let command = preview_file_command(path);
    let output = execute_chain(chain, &command, options).await?;
    if output.exit_status != 0 {
        return Err(RusshError::ForwardTask(format!(
            "remote preview failed with exit status {}",
            output.exit_status
        )));
    }

    let payload: RemoteFilePreviewPayload =
        serde_json::from_slice(&output.stdout).map_err(|error| {
            RusshError::ForwardTask(format!(
                "failed to parse file preview: {error}; stdout={}; stderr={}",
                String::from_utf8_lossy(&output.stdout)
                    .chars()
                    .take(240)
                    .collect::<String>(),
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(240)
                    .collect::<String>(),
            ))
        })?;

    Ok(RemoteFilePreview {
        path: payload.path,
        content: payload.content,
        is_binary: payload.is_binary,
        truncated: payload.truncated,
    })
}

struct ClientConnection {
    handle: Handle<ClientHandler>,
}

async fn connect_chain(
    chain: &ResolvedEndpointChain,
    options: &ClientOptions,
) -> Result<ClientConnection, RusshError> {
    let mut current_handle: Option<Handle<ClientHandler>> = None;

    for endpoint in chain.hops() {
        let next = if let Some(handle) = current_handle.take() {
            connect_via_handle(handle, endpoint, options).await?
        } else {
            connect_endpoint(endpoint, options).await?
        };

        current_handle = Some(next);
    }

    Ok(ClientConnection {
        handle: current_handle.ok_or_else(|| {
            RusshError::ResolveDestination(String::from("resolved ssh chain was empty"))
        })?,
    })
}

async fn connect_endpoint(
    endpoint: &ResolvedEndpoint,
    options: &ClientOptions,
) -> Result<Handle<ClientHandler>, RusshError> {
    let config = client_config(options);
    let mut handle = client::connect(
        config,
        (endpoint.host.as_str(), endpoint.port),
        ClientHandler,
    )
    .await?;
    authenticate_endpoint(&mut handle, endpoint).await?;
    Ok(handle)
}

async fn connect_via_handle(
    handle: Handle<ClientHandler>,
    endpoint: &ResolvedEndpoint,
    options: &ClientOptions,
) -> Result<Handle<ClientHandler>, RusshError> {
    let channel = handle
        .channel_open_direct_tcpip(
            endpoint.host.clone(),
            endpoint.port.into(),
            String::from("127.0.0.1"),
            0,
        )
        .await?;
    let stream = channel.into_stream();
    let config = client_config(options);
    let mut nested = client::connect_stream(config, stream, ClientHandler).await?;
    authenticate_endpoint(&mut nested, endpoint).await?;
    Ok(nested)
}

async fn forward_connection(
    mut inbound: TcpStream,
    handle: &Handle<ClientHandler>,
    remote_host: &str,
    remote_port: u16,
) -> Result<(), RusshError> {
    let channel = handle
        .channel_open_direct_tcpip(
            remote_host.to_string(),
            remote_port.into(),
            String::from("127.0.0.1"),
            0,
        )
        .await?;
    let mut outbound = channel.into_stream();

    let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
    let _ = outbound.shutdown().await;

    Ok(())
}

fn is_benign_forward_error(error: &RusshError) -> bool {
    match error {
        RusshError::Io(source) => matches!(
            source.kind(),
            ErrorKind::BrokenPipe
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::UnexpectedEof
                | ErrorKind::NotConnected
        ),
        RusshError::Russh(source) => {
            let message = source.to_string().to_ascii_lowercase();
            message.contains("channel closed")
                || message.contains("channel eof")
                || message.contains("connection reset")
                || message.contains("send error")
        }
        _ => false,
    }
}

async fn authenticate_endpoint(
    handle: &mut Handle<ClientHandler>,
    endpoint: &ResolvedEndpoint,
) -> Result<(), RusshError> {
    let mut candidates = endpoint.identity_files().to_vec();
    if candidates.is_empty() {
        candidates.extend(default_identity_files());
    }

    for path in candidates {
        if fs::metadata(&path).is_err() {
            continue;
        }

        let key = russh::keys::load_secret_key(&path, None)?;
        let auth = handle
            .authenticate_publickey(
                endpoint.user.clone(),
                PrivateKeyWithHashAlg::new(
                    Arc::new(key),
                    handle.best_supported_rsa_hash().await?.flatten(),
                ),
            )
            .await?;

        if auth.success() {
            return Ok(());
        }
    }

    if endpoint.identity_files().is_empty() && default_identity_files().is_empty() {
        return Err(RusshError::MissingIdentity(endpoint.alias.clone()));
    }

    Err(RusshError::Auth(
        endpoint.user.clone(),
        endpoint.host.clone(),
    ))
}

fn client_config(options: &ClientOptions) -> Arc<client::Config> {
    Arc::new(client::Config {
        inactivity_timeout: options.inactivity_timeout,
        preferred: Preferred {
            kex: std::borrow::Cow::Owned(vec![
                russh::kex::CURVE25519_PRE_RFC_8731,
                russh::kex::EXTENSION_SUPPORT_AS_CLIENT,
            ]),
            ..Default::default()
        },
        nodelay: true,
        ..Default::default()
    })
}

#[derive(Clone)]
struct ClientHandler;

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

fn default_identity_files() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let home = env::var("HOME").ok();
    let Some(home) = home else {
        return paths;
    };

    for name in ["id_ed25519", "id_rsa"] {
        let path = PathBuf::from(&home).join(".ssh").join(name);
        if fs::metadata(&path).is_ok() {
            paths.push(path);
        }
    }

    paths
}

const MAX_DIRECTORY_ENTRIES: usize = 512;
const MAX_PREVIEW_BYTES: usize = 131_072;
const DEFAULT_REMOTE_ROOT: &str = "~/repos/hvac-workbench";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteDirectoryListingPayload {
    path: String,
    parent_path: Option<String>,
    entries: Vec<RemoteFileEntryPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteFileEntryPayload {
    name: String,
    path: String,
    kind: String,
    size: u64,
    is_hidden: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteFilePreviewPayload {
    path: String,
    content: String,
    is_binary: bool,
    truncated: bool,
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn list_directory_command(path: Option<&str>) -> String {
    let path = path.unwrap_or(DEFAULT_REMOTE_ROOT);

    format!(
        r#"UNIVERS_ARK_PATH={} python3 - <<'PY'
import json
import os

requested_path = os.environ.get("UNIVERS_ARK_PATH") or "{default_root}"
path = os.path.abspath(os.path.expanduser(requested_path))
if requested_path == "{default_root}" and not os.path.isdir(path):
    path = os.path.abspath(os.path.expanduser("~"))
if not os.path.isdir(path):
    raise SystemExit(f"Not a directory: {{path}}")

entries = []
with os.scandir(path) as iterator:
    sorted_entries = sorted(
        iterator,
        key=lambda entry: (not entry.is_dir(follow_symlinks=False), entry.name.lower()),
    )
    for entry in sorted_entries[:{max_entries}]:
        try:
            stat_result = entry.stat(follow_symlinks=False)
            if entry.is_dir(follow_symlinks=False):
                kind = "directory"
            elif entry.is_file(follow_symlinks=False):
                kind = "file"
            elif entry.is_symlink():
                kind = "symlink"
            else:
                kind = "other"
            entries.append(
                {{
                    "name": entry.name,
                    "path": entry.path,
                    "kind": kind,
                    "size": int(getattr(stat_result, "st_size", 0)),
                    "isHidden": entry.name.startswith("."),
                }}
            )
        except OSError:
            entries.append(
                {{
                    "name": entry.name,
                    "path": entry.path,
                    "kind": "other",
                    "size": 0,
                    "isHidden": entry.name.startswith("."),
                }}
            )

parent = os.path.dirname(path)
if not parent or parent == path:
    parent = None

print(
    json.dumps(
        {{
            "path": path,
            "parentPath": parent,
            "entries": entries,
        }},
        ensure_ascii=False,
    )
)
PY"#,
        shell_single_quote(path),
        default_root = DEFAULT_REMOTE_ROOT,
        max_entries = MAX_DIRECTORY_ENTRIES,
    )
}

fn preview_file_command(path: &str) -> String {
    format!(
        r#"UNIVERS_ARK_PATH={} python3 - <<'PY'
import json
import os

path = os.path.abspath(os.path.expanduser(os.environ.get("UNIVERS_ARK_PATH") or "~"))
if not os.path.isfile(path):
    raise SystemExit(f"Not a file: {{path}}")

with open(path, "rb") as file_handle:
    chunk = file_handle.read({max_bytes} + 1)

is_binary = b"\x00" in chunk
truncated = len(chunk) > {max_bytes}
content = ""

if not is_binary:
    content = chunk[:{max_bytes}].decode("utf-8", errors="replace")

print(
    json.dumps(
        {{
            "path": path,
            "content": content,
            "isBinary": is_binary,
            "truncated": truncated,
        }},
        ensure_ascii=False,
    )
)
PY"#,
        shell_single_quote(path),
        max_bytes = MAX_PREVIEW_BYTES,
    )
}
