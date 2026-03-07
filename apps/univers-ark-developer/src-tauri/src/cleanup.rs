use crate::{
    config::read_targets_file,
    constants::{
        INTERNAL_TUNNEL_PORT_END, INTERNAL_TUNNEL_PORT_START, SURFACE_PORT_END, SURFACE_PORT_START,
    },
    models::{ManagedTunnelSignature, ObservedTunnelProcess},
    runtime::replace_known_tunnel_placeholders,
};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn managed_tunnel_port_range_contains(local_port: u16) -> bool {
    (SURFACE_PORT_START..=SURFACE_PORT_END).contains(&local_port)
        || (INTERNAL_TUNNEL_PORT_START..=INTERNAL_TUNNEL_PORT_END).contains(&local_port)
}

fn parse_forward_spec(forward_spec: &str) -> Option<(u16, String, u16)> {
    let mut parts = forward_spec.rsplitn(3, ':');
    let remote_port = parts.next()?.parse().ok()?;
    let remote_host = parts.next()?.to_string();
    let local_spec = parts.next()?;
    let local_port = local_spec.rsplit(':').next()?.parse().ok()?;

    Some((local_port, remote_host, remote_port))
}

fn parse_ssh_tunnel_process(command_line: &str) -> Option<(u16, String, u16, String)> {
    let tokens = command_line.split_whitespace().collect::<Vec<_>>();
    let first = tokens.first()?;

    if *first != "ssh" {
        return None;
    }

    let mut forward_spec = None;

    for index in 0..tokens.len() {
        if tokens[index] == "-L" {
            forward_spec = tokens.get(index + 1).copied();
            break;
        }

        if let Some(value) = tokens[index].strip_prefix("-L") {
            if !value.is_empty() {
                forward_spec = Some(value);
                break;
            }
        }
    }

    let (local_port, remote_host, remote_port) = parse_forward_spec(forward_spec?)?;
    let ssh_destination = tokens
        .iter()
        .rev()
        .find(|token| !token.starts_with('-'))?
        .to_string();

    Some((local_port, remote_host, remote_port, ssh_destination))
}

fn managed_tunnel_signature_for_command(command_line: &str) -> Option<ManagedTunnelSignature> {
    let (_, remote_host, remote_port, ssh_destination) = parse_ssh_tunnel_process(command_line)?;

    Some(ManagedTunnelSignature {
        ssh_destination,
        remote_host,
        remote_port,
    })
}

fn collect_managed_tunnel_signatures() -> Result<Vec<ManagedTunnelSignature>, String> {
    let targets_file = read_targets_file()?;
    let mut signatures = Vec::new();

    for target in &targets_file.targets {
        for surface in &target.surfaces {
            if !surface.tunnel_command.trim().is_empty() {
                let resolved_command = replace_known_tunnel_placeholders(
                    &surface.tunnel_command,
                    &surface.remote_url,
                    0,
                );

                if let Some(signature) = managed_tunnel_signature_for_command(&resolved_command) {
                    signatures.push(signature);
                }
            }

            if !surface.vite_hmr_tunnel_command.trim().is_empty() {
                let resolved_command = surface.vite_hmr_tunnel_command.replace("{localPort}", "0");

                if let Some(signature) = managed_tunnel_signature_for_command(&resolved_command) {
                    signatures.push(signature);
                }
            }
        }
    }

    Ok(signatures)
}

fn list_processes() -> Result<Vec<(u32, String)>, String> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,command="])
        .output()
        .map_err(|error| format!("Failed to inspect running processes: {}", error))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to inspect running processes: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let mut processes = Vec::new();

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim_start();

        if line.is_empty() {
            continue;
        }

        let Some(separator_index) = line.find(char::is_whitespace) else {
            continue;
        };

        let Ok(pid) = line[..separator_index].trim().parse::<u32>() else {
            continue;
        };

        let command_line = line[separator_index..].trim().to_string();

        if !command_line.is_empty() {
            processes.push((pid, command_line));
        }
    }

    Ok(processes)
}

fn process_exists(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn terminate_process(pid: u32) {
    let _ = Command::new("/bin/kill")
        .args(["-TERM", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let deadline = Instant::now() + Duration::from_millis(350);

    while process_exists(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(40));
    }

    if process_exists(pid) {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn matches_managed_tunnel(
    process: &ObservedTunnelProcess,
    signatures: &[ManagedTunnelSignature],
) -> bool {
    managed_tunnel_port_range_contains(process.local_port)
        && signatures.iter().any(|signature| {
            process.ssh_destination == signature.ssh_destination
                && process.remote_host == signature.remote_host
                && process.remote_port == signature.remote_port
        })
}

pub(crate) fn cleanup_stale_ssh_tunnels() -> Result<usize, String> {
    let signatures = collect_managed_tunnel_signatures()?;

    if signatures.is_empty() {
        return Ok(0);
    }

    let stale_processes = list_processes()?
        .into_iter()
        .filter_map(|(pid, command_line)| {
            let (local_port, remote_host, remote_port, ssh_destination) =
                parse_ssh_tunnel_process(&command_line)?;

            let observed = ObservedTunnelProcess {
                pid,
                local_port,
                ssh_destination,
                remote_host,
                remote_port,
            };

            matches_managed_tunnel(&observed, &signatures).then_some(observed)
        })
        .collect::<Vec<_>>();

    let cleaned = stale_processes.len();

    for process in stale_processes {
        terminate_process(process.pid);
    }

    Ok(cleaned)
}
