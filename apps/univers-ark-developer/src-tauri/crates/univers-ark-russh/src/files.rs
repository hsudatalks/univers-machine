use serde::Deserialize;

use crate::{
    exec::execute_chain,
    ssh_config::ResolvedEndpointChain,
    types::{
        ClientOptions, RemoteDirectoryListing, RemoteFileEntry, RemoteFilePreview, RusshError,
    },
};

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
