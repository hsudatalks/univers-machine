use crate::models::AppSettings;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

const SETTINGS_FILE_NAME: &str = "settings.json";

fn sanitize_theme_mode(theme_mode: &str) -> String {
    match theme_mode {
        "light" | "dark" | "system" => theme_mode.to_string(),
        _ => String::from("system"),
    }
}

fn sanitize_settings(settings: AppSettings) -> AppSettings {
    AppSettings {
        theme_mode: sanitize_theme_mode(&settings.theme_mode),
    }
}

fn settings_file_path<R: Runtime>(app_handle: &AppHandle<R>) -> Result<PathBuf, String> {
    let app_config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|error| format!("Failed to resolve app config directory: {}", error))?;

    fs::create_dir_all(&app_config_dir).map_err(|error| {
        format!(
            "Failed to create app config directory {}: {}",
            app_config_dir.display(),
            error
        )
    })?;

    Ok(app_config_dir.join(SETTINGS_FILE_NAME))
}

pub(crate) fn load_app_settings<R: Runtime>(app_handle: &AppHandle<R>) -> Result<AppSettings, String> {
    let path = settings_file_path(app_handle)?;

    if !path.exists() {
        return Ok(AppSettings::default());
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
        .map_err(|error| format!("Failed to serialize app settings: {}", error))?;

    fs::write(&path, content)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;

    Ok(sanitized)
}
