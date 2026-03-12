use crate::models::ManagedContainerKind;
use csv::ReaderBuilder;

use super::super::super::ssh::{build_host_ssh_command, shell_single_quote};
use super::super::super::{ContainerManagerType, DiscoveredContainer, RemoteContainerServer};
use super::super::{extract_ipv4, trim_quotes};

#[derive(serde::Deserialize)]
struct OrbListItem {
    name: String,
    state: String,
}

#[derive(serde::Deserialize)]
struct OrbInfoRecord {
    name: String,
    state: String,
    config: OrbInfoConfig,
}

#[derive(serde::Deserialize)]
struct OrbInfoConfig {
    #[serde(default)]
    default_username: String,
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
                    let output = crate::infra::shell::shell_command(&info_command)
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
                        ssh_user: info.record.config.default_username.clone(),
                        ssh_user_candidates: if info.record.config.default_username.trim().is_empty()
                        {
                            vec![]
                        } else {
                            vec![info.record.config.default_username]
                        },
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

pub(super) fn parse_discovered_containers_for_manager(
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
