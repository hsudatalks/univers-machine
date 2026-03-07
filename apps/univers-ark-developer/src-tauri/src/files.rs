use crate::{
    config::run_target_shell_command,
    models::{RemoteDirectoryListing, RemoteFileEntry, RemoteFilePreview},
};
use serde::{de::DeserializeOwned, Deserialize};

const MAX_DIRECTORY_ENTRIES: usize = 512;
const MAX_PREVIEW_BYTES: usize = 131_072;

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

fn run_target_json_command<T: DeserializeOwned>(
    target_id: &str,
    shell_command: &str,
) -> Result<T, String> {
    let output = run_target_shell_command(target_id, shell_command)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("Command exited with {}", output.status)
        };

        return Err(detail);
    }

    serde_json::from_slice::<T>(&output.stdout).map_err(|error| {
        format!(
            "Failed to parse remote response for {}: {}",
            target_id, error
        )
    })
}

fn list_directory_command(path: Option<&str>) -> String {
    let path = path.unwrap_or("~");

    format!(
        r#"UNIVERS_ARK_PATH={} python3 - <<'PY'
import json
import os

path = os.path.abspath(os.path.expanduser(os.environ.get("UNIVERS_ARK_PATH") or "~"))
if not os.path.isdir(path):
    raise SystemExit(f"Not a directory: {{path}}")

entries = []
with os.scandir(path) as iterator:
    sorted_entries = sorted(
        iterator,
        key=lambda entry: (not entry.is_dir(follow_symlinks=False), entry.name.lower()),
    )
    for entry in sorted_entries[:{}]:
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
        MAX_DIRECTORY_ENTRIES,
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
    chunk = file_handle.read({} + 1)

is_binary = b"\x00" in chunk
truncated = len(chunk) > {}
content = ""

if not is_binary:
    content = chunk[:{}].decode("utf-8", errors="replace")

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
        MAX_PREVIEW_BYTES,
        MAX_PREVIEW_BYTES,
        MAX_PREVIEW_BYTES,
    )
}

pub(crate) fn list_remote_directory(
    target_id: &str,
    path: Option<String>,
) -> Result<RemoteDirectoryListing, String> {
    let payload = run_target_json_command::<RemoteDirectoryListingPayload>(
        target_id,
        &list_directory_command(path.as_deref()),
    )?;

    Ok(RemoteDirectoryListing {
        target_id: target_id.to_string(),
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

pub(crate) fn read_remote_file_preview(
    target_id: &str,
    path: &str,
) -> Result<RemoteFilePreview, String> {
    let payload = run_target_json_command::<RemoteFilePreviewPayload>(
        target_id,
        &preview_file_command(path),
    )?;

    Ok(RemoteFilePreview {
        target_id: target_id.to_string(),
        path: payload.path,
        content: payload.content,
        is_binary: payload.is_binary,
        truncated: payload.truncated,
    })
}
