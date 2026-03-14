use super::{
    discovery::{
        cached_remote_server_inventory, inventory_from_discovered_containers,
        inventory_from_scan_error, scan_server_containers,
    },
    profiles::{apply_profile_defaults_to_remote_server, ContainerProfiles},
    repository::{
        read_machine_inventory_snapshot, read_raw_targets_file, save_machine_inventory_snapshot,
        save_raw_targets_file,
    },
    CachedResolvedInventory, DiscoveredContainer, MachineContainerConfig,
    RemoteContainerServer, ResolvedInventory,
};
use crate::models::{
    ContainerWorkspace, DeveloperService, DeveloperTarget, ManagedContainerKind, ManagedServer,
    TargetsFile,
};
use std::sync::{Mutex, OnceLock};

fn targets_cache() -> &'static Mutex<Option<CachedResolvedInventory>> {
    static TARGETS_CACHE: OnceLock<Mutex<Option<CachedResolvedInventory>>> = OnceLock::new();

    TARGETS_CACHE.get_or_init(|| Mutex::new(None))
}

pub(super) fn clear_targets_cache() {
    if let Ok(mut cache) = targets_cache().lock() {
        *cache = None;
    }
}

pub(super) fn load_inventory(force_refresh: bool) -> Result<ResolvedInventory, String> {
    if !force_refresh {
        if let Ok(cache) = targets_cache().lock() {
            if let Some(cached) = cache.as_ref() {
                return Ok(cached.inventory.clone());
            }
        }
    }

    let mut raw_targets_file = read_raw_targets_file()?;
    let profiles: ContainerProfiles = raw_targets_file.profiles.clone();
    let default_profile = raw_targets_file.default_profile.clone();
    let mut targets = Vec::new();
    let mut servers = Vec::new();

    raw_targets_file.machines.iter_mut().for_each(|server| {
        apply_profile_defaults_to_remote_server(server, &profiles, default_profile.as_deref())
    });

    let discovered: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = raw_targets_file
            .machines
            .iter()
            .map(|server| {
                scope.spawn(|| {
                    if force_refresh {
                        match scan_server_containers(server) {
                            Ok(containers) => {
                                let effective =
                                    effective_discovered_containers(server, &containers);
                                let _ = save_machine_inventory_snapshot(&server.id, &containers);
                                inventory_from_discovered_containers(server, effective, true)
                            }
                            Err(error) => inventory_from_scan_error(server, error),
                        }
                    } else {
                        load_cached_server_inventory(server)
                    }
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });

    for inventory in discovered {
        targets.extend(inventory.available_targets);
        servers.push(inventory.server);
    }

    let inventory = ResolvedInventory {
        targets_file: TargetsFile {
            selected_target_id: raw_targets_file.selected_target_id,
            default_profile,
            targets,
        },
        servers,
    };

    if let Ok(mut cache) = targets_cache().lock() {
        *cache = Some(CachedResolvedInventory {
            inventory: inventory.clone(),
        });
    }

    Ok(inventory)
}

fn has_workspace_override(workspace: &ContainerWorkspace) -> bool {
    !workspace.profile.trim().is_empty()
        || !workspace.default_tool.trim().is_empty()
        || !workspace.project_path.trim().is_empty()
        || !workspace.files_root.trim().is_empty()
        || !workspace.primary_web_service_id.trim().is_empty()
        || !workspace.tmux_command_service_id.trim().is_empty()
}

fn merged_container_services(
    discovered: &DiscoveredContainer,
    existing: Option<&MachineContainerConfig>,
) -> Vec<DeveloperService> {
    existing
        .filter(|item| !item.services.is_empty())
        .map(|item| item.services.clone())
        .unwrap_or_else(|| discovered.services.clone())
}

fn merged_container_workspace(
    discovered: &DiscoveredContainer,
    existing: Option<&MachineContainerConfig>,
) -> Option<ContainerWorkspace> {
    if let Some(existing) = existing.filter(|item| has_workspace_override(&item.workspace)) {
        return Some(existing.workspace.clone());
    }

    discovered.workspace.clone()
}

pub(super) fn merge_discovered_container_with_manual_config(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
    existing: Option<&MachineContainerConfig>,
) -> DiscoveredContainer {
    let id = if container.id.trim().is_empty() {
        existing
            .map(|item| item.id.clone())
            .unwrap_or_else(|| container.name.clone())
    } else {
        container.id.clone()
    };
    let source = if matches!(container.kind, ManagedContainerKind::Host) {
        String::from("host")
    } else if let Some(existing) = existing {
        if existing.source.trim().is_empty() || existing.source == "unknown" {
            container.source.clone()
        } else {
            existing.source.clone()
        }
    } else {
        container.source.clone()
    };
    let ssh_user = if matches!(container.kind, ManagedContainerKind::Host) {
        server.ssh_user.clone()
    } else if !container.ssh_user.trim().is_empty() {
        container.ssh_user.clone()
    } else if let Some(existing) = existing {
        if !existing.ssh_user.trim().is_empty() {
            existing.ssh_user.clone()
        } else if !server.container_ssh_user.trim().is_empty() {
            server.container_ssh_user.clone()
        } else {
            server.ssh_user.clone()
        }
    } else if !server.container_ssh_user.trim().is_empty() {
        server.container_ssh_user.clone()
    } else {
        server.ssh_user.clone()
    };
    let mut ssh_user_candidates = Vec::new();
    if !ssh_user.trim().is_empty() {
        ssh_user_candidates.push(ssh_user.clone());
    }
    ssh_user_candidates.extend(container.ssh_user_candidates.iter().cloned());
    if let Some(existing) = existing {
        ssh_user_candidates.extend(existing.ssh_user_candidates.iter().cloned());
    }
    let mut seen_ssh_users = std::collections::HashSet::new();
    ssh_user_candidates.retain(|candidate| {
        let candidate = candidate.trim();
        !candidate.is_empty() && seen_ssh_users.insert(candidate.to_string())
    });

    DiscoveredContainer {
        id,
        kind: container.kind,
        name: container.name.clone(),
        source,
        ssh_user,
        ssh_user_candidates,
        status: container.status.clone(),
        ipv4: container.ipv4.clone(),
        label: existing
            .filter(|item| !item.label.trim().is_empty())
            .map(|item| item.label.clone())
            .or_else(|| container.label.clone()),
        description: existing
            .filter(|item| !item.description.trim().is_empty())
            .map(|item| item.description.clone())
            .or_else(|| container.description.clone()),
        workspace: merged_container_workspace(container, existing),
        services: merged_container_services(container, existing),
        surfaces: existing
            .filter(|item| !item.surfaces.is_empty())
            .map(|item| item.surfaces.clone())
            .unwrap_or_else(|| container.surfaces.clone()),
    }
}

fn effective_discovered_containers(
    server: &RemoteContainerServer,
    discovered: &[DiscoveredContainer],
) -> Vec<DiscoveredContainer> {
    discovered
        .iter()
        .map(|container| {
            let existing = server
                .containers
                .iter()
                .find(|item| item.name == container.name);
            merge_discovered_container_with_manual_config(server, container, existing)
        })
        .filter(|container| {
            matches!(container.kind, ManagedContainerKind::Host) || {
                server
                    .containers
                    .iter()
                    .find(|item| item.name == container.name)
                    .map(|item| item.enabled)
                    .unwrap_or_else(|| {
                        server.container_name_suffix.trim().is_empty()
                            || container.name.ends_with(&server.container_name_suffix)
                    })
            }
        })
        .collect()
}

fn load_cached_server_inventory(server: &RemoteContainerServer) -> crate::machine::DiscoveredServerInventory {
    if let Some(discovered) = read_machine_inventory_snapshot(&server.id)
        .ok()
        .flatten()
    {
        return inventory_from_discovered_containers(
            server,
            effective_discovered_containers(server, &discovered),
            false,
        );
    }

    cached_remote_server_inventory(server)
}

fn containers_from_scan(
    existing: &[MachineContainerConfig],
    discovered: &[DiscoveredContainer],
) -> Vec<MachineContainerConfig> {
    discovered
        .iter()
        .filter(|container| !matches!(container.kind, ManagedContainerKind::Host))
        .map(|container| {
            let existing_entry = existing
                .iter()
                .find(|entry| entry.name == container.name);

            MachineContainerConfig {
                id: if !container.id.trim().is_empty() {
                    container.id.clone()
                } else {
                    existing_entry
                        .map(|entry| entry.id.clone())
                        .unwrap_or_else(|| container.name.clone())
                },
                name: container.name.clone(),
                kind: container.kind,
                enabled: existing_entry
                    .map(|entry| entry.enabled)
                    .unwrap_or(false),
                source: container.source.clone(),
                ssh_user: existing_entry
                    .filter(|entry| !entry.ssh_user.trim().is_empty())
                    .map(|entry| entry.ssh_user.clone())
                    .unwrap_or_else(|| container.ssh_user.clone()),
                ssh_user_candidates: container.ssh_user_candidates.clone(),
                label: existing_entry
                    .filter(|entry| !entry.label.trim().is_empty())
                    .map(|entry| entry.label.clone())
                    .unwrap_or_else(|| container.label.clone().unwrap_or_default()),
                description: existing_entry
                    .filter(|entry| !entry.description.trim().is_empty())
                    .map(|entry| entry.description.clone())
                    .unwrap_or_else(|| container.description.clone().unwrap_or_default()),
                ipv4: container.ipv4.clone(),
                status: container.status.clone(),
                workspace: existing_entry
                    .map(|entry| entry.workspace.clone())
                    .unwrap_or_else(|| container.workspace.clone().unwrap_or_default()),
                services: existing_entry
                    .filter(|entry| !entry.services.is_empty())
                    .map(|entry| entry.services.clone())
                    .unwrap_or_else(|| container.services.clone()),
                surfaces: existing_entry
                    .filter(|entry| !entry.surfaces.is_empty())
                    .map(|entry| entry.surfaces.clone())
                    .unwrap_or_else(|| container.surfaces.clone()),
            }
        })
        .collect()
}

pub(crate) fn scan_and_store_server_inventory(server_id: &str) -> Result<ManagedServer, String> {
    let mut raw_targets_file = read_raw_targets_file()?;
    let profiles: ContainerProfiles = raw_targets_file.profiles.clone();
    let default_profile = raw_targets_file.default_profile.clone();
    raw_targets_file.machines.iter_mut().for_each(|server| {
        apply_profile_defaults_to_remote_server(server, &profiles, default_profile.as_deref())
    });

    let Some(server_index) = raw_targets_file
        .machines
        .iter()
        .position(|server| server.id == server_id)
    else {
        return Err(format!("Unknown server: {}", server_id));
    };

    let server = &raw_targets_file.machines[server_index];
    let discovered = scan_server_containers(server)?;
    save_machine_inventory_snapshot(server_id, &discovered)?;

    let updated_containers = containers_from_scan(&server.containers, &discovered);
    let server = raw_targets_file.machines[server_index].clone();

    // Write discovered containers back to machine config
    let mut persist_targets = read_raw_targets_file()?;
    if let Some(persist_index) = persist_targets
        .machines
        .iter()
        .position(|m| m.id == server_id)
    {
        persist_targets.machines[persist_index].containers = updated_containers;
        let _ = save_raw_targets_file(&persist_targets);
    }

    let inventory = inventory_from_discovered_containers(
        &server,
        effective_discovered_containers(&server, &discovered),
        false,
    );

    Ok(inventory.server)
}

pub(crate) fn read_server_inventory(force_refresh: bool) -> Result<Vec<ManagedServer>, String> {
    load_inventory(force_refresh).map(|inventory| inventory.servers)
}

pub(crate) fn read_targets_file() -> Result<TargetsFile, String> {
    load_inventory(false).map(|inventory| inventory.targets_file)
}

pub(crate) fn resolve_raw_target(target_id: &str) -> Result<DeveloperTarget, String> {
    let targets_file = read_targets_file()?;

    if let Some(target) = targets_file
        .targets
        .into_iter()
        .find(|target| target.id == target_id)
    {
        return Ok(target);
    }
    Err(format!("Unknown target: {}", target_id))
}

pub(crate) fn read_bootstrap_data(
    force_refresh: bool,
) -> Result<(TargetsFile, Vec<ManagedServer>), String> {
    let inventory = load_inventory(force_refresh)?;
    Ok((inventory.targets_file, inventory.servers))
}
