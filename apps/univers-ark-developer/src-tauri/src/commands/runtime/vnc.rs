use crate::{
    machine::resolve_target_ssh_chain,
    models::VncState,
};
use serde::Serialize;
use tauri::State;
use univers_infra_ssh::{start_vnc_ws_forward_chain, ClientOptions as RusshClientOptions};

const DEFAULT_VNC_PORT: u16 = 5900;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VncSessionInfo {
    pub(crate) target_id: String,
    pub(crate) ws_port: u16,
}

#[tauri::command]
pub(crate) async fn start_vnc_session(
    vnc_state: State<'_, VncState>,
    target_id: String,
    vnc_port: Option<u16>,
) -> Result<VncSessionInfo, String> {
    // Check if session already exists
    {
        if let Ok(sessions) = vnc_state.sessions.lock() {
            if let Some(session) = sessions.get(&target_id) {
                if session.forward.is_running() {
                    return Ok(VncSessionInfo {
                        target_id,
                        ws_port: session.local_ws_port,
                    });
                }
            }
        }
    }

    let chain = {
        let target_id = target_id.clone();
        tokio::task::spawn_blocking(move || resolve_target_ssh_chain(&target_id))
            .await
            .map_err(|error| format!("Failed to resolve SSH chain: {}", error))?
    }?;

    let remote_port = vnc_port.unwrap_or(DEFAULT_VNC_PORT);

    let forward = start_vnc_ws_forward_chain(
        &chain,
        "127.0.0.1",
        remote_port,
        &RusshClientOptions::default(),
    )
    .await
    .map_err(|error| format!("Failed to start VNC session: {}", error))?;

    let ws_port = forward.local_port();

    vnc_state
        .sessions
        .lock()
        .map_err(|_| String::from("VNC state is unavailable"))?
        .insert(
            target_id.clone(),
            crate::models::VncSession {
                forward,
                local_ws_port: ws_port,
            },
        );

    Ok(VncSessionInfo {
        target_id,
        ws_port,
    })
}

#[tauri::command]
pub(crate) async fn stop_vnc_session(
    vnc_state: State<'_, VncState>,
    target_id: String,
) -> Result<(), String> {
    let session = vnc_state
        .sessions
        .lock()
        .map_err(|_| String::from("VNC state is unavailable"))?
        .remove(&target_id);

    if let Some(session) = session {
        session.forward.request_stop();
    }

    Ok(())
}
