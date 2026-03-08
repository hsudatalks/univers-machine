use crate::models::{BrowserSurface, DeveloperTarget, ManagedContainer, ManagedServer};
use csv::ReaderBuilder;

use super::{
    DiscoveredContainer, DiscoveredServerInventory, RemoteContainerContext, RemoteContainerServer,
};
use super::ssh::{
    probe_managed_container_ssh, ssh_destination, terminal_command_for_server,
};
use crate::shell;

pub(super) fn default_discovery_command(server: &RemoteContainerServer) -> String {
    format!("ssh {} 'lxc list --format csv -c ns4'", server.host)
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
    BrowserSurface {
        id: surface.id.clone(),
        label: replace_remote_placeholders(&surface.label, context),
        tunnel_command: replace_remote_placeholders(&surface.tunnel_command, context),
        local_url: replace_remote_placeholders(&surface.local_url, context),
        remote_url: replace_remote_placeholders(&surface.remote_url, context),
        vite_hmr_tunnel_command: replace_remote_placeholders(
            &surface.vite_hmr_tunnel_command,
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

pub(super) fn parse_discovered_containers(
    server: &RemoteContainerServer,
    discovery_output: &str,
) -> Result<Vec<DiscoveredContainer>, String> {
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
            name: name.to_string(),
            status: status.to_string(),
            ipv4,
        });
    }

    Ok(containers)
}

fn discover_server_containers(
    server: &RemoteContainerServer,
) -> Result<Vec<DiscoveredContainer>, String> {
    let output = discover_server_containers_output(server)?;
    parse_discovered_containers(server, &output)
}

pub(super) fn build_target_from_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> DeveloperTarget {
    let container_label = default_container_label(&container.name, &server.container_name_suffix);
    let context = RemoteContainerContext {
        container_ip: &container.ipv4,
        container_label: &container_label,
        container_name: &container.name,
        server,
    };

    let label = render_template(&server.target_label_template, &context, || {
        container_label.clone()
    });
    let host = render_template(&server.target_host_template, &context, || {
        server.host.clone()
    });
    let description = render_template(&server.target_description_template, &context, || {
        format!(
            "{} development container on {} ({})",
            container_label, server.label, container.status
        )
    });
    let terminal_command = terminal_command_for_server(server, &context);
    let notes = server
        .notes
        .iter()
        .map(|note| replace_remote_placeholders(note, &context))
        .collect::<Vec<_>>();
    let surfaces = server
        .surfaces
        .iter()
        .map(|surface| render_surface(surface, &context))
        .collect::<Vec<_>>();

    DeveloperTarget {
        id: format!("{}::{}", server.id, container.name),
        label,
        host,
        description,
        terminal_command,
        notes,
        surfaces,
    }
}

fn build_managed_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> (ManagedContainer, Option<DeveloperTarget>) {
    let target = build_target_from_container(server, container);
    let ssh_command = target.terminal_command.clone();
    let ssh_dest = ssh_destination(server, &container.ipv4);
    let (ssh_reachable, ssh_state, ssh_message) =
        probe_managed_container_ssh(server, &container.ipv4, &container.name);

    (
        ManagedContainer {
            server_id: server.id.clone(),
            server_label: server.label.clone(),
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
        ssh_reachable.then_some(target),
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

pub(super) fn discover_remote_server_inventory(
    server: &RemoteContainerServer,
) -> DiscoveredServerInventory {
    match discover_server_containers(server) {
        Ok(containers) => {
            let mut managed_containers = Vec::new();
            let mut available_targets = Vec::new();

            for container in containers {
                let (managed_container, available_target) =
                    build_managed_container(server, &container);
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
                    host: server.host.clone(),
                    description: server.description.clone(),
                    state,
                    message,
                    containers: managed_containers,
                },
                available_targets,
            }
        }
        Err(error) => DiscoveredServerInventory {
            server: ManagedServer {
                id: server.id.clone(),
                label: server.label.clone(),
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
