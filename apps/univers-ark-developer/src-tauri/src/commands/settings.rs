use crate::{
    models::AppSettings,
    settings::{load_app_settings as read_app_settings, save_app_settings as write_app_settings},
};
use tauri::AppHandle;

#[tauri::command]
pub(crate) fn load_app_settings(app_handle: AppHandle) -> Result<AppSettings, String> {
    read_app_settings(&app_handle)
}

#[tauri::command]
pub(crate) fn save_app_settings(
    app_handle: AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    write_app_settings(&app_handle, settings)
}
