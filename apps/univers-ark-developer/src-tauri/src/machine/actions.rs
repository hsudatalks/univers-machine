use super::{
    ContainerManagerType, manager_priority,
    repository::read_raw_targets_file,
    ssh::{build_host_ssh_command, shell_single_quote},
};
use crate::models::MachineTransport;

pub(crate) fn restart_container(server_id: &str, container_name: &str) -> Result<(), String> {
    let raw_targets_file = read_raw_targets_file()?;
    let server = raw_targets_file
        .machines
        .iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| format!("Unknown machine: {}", server_id))?;
    if matches!(server.transport, MachineTransport::Local) {
        return Err(String::from(
            "Local host container cannot be restarted from machine inventory.",
        ));
    }

    let mut errors = Vec::new();

    for manager_type in manager_priority(server) {
        let restart_command = match manager_type {
            ContainerManagerType::Orbstack => build_host_ssh_command(
                server,
                &[],
                Some(&shell_single_quote(&format!(
                    "/opt/homebrew/bin/orb restart {}",
                    container_name
                ))),
            ),
            ContainerManagerType::Docker => build_host_ssh_command(
                server,
                &[],
                Some(&shell_single_quote(&format!(
                    "docker restart {}",
                    container_name
                ))),
            ),
            ContainerManagerType::Lxd => build_host_ssh_command(
                server,
                &[],
                Some(&shell_single_quote(&format!(
                    "lxc restart {} --force",
                    container_name
                ))),
            ),
            ContainerManagerType::None => continue,
        };

        let output = crate::infra::shell::shell_command(&restart_command)
            .output()
            .map_err(|error| {
                format!(
                    "Failed to restart container {} on {}: {}",
                    container_name, server.host, error
                )
            })?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!(
                "Failed to restart container {} on {}: exit code {}",
                container_name, server.host, output.status
            )
        };

        let normalized = detail.to_ascii_lowercase();
        if normalized.contains("command not found")
            || normalized.contains("no such file or directory")
            || normalized.contains("not found")
        {
            continue;
        }

        errors.push(detail);
    }

    if errors.is_empty() {
        Err(format!(
            "Failed to restart container {} on {} with supported container managers.",
            container_name, server.host
        ))
    } else {
        Err(errors.join("\n"))
    }
}
