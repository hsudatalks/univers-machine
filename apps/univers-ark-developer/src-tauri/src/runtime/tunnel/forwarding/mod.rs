mod targets;
mod vite;

use crate::{
    infra::ssh::start_local_forward_chain_blocking,
    machine::{resolve_raw_target, resolve_target_ssh_chain},
    models::{
        BrowserServiceType, BrowserSurface, MachineTransport, RusshTunnelForward, TunnelState,
        TunnelStatus,
    },
    services::runtime::{surface_key, surface_local_port},
};
use tauri::{AppHandle, Runtime};
use univers_infra_ssh::{ClientOptions as RusshClientOptions, ResolvedEndpointChain};

use self::{targets::remote_forward_target, vite::spawn_vite_proxy_session};
use super::{
    session::{spawn_managed_tunnel_session, ManagedTunnelSessionSpec},
    starting_tunnel_status,
};

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
    .map_err(|error| format!("Failed to start russh forward: {error}"))?;

    Ok(RusshTunnelForward {
        label: label.into(),
        forward,
    })
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
        let local_bind_addr = format!("127.0.0.1:{local_port}");
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
            ManagedTunnelSessionSpec {
                session_id,
                target_id: target_id.to_string(),
                surface: surface.clone(),
                processes: Vec::new(),
                russh_forwards: vec![forward],
                proxy: None,
                probe_urls: vec![surface.local_url.clone()],
            },
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
        let (host, port) =
            super::vite::vite_hmr_forward_target(&fixture_vite_surface()).expect("hmr target");

        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 3433);
    }

    #[test]
    fn prefers_explicit_vite_hmr_forward_target_when_present() {
        let mut surface = fixture_vite_surface();
        surface.vite_hmr_tunnel_command =
            String::from("ssh -NT -L 43001:127.0.0.1:5173 mechanism-dev");

        let (host, port) = super::vite::vite_hmr_forward_target(&surface).expect("hmr target");

        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 5173);
    }
}
