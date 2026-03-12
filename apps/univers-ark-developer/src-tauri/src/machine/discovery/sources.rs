use crate::{
    infra::shell,
    models::{MachineTransport, ManagedContainerKind},
};
use csv::ReaderBuilder;
use std::collections::HashSet;

use super::super::ssh::{build_host_ssh_command, shell_single_quote};
use super::super::{
    ContainerDiscoveryMode, ContainerManagerType, DiscoveredContainer, MachineContainerConfig,
    RemoteContainerServer,
};
use super::ssh_users::enrich_discovered_container_ssh_users;
use super::{extract_ipv4, trim_quotes};

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

pub(crate) fn parse_discovered_containers(
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
        let output = run_discovery_command(server, &server.discovery_command)?;
        containers.extend(parse_discovered_containers(server, &output)?);
        return Ok(enrich_discovered_container_ssh_users(server, containers));
    }

    containers.extend(discover_containers_with_supported_managers(server)?);
    Ok(enrich_discovered_container_ssh_users(server, containers))
}

pub(crate) fn cached_server_containers(server: &RemoteContainerServer) -> Vec<DiscoveredContainer> {
    discover_manual_containers(server)
}
