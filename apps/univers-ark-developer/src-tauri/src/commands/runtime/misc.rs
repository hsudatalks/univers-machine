use super::{CommandServiceActionSpec, RuntimeActivityInput};
use crate::{
    machine::resolve_raw_target,
    models::RuntimeActivityState,
    runtime::activity::update_runtime_activity as apply_runtime_activity,
    services::{actions::execute_command_service_action, catalog::tmux_command_service},
};
use tauri::{async_runtime, AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[tauri::command]
pub(crate) async fn restart_tmux(app: AppHandle, target_id: String) -> Result<(), String> {
    async_runtime::spawn_blocking(move || {
        let target = resolve_raw_target(&target_id)?;
        let service_id = tmux_command_service(&target)
            .map(|service| service.id.clone())
            .unwrap_or_else(|| String::from("tmux-developer"));

        execute_command_service_action(Some(&app), &target_id, &service_id, "restart")
    })
    .await
    .map_err(|error| format!("Failed to join restart tmux task: {}", error))?
}

#[tauri::command]
pub(crate) async fn execute_command_service(
    app: AppHandle,
    spec: CommandServiceActionSpec,
) -> Result<(), String> {
    async_runtime::spawn_blocking(move || {
        execute_command_service_action(Some(&app), &spec.target_id, &spec.service_id, &spec.action)
    })
    .await
    .map_err(|error| format!("Failed to join command service task: {}", error))?
}

#[tauri::command]
pub(crate) async fn clipboard_write(app: AppHandle, text: String) -> Result<(), String> {
    async_runtime::spawn_blocking(move || {
        app.clipboard()
            .write_text(text)
            .map_err(|error| format!("Failed to write to clipboard: {}", error))
    })
    .await
    .map_err(|error| format!("Failed to join clipboard write task: {}", error))?
}

#[tauri::command]
pub(crate) async fn clipboard_read(app: AppHandle) -> Result<String, String> {
    async_runtime::spawn_blocking(move || {
        app.clipboard()
            .read_text()
            .map_err(|error| format!("Failed to read clipboard: {}", error))
    })
    .await
    .map_err(|error| format!("Failed to join clipboard read task: {}", error))?
}

#[tauri::command]
pub(crate) fn update_runtime_activity(
    activity: RuntimeActivityInput,
    activity_state: State<'_, RuntimeActivityState>,
) -> Result<(), String> {
    apply_runtime_activity(
        activity_state.inner(),
        activity.visible,
        activity.focused,
        activity.online,
        activity.active_machine_id,
        activity.active_target_id,
    );
    Ok(())
}
