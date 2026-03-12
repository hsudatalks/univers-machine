use crate::{
    models::{
        BrowserServiceType, BrowserSurface, ContainerWorkspace, DeveloperService, DeveloperTarget,
        MachineTransport, ManagedContainer, ManagedContainerKind, ManagedServer,
    },
    services::catalog::web_service,
};
use csv::ReaderBuilder;
use std::collections::HashSet;

use super::ssh::{
    build_host_ssh_command, default_terminal_startup_command, managed_container_ssh_user,
    probe_machine_host_ssh, probe_managed_container_ssh, profile_terminal_startup_command,
    shell_single_quote, ssh_destination, terminal_command_for_server,
};
use super::{
    ContainerDiscoveryMode, ContainerManagerType, DiscoveredContainer, DiscoveredServerInventory,
    MachineContainerConfig, RemoteContainerContext, RemoteContainerServer,
};
use crate::shell;

const CONTAINER_LOGIN_USERS_QUERY: &str = r#"if command -v getent >/dev/null 2>&1; then getent passwd; elif [ -r /etc/passwd ]; then cat /etc/passwd; fi | awk -F: '($1 == "root" || $3 >= 1000) && $6 ~ /^\// && $7 !~ /(nologin|false)$/ { print $1 }'"#;
const CONTAINER_USER_DISCOVERY_TIMEOUT_SECONDS: u64 = 5;

fn default_discovery_command_for_manager(
    server: &RemoteContainerServer,
    manager_type: ContainerManagerType,
) -> String {
    match manager_type {
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

fn run_discovery_command(server: &RemoteContainerServer, command: &str) -> Result<String, String> {
    let output = shell::shell_command(&command).output().map_err(|error| {
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

    let items = list
        .into_iter()
        .filter(|item| server.include_stopped || item.state.eq_ignore_ascii_case("running"))
        .collect::<Vec<_>>();

    std::thread::scope(|scope| {
        let handles = items
            .into_iter()
            .map(|item| {
                scope.spawn(move || -> Result<Option<DiscoveredContainer>, String> {
                    let info_command = build_host_ssh_command(
                        server,
                        &[],
                        Some(&shell_single_quote(&format!(
                            "/opt/homebrew/bin/orb info {} --format json",
                            item.name
                        ))),
                    );
                    let output = shell::shell_command(&info_command)
                        .output()
                        .map_err(|error| {
                            format!(
                                "Failed to read OrbStack info for {} on {}: {}",
                                item.name, server.host, error
                            )
                        })?;

                    if !output.status.success() {
                        return Ok(None);
                    }

                    let info: OrbInfoResponse =
                        serde_json::from_slice(&output.stdout).map_err(|error| {
                            format!(
                                "Failed to parse OrbStack info for {} on {}: {}",
                                item.name, server.host, error
                            )
                        })?;

                    if info.ip4.trim().is_empty() {
                        return Ok(None);
                    }

                    Ok(Some(DiscoveredContainer {
                        id: info.record.name.clone(),
                        kind: ManagedContainerKind::Managed,
                        name: info.record.name,
                        source: String::from("orbstack"),
                        ssh_user: String::new(),
                        ssh_user_candidates: vec![],
                        status: info.record.state.to_uppercase(),
                        ipv4: info.ip4,
                        label: None,
                        description: None,
                        workspace: None,
                        services: vec![],
                        surfaces: vec![],
                    }))
                })
            })
            .collect::<Vec<_>>();

        let mut containers = Vec::new();
        for handle in handles {
            let result = handle.join().map_err(|_| {
                format!(
                    "OrbStack discovery worker panicked while scanning {}",
                    server.host
                )
            })?;
            if let Some(container) = result? {
                containers.push(container);
            }
        }

        Ok(containers)
    })
}

fn machine_container_to_discovered(container: &MachineContainerConfig) -> DiscoveredContainer {
    let has_workspace_override = !container.workspace.profile.trim().is_empty()
        || !container.workspace.default_tool.trim().is_empty()
        || !container.workspace.project_path.trim().is_empty()
        || !container.workspace.files_root.trim().is_empty()
        || !container.workspace.primary_web_service_id.trim().is_empty()
        || !container
            .workspace
            .tmux_command_service_id
            .trim()
            .is_empty();

    DiscoveredContainer {
        id: if container.id.trim().is_empty() {
            container.name.clone()
        } else {
            container.id.clone()
        },
        kind: container.kind,
        name: container.name.clone(),
        source: if container.source.trim().is_empty() {
            if matches!(container.kind, ManagedContainerKind::Host) {
                String::from("host")
            } else {
                String::from("manual")
            }
        } else {
            container.source.clone()
        },
        ssh_user: container.ssh_user.clone(),
        ssh_user_candidates: container.ssh_user_candidates.clone(),
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

fn machine_host_target_id(server: &RemoteContainerServer) -> String {
    format!("{}::host", server.id)
}

fn discover_manual_containers(server: &RemoteContainerServer) -> Vec<DiscoveredContainer> {
    server
        .containers
        .iter()
        .filter(|container| matches!(container.kind, ManagedContainerKind::Managed))
        .filter(|container| container.enabled)
        .filter(|container| !container.name.trim().is_empty() && !container.ipv4.trim().is_empty())
        .map(machine_container_to_discovered)
        .collect()
}

fn default_container_ssh_user_candidates(server: &RemoteContainerServer) -> Vec<String> {
    [
        managed_container_ssh_user(server),
        &server.ssh_user,
        "ubuntu",
        "root",
        "admin",
        "ec2-user",
        "debian",
        "core",
        "opc",
    ]
    .into_iter()
    .filter_map(|candidate| {
        let candidate = candidate.trim();
        (!candidate.is_empty()).then(|| candidate.to_string())
    })
    .collect()
}

fn candidate_container_ssh_users(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> Vec<String> {
    let existing = server
        .containers
        .iter()
        .find(|existing| existing.name == container.name);
    let mut users = Vec::new();

    if let Some(existing) = existing {
        if !existing.ssh_user.trim().is_empty() {
            users.push(existing.ssh_user.clone());
        }
        users.extend(
            existing
                .ssh_user_candidates
                .iter()
                .map(String::as_str)
                .map(str::trim)
                .filter(|candidate| !candidate.is_empty())
                .map(ToOwned::to_owned),
        );
    }

    users.extend(default_container_ssh_user_candidates(server));

    let mut seen = HashSet::new();
    users
        .into_iter()
        .filter(|candidate| seen.insert(candidate.clone()))
        .collect()
}

fn manager_type_for_container(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> Option<ContainerManagerType> {
    match container.source.trim() {
        "lxd" => Some(ContainerManagerType::Lxd),
        "docker" => Some(ContainerManagerType::Docker),
        "orbstack" => Some(ContainerManagerType::Orbstack),
        _ => match server.manager_type {
            ContainerManagerType::None => None,
            manager_type => Some(manager_type),
        },
    }
}

fn host_container_users_command(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> Option<String> {
    let manager_type = manager_type_for_container(server, container)?;
    let container_name = shell_single_quote(&container.name);
    let query = shell_single_quote(CONTAINER_LOGIN_USERS_QUERY);
    let exec_command = match manager_type {
        ContainerManagerType::Lxd => {
            format!("lxc exec {} -- sh -lc {}", container_name, query)
        }
        ContainerManagerType::Docker => {
            format!("docker exec {} sh -lc {}", container_name, query)
        }
        ContainerManagerType::Orbstack => {
            format!(
                "/opt/homebrew/bin/orb run -m {} sh -lc {}",
                container_name, query
            )
        }
        ContainerManagerType::None => return None,
    };
    let remote_command = format!(
        "if command -v timeout >/dev/null 2>&1; then timeout {seconds} {command}; elif command -v gtimeout >/dev/null 2>&1; then gtimeout {seconds} {command}; else {command}; fi",
        seconds = CONTAINER_USER_DISCOVERY_TIMEOUT_SECONDS,
        command = exec_command,
    );

    Some(build_host_ssh_command(
        server,
        &[],
        Some(&shell_single_quote(&remote_command)),
    ))
}

fn parse_discovered_container_users(output: &str) -> Vec<String> {
    let mut seen = HashSet::new();

    output
        .lines()
        .map(str::trim)
        .filter(|user| !user.is_empty())
        .filter(|user| seen.insert((*user).to_string()))
        .map(ToOwned::to_owned)
        .collect()
}

fn discover_container_ssh_users_via_host(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> Vec<String> {
    let Some(command) = host_container_users_command(server, container) else {
        return Vec::new();
    };

    let Ok(output) = shell::shell_command(&command).output() else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    parse_discovered_container_users(&String::from_utf8_lossy(&output.stdout))
}

fn preferred_available_container_ssh_users(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
    available_users: Vec<String>,
) -> Vec<String> {
    if available_users.is_empty() {
        return candidate_container_ssh_users(server, container);
    }

    let available_user_set = available_users
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut ordered = candidate_container_ssh_users(server, container)
        .into_iter()
        .filter(|candidate| available_user_set.contains(candidate.as_str()))
        .collect::<Vec<_>>();

    for available_user in available_users {
        if !ordered.iter().any(|candidate| candidate == &available_user) {
            ordered.push(available_user);
        }
    }

    ordered
}

fn resolve_container_ssh_user(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> String {
    if matches!(container.kind, ManagedContainerKind::Host) {
        return server.ssh_user.clone();
    }

    if !container.ssh_user.trim().is_empty() {
        return container.ssh_user.clone();
    }

    managed_container_ssh_user(server).to_string()
}

fn enrich_discovered_container_ssh_users(
    server: &RemoteContainerServer,
    containers: Vec<DiscoveredContainer>,
) -> Vec<DiscoveredContainer> {
    std::thread::scope(|scope| {
        let handles = containers
            .into_iter()
            .map(|container| {
                scope.spawn(move || {
                    if matches!(container.kind, ManagedContainerKind::Host) {
                        return DiscoveredContainer {
                            ssh_user: server.ssh_user.clone(),
                            ssh_user_candidates: vec![server.ssh_user.clone()],
                            ..container
                        };
                    }

                    let discovered_users =
                        discover_container_ssh_users_via_host(server, &container);
                    let ssh_user_candidates = preferred_available_container_ssh_users(
                        server,
                        &container,
                        discovered_users,
                    );
                    let ssh_user = ssh_user_candidates
                        .first()
                        .cloned()
                        .unwrap_or_else(|| managed_container_ssh_user(server).to_string());

                    DiscoveredContainer {
                        ssh_user,
                        ssh_user_candidates,
                        ..container
                    }
                })
            })
            .collect::<Vec<_>>();

        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{
        ContainerDiscoveryMode, ContainerManagerType, DiscoveredContainer, RemoteContainerServer,
    };
    use crate::models::{ContainerWorkspace, MachineTransport, ManagedContainerKind};

    fn fixture_server() -> RemoteContainerServer {
        RemoteContainerServer {
            id: String::from("qa-dev"),
            label: String::from("QA"),
            transport: MachineTransport::Ssh,
            host: String::from("qa-dev"),
            port: 22,
            description: String::new(),
            manager_type: ContainerManagerType::Lxd,
            discovery_mode: ContainerDiscoveryMode::Auto,
            discovery_command: String::new(),
            ssh_user: String::from("david"),
            container_ssh_user: String::from("david"),
            identity_files: vec![],
            jump_chain: vec![],
            known_hosts_path: String::new(),
            strict_host_key_checking: false,
            container_name_suffix: String::new(),
            include_stopped: false,
            target_label_template: String::new(),
            target_host_template: String::new(),
            target_description_template: String::new(),
            terminal_command_template: String::new(),
            notes: vec![],
            workspace: ContainerWorkspace::default(),
            services: vec![],
            surfaces: vec![],
            containers: vec![],
        }
    }

    #[test]
    fn prefers_available_container_users_over_stale_machine_default() {
        let server = fixture_server();
        let container = DiscoveredContainer {
            id: String::from("maintenance-dev"),
            kind: ManagedContainerKind::Managed,
            name: String::from("maintenance-dev"),
            source: String::from("lxd"),
            ssh_user: String::new(),
            ssh_user_candidates: vec![],
            status: String::from("RUNNING"),
            ipv4: String::from("10.0.0.2"),
            label: None,
            description: None,
            workspace: None,
            services: vec![],
            surfaces: vec![],
        };

        let users = preferred_available_container_ssh_users(
            &server,
            &container,
            vec![String::from("root"), String::from("ubuntu")],
        );

        assert_eq!(users, vec![String::from("ubuntu"), String::from("root")]);
    }

    #[test]
    fn parses_direct_container_user_query_output() {
        let users = parse_discovered_container_users("ubuntu\nroot\nubuntu\n");
        assert_eq!(users, vec![String::from("ubuntu"), String::from("root")]);
    }
}

fn parse_csv_discovered_containers(
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

        let Some(ipv4) = extract_ipv4(raw_ipv4) else {
            continue;
        };

        containers.push(DiscoveredContainer {
            id: name.to_string(),
            kind: ManagedContainerKind::Managed,
            name: name.to_string(),
            source: String::from("unknown"),
            ssh_user: String::new(),
            ssh_user_candidates: vec![],
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

fn parse_discovered_containers_for_manager(
    server: &RemoteContainerServer,
    manager_type: ContainerManagerType,
    discovery_output: &str,
) -> Result<Vec<DiscoveredContainer>, String> {
    if matches!(manager_type, ContainerManagerType::Orbstack) {
        return parse_orbstack_containers(server, discovery_output);
    }

    let mut containers = parse_csv_discovered_containers(server, discovery_output)?;
    let source = match manager_type {
        ContainerManagerType::Docker => "docker",
        ContainerManagerType::Lxd => "lxd",
        ContainerManagerType::Orbstack => "orbstack",
        ContainerManagerType::None => "unknown",
    };
    containers.iter_mut().for_each(|container| {
        container.source = source.to_string();
    });
    Ok(containers)
}

pub(super) fn parse_discovered_containers(
    server: &RemoteContainerServer,
    discovery_output: &str,
) -> Result<Vec<DiscoveredContainer>, String> {
    let trimmed = discovery_output.trim_start();

    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        if let Ok(containers) = parse_orbstack_containers(server, discovery_output) {
            return Ok(containers);
        }
    }

    let mut containers = parse_csv_discovered_containers(server, discovery_output)?;
    containers.iter_mut().for_each(|container| {
        container.source = String::from("custom");
    });
    Ok(containers)
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
                    let command = default_discovery_command_for_manager(server, manager_type);
                    if command.trim().is_empty() {
                        return ManagerDiscoveryResult {
                            manager_type,
                            attempted: false,
                            containers: Ok(Vec::new()),
                        };
                    }
                    let output = match run_discovery_command(server, &command) {
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

pub(super) fn scan_server_containers(
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
        let output = run_discovery_command(server, &server.discovery_command)?;
        containers.extend(parse_discovered_containers(server, &output)?);
        return Ok(enrich_discovered_container_ssh_users(server, containers));
    }

    containers.extend(discover_containers_with_supported_managers(server)?);
    Ok(enrich_discovered_container_ssh_users(server, containers))
}

pub(super) fn cached_server_containers(server: &RemoteContainerServer) -> Vec<DiscoveredContainer> {
    discover_manual_containers(server)
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
        .map(|note| replace_remote_placeholders(note, &context))
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

pub(super) fn build_target_from_container(
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
        .map(|note| replace_remote_placeholders(note, &context))
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
pub(super) fn server_state_for_containers(containers: &[ManagedContainer]) -> (String, String) {
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

pub(super) fn inventory_from_scanned_containers(
    server: &RemoteContainerServer,
    containers: Vec<DiscoveredContainer>,
) -> DiscoveredServerInventory {
    build_server_inventory(server, containers, false)
}

pub(super) fn discover_remote_server_inventory(
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

pub(super) fn cached_remote_server_inventory(
    server: &RemoteContainerServer,
) -> DiscoveredServerInventory {
    let containers = cached_server_containers(server);
    build_server_inventory(server, containers, false)
}
