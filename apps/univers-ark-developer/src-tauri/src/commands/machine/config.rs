use crate::machine::{
    delete_machine_config_view, import_machine_configs_view, load_machine_config_document_view,
    move_machine_config_view, read_targets_config, save_targets_config,
    update_default_profile_view, upsert_machine_config_view, upsert_profile_config_view,
};
use serde_json::Value;

#[tauri::command]
pub(crate) fn load_targets_config() -> Result<String, String> {
    read_targets_config()
}

#[tauri::command]
pub(crate) fn update_targets_config(content: String) -> Result<(), String> {
    save_targets_config(&content)
}

#[tauri::command]
pub(crate) fn load_machine_config_state() -> Result<Value, String> {
    load_machine_config_document_view()
}

#[tauri::command]
pub(crate) fn upsert_machine_config(
    machine: Value,
    previous_machine_id: Option<String>,
) -> Result<Value, String> {
    upsert_machine_config_view(previous_machine_id.as_deref(), machine)
}

#[tauri::command]
pub(crate) fn delete_machine_config(machine_id: String) -> Result<Value, String> {
    delete_machine_config_view(&machine_id)
}

#[tauri::command]
pub(crate) fn import_machine_configs(machines: Vec<Value>) -> Result<Value, String> {
    import_machine_configs_view(machines)
}

#[tauri::command]
pub(crate) fn upsert_profile_config(
    profile_id: String,
    profile: Value,
    previous_profile_id: Option<String>,
) -> Result<Value, String> {
    upsert_profile_config_view(&profile_id, previous_profile_id.as_deref(), profile)
}

#[tauri::command]
pub(crate) fn update_default_profile(
    profile_id: Option<String>,
) -> Result<Value, String> {
    update_default_profile_view(profile_id.as_deref())
}

#[tauri::command]
pub(crate) fn move_machine_config(machine_id: String, direction: i32) -> Result<Value, String> {
    move_machine_config_view(&machine_id, direction)
}
