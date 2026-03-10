use crate::{
    config::resolve_target_ssh_chain,
    models::{RemoteDirectoryListing, RemoteFileEntry, RemoteFilePreview},
};
use univers_ark_russh::{
    list_directory_chain, read_file_preview_chain, ClientOptions as RusshClientOptions,
    RemoteDirectoryListing as RusshDirectoryListing, RemoteFilePreview as RusshFilePreview,
};

fn list_remote_directory_via_russh(
    target_id: &str,
    path: Option<&str>,
) -> Result<RemoteDirectoryListing, String> {
    let chain = resolve_target_ssh_chain(target_id)?;
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
        .map_err(|error| {
            format!(
                "russh directory listing failed for {}: {}",
                target_id, error
            )
        })?;

    Ok(map_russh_directory_listing(target_id, listing))
}

fn read_remote_file_preview_via_russh(
    target_id: &str,
    path: &str,
) -> Result<RemoteFilePreview, String> {
    let chain = resolve_target_ssh_chain(target_id)?;
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
