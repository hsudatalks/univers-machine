use super::super::TUNNEL_STOP_WAIT_TIMEOUT;
use crate::models::{RusshTunnelForward, TunnelProcess, TunnelSession};
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc, Mutex},
};

fn tunnel_output_excerpt(output: &Arc<Mutex<String>>) -> Option<String> {
    let current_output = output.lock().ok()?;
    let line = current_output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)?;

    if line.is_empty() {
        return None;
    }

    Some(line.chars().take(220).collect())
}

pub(super) fn tunnel_process_excerpt(processes: &[TunnelProcess]) -> Option<String> {
    processes.iter().find_map(|process| {
        tunnel_output_excerpt(&process.output)
            .map(|excerpt| format!("{}: {}", process.label, excerpt))
    })
}

pub(super) fn russh_forward_excerpt(forwards: &[RusshTunnelForward]) -> Option<String> {
    forwards.iter().find_map(|forward| {
        forward
            .forward
            .last_error()
            .map(|error| format!("{}: {}", forward.label, error))
    })
}

fn tunnel_process_is_alive(process: &TunnelProcess) -> Result<bool, String> {
    let mut child = process
        .child
        .lock()
        .map_err(|_| format!("{} process is unavailable", process.label))?;

    child
        .try_wait()
        .map(|status| status.is_none())
        .map_err(|error| format!("Failed to inspect {} process: {}", process.label, error))
}

pub(crate) fn tunnel_session_is_alive(session: &TunnelSession) -> Result<bool, String> {
    for process in &session.processes {
        if !tunnel_process_is_alive(process)? {
            return Ok(false);
        }
    }

    for forward in &session.russh_forwards {
        if !forward.forward.is_running() {
            return Ok(false);
        }
    }

    if let Some(proxy) = &session.proxy {
        if !proxy.running.load(Ordering::Acquire) {
            return Ok(false);
        }
    }

    Ok(true)
}

pub(super) fn tunnel_session_is_current(
    sessions: &Arc<Mutex<HashMap<String, TunnelSession>>>,
    key: &str,
    session_id: u64,
) -> bool {
    sessions
        .lock()
        .ok()
        .and_then(|active| {
            active
                .get(key)
                .map(|session| session.session_id == session_id)
        })
        .unwrap_or(false)
}

pub(crate) fn remove_tunnel_session_if_current(
    sessions: &Arc<Mutex<HashMap<String, TunnelSession>>>,
    key: &str,
    session_id: u64,
) -> bool {
    let Ok(mut active_sessions) = sessions.lock() else {
        return false;
    };

    match active_sessions.get(key) {
        Some(session) if session.session_id == session_id => {
            active_sessions.remove(key);
            true
        }
        _ => false,
    }
}

pub(crate) fn stop_tunnel_session(session: &TunnelSession) {
    if let Some(proxy) = &session.proxy {
        proxy.stop_requested.store(true, Ordering::Release);
    }

    for forward in &session.russh_forwards {
        forward.forward.request_stop();
    }

    for forward in &session.russh_forwards {
        let _ = forward.forward.wait_stopped(TUNNEL_STOP_WAIT_TIMEOUT);
    }

    for process in &session.processes {
        if let Ok(mut child) = process.child.lock() {
            let _ = child.kill();
        }
    }
}
