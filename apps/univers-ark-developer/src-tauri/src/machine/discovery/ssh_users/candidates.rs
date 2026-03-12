use crate::models::ManagedContainerKind;
use std::collections::HashSet;

use super::super::super::{DiscoveredContainer, RemoteContainerServer};

pub(super) fn managed_container_ssh_user_candidates(server: &RemoteContainerServer) -> Vec<String> {
    [
        super::super::super::ssh::managed_container_ssh_user(server),
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

    if !container.ssh_user.trim().is_empty() {
        users.push(container.ssh_user.clone());
    }
    users.extend(
        container
            .ssh_user_candidates
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|candidate| !candidate.is_empty())
            .map(ToOwned::to_owned),
    );

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

    users.extend(managed_container_ssh_user_candidates(server));

    let mut seen = HashSet::new();
    users
        .into_iter()
        .filter(|candidate| seen.insert(candidate.clone()))
        .collect()
}

pub(super) fn preferred_available_container_ssh_users(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
    available_users: Vec<String>,
) -> Vec<String> {
    if matches!(container.kind, ManagedContainerKind::Host) {
        return vec![server.ssh_user.clone()];
    }

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
