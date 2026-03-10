use crate::{
    config::{read_server_inventory, resolve_raw_target},
    models::{RemoteDirectoryListing, RemoteFileEntry, RemoteFilePreview},
};
use serde_json;
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

fn resolve_direct_chain(target_id: &str) -> Result<ResolvedEndpointChain, String> {
    let target = resolve_raw_target(target_id)?;
    let resolver = SshConfigResolver::from_default_path()
        .map_err(|e| format!("Failed to load SSH config: {}", e))?;
    resolver
        .resolve(&target.host)
        .map_err(|e| format!("Failed to resolve SSH destination {}: {}", target.host, e))
}

fn resolve_chain(target_id: &str) -> Result<ResolvedEndpointChain, String> {
    resolve_container_chain(target_id).or_else(|_| resolve_direct_chain(target_id))
}


fn list_remote_directory_via_russh(
    target_id: &str,
    path: Option<&str>,
) -> Result<RemoteDirectoryListing, String> {
    let chain = resolve_chain(target_id)?;
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
    let chain = resolve_chain(target_id)?;
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
// ── Docker exec-based file browsing ──────────────────────────────────────────

const DOCKER_LIST_SCRIPT: &str = r#"
import json, os, sys
path = os.path.abspath(os.path.expanduser(sys.argv[1] if len(sys.argv) > 1 else '~/repos'))
if not os.path.isdir(path):
    path = os.path.expanduser('~')
parent = os.path.dirname(path) if path != '/' else None
entries = []
try:
    with os.scandir(path) as it:
        for e in sorted(it, key=lambda e: (not e.is_dir(follow_symlinks=False), e.name.lower()))[:500]:
            try:
                s = e.stat(follow_symlinks=False)
                if e.is_dir(follow_symlinks=False): kind = 'directory'
                elif e.is_file(follow_symlinks=False): kind = 'file'
                else: kind = 'symlink'
                entries.append({'name': e.name, 'path': e.path, 'kind': kind, 'size': s.st_size, 'isHidden': e.name.startswith('.')})
            except: pass
except Exception as ex:
    print(json.dumps({'error': str(ex)})); sys.exit(1)
print(json.dumps({'path': path, 'parentPath': parent, 'entries': entries}))
"#;

const DOCKER_PREVIEW_SCRIPT: &str = r#"
import json, os, sys
path = sys.argv[1]
max_bytes = 65536
try:
    with open(path, 'rb') as f:
        raw = f.read(max_bytes + 1)
    truncated = len(raw) > max_bytes
    raw = raw[:max_bytes]
    is_binary = b'\x00' in raw
    content = '' if is_binary else raw.decode('utf-8', errors='replace')
    print(json.dumps({'path': path, 'content': content, 'isBinary': is_binary, 'truncated': truncated}))
except Exception as ex:
    print(json.dumps({'error': str(ex)})); import sys; sys.exit(1)
"#;

fn docker_exec_json(container_name: &str, script: &str, arg: Option<&str>) -> Result<serde_json::Value, String> {
    let mut args = vec!["exec", container_name, "python3", "-c", script];
    if let Some(a) = arg {
        args.push(a);
    }
    let output = std::process::Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("docker exec failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("docker exec error: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let val: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("Failed to parse docker output: {}: {}", e, stdout.trim()))?;

    if let Some(err) = val.get("error").and_then(|v| v.as_str()) {
        return Err(err.to_string());
    }

    Ok(val)
}

fn list_remote_directory_via_docker(
    target_id: &str,
    container_name: &str,
    path: Option<&str>,
) -> Result<RemoteDirectoryListing, String> {
    let val = docker_exec_json(container_name, DOCKER_LIST_SCRIPT, path)?;

    let resolved_path = val["path"].as_str().unwrap_or("/").to_string();
    let parent_path = val["parentPath"].as_str().map(|s| s.to_string());
    let entries = val["entries"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(RemoteFileEntry {
                        name: e["name"].as_str()?.to_string(),
                        path: e["path"].as_str()?.to_string(),
                        kind: e["kind"].as_str().unwrap_or("file").to_string(),
                        size: e["size"].as_u64().unwrap_or(0),
                        is_hidden: e["isHidden"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(RemoteDirectoryListing {
        target_id: target_id.to_string(),
        path: resolved_path,
        parent_path,
        entries,
    })
}

fn read_remote_file_preview_via_docker(
    target_id: &str,
    container_name: &str,
    path: &str,
) -> Result<RemoteFilePreview, String> {
    let val = docker_exec_json(container_name, DOCKER_PREVIEW_SCRIPT, Some(path))?;

    Ok(RemoteFilePreview {
        target_id: target_id.to_string(),
        path: val["path"].as_str().unwrap_or(path).to_string(),
        content: val["content"].as_str().unwrap_or("").to_string(),
        is_binary: val["isBinary"].as_bool().unwrap_or(false),
        truncated: val["truncated"].as_bool().unwrap_or(false),
    })
}

fn docker_container_name(target_id: &str) -> Option<&str> {
    target_id.strip_prefix("docker-")
}

fn is_localhost_target(target_id: &str) -> bool {
    resolve_raw_target(target_id)
        .map(|t| matches!(t.host.as_str(), "localhost" | "127.0.0.1" | "::1"))
        .unwrap_or(false)
}

// ── Local filesystem browsing ─────────────────────────────────────────────────

fn expand_home(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    std::path::PathBuf::from(path)
}

fn list_local_directory(
    target_id: &str,
    path: Option<&str>,
    files_root: &str,
) -> Result<RemoteDirectoryListing, String> {
    let requested = path.unwrap_or(files_root);
    let dir = expand_home(requested);
    let dir = if dir.is_dir() {
        dir
    } else {
        expand_home(files_root)
    };
    let dir = if dir.is_dir() {
        dir
    } else {
        dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
    };

    let parent_path = dir.parent().map(|p| p.to_string_lossy().into_owned());
    let dir_str = dir.to_string_lossy().into_owned();

    let mut entries: Vec<(bool, String, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| format!("Cannot read directory {}: {}", dir_str, e))?
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let name = entry.file_name().to_string_lossy().into_owned();
            (is_dir, name, entry.path())
        })
        .collect();

    entries.sort_by(|(a_dir, a_name, _), (b_dir, b_name, _)| {
        b_dir.cmp(a_dir).then(a_name.to_lowercase().cmp(&b_name.to_lowercase()))
    });

    let file_entries = entries
        .into_iter()
        .take(500)
        .map(|(is_dir, name, path)| {
            let size = if is_dir {
                0
            } else {
                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            };
            let kind = if is_dir { "directory" } else { "file" }.to_string();
            let is_hidden = name.starts_with('.');
            RemoteFileEntry {
                path: path.to_string_lossy().into_owned(),
                name,
                kind,
                size,
                is_hidden,
            }
        })
        .collect();

    Ok(RemoteDirectoryListing {
        target_id: target_id.to_string(),
        path: dir_str,
        parent_path,
        entries: file_entries,
    })
}

fn read_local_file_preview(target_id: &str, path: &str) -> Result<RemoteFilePreview, String> {
    const MAX_BYTES: usize = 65536;
    let raw = std::fs::read(path).map_err(|e| format!("Cannot read {}: {}", path, e))?;
    let truncated = raw.len() > MAX_BYTES;
    let raw = &raw[..raw.len().min(MAX_BYTES)];
    let is_binary = raw.contains(&0u8);
    let content = if is_binary {
        String::new()
    } else {
        String::from_utf8_lossy(raw).into_owned()
    };
    Ok(RemoteFilePreview {
        target_id: target_id.to_string(),
        path: path.to_string(),
        content,
        is_binary,
        truncated,
    })
}

// ── Public API ────────────────────────────────────────────────────────────────

pub(crate) fn list_remote_directory(
    target_id: &str,
    path: Option<String>,
) -> Result<RemoteDirectoryListing, String> {
    if let Some(name) = docker_container_name(target_id) {
        return list_remote_directory_via_docker(target_id, name, path.as_deref());
    }
    if is_localhost_target(target_id) {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string());
        return list_local_directory(target_id, path.as_deref(), &home);
    }
    list_remote_directory_via_russh(target_id, path.as_deref())
}

pub(crate) fn read_remote_file_preview(
    target_id: &str,
    path: &str,
) -> Result<RemoteFilePreview, String> {
    if let Some(name) = docker_container_name(target_id) {
        return read_remote_file_preview_via_docker(target_id, name, path);
    }
    if is_localhost_target(target_id) {
        return read_local_file_preview(target_id, path);
    }
    read_remote_file_preview_via_russh(target_id, path)
}
