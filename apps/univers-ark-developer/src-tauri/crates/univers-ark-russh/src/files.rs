use crate::{
    ssh_config::ResolvedEndpointChain,
    sftp::{sftp_list_directory, sftp_read_file_preview, sftp_write_file},
    types::{ClientOptions, RemoteDirectoryListing, RemoteFilePreview, RusshError},
};

pub async fn list_directory_chain(
    chain: &ResolvedEndpointChain,
    path: Option<&str>,
    options: &ClientOptions,
) -> Result<RemoteDirectoryListing, RusshError> {
    sftp_list_directory(chain, path, options).await
}

pub async fn read_file_preview_chain(
    chain: &ResolvedEndpointChain,
    path: &str,
    options: &ClientOptions,
) -> Result<RemoteFilePreview, RusshError> {
    sftp_read_file_preview(chain, path, options).await
}

pub async fn write_file_chain(
    chain: &ResolvedEndpointChain,
    path: &str,
    data: &[u8],
    options: &ClientOptions,
) -> Result<(), RusshError> {
    sftp_write_file(chain, path, data, options).await
}
