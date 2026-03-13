use tauri::async_runtime;

#[tauri::command]
pub(crate) async fn restart_container(
    server_id: String,
    container_name: String,
) -> Result<(), String> {
    async_runtime::spawn_blocking(move || {
        crate::machine::restart_container(&server_id, &container_name)
    })
    .await
    .map_err(|error| format!("Failed to join restart container task: {error}"))?
}
