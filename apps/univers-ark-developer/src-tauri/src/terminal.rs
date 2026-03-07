use crate::{
    constants::OUTPUT_BUFFER_LIMIT,
    models::{
        DeveloperTarget, TerminalExitEvent, TerminalOutputEvent, TerminalSession, TerminalSnapshot,
    },
};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::{
    collections::HashMap,
    io::Read,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter};

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
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 32,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to allocate PTY for {}: {}", target.id, error))?;

    let mut command = CommandBuilder::new("/bin/zsh");
    command.arg("-lc");
    command.arg(target.terminal_command.clone());
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");

    pair.slave
        .spawn_command(command)
        .map_err(|error| format!("Failed to start terminal for {}: {}", target.id, error))?;

    let mut reader = pair.master.try_clone_reader().map_err(|error| {
        format!(
            "Failed to open terminal reader for {}: {}",
            target.id, error
        )
    })?;

    let writer = pair.master.take_writer().map_err(|error| {
        format!(
            "Failed to open terminal writer for {}: {}",
            target.id, error
        )
    })?;

    let session = TerminalSession {
        master: Arc::new(Mutex::new(pair.master)),
        writer: Arc::new(Mutex::new(writer)),
        output: Arc::new(Mutex::new(String::new())),
    };

    let output = session.output.clone();
    let app_handle = app.clone();
    let target_id = target.id.clone();

    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        let mut exit_reason = String::from("session closed");

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read_count) => {
                    let chunk = String::from_utf8_lossy(&buffer[..read_count]).to_string();
                    append_output(&output, &chunk);
                    let _ = app_handle.emit(
                        "terminal-output",
                        TerminalOutputEvent {
                            target_id: target_id.clone(),
                            data: chunk,
                        },
                    );
                }
                Err(error) => {
                    exit_reason = format!("terminal read failed: {}", error);
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
