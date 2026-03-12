use crate::{
    constants::OUTPUT_BUFFER_LIMIT,
    machine::resolve_target_ssh_chain,
    models::{
        DeveloperTarget, RusshTerminalSession, TerminalExitEvent, TerminalOutputEvent,
        TerminalSession, TerminalSnapshot,
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use univers_ark_russh::{
    start_pty_session_chain, ClientOptions as RusshClientOptions, PtySessionEvent,
    ResolvedEndpointChain,
};

pub(crate) fn append_output(output: &Arc<Mutex<String>>, chunk: &str) {
    if let Ok(mut current_output) = output.lock() {
        current_output.push_str(chunk);

        if current_output.len() > OUTPUT_BUFFER_LIMIT {
            let mut drain_until = current_output.len() - OUTPUT_BUFFER_LIMIT;

            while drain_until < current_output.len()
                && !current_output.is_char_boundary(drain_until)
            {
                drain_until += 1;
            }

            current_output.drain(..drain_until);
        }
    }
}

pub(crate) fn snapshot_for(target_id: &str, session: &TerminalSession) -> TerminalSnapshot {
    let output = session
        .output
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default();

    TerminalSnapshot {
        target_id: target_id.to_string(),
        output,
    }
}

pub(crate) fn spawn_terminal_session(
    app: &AppHandle,
    sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
    target: &DeveloperTarget,
) -> Result<TerminalSession, String> {
    let chain = resolve_target_ssh_chain(&target.id)?;
    spawn_russh_terminal_session(app, sessions, target, chain)
}

pub(crate) fn stop_terminal_session(session: &TerminalSession) {
    session.russh.session.request_stop();
    let _ = session.russh.session.wait_stopped(Duration::from_secs(2));
}

pub(crate) fn write_to_terminal_session(
    target_id: &str,
    session: &TerminalSession,
    data: &str,
) -> Result<(), String> {
    session
        .russh
        .session
        .write(data.as_bytes().to_vec())
        .map_err(|error| format!("Failed to write to {}: {}", target_id, error))
}

pub(crate) fn resize_terminal_session(
    target_id: &str,
    session: &TerminalSession,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let cols = cols.max(40);
    let rows = rows.max(12);

    session
        .russh
        .session
        .resize(cols, rows)
        .map_err(|error| format!("Failed to resize {}: {}", target_id, error))
}

fn spawn_russh_terminal_session(
    app: &AppHandle,
    sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
    target: &DeveloperTarget,
    chain: ResolvedEndpointChain,
) -> Result<TerminalSession, String> {
    let (pty_session, receiver) = start_pty_session_chain(
        &chain,
        &target.terminal_startup_command,
        120,
        32,
        &RusshClientOptions::default(),
    )
    .map_err(|error| {
        format!(
            "Failed to start russh terminal for {}: {}",
            target.id, error
        )
    })?;

    let session = TerminalSession {
        russh: RusshTerminalSession {
            session: pty_session.clone(),
        },
        output: Arc::new(Mutex::new(String::new())),
    };

    let output = session.output.clone();
    let app_handle = app.clone();
    let target_id = target.id.clone();

    std::thread::spawn(move || {
        let mut exit_reason = String::from("terminal session closed");

        while let Ok(event) = receiver.recv() {
            match event {
                PtySessionEvent::Output(data) => {
                    let chunk = String::from_utf8_lossy(&data).to_string();
                    append_output(&output, &chunk);
                    let _ = app_handle.emit(
                        "terminal-output",
                        TerminalOutputEvent {
                            target_id: target_id.clone(),
                            data: chunk,
                        },
                    );
                }
                PtySessionEvent::Exit(reason) => {
                    exit_reason = reason;
                    break;
                }
            }
        }

        let _ = app_handle.emit(
            "terminal-exit",
            TerminalExitEvent {
                target_id: target_id.clone(),
                reason: exit_reason,
            },
        );

        if let Ok(mut active_sessions) = sessions.lock() {
            active_sessions.remove(&target_id);
        }
    });

    Ok(session)
}
