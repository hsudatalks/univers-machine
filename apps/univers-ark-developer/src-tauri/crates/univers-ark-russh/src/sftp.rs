use russh_sftp::client::SftpSession;

use crate::{
    connection::connect_chain,
    ssh_config::ResolvedEndpointChain,
    types::{ClientOptions, RemoteDirectoryListing, RemoteFileEntry, RemoteFilePreview, RusshError},
};
use tokio::io::AsyncWriteExt;

const MAX_DIRECTORY_ENTRIES: usize = 512;
const MAX_PREVIEW_BYTES: usize = 131_072;
const DEFAULT_REMOTE_ROOT: &str = "~";

pub async fn sftp_list_directory(
    chain: &ResolvedEndpointChain,
    path: Option<&str>,
    options: &ClientOptions,
) -> Result<RemoteDirectoryListing, RusshError> {
    let client = connect_chain(chain, options).await?;

    let channel = client.handle.channel_open_session().await.map_err(|e| {
        RusshError::Sftp(format!("failed to open channel: {e}"))
    })?;

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to request sftp subsystem: {e}")))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to create sftp session: {e}")))?;

    let path = path.unwrap_or(DEFAULT_REMOTE_ROOT);
    let remote_path = expand_remote_path(&sftp, path).await?;

    // read_dir returns a synchronous iterator
    let dir = sftp
        .read_dir(&remote_path)
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to read directory: {e}")))?;

    // Collect entries
    let mut entries_vec: Vec<_> = dir.collect();

    // Sort: directories first, then by name (case-insensitive)
    entries_vec.sort_by(|a, b| {
        let a_name = a.file_name();
        let b_name = b.file_name();
        let a_is_dir = a.file_type().is_dir();
        let b_is_dir = b.file_type().is_dir();
        if a_is_dir != b_is_dir {
            b_is_dir.cmp(&a_is_dir)
        } else {
            a_name.to_lowercase().cmp(&b_name.to_lowercase())
        }
    });

    let mut entries = Vec::new();

    for entry in entries_vec.into_iter().take(MAX_DIRECTORY_ENTRIES) {
        let name = entry.file_name();
        let is_hidden = name.starts_with('.');

        let kind = if entry.file_type().is_dir() {
            "directory"
        } else if entry.file_type().is_file() {
            "file"
        } else if entry.file_type().is_symlink() {
            "symlink"
        } else {
            "other"
        };

        let size = entry.metadata().len();

        // Build full path from directory path and file name
        let full_path = if remote_path.ends_with('/') {
            format!("{remote_path}{name}")
        } else {
            format!("{remote_path}/{name}")
        };

        entries.push(RemoteFileEntry {
            name,
            path: full_path,
            kind: kind.to_string(),
            size,
            is_hidden,
        });
    }

    let parent_path = std::path::Path::new(&remote_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string());

    Ok(RemoteDirectoryListing {
        path: remote_path,
        parent_path,
        entries,
    })
}

pub async fn sftp_read_file_preview(
    chain: &ResolvedEndpointChain,
    path: &str,
    options: &ClientOptions,
) -> Result<RemoteFilePreview, RusshError> {
    let client = connect_chain(chain, options).await?;

    let channel = client.handle.channel_open_session().await.map_err(|e| {
        RusshError::Sftp(format!("failed to open channel: {e}"))
    })?;

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to request sftp subsystem: {e}")))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to create sftp session: {e}")))?;

    let remote_path = expand_remote_path(&sftp, path).await?;

    let mut file = sftp
        .open(&remote_path)
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to open file: {e}")))?;

    use tokio::io::AsyncReadExt;
    let mut buffer = vec![0u8; MAX_PREVIEW_BYTES + 1];
    let bytes_read = file
        .read(&mut buffer)
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to read file: {e}")))?;

    let is_binary = buffer[..bytes_read].contains(&0);
    let truncated = bytes_read > MAX_PREVIEW_BYTES;
    let content = if is_binary {
        String::new()
    } else {
        String::from_utf8_lossy(&buffer[..bytes_read.min(MAX_PREVIEW_BYTES)]).to_string()
    };

    Ok(RemoteFilePreview {
        path: remote_path,
        content,
        is_binary,
        truncated,
    })
}

pub async fn sftp_write_file(
    chain: &ResolvedEndpointChain,
    path: &str,
    data: &[u8],
    options: &ClientOptions,
) -> Result<(), RusshError> {
    let client = connect_chain(chain, options).await?;

    let channel = client.handle.channel_open_session().await.map_err(|e| {
        RusshError::Sftp(format!("failed to open channel: {e}"))
    })?;

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to request sftp subsystem: {e}")))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to create sftp session: {e}")))?;

    let remote_path = expand_remote_path(&sftp, path).await?;
    ensure_parent_directories(&sftp, &remote_path).await?;

    let mut file = sftp
        .create(&remote_path)
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to create file: {e}")))?;

    file.write_all(data)
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to write file: {e}")))?;
    file.flush()
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to flush file: {e}")))?;
    file.shutdown()
        .await
        .map_err(|e| RusshError::Sftp(format!("failed to close file: {e}")))?;

    Ok(())
}

async fn ensure_parent_directories(
    sftp: &SftpSession,
    remote_path: &str,
) -> Result<(), RusshError> {
    let Some(parent_path) = std::path::Path::new(remote_path).parent() else {
        return Ok(());
    };

    let parent = parent_path.to_string_lossy();
    if parent.is_empty() || parent == "." {
        return Ok(());
    }

    let mut current = String::new();

    for component in parent.split('/') {
        if component.is_empty() {
            continue;
        }

        if current.is_empty() {
            current.push_str(component);
        } else {
            current.push('/');
            current.push_str(component);
        }

        let exists = sftp
            .try_exists(&current)
            .await
            .map_err(|e| RusshError::Sftp(format!("failed to stat directory: {e}")))?;

        if !exists {
            sftp.create_dir(&current)
                .await
                .map_err(|e| RusshError::Sftp(format!("failed to create directory: {e}")))?;
        }
    }

    Ok(())
}

async fn expand_remote_path(_sftp: &SftpSession, path: &str) -> Result<String, RusshError> {
    if path.starts_with('~') {
        // Try to get the home directory by opening "."
        // The SFTP session typically starts at the user's home directory
        if path == "~" {
            return Ok(".".to_string());
        }

        // For ~/path
        let rest = &path[2..]; // Remove ~/
        return Ok(format!("./{rest}"));
    }

    if path.starts_with('/') {
        return Ok(path.to_string());
    }

    // Relative path - use default remote root (~)
    // Since ~ expands to current directory in SFTP, just use the path as-is
    if DEFAULT_REMOTE_ROOT == "~" {
        return Ok(format!("./{path}"));
    }

    Ok(format!("{DEFAULT_REMOTE_ROOT}/{path}"))
}
