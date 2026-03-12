use super::{session::spawn_managed_tunnel_session, starting_tunnel_status};
use crate::{
    infra::russh::start_local_forward_chain_blocking,
    machine::{resolve_raw_target, resolve_target_ssh_chain},
    models::{
        BrowserServiceType, BrowserSurface, MachineTransport, RusshTunnelForward, TunnelSession,
        TunnelState, TunnelStatus,
    },
    proxy::start_vite_proxy,
    services::runtime::{
        allocate_internal_tunnel_port, internal_probe_url, resolve_runtime_vite_hmr_tunnel_command,
        surface_key, surface_local_port,
    },
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Runtime};
use univers_ark_russh::{ClientOptions as RusshClientOptions, ResolvedEndpointChain};
use url::Url;

fn resolve_container_chain(target_id: &str) -> Result<ResolvedEndpointChain, String> {
    resolve_target_ssh_chain(target_id)
}

pub(super) fn should_manage_runtime_surface_tunnel(
    target_id: &str,
    surface: &BrowserSurface,
) -> Result<bool, String> {
    if !surface.tunnel_command.trim().is_empty() {
        return Ok(true);
    }

    let target = resolve_raw_target(target_id)?;
    Ok(matches!(target.transport, MachineTransport::Ssh))
}

fn parse_forward_target(command_line: &str) -> Result<(String, u16), String> {
    let tokens = command_line.split_whitespace().collect::<Vec<_>>();

    for index in 0..tokens.len() {
        let forward_spec = if tokens[index] == "-L" {
            tokens.get(index + 1).copied()
        } else {
            tokens[index].strip_prefix("-L")
        };

        let Some(forward_spec) = forward_spec else {
            continue;
        };

        let Some((before_port, remote_port)) = forward_spec.rsplit_once(':') else {
            continue;
        };
        let remote_port = remote_port.parse::<u16>().map_err(|error| {
            format!("Invalid remote forward port in {}: {}", forward_spec, error)
        })?;
        let Some(remote_host) = before_port.rsplit(':').next() else {
            continue;
        };

        return Ok((remote_host.to_string(), remote_port));
    }

    Err(format!(
        "Failed to parse -L forward target from tunnel command: {}",
        command_line
    ))
}

fn remote_forward_target(surface: &BrowserSurface) -> Result<(String, u16), String> {
    let remote_url = Url::parse(&surface.remote_url).map_err(|error| {
        format!(
            "Failed to parse remote URL for {} surface: {}",
            surface.id, error
        )
    })?;
    let remote_host = remote_url
        .host_str()
        .ok_or_else(|| format!("Remote URL for {} surface is missing a host", surface.id))?;
    let remote_port = remote_url
        .port_or_known_default()
        .ok_or_else(|| format!("Remote URL for {} surface is missing a port", surface.id))?;

    Ok((remote_host.to_string(), remote_port))
}

fn vite_hmr_forward_target(surface: &BrowserSurface) -> Result<(String, u16), String> {
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

fn spawn_russh_forward(
    target_id: &str,
    local_bind_addr: &str,
    remote_host: &str,
    remote_port: u16,
    label: impl Into<String>,
) -> Result<RusshTunnelForward, String> {
    let chain = resolve_container_chain(target_id)?;
    let forward = start_local_forward_chain_blocking(
        &chain,
        local_bind_addr,
        remote_host,
        remote_port,
        &RusshClientOptions::default(),
    )
    .map_err(|error| format!("Failed to start russh forward: {}", error))?;

    Ok(RusshTunnelForward {
        label: label.into(),
        forward,
    })
}

fn spawn_vite_proxy_session<R: Runtime>(
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

    let local_http_bind = format!("127.0.0.1:{}", http_forward_port);
    let http_forward = spawn_russh_forward(
        target_id,
        &local_http_bind,
        &remote_host,
        remote_port,
        format!("{} HTTP tunnel", surface.label),
    )?;

    let local_hmr_bind = format!("127.0.0.1:{}", hmr_forward_port);
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
        session_id,
        target_id,
        surface,
        Vec::new(),
        russh_forwards,
        Some(proxy),
        vec![
            internal_probe_url(http_forward_port),
            internal_probe_url(hmr_forward_port),
        ],
    ))
}

fn uses_vite_forwarding(surface: &BrowserSurface) -> bool {
    matches!(surface.service_type, BrowserServiceType::Vite)
        || !surface.vite_hmr_tunnel_command.trim().is_empty()
}

pub(crate) fn start_tunnel<R: Runtime>(
    app: &AppHandle<R>,
    tunnel_state: &TunnelState,
    target_id: &str,
    surface: &BrowserSurface,
) -> Result<TunnelStatus, String> {
    let session_id = tunnel_state
        .next_session_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let session = if uses_vite_forwarding(surface) {
        spawn_vite_proxy_session(
            app,
            tunnel_state.sessions.clone(),
            tunnel_state,
            session_id,
            target_id,
            surface,
        )?
    } else {
        let local_port = surface_local_port(surface)?;
        let (remote_host, remote_port) = remote_forward_target(surface)?;
        let local_bind_addr = format!("127.0.0.1:{}", local_port);
        let forward = spawn_russh_forward(
            target_id,
            &local_bind_addr,
            &remote_host,
            remote_port,
            format!("{} tunnel", surface.label),
        )?;

        spawn_managed_tunnel_session(
            app,
            tunnel_state.sessions.clone(),
            tunnel_state.status_snapshots.clone(),
            tunnel_state.telemetry.clone(),
            session_id,
            target_id,
            surface,
            Vec::new(),
            vec![forward],
            None,
            vec![surface.local_url.clone()],
        )
    };

    tunnel_state
        .sessions
        .lock()
        .map_err(|_| String::from("Tunnel session state is unavailable"))?
        .insert(surface_key(target_id, &surface.id), session);

    Ok(starting_tunnel_status(target_id, surface))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_vite_surface() -> BrowserSurface {
        BrowserSurface {
            id: String::from("development"),
            label: String::from("Development"),
            service_type: BrowserServiceType::Vite,
            background_prerender: true,
            tunnel_command: String::new(),
            local_url: String::from("http://127.0.0.1:43000/"),
            remote_url: String::from("http://127.0.0.1:3432/"),
            vite_hmr_tunnel_command: String::new(),
        }
    }

    #[test]
    fn infers_vite_hmr_port_from_remote_url_when_not_explicit() {
        let (host, port) = vite_hmr_forward_target(&fixture_vite_surface()).expect("hmr target");

        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 3433);
    }

    #[test]
    fn prefers_explicit_vite_hmr_forward_target_when_present() {
        let mut surface = fixture_vite_surface();
        surface.vite_hmr_tunnel_command =
            String::from("ssh -NT -L 43001:127.0.0.1:5173 mechanism-dev");

        let (host, port) = vite_hmr_forward_target(&surface).expect("hmr target");

        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 5173);
    }
}
