use super::keys::surface_key;
use crate::{
    constants::{
        INTERNAL_TUNNEL_PORT_END, INTERNAL_TUNNEL_PORT_START, SURFACE_HOST, SURFACE_PORT_END,
        SURFACE_PORT_START,
    },
    models::{BrowserSurface, TunnelState},
};
use std::net::TcpListener;
use url::Url;

fn tunnel_port_key(target_id: &str, surface_id: &str, suffix: &str) -> String {
    format!("{target_id}::{surface_id}::{suffix}")
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
        let session_exists = tunnel_state
            .sessions
            .lock()
            .map(|sessions| sessions.contains_key(key))
            .unwrap_or(false);

        if session_exists || port_is_available(port) {
            return Ok(port);
        }

        local_ports.remove(key);
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
        "No free browser surface ports available in {start}-{end}."
    ))
}

pub(super) fn allocate_surface_port(
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

pub(crate) fn internal_probe_url(port: u16) -> String {
    format!("http://{SURFACE_HOST}:{port}/")
}
