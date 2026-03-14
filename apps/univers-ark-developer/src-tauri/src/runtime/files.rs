use crate::{
    infra::ssh::{
        list_directory_chain_blocking, read_file_preview_chain_blocking, write_file_chain_blocking,
    },
    machine::resolve_target_ssh_chain,
    models::{RemoteDirectoryListing, RemoteFileEntry, RemoteFilePreview},
};
use univers_infra_ssh::{
    ClientOptions as RusshClientOptions, RemoteDirectoryListing as RusshDirectoryListing,
    RemoteFilePreview as RusshFilePreview,
};

fn list_remote_directory_via_russh(
    target_id: &str,
    path: Option<&str>,
) -> Result<RemoteDirectoryListing, String> {
    let chain = resolve_target_ssh_chain(target_id)?;
    let listing = list_directory_chain_blocking(&chain, path, &RusshClientOptions::default())
        .map_err(|error| {
            format!(
                "russh directory listing failed for {target_id}: {error}"
            )
        })?;

    Ok(map_russh_directory_listing(target_id, listing))
}

fn read_remote_file_preview_via_russh(
    target_id: &str,
    path: &str,
) -> Result<RemoteFilePreview, String> {
    let chain = resolve_target_ssh_chain(target_id)?;
    let preview = read_file_preview_chain_blocking(&chain, path, &RusshClientOptions::default())
        .map_err(|error| format!("russh file preview failed for {target_id}: {error}"))?;

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

pub(crate) fn write_remote_file(target_id: &str, path: &str, data: &[u8]) -> Result<(), String> {
    let chain = resolve_target_ssh_chain(target_id)?;
    write_file_chain_blocking(&chain, path, data, &RusshClientOptions::default())
        .map_err(|error| format!("russh file write failed for {target_id}: {error}"))
}
