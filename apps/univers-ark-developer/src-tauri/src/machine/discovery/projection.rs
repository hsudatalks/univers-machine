use crate::{
    models::{
        DeveloperTarget, MachineTransport, ManagedContainer, ManagedContainerKind, ManagedServer,
    },
    services::catalog::web_service,
};

use super::super::ssh::{
    build_host_ssh_command, default_terminal_startup_command, probe_machine_host_ssh,
    probe_managed_container_ssh, profile_terminal_startup_command, shell_single_quote,
    ssh_destination, terminal_command_for_server,
};
use super::super::{
    DiscoveredContainer, DiscoveredServerInventory, RemoteContainerContext, RemoteContainerServer,
};
use super::sources::{cached_server_containers, scan_server_containers};
use super::ssh_users::resolve_container_ssh_user;
use super::{
    default_container_label, render_service, render_surface, render_template, render_workspace,
};

fn machine_host_target_id(server: &RemoteContainerServer) -> String {
    format!("{}::host", server.id)
}

fn build_machine_host_target(server: &RemoteContainerServer) -> DeveloperTarget {
    let container_ip = if matches!(server.transport, MachineTransport::Local) {
        "127.0.0.1"
    } else {
        server.host.as_str()
    };
    let context = RemoteContainerContext {
        container_ip,
        container_label: "Host",
        container_name: "host",
        ssh_user: &server.ssh_user,
        server,
    };
    let label = render_template(&server.target_label_template, &context, || {
        String::from("Host")
    });
    let host = if matches!(server.transport, MachineTransport::Local) {
        String::from("localhost")
    } else {
        render_template(&server.target_host_template, &context, || {
            server.host.clone()
        })
    };
    let description = render_template(&server.target_description_template, &context, || {
        format!("Host workspace on {}.", server.label)
    });
    let terminal_command = if matches!(server.transport, MachineTransport::Local) {
        String::from("exec /bin/zsh -l")
    } else {
        build_host_ssh_command(
            server,
            &["-tt"],
            Some(&shell_single_quote(&default_terminal_startup_command())),
        )
    };
    let notes = server
        .notes
        .iter()
        .map(|note| super::replace_remote_placeholders(note, &context))
        .collect::<Vec<_>>();
    let workspace = render_workspace(&server.workspace, &context);
    let services = if server.services.is_empty() {
        server
            .surfaces
            .iter()
            .map(|surface| web_service(&render_surface(surface, &context)))
            .collect::<Vec<_>>()
    } else {
        server
            .services
            .iter()
            .map(|service| render_service(service, &context))
            .collect::<Vec<_>>()
    };
    let surfaces = services
        .iter()
        .filter_map(|service| service.web.clone())
        .collect::<Vec<_>>();

    DeveloperTarget {
        id: machine_host_target_id(server),
        machine_id: server.id.clone(),
        container_id: String::from("host"),
        transport: server.transport,
        container_kind: ManagedContainerKind::Host,
        label,
        host,
        description,
        terminal_command,
        terminal_startup_command: default_terminal_startup_command(),
        notes,
        workspace,
        services,
        surfaces,
    }
}

pub(crate) fn build_target_from_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> DeveloperTarget {
    let container_label = container
        .label
        .clone()
        .unwrap_or_else(|| default_container_label(&container.name, &server.container_name_suffix));
    let container_ssh_user = resolve_container_ssh_user(server, container);
    let context = RemoteContainerContext {
        container_ip: &container.ipv4,
        container_label: &container_label,
        container_name: &container.name,
        ssh_user: if matches!(container.kind, ManagedContainerKind::Host) {
            &server.ssh_user
        } else {
            &container_ssh_user
        },
        server,
    };

    let label = render_template(&server.target_label_template, &context, || {
        container_label.clone()
    });
    let host = if matches!(server.transport, MachineTransport::Local) {
        String::from("localhost")
    } else {
        render_template(&server.target_host_template, &context, || {
            server.host.clone()
        })
    };
    let description = render_template(&server.target_description_template, &context, || {
        container.description.clone().unwrap_or_else(|| {
            format!(
                "{} development container on {} ({})",
                container_label, server.label, container.status
            )
        })
    });
    let workspace_source = container.workspace.as_ref().unwrap_or(&server.workspace);
    let workspace = render_workspace(workspace_source, &context);
    let terminal_startup_command = if matches!(container.kind, ManagedContainerKind::Managed) {
        profile_terminal_startup_command(&workspace.profile)
    } else {
        default_terminal_startup_command()
    };
    let terminal_command = if matches!(server.transport, MachineTransport::Local) {
        String::from("exec /bin/zsh -l")
    } else if matches!(container.kind, ManagedContainerKind::Host) {
        build_host_ssh_command(
            server,
            &["-tt"],
            Some(&shell_single_quote(&default_terminal_startup_command())),
        )
    } else {
        terminal_command_for_server(server, &context, &terminal_startup_command)
    };
    let notes = server
        .notes
        .iter()
        .map(|note| super::replace_remote_placeholders(note, &context))
        .collect::<Vec<_>>();
    let services_source = if !container.services.is_empty() {
        &container.services
    } else {
        &server.services
    };
    let surfaces_source = if !container.surfaces.is_empty() {
        &container.surfaces
    } else {
        &server.surfaces
    };
    let services = if services_source.is_empty() {
        surfaces_source
            .iter()
            .map(|surface| web_service(&render_surface(surface, &context)))
            .collect::<Vec<_>>()
    } else {
        services_source
            .iter()
            .map(|service| render_service(service, &context))
            .collect::<Vec<_>>()
    };
    let surfaces = services
        .iter()
        .filter_map(|service| service.web.clone())
        .collect::<Vec<_>>();

    DeveloperTarget {
        id: format!("{}::{}", server.id, container.id),
        machine_id: server.id.clone(),
        container_id: container.id.clone(),
        transport: server.transport,
        container_kind: container.kind,
        label,
        host,
        description,
        terminal_command,
        terminal_startup_command,
        notes,
        workspace,
        services,
        surfaces,
    }
}

fn build_managed_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
    probe_ssh: bool,
) -> (ManagedContainer, Option<DeveloperTarget>) {
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
            probe_managed_container_ssh(server, &container.ipv4, &container.name, &ssh_user)
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

fn machine_host_state(server: &RemoteContainerServer, probe_ssh: bool) -> (bool, String, String) {
    if matches!(server.transport, MachineTransport::Local) {
        return (
            true,
            String::from("ready"),
            String::from("Local machine is ready."),
        );
    }

    if probe_ssh {
        return probe_machine_host_ssh(server);
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
            format!("{} development container(s) are SSH reachable.", reachable),
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
            format!("Machine host is unreachable: {}", host_message),
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
                "Machine host is ready. {} managed container(s) are SSH reachable.",
                total
            ),
        );
    }

    if host_message == "Waiting for background SSH probe." || reachable > 0 && !host_reachable {
        return (
            String::from("checking"),
            format!("Checking machine host and {} managed container(s).", total),
        );
    }

    if host_reachable {
        return (
            String::from("error"),
            format!(
                "Machine host is ready, but only {} of {} managed container(s) are SSH reachable.",
                reachable, total
            ),
        );
    }

    (
        String::from("error"),
        format!(
            "Machine host is unreachable: {} Detected {} managed container(s), but none are SSH reachable.",
            host_message, total
        ),
    )
}

fn build_server_inventory(
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
        let (host_reachable, _, host_message) = machine_host_state(server, true);
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
            state,
            message,
            containers: managed_containers,
        },
        available_targets,
    }
}

pub(crate) fn inventory_from_scanned_containers(
    server: &RemoteContainerServer,
    containers: Vec<DiscoveredContainer>,
) -> DiscoveredServerInventory {
    build_server_inventory(server, containers, false)
}

pub(crate) fn discover_remote_server_inventory(
    server: &RemoteContainerServer,
) -> DiscoveredServerInventory {
    match scan_server_containers(server) {
        Ok(containers) => build_server_inventory(server, containers, true),
        Err(error) => {
            let host_target = build_machine_host_target(server);
            let (host_reachable, _, host_message) = machine_host_state(server, true);
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
                    state: String::from("error"),
                    message,
                    containers: Vec::new(),
                },
                available_targets: vec![host_target],
            }
        }
    }
}

pub(crate) fn cached_remote_server_inventory(
    server: &RemoteContainerServer,
) -> DiscoveredServerInventory {
    let containers = cached_server_containers(server);
    build_server_inventory(server, containers, false)
}
