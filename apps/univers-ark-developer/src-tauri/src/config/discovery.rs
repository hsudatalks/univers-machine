use crate::models::{
    BrowserServiceType, BrowserSurface, ContainerWorkspace, DeveloperService, DeveloperTarget,
    ManagedContainer, ManagedContainerKind, ManagedServer, MachineTransport, web_service,
};
use csv::ReaderBuilder;

use super::{
    ContainerDiscoveryMode, ContainerManagerType, DiscoveredContainer,
    DiscoveredServerInventory, MachineContainerConfig, RemoteContainerContext,
    RemoteContainerServer,
};
use super::ssh::{
    build_host_ssh_command, probe_machine_host_ssh, probe_managed_container_ssh, ssh_destination,
    terminal_command_for_server, shell_single_quote,
};
use crate::shell;

pub(super) fn default_discovery_command(server: &RemoteContainerServer) -> String {
    match server.manager_type {
        ContainerManagerType::None => String::new(),
        ContainerManagerType::Lxd => build_host_ssh_command(
            server,
            &[],
            Some(&shell_single_quote("lxc list --format csv -c ns4")),
        ),
        ContainerManagerType::Docker => build_host_ssh_command(
            server,
            &[],
            Some(&shell_single_quote(
                "docker ps --format \"{{.Names}}\" | while read -r name; do [ -z \"$name\" ] && continue; ip=$(docker inspect -f \"{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}\" \"$name\" 2>/dev/null); printf \"%s,RUNNING,%s\\n\" \"$name\" \"$ip\"; done",
            )),
        ),
        ContainerManagerType::Orbstack => build_host_ssh_command(
            server,
            &[],
            Some(&shell_single_quote("/opt/homebrew/bin/orb list --format json")),
        ),
    }
}

pub(super) fn trim_quotes(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'')
}

fn extract_ipv4(raw_ipv4: &str) -> Option<String> {
    let mut interface_matches = Vec::new();

    for raw_entry in raw_ipv4.split(['\n', ';']) {
        let entry = trim_quotes(raw_entry);
        if entry.is_empty() {
            continue;
        }

        let Some(ipv4) = entry
            .split(|character: char| {
                character.is_whitespace() || matches!(character, ',' | ';' | '(' | ')')
            })
            .map(str::trim)
            .find(|token| !token.is_empty() && token.parse::<std::net::Ipv4Addr>().is_ok())
        else {
            continue;
        };

        let interface = entry
            .split_once('(')
            .and_then(|(_, tail)| tail.split_once(')'))
            .map(|(name, _)| name.trim())
            .unwrap_or_default()
            .to_string();

        interface_matches.push((ipv4.to_string(), interface));
    }

    interface_matches
        .iter()
        .find(|(_, interface)| interface.eq_ignore_ascii_case("eth0"))
        .map(|(ipv4, _)| ipv4.clone())
        .or_else(|| {
            interface_matches
                .iter()
                .find(|(_, interface)| {
                    !interface.is_empty()
                        && !interface.eq_ignore_ascii_case("docker0")
                        && !interface.starts_with("br-")
                        && !interface.eq_ignore_ascii_case("lxdbr0")
                        && !interface.eq_ignore_ascii_case("lo")
                })
                .map(|(ipv4, _)| ipv4.clone())
        })
        .or_else(|| interface_matches.first().map(|(ipv4, _)| ipv4.clone()))
}

fn title_case_word(word: &str) -> String {
    let mut characters = word.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };

    let mut title_cased = String::new();
    title_cased.extend(first.to_uppercase());
    title_cased.push_str(characters.as_str());
    title_cased
}

fn default_container_label(name: &str, suffix: &str) -> String {
    let trimmed = if !suffix.is_empty() && name.ends_with(suffix) {
        &name[..name.len() - suffix.len()]
    } else {
        name
    };

    trimmed
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(title_case_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn replace_remote_placeholders(template: &str, context: &RemoteContainerContext<'_>) -> String {
    template
        .replace("{serverId}", &context.server.id)
        .replace("{serverLabel}", &context.server.label)
        .replace("{serverHost}", &context.server.host)
        .replace("{serverDescription}", &context.server.description)
        .replace("{machineId}", &context.server.id)
        .replace("{machineLabel}", &context.server.label)
        .replace("{machineHost}", &context.server.host)
        .replace("{machineDescription}", &context.server.description)
        .replace("{containerIp}", context.container_ip)
        .replace("{containerLabel}", context.container_label)
        .replace("{containerName}", context.container_name)
        .replace(
            "{containerHostKeyAlias}",
            &super::ssh::container_host_key_alias(context.server, context.container_name),
        )
        .replace(
            "{sshOptions}",
            &super::ssh::ssh_options_for_context(context.server, context.container_name),
        )
        .replace("{sshUser}", &context.server.ssh_user)
}

pub(super) fn render_template(
    template: &str,
    context: &RemoteContainerContext<'_>,
    fallback: impl FnOnce() -> String,
) -> String {
    if template.trim().is_empty() {
        return fallback();
    }

    replace_remote_placeholders(template, context)
}

fn render_surface(
    surface: &BrowserSurface,
    context: &RemoteContainerContext<'_>,
) -> BrowserSurface {
    let service_type = if matches!(surface.service_type, BrowserServiceType::Http)
        && !surface.vite_hmr_tunnel_command.trim().is_empty()
    {
        BrowserServiceType::Vite
    } else {
        surface.service_type
    };

    BrowserSurface {
        id: surface.id.clone(),
        label: replace_remote_placeholders(&surface.label, context),
        service_type,
        tunnel_command: replace_remote_placeholders(&surface.tunnel_command, context),
        local_url: replace_remote_placeholders(&surface.local_url, context),
        remote_url: replace_remote_placeholders(&surface.remote_url, context),
        vite_hmr_tunnel_command: replace_remote_placeholders(
            &surface.vite_hmr_tunnel_command,
            context,
        ),
    }
}

fn render_service(
    service: &DeveloperService,
    context: &RemoteContainerContext<'_>,
) -> DeveloperService {
    let mut rendered = service.clone();
    rendered.id = replace_remote_placeholders(&service.id, context);
    rendered.label = replace_remote_placeholders(&service.label, context);
    rendered.description = replace_remote_placeholders(&service.description, context);
    rendered.web = service
        .web
        .as_ref()
        .map(|surface| render_surface(surface, context));
    rendered.endpoint = service.endpoint.as_ref().map(|endpoint| {
        let mut rendered_endpoint = endpoint.clone();
        rendered_endpoint.host = replace_remote_placeholders(&endpoint.host, context);
        rendered_endpoint.path = replace_remote_placeholders(&endpoint.path, context);
        rendered_endpoint.url = replace_remote_placeholders(&endpoint.url, context);
        rendered_endpoint
    });
    rendered
}

fn render_workspace(
    workspace: &ContainerWorkspace,
    context: &RemoteContainerContext<'_>,
) -> ContainerWorkspace {
    ContainerWorkspace {
        profile: replace_remote_placeholders(&workspace.profile, context),
        default_tool: replace_remote_placeholders(&workspace.default_tool, context),
        project_path: replace_remote_placeholders(&workspace.project_path, context),
        files_root: replace_remote_placeholders(&workspace.files_root, context),
        primary_web_service_id: replace_remote_placeholders(
            &workspace.primary_web_service_id,
            context,
        ),
        tmux_command_service_id: replace_remote_placeholders(
            &workspace.tmux_command_service_id,
            context,
        ),
    }
}

fn discover_server_containers_output(server: &RemoteContainerServer) -> Result<String, String> {
    let command = if server.discovery_command.trim().is_empty() {
        default_discovery_command(server)
    } else {
        server.discovery_command.clone()
    };

    let output = shell::shell_command(&command)
        .output()
        .map_err(|error| {
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

#[derive(serde::Deserialize)]
struct OrbListItem {
    name: String,
    state: String,
}

#[derive(serde::Deserialize)]
struct OrbInfoRecord {
    name: String,
    state: String,
}

#[derive(serde::Deserialize)]
struct OrbInfoResponse {
    record: OrbInfoRecord,
    ip4: String,
}

fn parse_orbstack_containers(
    server: &RemoteContainerServer,
    discovery_output: &str,
) -> Result<Vec<DiscoveredContainer>, String> {
    let list: Vec<OrbListItem> = serde_json::from_str(discovery_output).map_err(|error| {
        format!(
            "Failed to parse OrbStack discovery output for {}: {}",
            server.host, error
        )
    })?;

    let mut containers = Vec::new();
    for item in list {
        if !server.container_name_suffix.is_empty()
            && !item.name.ends_with(&server.container_name_suffix)
        {
            continue;
        }

        if !server.include_stopped && !item.state.eq_ignore_ascii_case("running") {
            continue;
        }

        let info_command = build_host_ssh_command(
            server,
            &[],
            Some(&shell_single_quote(&format!(
                "/opt/homebrew/bin/orb info {} --format json",
                item.name
            ))),
        );
        let output = shell::shell_command(&info_command).output().map_err(|error| {
            format!(
                "Failed to read OrbStack info for {} on {}: {}",
                item.name, server.host, error
            )
        })?;

        if !output.status.success() {
            continue;
        }

        let info: OrbInfoResponse = serde_json::from_slice(&output.stdout).map_err(|error| {
            format!(
                "Failed to parse OrbStack info for {} on {}: {}",
                item.name, server.host, error
            )
        })?;

        if info.ip4.trim().is_empty() {
            continue;
        }

        containers.push(DiscoveredContainer {
            id: info.record.name.clone(),
            kind: ManagedContainerKind::Managed,
            name: info.record.name,
            status: info.record.state.to_uppercase(),
            ipv4: info.ip4,
            label: None,
            description: None,
            workspace: None,
            services: vec![],
            surfaces: vec![],
        });
    }

    Ok(containers)
}

fn machine_container_to_discovered(container: &MachineContainerConfig) -> DiscoveredContainer {
    let has_workspace_override = !container.workspace.profile.trim().is_empty()
        || !container.workspace.default_tool.trim().is_empty()
        || !container.workspace.project_path.trim().is_empty()
        || !container.workspace.files_root.trim().is_empty()
        || !container.workspace.primary_web_service_id.trim().is_empty()
        || !container.workspace.tmux_command_service_id.trim().is_empty();

    DiscoveredContainer {
        id: if container.id.trim().is_empty() {
            container.name.clone()
        } else {
            container.id.clone()
        },
        kind: container.kind,
        name: container.name.clone(),
        status: container.status.clone(),
        ipv4: container.ipv4.clone(),
        label: (!container.label.trim().is_empty()).then(|| container.label.clone()),
        description: (!container.description.trim().is_empty())
            .then(|| container.description.clone()),
        workspace: has_workspace_override.then(|| container.workspace.clone()),
        services: container.services.clone(),
        surfaces: container.surfaces.clone(),
    }
}

fn discover_host_container(server: &RemoteContainerServer) -> DiscoveredContainer {
    let container = server
        .containers
        .iter()
        .find(|container| {
            matches!(container.kind, ManagedContainerKind::Host) || container.id == "host"
        })
        .cloned()
        .unwrap_or_else(|| MachineContainerConfig {
            id: String::from("host"),
            name: String::from("host"),
            kind: ManagedContainerKind::Host,
            label: String::from("Host"),
            description: String::new(),
            ipv4: String::new(),
            status: String::from("RUNNING"),
            workspace: server.workspace.clone(),
            services: Vec::new(),
            surfaces: Vec::new(),
        });

    DiscoveredContainer {
        id: if container.id.trim().is_empty() {
            String::from("host")
        } else {
            container.id
        },
        kind: ManagedContainerKind::Host,
        name: if container.name.trim().is_empty() {
            String::from("host")
        } else {
            container.name
        },
        status: if container.status.trim().is_empty() {
            String::from("RUNNING")
        } else {
            container.status
        },
        ipv4: container.ipv4,
        label: Some(if container.label.trim().is_empty() {
            String::from("Host")
        } else {
            container.label
        }),
        description: (!container.description.trim().is_empty()).then(|| container.description),
        workspace: Some(container.workspace),
        services: container.services,
        surfaces: container.surfaces,
    }
}

fn discover_manual_containers(server: &RemoteContainerServer) -> Vec<DiscoveredContainer> {
    server
        .containers
        .iter()
        .filter(|container| matches!(container.kind, ManagedContainerKind::Managed))
        .filter(|container| !container.name.trim().is_empty() && !container.ipv4.trim().is_empty())
        .map(machine_container_to_discovered)
        .collect()
}

pub(super) fn parse_discovered_containers(
    server: &RemoteContainerServer,
    discovery_output: &str,
) -> Result<Vec<DiscoveredContainer>, String> {
    if matches!(server.manager_type, ContainerManagerType::Orbstack) {
        return parse_orbstack_containers(server, discovery_output);
    }

    let mut containers = Vec::new();
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .from_reader(discovery_output.as_bytes());

    for row in reader.records() {
        let row = row.map_err(|error| {
            format!(
                "Failed to parse discovery output for {} as CSV: {}",
                server.host, error
            )
        })?;

        let name = trim_quotes(row.get(0).unwrap_or_default());
        let status = trim_quotes(row.get(1).unwrap_or_default());
        let raw_ipv4 = trim_quotes(row.get(2).unwrap_or_default());

        if name.is_empty() || status.is_empty() {
            continue;
        }

        if !server.include_stopped && !status.eq_ignore_ascii_case("running") {
            continue;
        }

        if !server.container_name_suffix.is_empty()
            && !name.ends_with(&server.container_name_suffix)
        {
            continue;
        }

        let Some(ipv4) = extract_ipv4(raw_ipv4) else {
            continue;
        };

        containers.push(DiscoveredContainer {
            id: name.to_string(),
            kind: ManagedContainerKind::Managed,
            name: name.to_string(),
            status: status.to_string(),
            ipv4,
            label: None,
            description: None,
            workspace: None,
            services: vec![],
            surfaces: vec![],
        });
    }

    Ok(containers)
}

pub(super) fn scan_server_containers(
    server: &RemoteContainerServer,
) -> Result<Vec<DiscoveredContainer>, String> {
    if matches!(server.transport, MachineTransport::Local) {
        return Ok(vec![discover_host_container(server)]);
    }

    if matches!(server.discovery_mode, ContainerDiscoveryMode::HostOnly) {
        return Ok(vec![discover_host_container(server)]);
    }

    if matches!(server.discovery_mode, ContainerDiscoveryMode::Manual) {
        let mut containers = vec![discover_host_container(server)];
        containers.extend(discover_manual_containers(server));
        return Ok(containers);
    }

    let output = discover_server_containers_output(server)?;
    let mut containers = vec![discover_host_container(server)];
    containers.extend(parse_discovered_containers(server, &output)?);
    Ok(containers)
}

pub(super) fn cached_server_containers(
    server: &RemoteContainerServer,
) -> Vec<DiscoveredContainer> {
    let mut containers = vec![discover_host_container(server)];
    containers.extend(discover_manual_containers(server));
    containers
}

pub(super) fn build_target_from_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> DeveloperTarget {
    let container_label = container
        .label
        .clone()
        .unwrap_or_else(|| default_container_label(&container.name, &server.container_name_suffix));
    let context = RemoteContainerContext {
        container_ip: &container.ipv4,
        container_label: &container_label,
        container_name: &container.name,
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
    let terminal_command = if matches!(server.transport, MachineTransport::Local) {
        String::from("exec /bin/zsh -l")
    } else if matches!(container.kind, ManagedContainerKind::Host) {
        build_host_ssh_command(
            server,
            &["-tt"],
            Some(&shell_single_quote(
                "tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l",
            )),
        )
    } else {
        terminal_command_for_server(server, &context)
    };
    let notes = server
        .notes
        .iter()
        .map(|note| replace_remote_placeholders(note, &context))
        .collect::<Vec<_>>();
    let workspace_source = container.workspace.as_ref().unwrap_or(&server.workspace);
    let workspace = render_workspace(workspace_source, &context);
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
        terminal_startup_command: String::from(
            "tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l",
        ),
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
    let ssh_dest = if matches!(container.kind, ManagedContainerKind::Host) {
        if matches!(server.transport, MachineTransport::Local) {
            String::from("local")
        } else {
            format!("{}@{}", server.ssh_user, server.host)
        }
    } else {
        ssh_destination(server, &container.ipv4)
    };
    let (ssh_reachable, ssh_state, ssh_message) = if matches!(server.transport, MachineTransport::Local) {
        (
            true,
            String::from("ready"),
            String::from("Local workspace is ready."),
        )
    } else if probe_ssh && matches!(container.kind, ManagedContainerKind::Host) {
        probe_machine_host_ssh(server)
    } else if probe_ssh {
        probe_managed_container_ssh(server, &container.ipv4, &container.name)
    } else {
        (
            true,
            String::from("cached"),
            String::from("Using cached container snapshot."),
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
            ssh_user: server.ssh_user.clone(),
            ssh_destination: ssh_dest,
            ssh_command,
            ssh_state,
            ssh_message,
            ssh_reachable,
        },
        Some(target),
    )
}

pub(super) fn server_state_for_containers(containers: &[ManagedContainer]) -> (String, String) {
    if containers.is_empty() {
        return (
            String::from("empty"),
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
            String::from("degraded"),
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

fn build_server_inventory(
    server: &RemoteContainerServer,
    containers: Vec<DiscoveredContainer>,
    probe_ssh: bool,
) -> DiscoveredServerInventory {
    let mut managed_containers = Vec::new();
    let mut available_targets = Vec::new();

    for container in containers {
        let (managed_container, available_target) =
            build_managed_container(server, &container, probe_ssh);
        managed_containers.push(managed_container);

        if let Some(available_target) = available_target {
            available_targets.push(available_target);
        }
    }

    let (state, message) = server_state_for_containers(&managed_containers);

    DiscoveredServerInventory {
        server: ManagedServer {
            id: server.id.clone(),
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

pub(super) fn discover_remote_server_inventory(
    server: &RemoteContainerServer,
) -> DiscoveredServerInventory {
    match scan_server_containers(server) {
        Ok(containers) => {
            build_server_inventory(server, containers, true)
        }
        Err(error) => DiscoveredServerInventory {
            server: ManagedServer {
                id: server.id.clone(),
                label: server.label.clone(),
                transport: server.transport,
                host: server.host.clone(),
                description: server.description.clone(),
                state: String::from("error"),
                message: error,
                containers: Vec::new(),
            },
            available_targets: Vec::new(),
        },
    }
}

pub(super) fn cached_remote_server_inventory(
    server: &RemoteContainerServer,
) -> DiscoveredServerInventory {
    let containers = cached_server_containers(server);
    build_server_inventory(server, containers, false)
}
