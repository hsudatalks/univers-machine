use super::{
    discovery::{
        cached_remote_server_inventory, discover_remote_server_inventory,
        inventory_from_scanned_containers, scan_server_containers,
    },
    fs_store::{read_targets_file_content, sanitize_targets_json_content, targets_file_path},
    profiles::{apply_profile_defaults_to_remote_server, ContainerProfiles},
    repository::{read_raw_targets_file, save_targets_config},
    CachedResolvedInventory, DiscoveredContainer, MachineContainerConfig, RawTargetsFile,
    RemoteContainerServer, ResolvedInventory,
};
use crate::models::{DeveloperTarget, ManagedContainerKind, ManagedServer, TargetsFile};
use serde_json::{json, Value};
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
                        discover_remote_server_inventory(server)
                    } else {
                        cached_remote_server_inventory(server)
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

pub(super) fn discovered_container_to_manual_value(
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
    existing: Option<&MachineContainerConfig>,
) -> Value {
    let id = if container.id.trim().is_empty() {
        existing
            .map(|item| item.id.clone())
            .unwrap_or_else(|| container.name.clone())
    } else {
        container.id.clone()
    };
    let label = container
        .label
        .as_ref()
        .cloned()
        .or_else(|| existing.map(|item| item.label.clone()))
        .unwrap_or_default();
    let description = container
        .description
        .as_ref()
        .cloned()
        .or_else(|| existing.map(|item| item.description.clone()))
        .unwrap_or_default();
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
    let enabled = if matches!(container.kind, ManagedContainerKind::Host) {
        true
    } else if let Some(existing) = existing {
        existing.enabled
    } else if server.container_name_suffix.trim().is_empty() {
        true
    } else {
        container.name.ends_with(&server.container_name_suffix)
    };
    let workspace = existing
        .map(|item| serde_json::to_value(&item.workspace).unwrap_or_else(|_| json!({})))
        .unwrap_or_else(|| json!({}));
    let services = existing
        .map(|item| serde_json::to_value(&item.services).unwrap_or_else(|_| json!([])))
        .unwrap_or_else(|| json!([]));
    let surfaces = existing
        .map(|item| serde_json::to_value(&item.surfaces).unwrap_or_else(|_| json!([])))
        .unwrap_or_else(|| json!([]));
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

    json!({
        "id": id,
        "name": container.name,
        "kind": container.kind,
        "enabled": enabled,
        "source": source,
        "sshUser": ssh_user,
        "sshUserCandidates": ssh_user_candidates,
        "label": label,
        "description": description,
        "ipv4": container.ipv4,
        "status": container.status,
        "workspace": workspace,
        "services": services,
        "surfaces": surfaces
    })
}

pub(crate) fn scan_and_store_server_inventory(server_id: &str) -> Result<ManagedServer, String> {
    let config_path = targets_file_path();
    let raw_content = read_targets_file_content()?;
    let sanitized_content = sanitize_targets_json_content(&raw_content)?;
    let mut raw_json: Value = serde_json::from_str(&sanitized_content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))?;
    let mut raw_targets_file: RawTargetsFile = serde_json::from_str(&sanitized_content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))?;

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
        return Err(format!("Unknown server: {server_id}"));
    };

    let server = raw_targets_file.machines[server_index].clone();
    let discovered = scan_server_containers(&server)?;
    let inventory = inventory_from_scanned_containers(&server, discovered.clone());
    let existing_manual = raw_targets_file.machines[server_index].containers.clone();
    let manual_values = discovered
        .iter()
        .map(|container| {
            let existing = existing_manual
                .iter()
                .find(|item| item.name == container.name);
            discovered_container_to_manual_value(&server, container, existing)
        })
        .collect::<Vec<_>>();

    let Some(remote_servers) = raw_json.get_mut("machines").and_then(Value::as_array_mut) else {
        return Err(String::from("Config is missing machines."));
    };

    let Some(server_json) = remote_servers
        .iter_mut()
        .find(|server_json| server_json.get("id").and_then(Value::as_str) == Some(server_id))
    else {
        return Err(format!("Unknown server: {server_id}"));
    };

    server_json["containers"] = Value::Array(manual_values);
    let next_content = serde_json::to_string_pretty(&raw_json)
        .map_err(|error| format!("Failed to serialize updated config: {error}"))?;
    save_targets_config(&next_content)?;

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
    Err(format!("Unknown target: {target_id}"))
}

pub(crate) fn read_bootstrap_data(
    force_refresh: bool,
) -> Result<(TargetsFile, Vec<ManagedServer>), String> {
    let inventory = load_inventory(force_refresh)?;
    Ok((inventory.targets_file, inventory.servers))
}
