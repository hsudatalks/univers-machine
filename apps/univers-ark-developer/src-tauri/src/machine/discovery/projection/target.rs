use crate::{
    models::{DeveloperTarget, MachineTransport, ManagedContainerKind},
    services::catalog::web_service,
};

use super::super::super::ssh::{
    build_host_ssh_command, host_terminal_startup_command, profile_terminal_startup_command,
    shell_single_quote, terminal_command_for_server,
};
use super::super::super::{DiscoveredContainer, RemoteContainerContext, RemoteContainerServer};
use super::super::ssh_users::resolve_container_ssh_user;
use super::super::{
    default_container_label, render_service, render_surface, render_template, render_workspace,
};

fn machine_host_target_id(server: &RemoteContainerServer) -> String {
    format!("{}::host", server.id)
}

pub(super) fn build_machine_host_target(server: &RemoteContainerServer) -> DeveloperTarget {
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
    let terminal_startup_command = host_terminal_startup_command(server);
    let terminal_command = if matches!(server.transport, MachineTransport::Local) {
        terminal_startup_command.clone()
    } else {
        build_host_ssh_command(
            server,
            &["-tt"],
            Some(&shell_single_quote(&terminal_startup_command)),
        )
    };
    let notes = server
        .notes
        .iter()
        .map(|note| super::super::replace_remote_placeholders(note, &context))
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
        terminal_startup_command,
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
        host_terminal_startup_command(server)
    };
    let terminal_command = if matches!(server.transport, MachineTransport::Local) {
        terminal_startup_command.clone()
    } else if matches!(container.kind, ManagedContainerKind::Host) {
        build_host_ssh_command(
            server,
            &["-tt"],
            Some(&shell_single_quote(&terminal_startup_command)),
        )
    } else {
        terminal_command_for_server(server, &context, &terminal_startup_command)
    };
    let notes = server
        .notes
        .iter()
        .map(|note| super::super::replace_remote_placeholders(note, &context))
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
