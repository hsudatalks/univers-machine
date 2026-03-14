mod projection;
mod sources;
mod ssh_users;

use super::RemoteContainerContext;
use crate::models::{BrowserServiceType, BrowserSurface, ContainerWorkspace, DeveloperService};

pub(super) use self::projection::{
    cached_remote_server_inventory, inventory_from_discovered_containers,
    inventory_from_scan_error,
};
pub(super) use self::sources::scan_server_containers;
#[cfg(test)]
pub(super) use self::{
    projection::{build_target_from_container, server_state_for_containers},
    sources::parse_discovered_containers,
};

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
        .replace("{sshUser}", context.ssh_user)
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
        background_prerender: surface.background_prerender,
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
