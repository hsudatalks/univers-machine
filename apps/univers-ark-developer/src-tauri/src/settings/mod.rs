use crate::{machine::univers_config_dir, models::AppSettings};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

fn settings_file_name() -> &'static str {
    if cfg!(debug_assertions) {
        "univers-ark-developer.dev.settings.json"
    } else {
        "univers-ark-developer.settings.json"
    }
}

fn legacy_settings_file_path<R: Runtime>(app_handle: &AppHandle<R>) -> Option<PathBuf> {
    app_handle
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("settings.json"))
        .filter(|path| path.exists())
}

fn sanitize_theme_mode(theme_mode: &str) -> String {
    match theme_mode {
        "light" | "dark" | "system" => theme_mode.to_string(),
        _ => String::from("system"),
    }
}

fn sanitize_dashboard_refresh_seconds(refresh_seconds: u64) -> u64 {
    match refresh_seconds {
        0 | 15 | 30 | 60 | 300 => refresh_seconds,
        _ => 30,
    }
}

fn sanitize_settings(settings: AppSettings) -> AppSettings {
    AppSettings {
        theme_mode: sanitize_theme_mode(&settings.theme_mode),
        dashboard_refresh_seconds: sanitize_dashboard_refresh_seconds(
            settings.dashboard_refresh_seconds,
        ),
    }
}

fn settings_file_path<R: Runtime>(app_handle: &AppHandle<R>) -> Result<PathBuf, String> {
    let _ = app_handle;
    let app_config_dir = univers_config_dir()?;

    fs::create_dir_all(&app_config_dir).map_err(|error| {
        format!(
            "Failed to create config directory {}: {}",
            app_config_dir.display(),
            error
        )
    })?;

    Ok(app_config_dir.join(settings_file_name()))
}

pub(crate) fn load_app_settings<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> Result<AppSettings, String> {
    let path = settings_file_path(app_handle)?;

    if !path.exists() {
        if let Some(legacy_path) = legacy_settings_file_path(app_handle) {
            fs::copy(&legacy_path, &path).map_err(|error| {
                format!(
                    "Failed to migrate settings from {} to {}: {}",
                    legacy_path.display(),
                    path.display(),
                    error
                )
            })?;
        } else {
            let defaults = AppSettings::default();
            let content = serde_json::to_string_pretty(&defaults)
                .map_err(|error| format!("Failed to serialize default app settings: {error}"))?;
            fs::write(&path, content)
                .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
            return Ok(defaults);
        }
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;

    let settings = serde_json::from_str::<AppSettings>(&content)
        .map_err(|error| format!("Failed to parse {}: {}", path.display(), error))?;

    Ok(sanitize_settings(settings))
}

pub(crate) fn save_app_settings<R: Runtime>(
    app_handle: &AppHandle<R>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let path = settings_file_path(app_handle)?;
    let sanitized = sanitize_settings(settings);
    let content = serde_json::to_string_pretty(&sanitized)
        .map_err(|error| format!("Failed to serialize app settings: {error}"))?;

    fs::write(&path, content)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;

    Ok(sanitized)
}
