mod connection;
mod exec;
mod files;
mod forward;
mod pty;
mod ssh_config;
mod types;

pub use exec::{execute_chain, probe_http_chain, probe_pty_shell_chain};
pub use files::{list_directory_chain, read_file_preview_chain};
pub use forward::start_local_forward_chain;
pub use pty::start_pty_session_chain;
pub use ssh_config::{ResolvedEndpoint, ResolvedEndpointChain, SshConfigResolver};
pub use types::{
    ClientOptions, ExecOutput, HttpProbeOutput, LocalForward, PtySession, PtySessionEvent,
    PtyShellProbeOutput, RemoteDirectoryListing, RemoteFileEntry, RemoteFilePreview, RusshError,
};

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

pub fn start_pty_session_alias(
    resolver: &SshConfigResolver,
    destination: &str,
    startup_command: &str,
    cols: u16,
    rows: u16,
    options: &ClientOptions,
) -> Result<(PtySession, std::sync::mpsc::Receiver<PtySessionEvent>), RusshError> {
    let chain = resolver
        .resolve(destination)
        .map_err(|error| RusshError::ResolveDestination(error.to_string()))?;
    start_pty_session_chain(&chain, startup_command, cols, rows, options)
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
