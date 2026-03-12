mod manual;
mod parse;

use crate::{infra::shell, machine::execute_target_command_via_russh, models::MachineTransport};
use std::collections::HashSet;

pub(crate) use self::manual::cached_server_containers;
#[cfg(test)]
pub(crate) use self::parse::parse_discovered_containers;
use self::{manual::discover_manual_containers, parse::parse_discovered_containers_for_manager};
use super::super::{
    ContainerDiscoveryMode, ContainerManagerType, DiscoveredContainer, RemoteContainerServer,
};
use super::ssh_users::enrich_discovered_container_ssh_users;

fn default_discovery_command_for_manager(manager_type: ContainerManagerType) -> String {
    match manager_type {
        ContainerManagerType::None => String::new(),
        ContainerManagerType::Lxd => String::from("lxc list --format csv -c ns4"),
        ContainerManagerType::Docker => String::from(
            "docker ps --format \"{{.Names}}\" | while read -r name; do [ -z \"$name\" ] && continue; ip=$(docker inspect -f \"{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}\" \"$name\" 2>/dev/null); printf \"%s,RUNNING,%s\\n\" \"$name\" \"$ip\"; done",
        ),
        ContainerManagerType::Orbstack => String::from("/opt/homebrew/bin/orb list --format json"),
    }
}

fn discovery_managers(server: &RemoteContainerServer) -> Vec<ContainerManagerType> {
    match server.manager_type {
        ContainerManagerType::None => vec![
            ContainerManagerType::Orbstack,
            ContainerManagerType::Docker,
            ContainerManagerType::Lxd,
        ],
        manager_type => vec![manager_type],
    }
}

fn host_target_id(server: &RemoteContainerServer) -> String {
    format!("{}::host", server.id)
}

fn should_use_russh_discovery(server: &RemoteContainerServer) -> bool {
    cfg!(mobile)
        || !server.ssh_credential_id.trim().is_empty()
        || server
            .jump_chain
            .iter()
            .any(|jump| !jump.ssh_credential_id.trim().is_empty())
}

fn run_discovery_command_locally(
    server: &RemoteContainerServer,
    command: &str,
) -> Result<String, String> {
    let output = shell::shell_command(command).output().map_err(|error| {
        format!(
            "Failed to discover containers on {} with `{}`: {}",
            server.host, command, error
        )
    })?;

    if !output.status.success() {
        return Err(format!(
            "Failed to discover containers on {} with `{}`: {}",
            server.host,
            command,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_discovery_command_via_russh(
    server: &RemoteContainerServer,
    command: &str,
) -> Result<String, String> {
    let output =
        execute_target_command_via_russh(&host_target_id(server), command).map_err(|error| {
            format!(
                "Failed to discover containers on {}: {}",
                server.host, error
            )
        })?;

    if output.exit_status != 0 {
        return Err(format!(
            "Failed to discover containers on {} with `{}`: {}",
            server.host,
            command,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn is_ignorable_discovery_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();

    normalized.contains("command not found")
        || normalized.contains("no such file or directory")
        || normalized.contains("not installed")
        || normalized.contains("unknown command")
}

fn manager_type_label(manager_type: ContainerManagerType) -> &'static str {
    match manager_type {
        ContainerManagerType::None => "none",
        ContainerManagerType::Orbstack => "orbstack",
        ContainerManagerType::Docker => "docker",
        ContainerManagerType::Lxd => "lxd",
    }
}

struct ManagerDiscoveryResult {
    manager_type: ContainerManagerType,
    attempted: bool,
    containers: Result<Vec<DiscoveredContainer>, String>,
}

fn discover_containers_with_supported_managers(
    server: &RemoteContainerServer,
) -> Result<Vec<DiscoveredContainer>, String> {
    let mut discovered = Vec::new();
    let mut seen_names = HashSet::new();
    let mut failures = Vec::new();
    let mut attempted_managers = 0;

    let manager_results = std::thread::scope(|scope| {
        let handles = discovery_managers(server)
            .into_iter()
            .map(|manager_type| {
                scope.spawn(move || {
                    let command = default_discovery_command_for_manager(manager_type);
                    if command.trim().is_empty() {
                        return ManagerDiscoveryResult {
                            manager_type,
                            attempted: false,
                            containers: Ok(Vec::new()),
                        };
                    }
                    let output = match run_discovery_command_via_russh(server, &command) {
                        Ok(output) => output,
                        Err(error) => {
                            return ManagerDiscoveryResult {
                                manager_type,
                                attempted: true,
                                containers: Err(error),
                            };
                        }
                    };
                    let containers =
                        parse_discovered_containers_for_manager(server, manager_type, &output);
                    ManagerDiscoveryResult {
                        manager_type,
                        attempted: true,
                        containers,
                    }
                })
            })
            .collect::<Vec<_>>();

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            let result = handle.join().map_err(|_| {
                format!(
                    "Container discovery worker panicked while scanning {}",
                    server.host
                )
            })?;
            results.push(result);
        }

        Ok::<Vec<ManagerDiscoveryResult>, String>(results)
    })?;

    for result in manager_results {
        if !result.attempted {
            continue;
        }

        attempted_managers += 1;

        match result.containers {
            Ok(containers) => {
                for container in containers {
                    if seen_names.insert(container.name.clone()) {
                        discovered.push(container);
                    }
                }
            }
            Err(error) => {
                if !is_ignorable_discovery_error(&error) {
                    failures.push(format!(
                        "[{}] {}",
                        manager_type_label(result.manager_type),
                        error
                    ));
                }
            }
        }
    }

    if !discovered.is_empty() || attempted_managers == 0 || failures.is_empty() {
        return Ok(discovered);
    }

    Err(failures.join("\n"))
}

pub(crate) fn scan_server_containers(
    server: &RemoteContainerServer,
) -> Result<Vec<DiscoveredContainer>, String> {
    if matches!(server.transport, MachineTransport::Local) {
        return Ok(Vec::new());
    }

    if matches!(server.discovery_mode, ContainerDiscoveryMode::HostOnly) {
        return Ok(Vec::new());
    }

    if matches!(server.discovery_mode, ContainerDiscoveryMode::Manual) {
        return Ok(enrich_discovered_container_ssh_users(
            server,
            discover_manual_containers(server),
        ));
    }

    let mut containers = Vec::new();

    if !server.discovery_command.trim().is_empty() {
        let output = if should_use_russh_discovery(server) {
            run_discovery_command_via_russh(server, &server.discovery_command)?
        } else {
            run_discovery_command_locally(server, &server.discovery_command)?
        };
        containers.extend(parse::parse_discovered_containers(server, &output)?);
        return Ok(enrich_discovered_container_ssh_users(server, containers));
    }

    containers.extend(discover_containers_with_supported_managers(server)?);
    Ok(enrich_discovered_container_ssh_users(server, containers))
}
