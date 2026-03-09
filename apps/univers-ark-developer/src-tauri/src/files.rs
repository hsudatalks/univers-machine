use crate::{
    config::read_server_inventory,
    models::{RemoteDirectoryListing, RemoteFileEntry, RemoteFilePreview},
};
use univers_ark_russh::{
    list_directory_chain, read_file_preview_chain, ClientOptions as RusshClientOptions,
    RemoteDirectoryListing as RusshDirectoryListing, RemoteFilePreview as RusshFilePreview,
    ResolvedEndpoint, ResolvedEndpointChain, SshConfigResolver,
};

fn resolve_container_chain(target_id: &str) -> Result<ResolvedEndpointChain, String> {
    let servers = read_server_inventory(false)?;
    let Some((server_host, container_ip, ssh_user, container_name)) = servers
        .iter()
        .find_map(|server| {
            server
                .containers
                .iter()
                .find(|container| container.target_id == target_id)
                .map(|container| {
                    (
                        server.host.clone(),
                        container.ipv4.clone(),
                        container.ssh_user.clone(),
                        container.name.clone(),
                    )
                })
        })
    else {
        return Err(format!("No container inventory found for {}", target_id));
    };

    let resolver =
        SshConfigResolver::from_default_path().map_err(|error| format!("Failed to load SSH config: {}", error))?;
    let mut chain = resolver
        .resolve(&server_host)
        .map_err(|error| format!("Failed to resolve SSH destination {}: {}", server_host, error))?;
    chain.push(ResolvedEndpoint::new(
        format!("{}::{}", server_host, container_name),
        container_ip,
        ssh_user,
        22,
        Vec::new(),
    ));

    Ok(chain)
}

fn list_remote_directory_via_russh(
    target_id: &str,
    path: Option<&str>,
) -> Result<RemoteDirectoryListing, String> {
    let chain = resolve_container_chain(target_id)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Failed to build russh runtime: {}", error))?;
    let listing = runtime
        .block_on(list_directory_chain(
            &chain,
            path,
            &RusshClientOptions::default(),
        ))
        .map_err(|error| format!("russh directory listing failed for {}: {}", target_id, error))?;

    Ok(map_russh_directory_listing(target_id, listing))
}

fn read_remote_file_preview_via_russh(
    target_id: &str,
    path: &str,
) -> Result<RemoteFilePreview, String> {
    let chain = resolve_container_chain(target_id)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Failed to build russh runtime: {}", error))?;
    let preview = runtime
        .block_on(read_file_preview_chain(
            &chain,
            path,
            &RusshClientOptions::default(),
        ))
        .map_err(|error| format!("russh file preview failed for {}: {}", target_id, error))?;

    Ok(map_russh_file_preview(target_id, preview))
}

fn map_russh_directory_listing(
    target_id: &str,
    listing: RusshDirectoryListing,
) -> RemoteDirectoryListing {
    RemoteDirectoryListing {
        target_id: target_id.to_string(),
        path: listing.path,
        parent_path: listing.parent_path,
        entries: listing
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
    }
}

fn map_russh_file_preview(target_id: &str, preview: RusshFilePreview) -> RemoteFilePreview {
    RemoteFilePreview {
        target_id: target_id.to_string(),
        path: preview.path,
        content: preview.content,
        is_binary: preview.is_binary,
        truncated: preview.truncated,
    }
}
pub(crate) fn list_remote_directory(
    target_id: &str,
    path: Option<String>,
) -> Result<RemoteDirectoryListing, String> {
    list_remote_directory_via_russh(target_id, path.as_deref())
}

pub(crate) fn read_remote_file_preview(
    target_id: &str,
    path: &str,
) -> Result<RemoteFilePreview, String> {
    read_remote_file_preview_via_russh(target_id, path)
}
