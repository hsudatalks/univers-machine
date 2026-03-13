use super::{
    spawn_russh_forward,
    targets::{parse_forward_target, remote_forward_target},
};
use crate::{
    models::{BrowserSurface, TunnelSession, TunnelState},
    services::runtime::{
        allocate_internal_tunnel_port, internal_probe_url, resolve_runtime_vite_hmr_tunnel_command,
        surface_local_port,
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Runtime};

use super::super::{
    proxy::start_vite_proxy,
    session::{spawn_managed_tunnel_session, ManagedTunnelSessionSpec},
};

pub(super) fn vite_hmr_forward_target(surface: &BrowserSurface) -> Result<(String, u16), String> {
    if !surface.vite_hmr_tunnel_command.trim().is_empty() {
        return parse_forward_target(&surface.vite_hmr_tunnel_command);
    }

    let (remote_host, remote_port) = remote_forward_target(surface)?;
    let hmr_remote_port = remote_port.checked_add(1).ok_or_else(|| {
        format!(
            "Failed to derive Vite HMR port for {} from {}.",
            surface.id, remote_port
        )
    })?;

    Ok((remote_host, hmr_remote_port))
}

pub(super) fn spawn_vite_proxy_session<R: Runtime>(
    app: &AppHandle<R>,
    sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    tunnel_state: &TunnelState,
    session_id: u64,
    target_id: &str,
    surface: &BrowserSurface,
) -> Result<TunnelSession, String> {
    let public_port = surface_local_port(surface)?;
    let http_forward_port =
        allocate_internal_tunnel_port(tunnel_state, target_id, &surface.id, "http-forward")?;
    let hmr_forward_port =
        allocate_internal_tunnel_port(tunnel_state, target_id, &surface.id, "vite-hmr")?;

    let (remote_host, remote_port) = remote_forward_target(surface)?;
    let (hmr_remote_host, hmr_remote_port) = if !surface.vite_hmr_tunnel_command.trim().is_empty() {
        parse_forward_target(&resolve_runtime_vite_hmr_tunnel_command(
            &surface.vite_hmr_tunnel_command,
            hmr_forward_port,
        ))?
    } else {
        vite_hmr_forward_target(surface)?
    };

    let local_http_bind = format!("127.0.0.1:{http_forward_port}");
    let http_forward = spawn_russh_forward(
        target_id,
        &local_http_bind,
        &remote_host,
        remote_port,
        format!("{} HTTP tunnel", surface.label),
    )?;

    let local_hmr_bind = format!("127.0.0.1:{hmr_forward_port}");
    let hmr_forward = match spawn_russh_forward(
        target_id,
        &local_hmr_bind,
        &hmr_remote_host,
        hmr_remote_port,
        format!("{} HMR tunnel", surface.label),
    ) {
        Ok(forward) => forward,
        Err(error) => {
            http_forward.forward.request_stop();
            return Err(error);
        }
    };

    let russh_forwards = vec![http_forward, hmr_forward];
    let proxy = match start_vite_proxy(public_port, http_forward_port, hmr_forward_port) {
        Ok(proxy) => proxy,
        Err(error) => {
            for forward in &russh_forwards {
                forward.forward.request_stop();
            }
            return Err(error);
        }
    };

    Ok(spawn_managed_tunnel_session(
        app,
        sessions,
        tunnel_state.status_snapshots.clone(),
        tunnel_state.telemetry.clone(),
        ManagedTunnelSessionSpec {
            session_id,
            target_id: target_id.to_string(),
            surface: surface.clone(),
            processes: Vec::new(),
            russh_forwards,
            proxy: Some(proxy),
            probe_urls: vec![
                internal_probe_url(http_forward_port),
                internal_probe_url(hmr_forward_port),
            ],
        },
    ))
}
