use std::future::Future;
use univers_infra_ssh::{
    execute_chain, list_directory_chain, read_file_preview_chain, start_local_forward_chain,
    write_file_chain, ClientOptions as RusshClientOptions, ExecOutput as RusshExecOutput,
    LocalForward, RemoteDirectoryListing as RusshDirectoryListing,
    RemoteFilePreview as RusshFilePreview, ResolvedEndpointChain, RusshError,
};

fn block_on_russh<T, F>(future: F) -> Result<T, String>
where
    F: Future<Output = Result<T, RusshError>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Failed to build russh runtime: {error}"))?;

    runtime.block_on(future).map_err(|error| error.to_string())
}

pub(crate) fn execute_chain_blocking(
    chain: &ResolvedEndpointChain,
    command: &str,
    options: &RusshClientOptions,
) -> Result<RusshExecOutput, String> {
    block_on_russh(execute_chain(chain, command, options))
}

pub(crate) fn list_directory_chain_blocking(
    chain: &ResolvedEndpointChain,
    path: Option<&str>,
    options: &RusshClientOptions,
) -> Result<RusshDirectoryListing, String> {
    block_on_russh(list_directory_chain(chain, path, options))
}

pub(crate) fn read_file_preview_chain_blocking(
    chain: &ResolvedEndpointChain,
    path: &str,
    options: &RusshClientOptions,
) -> Result<RusshFilePreview, String> {
    block_on_russh(read_file_preview_chain(chain, path, options))
}

pub(crate) fn write_file_chain_blocking(
    chain: &ResolvedEndpointChain,
    path: &str,
    data: &[u8],
    options: &RusshClientOptions,
) -> Result<(), String> {
    block_on_russh(write_file_chain(chain, path, data, options))
}

pub(crate) fn start_local_forward_chain_blocking(
    chain: &ResolvedEndpointChain,
    local_bind_addr: &str,
    remote_host: &str,
    remote_port: u16,
    options: &RusshClientOptions,
) -> Result<LocalForward, String> {
    block_on_russh(start_local_forward_chain(
        chain,
        local_bind_addr,
        remote_host,
        remote_port,
        options,
    ))
}
