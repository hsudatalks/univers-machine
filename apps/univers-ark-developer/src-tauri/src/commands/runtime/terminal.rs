use crate::{
    machine::resolve_raw_target,
    models::{RemoteDirectoryListing, RemoteFilePreview, TerminalSnapshot, TerminalState},
    runtime::files::{
        list_remote_directory as load_remote_directory,
        read_remote_file_preview as load_remote_file_preview,
    },
    runtime::terminal::{
        resize_terminal_session, snapshot_for, spawn_terminal_session, stop_terminal_session,
        write_to_terminal_session,
    },
};
use tauri::{async_runtime, AppHandle, State};

#[tauri::command]
pub(crate) async fn attach_terminal(
    app: AppHandle,
    terminal_state: State<'_, TerminalState>,
    target_id: String,
) -> Result<TerminalSnapshot, String> {
    let sessions_arc = terminal_state.sessions.clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        let mut sessions = sessions_arc
            .lock()
            .map_err(|_| String::from("Terminal session state is unavailable"))?;

        if let Some(session) = sessions.get(&target_id) {
            return Ok(snapshot_for(&target_id, session));
        }

        let target = resolve_raw_target(&target_id)?;
        let session = spawn_terminal_session(&app_clone, sessions_arc.clone(), &target)?;
        let snapshot = snapshot_for(&target_id, &session);
        sessions.insert(target_id.clone(), session);

        Ok(snapshot)
    })
    .await
    .map_err(|error| format!("Failed to join attach terminal task: {}", error))?
}

#[tauri::command]
pub(crate) async fn restart_terminal(
    app: AppHandle,
    terminal_state: State<'_, TerminalState>,
    target_id: String,
) -> Result<TerminalSnapshot, String> {
    let sessions_arc = terminal_state.sessions.clone();
    let app_clone = app.clone();

    async_runtime::spawn_blocking(move || {
        let old_session = sessions_arc
            .lock()
            .map_err(|_| String::from("Terminal session state is unavailable"))?
            .remove(&target_id);

        if let Some(session) = old_session.as_ref() {
            stop_terminal_session(session);
        }
        drop(old_session);

        let target = resolve_raw_target(&target_id)?;
        let session = spawn_terminal_session(&app_clone, sessions_arc.clone(), &target)?;
        let snapshot = snapshot_for(&target_id, &session);

        sessions_arc
            .lock()
            .map_err(|_| String::from("Terminal session state is unavailable"))?
            .insert(target_id.clone(), session);

        Ok(snapshot)
    })
    .await
    .map_err(|error| format!("Failed to join restart terminal task: {}", error))?
}

#[tauri::command]
pub(crate) fn write_terminal(
    terminal_state: State<TerminalState>,
    target_id: String,
    data: String,
) -> Result<(), String> {
    let session = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Terminal session state is unavailable"))?
        .get(&target_id)
        .cloned()
        .ok_or_else(|| format!("No active terminal session for {}", target_id))?;

    write_to_terminal_session(&target_id, &session, &data)
}

#[tauri::command]
pub(crate) fn resize_terminal(
    terminal_state: State<TerminalState>,
    target_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let session = terminal_state
        .sessions
        .lock()
        .map_err(|_| String::from("Terminal session state is unavailable"))?
        .get(&target_id)
        .cloned()
        .ok_or_else(|| format!("No active terminal session for {}", target_id))?;

    resize_terminal_session(&target_id, &session, cols, rows)
}

#[tauri::command]
pub(crate) async fn list_remote_directory(
    target_id: String,
    path: Option<String>,
) -> Result<RemoteDirectoryListing, String> {
    async_runtime::spawn_blocking(move || load_remote_directory(&target_id, path))
        .await
        .map_err(|error| format!("Failed to join remote directory task: {}", error))?
}

#[tauri::command]
pub(crate) async fn read_remote_file_preview(
    target_id: String,
    path: String,
) -> Result<RemoteFilePreview, String> {
    async_runtime::spawn_blocking(move || load_remote_file_preview(&target_id, &path))
        .await
        .map_err(|error| format!("Failed to join remote file preview task: {}", error))?
}
