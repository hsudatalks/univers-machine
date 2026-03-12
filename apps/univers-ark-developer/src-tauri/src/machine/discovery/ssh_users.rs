use crate::{infra::shell, models::ManagedContainerKind};
use std::collections::HashSet;

use super::super::ssh::{build_host_ssh_command, managed_container_ssh_user, shell_single_quote};
use super::super::{ContainerManagerType, DiscoveredContainer, RemoteContainerServer};

const CONTAINER_LOGIN_USERS_QUERY: &str = r#"if command -v getent >/dev/null 2>&1; then getent passwd; elif [ -r /etc/passwd ]; then cat /etc/passwd; fi | awk -F: '($1 == "root" || $3 >= 1000) && $6 ~ /^\// && $7 !~ /(nologin|false)$/ { print $1 }'"#;
const CONTAINER_USER_DISCOVERY_TIMEOUT_SECONDS: u64 = 5;

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

pub(super) fn resolve_container_ssh_user(
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

pub(super) fn enrich_discovered_container_ssh_users(
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
    use crate::models::{ContainerWorkspace, MachineTransport};

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
