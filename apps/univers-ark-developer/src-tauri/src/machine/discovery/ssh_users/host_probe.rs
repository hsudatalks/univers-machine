use crate::{
    machine::{execute_target_command_via_russh, ssh::shell_single_quote},
    models::ManagedContainerKind,
};
use std::collections::HashSet;

use super::super::super::{ContainerManagerType, DiscoveredContainer, RemoteContainerServer};

const CONTAINER_LOGIN_USERS_QUERY: &str = r#"if command -v getent >/dev/null 2>&1; then getent passwd; elif [ -r /etc/passwd ]; then cat /etc/passwd; fi | awk -F: '($1 == "root" || $3 >= 1000) && $6 ~ /^\// && $7 !~ /(nologin|false)$/ { print $1 }'"#;
const CONTAINER_USER_DISCOVERY_TIMEOUT_SECONDS: u64 = 5;

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

    let _ = server;
    Some(remote_command)
}

pub(super) fn parse_discovered_container_users(output: &str) -> Vec<String> {
    let mut seen = HashSet::new();

    output
        .lines()
        .map(str::trim)
        .filter(|user| !user.is_empty())
        .filter(|user| seen.insert((*user).to_string()))
        .map(ToOwned::to_owned)
        .collect()
}

pub(super) fn discover_container_ssh_users_via_host(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
) -> Vec<String> {
    if matches!(container.kind, ManagedContainerKind::Host) {
        return vec![server.ssh_user.clone()];
    }

    let Some(command) = host_container_users_command(server, container) else {
        return Vec::new();
    };

    let Ok(output) = execute_target_command_via_russh(&format!("{}::host", server.id), &command)
    else {
        return Vec::new();
    };

    if output.exit_status != 0 {
        return Vec::new();
    }

    parse_discovered_container_users(&String::from_utf8_lossy(&output.stdout))
}
