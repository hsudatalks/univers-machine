use crate::machine::{
    delete_machine_config_view, load_machine_config_document_view, read_targets_config,
    save_targets_config, upsert_machine_config_view,
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
