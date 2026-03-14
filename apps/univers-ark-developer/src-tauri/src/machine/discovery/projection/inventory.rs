use crate::machine::execute_target_command_via_russh;
use crate::models::{MachineTransport, ManagedContainer, ManagedServer};

use super::super::super::ssh::ssh_destination;
use super::super::super::{DiscoveredContainer, DiscoveredServerInventory, RemoteContainerServer};
use super::super::sources::cached_server_containers;
use super::super::ssh_users::resolve_container_ssh_user;
use super::target::{build_machine_host_target, build_target_from_container};

fn probe_target_ssh(target_id: &str, success_message: String) -> (bool, String, String) {
    match execute_target_command_via_russh(target_id, "true") {
        Ok(output) if output.exit_status == 0 => (true, String::from("ready"), success_message),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("SSH probe exited with {}", output.exit_status)
            };

            (false, String::from("error"), detail)
        }
        Err(error) => (false, String::from("error"), error),
    }
}

fn build_managed_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
    probe_ssh: bool,
) -> (ManagedContainer, Option<crate::models::DeveloperTarget>) {
    let target = build_target_from_container(server, container);
    let ssh_command = target.terminal_command.clone();
    let ssh_dest = if matches!(server.transport, MachineTransport::Local) {
        String::from("local")
    } else {
        ssh_destination(
            &container.ipv4,
            &resolve_container_ssh_user(server, container),
        )
    };
    let ssh_user = resolve_container_ssh_user(server, container);
    let (ssh_reachable, ssh_state, ssh_message) =
        if matches!(server.transport, MachineTransport::Local) {
            (
                true,
                String::from("ready"),
                String::from("Local workspace is ready."),
            )
        } else if probe_ssh {
            probe_target_ssh(&target.id, format!("SSH ready via {}.", server.host))
        } else {
            (
                false,
                String::from("checking"),
                String::from("Waiting for background SSH probe."),
            )
        };

    (
        ManagedContainer {
            server_id: server.id.clone(),
            server_label: server.label.clone(),
            container_id: target.container_id.clone(),
            kind: container.kind,
            transport: server.transport,
            target_id: target.id.clone(),
            name: container.name.clone(),
            label: target.label.clone(),
            status: container.status.clone(),
            ipv4: container.ipv4.clone(),
            ssh_user,
            ssh_destination: ssh_dest,
            ssh_command,
            ssh_state,
            ssh_message,
            ssh_reachable,
        },
        Some(target),
    )
}

fn machine_host_state(
    server: &RemoteContainerServer,
    host_target_id: &str,
    probe_ssh: bool,
) -> (bool, String, String) {
    if matches!(server.transport, MachineTransport::Local) {
        return (
            true,
            String::from("ready"),
            String::from("Local machine is ready."),
        );
    }

    if probe_ssh {
        return probe_target_ssh(host_target_id, format!("SSH ready via {}.", server.host));
    }

    (
        false,
        String::from("checking"),
        String::from("Waiting for background SSH probe."),
    )
}

#[cfg(test)]
pub(crate) fn server_state_for_containers(containers: &[ManagedContainer]) -> (String, String) {
    if containers.is_empty() {
        return (
            String::from("error"),
            String::from("No matching development containers were detected."),
        );
    }

    let reachable = containers
        .iter()
        .filter(|container| container.ssh_reachable)
        .count();

    if reachable == containers.len() {
        return (
            String::from("ready"),
            format!("{reachable} development container(s) are SSH reachable."),
        );
    }

    if reachable > 0 {
        return (
            String::from("error"),
            format!(
                "{} of {} development container(s) are SSH reachable.",
                reachable,
                containers.len()
            ),
        );
    }

    (
        String::from("error"),
        format!(
            "Detected {} development container(s), but none are SSH reachable.",
            containers.len()
        ),
    )
}

fn server_state_for_machine(
    host_reachable: bool,
    host_message: &str,
    containers: &[ManagedContainer],
) -> (String, String) {
    if containers.is_empty() {
        if host_reachable {
            return (
                String::from("ready"),
                String::from("Machine host is ready. No managed containers detected."),
            );
        }

        if host_message == "Waiting for background SSH probe." {
            return (
                String::from("checking"),
                String::from("Checking machine host. No managed containers detected."),
            );
        }

        return (
            String::from("error"),
            format!("Machine host is unreachable: {host_message}"),
        );
    }

    let reachable = containers
        .iter()
        .filter(|container| container.ssh_reachable)
        .count();
    let total = containers.len();

    if host_reachable && reachable == total {
        return (
            String::from("ready"),
            format!(
                "Machine host is ready. {total} managed container(s) are SSH reachable."
            ),
        );
    }

    if host_message == "Waiting for background SSH probe." || reachable > 0 && !host_reachable {
        return (
            String::from("checking"),
            format!("Checking machine host and {total} managed container(s)."),
        );
    }

    if host_reachable {
        return (
            String::from("error"),
            format!(
                "Machine host is ready, but only {reachable} of {total} managed container(s) are SSH reachable."
            ),
        );
    }

    (
        String::from("error"),
        format!(
            "Machine host is unreachable: {host_message} Detected {total} managed container(s), but none are SSH reachable."
        ),
    )
}

pub(crate) fn inventory_from_discovered_containers(
    server: &RemoteContainerServer,
    containers: Vec<DiscoveredContainer>,
    probe_ssh: bool,
) -> DiscoveredServerInventory {
    let mut managed_containers = Vec::new();
    let host_target = build_machine_host_target(server);
    let mut available_targets = vec![host_target.clone()];

    for container in containers {
        let (managed_container, available_target) =
            build_managed_container(server, &container, probe_ssh);
        managed_containers.push(managed_container);

        if let Some(available_target) = available_target {
            available_targets.push(available_target);
        }
    }

    let (state, message) = if probe_ssh {
        let (host_reachable, _, host_message) = machine_host_state(server, &host_target.id, true);
        server_state_for_machine(host_reachable, &host_message, &managed_containers)
    } else {
        (
            String::from("checking"),
            if managed_containers.is_empty() {
                String::from("Checking machine host. No managed containers detected.")
            } else {
                format!(
                    "Checking machine host and {} managed container(s).",
                    managed_containers.len()
                )
            },
        )
    };

    DiscoveredServerInventory {
        server: ManagedServer {
            id: server.id.clone(),
            host_target_id: host_target.id.clone(),
            label: server.label.clone(),
            transport: server.transport,
            host: server.host.clone(),
            description: server.description.clone(),
            os: String::from(server.os.as_str()),
            state,
            message,
            containers: managed_containers,
        },
        available_targets,
    }
}

pub(crate) fn inventory_from_scan_error(
    server: &RemoteContainerServer,
    error: String,
) -> DiscoveredServerInventory {
    let host_target = build_machine_host_target(server);
    let (host_reachable, _, host_message) = machine_host_state(server, &host_target.id, true);
    let message = if host_reachable {
        format!(
            "Machine host is ready, but container discovery failed: {}",
            error
        )
    } else {
        format!(
            "Machine host is unreachable: {} Container discovery failed: {}",
            host_message, error
        )
    };

    DiscoveredServerInventory {
        server: ManagedServer {
            id: server.id.clone(),
            host_target_id: host_target.id.clone(),
            label: server.label.clone(),
            transport: server.transport,
            host: server.host.clone(),
            description: server.description.clone(),
            os: String::from(server.os.as_str()),
            state: String::from("error"),
            message,
            containers: Vec::new(),
        },
        available_targets: vec![host_target],
    }
}

pub(crate) fn cached_remote_server_inventory(
    server: &RemoteContainerServer,
) -> DiscoveredServerInventory {
    let containers = cached_server_containers(server);
    inventory_from_discovered_containers(server, containers, false)
}
