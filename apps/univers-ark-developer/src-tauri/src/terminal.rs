use crate::{
    config::resolve_target_ssh_chain,
    constants::OUTPUT_BUFFER_LIMIT,
    models::{
        DeveloperTarget, LocalTerminalSession, RusshTerminalSession, TerminalExitEvent,
        TerminalOutputEvent, TerminalSession, TerminalSnapshot,
    },
};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::{
    collections::HashMap,
    io::{Read, Write},
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
    if let Ok(chain) = resolve_target_ssh_chain(&target.id) {
        return spawn_russh_terminal_session(app, sessions, target, chain);
    }

    spawn_local_terminal_session(app, sessions, target)
}

pub(crate) fn stop_terminal_session(session: &TerminalSession) {
    if let Some(russh) = &session.russh {
        russh.session.request_stop();
        let _ = russh.session.wait_stopped(Duration::from_secs(2));
    }
}

pub(crate) fn write_to_terminal_session(
    target_id: &str,
    session: &TerminalSession,
    data: &str,
) -> Result<(), String> {
    if let Some(local) = &session.local {
        let mut writer = local
            .writer
            .lock()
            .map_err(|_| format!("Terminal writer is locked for {}", target_id))?;

        writer
            .write_all(data.as_bytes())
            .map_err(|error| format!("Failed to write to {}: {}", target_id, error))?;
        writer
            .flush()
            .map_err(|error| format!("Failed to flush {}: {}", target_id, error))?;
        return Ok(());
    }

    if let Some(russh) = &session.russh {
        russh
            .session
            .write(data.as_bytes().to_vec())
            .map_err(|error| format!("Failed to write to {}: {}", target_id, error))?;
        return Ok(());
    }

    Err(format!("No terminal backend available for {}", target_id))
}

pub(crate) fn resize_terminal_session(
    target_id: &str,
    session: &TerminalSession,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let cols = cols.max(40);
    let rows = rows.max(12);

    if let Some(local) = &session.local {
        let master = local
            .master
            .lock()
            .map_err(|_| format!("Terminal master is locked for {}", target_id))?;

        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("Failed to resize {}: {}", target_id, error))?;
        return Ok(());
    }

    if let Some(russh) = &session.russh {
        russh
            .session
            .resize(cols, rows)
            .map_err(|error| format!("Failed to resize {}: {}", target_id, error))?;
        return Ok(());
    }

    Err(format!("No terminal backend available for {}", target_id))
}

fn spawn_local_terminal_session(
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

    let (program, args) = crate::shell::pty_program_and_args(&target.terminal_command);
    let mut command = CommandBuilder::new(program);
    for arg in args {
        command.arg(arg);
    }
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
        local: Some(LocalTerminalSession {
            master: Arc::new(Mutex::new(pair.master)),
            writer: Arc::new(Mutex::new(writer)),
        }),
        russh: None,
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
    .map_err(|error| format!("Failed to start russh terminal for {}: {}", target.id, error))?;

    let session = TerminalSession {
        local: None,
        russh: Some(RusshTerminalSession {
            session: pty_session.clone(),
        }),
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
