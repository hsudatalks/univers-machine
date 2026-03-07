use crate::{
    config::{read_targets_file, resolve_raw_target},
    constants::{
        INTERNAL_TUNNEL_PORT_END, INTERNAL_TUNNEL_PORT_START, SURFACE_HOST, SURFACE_PORT_END,
        SURFACE_PORT_START,
    },
    models::{BrowserSurface, DeveloperTarget, TargetsFile, TunnelState},
};
use std::net::TcpListener;
use url::Url;

pub(crate) fn surface_key(target_id: &str, surface_id: &str) -> String {
    format!("{}::{}", target_id, surface_id)
}

fn tunnel_port_key(target_id: &str, surface_id: &str, suffix: &str) -> String {
    format!("{}::{}::{}", target_id, surface_id, suffix)
}

fn port_span(start: u16, end: u16) -> usize {
    usize::from(end - start) + 1
}

fn stable_port_offset(key: &str, start: u16, end: u16) -> usize {
    let mut hash = 2_166_136_261_u32;

    for byte in key.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }

    usize::try_from(hash).unwrap_or(0) % port_span(start, end)
}

fn port_is_available(port: u16) -> bool {
    TcpListener::bind((SURFACE_HOST, port)).is_ok()
}

fn allocate_stable_port(
    tunnel_state: &TunnelState,
    key: &str,
    start: u16,
    end: u16,
) -> Result<u16, String> {
    let mut local_ports = tunnel_state
        .local_ports
        .lock()
        .map_err(|_| String::from("Surface port state is unavailable"))?;

    if let Some(port) = local_ports.get(key).copied() {
        return Ok(port);
    }

    let span = port_span(start, end);
    let start_offset = stable_port_offset(key, start, end);

    for step in 0..span {
        let candidate = start + ((start_offset + step) % span) as u16;

        if local_ports.values().any(|assigned| *assigned == candidate) {
            continue;
        }

        if port_is_available(candidate) {
            local_ports.insert(key.to_string(), candidate);
            return Ok(candidate);
        }
    }

    Err(format!(
        "No free browser surface ports available in {}-{}.",
        start, end
    ))
}

pub(crate) fn allocate_surface_port(
    tunnel_state: &TunnelState,
    target_id: &str,
    surface_id: &str,
) -> Result<u16, String> {
    allocate_stable_port(
        tunnel_state,
        &surface_key(target_id, surface_id),
        SURFACE_PORT_START,
        SURFACE_PORT_END,
    )
}

pub(crate) fn allocate_internal_tunnel_port(
    tunnel_state: &TunnelState,
    target_id: &str,
    surface_id: &str,
    suffix: &str,
) -> Result<u16, String> {
    allocate_stable_port(
        tunnel_state,
        &tunnel_port_key(target_id, surface_id, suffix),
        INTERNAL_TUNNEL_PORT_START,
        INTERNAL_TUNNEL_PORT_END,
    )
}

pub(crate) fn replace_known_tunnel_placeholders(
    tunnel_command: &str,
    remote_url: &str,
    local_port: u16,
) -> String {
    let mut resolved = tunnel_command.replace("{localPort}", &local_port.to_string());

    if let Ok(remote_url) = Url::parse(remote_url) {
        if let Some(host) = remote_url.host_str() {
            resolved = resolved.replace("{remoteHost}", host);
            resolved = resolved.replace("{previewRemoteHost}", host);
        }

        if let Some(port) = remote_url.port_or_known_default() {
            resolved = resolved.replace("{remotePort}", &port.to_string());
            resolved = resolved.replace("{previewRemotePort}", &port.to_string());
        }
    }

    resolved
}

fn rewrite_forward_spec_local_port(forward_spec: &str, local_port: u16) -> String {
    match forward_spec.split_once(':') {
        Some((_, rest)) => format!("{}:{}", local_port, rest),
        None => forward_spec.to_string(),
    }
}

pub(crate) fn rewrite_tunnel_forward_port(tunnel_command: &str, local_port: u16) -> String {
    let mut tokens = tunnel_command
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();

    for index in 0..tokens.len() {
        if tokens[index] == "-L" {
            if let Some(forward_spec) = tokens.get_mut(index + 1) {
                *forward_spec = rewrite_forward_spec_local_port(forward_spec, local_port);
                return tokens.join(" ");
            }
        }

        if let Some(forward_spec) = tokens[index].strip_prefix("-L") {
            tokens[index] = format!(
                "-L{}",
                rewrite_forward_spec_local_port(forward_spec, local_port)
            );
            return tokens.join(" ");
        }
    }

    tunnel_command.to_string()
}

fn resolve_runtime_tunnel_command(
    tunnel_command: &str,
    remote_url: &str,
    local_port: u16,
) -> String {
    let placeholder_resolved =
        replace_known_tunnel_placeholders(tunnel_command, remote_url, local_port);

    if placeholder_resolved != tunnel_command {
        return placeholder_resolved;
    }

    rewrite_tunnel_forward_port(&placeholder_resolved, local_port)
}

pub(crate) fn resolve_runtime_vite_hmr_tunnel_command(
    tunnel_command: &str,
    local_port: u16,
) -> String {
    let placeholder_resolved = tunnel_command.replace("{localPort}", &local_port.to_string());

    if placeholder_resolved != tunnel_command {
        return placeholder_resolved;
    }

    rewrite_tunnel_forward_port(&placeholder_resolved, local_port)
}

fn resolve_runtime_local_url(local_url: &str, remote_url: &str, local_port: u16) -> String {
    let template = if local_url.trim().is_empty() {
        remote_url
    } else {
        local_url
    }
    .replace("{localPort}", &local_port.to_string());

    if let Ok(mut url) = Url::parse(&template) {
        let _ = url.set_host(Some(SURFACE_HOST));
        let _ = url.set_port(Some(local_port));
        return url.to_string();
    }

    if let Ok(mut remote_url) = Url::parse(remote_url) {
        let _ = remote_url.set_host(Some(SURFACE_HOST));
        let _ = remote_url.set_port(Some(local_port));
        return remote_url.to_string();
    }

    format!("http://{}:{}/", SURFACE_HOST, local_port)
}

pub(crate) fn surface_local_port(surface: &BrowserSurface) -> Result<u16, String> {
    let url = Url::parse(&surface.local_url).map_err(|error| {
        format!(
            "Failed to parse local URL for {} surface: {}",
            surface.id, error
        )
    })?;

    url.port_or_known_default().ok_or_else(|| {
        format!(
            "Local URL for {} surface is missing a port: {}",
            surface.id, surface.local_url
        )
    })
}

fn hydrate_surface(
    target_id: &str,
    surface: &BrowserSurface,
    tunnel_state: &TunnelState,
) -> Result<BrowserSurface, String> {
    if surface.tunnel_command.trim().is_empty() {
        return Ok(surface.clone());
    }

    let local_port = allocate_surface_port(tunnel_state, target_id, &surface.id)?;
    let mut runtime_surface = surface.clone();

    runtime_surface.tunnel_command = resolve_runtime_tunnel_command(
        &runtime_surface.tunnel_command,
        &runtime_surface.remote_url,
        local_port,
    );
    runtime_surface.local_url = resolve_runtime_local_url(
        &runtime_surface.local_url,
        &runtime_surface.remote_url,
        local_port,
    );

    Ok(runtime_surface)
}

fn hydrate_target(
    target: &DeveloperTarget,
    tunnel_state: &TunnelState,
) -> Result<DeveloperTarget, String> {
    let surfaces = target
        .surfaces
        .iter()
        .map(|surface| hydrate_surface(&target.id, surface, tunnel_state))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DeveloperTarget {
        surfaces,
        ..target.clone()
    })
}

pub(crate) fn read_runtime_targets_file(tunnel_state: &TunnelState) -> Result<TargetsFile, String> {
    let targets_file = read_targets_file()?;
    let targets = targets_file
        .targets
        .into_iter()
        .map(|target| hydrate_target(&target, tunnel_state))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TargetsFile {
        selected_target_id: targets_file.selected_target_id,
        targets,
    })
}

fn resolve_runtime_target(
    target_id: &str,
    tunnel_state: &TunnelState,
) -> Result<DeveloperTarget, String> {
    let target = resolve_raw_target(target_id)?;
    hydrate_target(&target, tunnel_state)
}

pub(crate) fn resolve_runtime_surface(
    target_id: &str,
    surface_id: &str,
    tunnel_state: &TunnelState,
) -> Result<BrowserSurface, String> {
    let target = resolve_runtime_target(target_id, tunnel_state)?;

    target
        .surfaces
        .into_iter()
        .find(|surface| surface.id == surface_id)
        .ok_or_else(|| {
            format!(
                "Unknown browser surface {} for target {}",
                surface_id, target_id
            )
        })
}

pub(crate) fn internal_probe_url(port: u16) -> String {
    format!("http://{}:{}/", SURFACE_HOST, port)
}
