use crate::models::BrowserSurface;
use url::Url;

pub(super) fn parse_forward_target(command_line: &str) -> Result<(String, u16), String> {
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
            format!("Invalid remote forward port in {forward_spec}: {error}")
        })?;
        let Some(remote_host) = before_port.rsplit(':').next() else {
            continue;
        };

        return Ok((remote_host.to_string(), remote_port));
    }

    Err(format!(
        "Failed to parse -L forward target from tunnel command: {command_line}"
    ))
}

pub(super) fn remote_forward_target(surface: &BrowserSurface) -> Result<(String, u16), String> {
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
