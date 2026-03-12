use super::targets_file_name;
use serde_json::Value;
use std::{fs, path::PathBuf, sync::OnceLock};
use tauri::{AppHandle, Manager, Runtime, path::BaseDirectory};

const BUNDLED_TARGETS_TEMPLATE_NAME: &str = "developer-targets.json";

fn configured_targets_path() -> &'static OnceLock<PathBuf> {
    static CONFIGURED_TARGETS_PATH: OnceLock<PathBuf> = OnceLock::new();
    &CONFIGURED_TARGETS_PATH
}

pub(crate) fn univers_config_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| String::from("Failed to resolve user home directory"))?;

    Ok(home.join(".univers"))
}

pub(super) fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub(crate) fn targets_file_path() -> PathBuf {
    configured_targets_path().get().cloned().unwrap_or_else(|| {
        univers_config_dir()
            .map(|dir| dir.join(targets_file_name()))
            .unwrap_or_else(|_| app_root().join(targets_file_name()))
    })
}

fn bundled_targets_file_path<R: Runtime>(app_handle: &AppHandle<R>) -> PathBuf {
    app_handle
        .path()
        .resolve(BUNDLED_TARGETS_TEMPLATE_NAME, BaseDirectory::Resource)
        .unwrap_or_else(|_| app_root().join(BUNDLED_TARGETS_TEMPLATE_NAME))
}

fn legacy_targets_file_path<R: Runtime>(app_handle: &AppHandle<R>) -> Option<PathBuf> {
    app_handle
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(BUNDLED_TARGETS_TEMPLATE_NAME))
        .filter(|path| path.exists())
}

pub(super) fn initialize_targets_file_storage<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> Result<PathBuf, String> {
    let app_config_dir = univers_config_dir()?;

    fs::create_dir_all(&app_config_dir).map_err(|error| {
        format!(
            "Failed to create config directory {}: {}",
            app_config_dir.display(),
            error
        )
    })?;

    let writable_targets_path = app_config_dir.join(targets_file_name());

    if !writable_targets_path.exists() {
        let source_path = legacy_targets_file_path(app_handle)
            .unwrap_or_else(|| bundled_targets_file_path(app_handle));

        fs::copy(&source_path, &writable_targets_path).map_err(|error| {
            format!(
                "Failed to initialize targets file from {} to {}: {}",
                source_path.display(),
                writable_targets_path.display(),
                error
            )
        })?;
    }

    let _ = configured_targets_path().set(writable_targets_path.clone());
    Ok(writable_targets_path)
}

fn sanitize_workspace_aliases(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(legacy_value) = map.remove("primaryBrowserServiceId") {
                map.entry(String::from("primaryWebServiceId"))
                    .or_insert(legacy_value);
            }

            map.values_mut().for_each(sanitize_workspace_aliases);
        }
        Value::Array(items) => items.iter_mut().for_each(sanitize_workspace_aliases),
        _ => {}
    }
}

pub(super) fn sanitize_targets_json_content(content: &str) -> Result<String, String> {
    let mut value: Value =
        serde_json::from_str(content).map_err(|error| format!("Invalid config JSON: {}", error))?;
    sanitize_workspace_aliases(&mut value);
    serde_json::to_string_pretty(&value)
        .map_err(|error| format!("Failed to serialize sanitized config JSON: {}", error))
}

pub(super) fn read_targets_file_content() -> Result<String, String> {
    let config_path = targets_file_path();
    fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))
}

pub(super) fn write_targets_file_content(content: &str) -> Result<(), String> {
    let config_path = targets_file_path();
    fs::write(&config_path, content)
        .map_err(|error| format!("Failed to write {}: {}", config_path.display(), error))
}
