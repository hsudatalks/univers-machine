use super::{
    inventory::{read_bootstrap_data, read_server_inventory, scan_and_store_server_inventory},
    profiles::ContainerProfileConfig,
    repository::{read_raw_targets_file, save_raw_targets_file},
    targets_file_path,
    RawTargetsFile, RemoteContainerServer,
};
use crate::{
    models::{AppBootstrap, ConnectivityState, ContainerWorkspace, ManagedServer, TunnelState},
    runtime::connectivity::apply_connectivity_snapshots,
    services::runtime::read_runtime_targets_file,
};
use serde_json::Value;

pub(crate) fn load_bootstrap_view(
    tunnel_state: &TunnelState,
    connectivity_state: &ConnectivityState,
    force_refresh: bool,
) -> Result<AppBootstrap, String> {
    let (targets_file, mut servers) = read_bootstrap_data(force_refresh)?;
    apply_connectivity_snapshots(&mut servers, connectivity_state);
    let hydrated_targets_file = read_runtime_targets_file(tunnel_state)?;
    let config_path = targets_file_path();

    Ok(AppBootstrap {
        app_name: "Ark Console".into(),
        config_path: config_path.display().to_string(),
        selected_target_id: targets_file.selected_target_id,
        targets: hydrated_targets_file.targets,
        machines: servers,
    })
}

pub(crate) fn load_machine_inventory_view(
    connectivity_state: &ConnectivityState,
    force_refresh: bool,
) -> Result<Vec<ManagedServer>, String> {
    let mut servers = read_server_inventory(force_refresh)?;
    apply_connectivity_snapshots(&mut servers, connectivity_state);
    Ok(servers)
}

pub(crate) fn scan_machine_inventory_view(
    machine_id: &str,
    connectivity_state: &ConnectivityState,
) -> Result<ManagedServer, String> {
    let mut server = scan_and_store_server_inventory(machine_id)?;
    let mut servers = vec![server.clone()];
    apply_connectivity_snapshots(&mut servers, connectivity_state);
    server = servers.into_iter().next().unwrap_or(server);
    Ok(server)
}

fn config_document_value(raw_targets_file: RawTargetsFile) -> Result<Value, String> {
    serde_json::to_value(raw_targets_file)
        .map_err(|error| format!("Failed to serialize machine config document: {}", error))
}

fn normalize_machine_id(value: &str) -> String {
    value.trim().to_string()
}

fn normalize_profile_id(value: &str) -> String {
    value.trim().to_string()
}

fn persist_config_document(raw_targets_file: RawTargetsFile) -> Result<Value, String> {
    save_raw_targets_file(&raw_targets_file)?;
    config_document_value(raw_targets_file)
}

fn rewrite_selected_target_prefix(
    selected_target_id: &mut Option<String>,
    previous_machine_id: &str,
    next_machine_id: &str,
) {
    if previous_machine_id == next_machine_id {
        return;
    }

    if selected_target_id
        .as_deref()
        .is_some_and(|target_id| target_id.starts_with(&format!("{previous_machine_id}::")))
    {
        *selected_target_id = selected_target_id.take().map(|target_id| {
            target_id.replacen(
                &format!("{previous_machine_id}::"),
                &format!("{next_machine_id}::"),
                1,
            )
        });
    }
}

fn upsert_machine_into_targets(
    raw_targets_file: &mut RawTargetsFile,
    previous_machine_id: Option<&str>,
    machine: RemoteContainerServer,
) -> Result<(), String> {
    let next_machine_id = normalize_machine_id(&machine.id);

    if next_machine_id.is_empty() {
        return Err(String::from("Provider ID is required."));
    }

    if raw_targets_file.machines.iter().any(|entry| {
        entry.id == next_machine_id && Some(entry.id.as_str()) != previous_machine_id
    }) {
        return Err(format!("Provider \"{}\" already exists.", next_machine_id));
    }

    let mut next_machine = machine;
    next_machine.id = next_machine_id.clone();
    let existing_index = previous_machine_id.and_then(|machine_id| {
        raw_targets_file
            .machines
            .iter()
            .position(|entry| entry.id == machine_id)
    });

    if let Some(existing_index) = existing_index {
        raw_targets_file.machines[existing_index] = next_machine;
    } else {
        raw_targets_file.machines.push(next_machine);
    }

    if let Some(previous_machine_id) =
        previous_machine_id.filter(|machine_id| *machine_id != next_machine_id)
    {
        rewrite_selected_target_prefix(
            &mut raw_targets_file.selected_target_id,
            previous_machine_id,
            &next_machine_id,
        );
    }

    Ok(())
}

pub(crate) fn load_machine_config_document_view() -> Result<Value, String> {
    config_document_value(read_raw_targets_file()?)
}

pub(crate) fn upsert_machine_config_view(
    previous_machine_id: Option<&str>,
    machine_value: Value,
) -> Result<Value, String> {
    let machine: RemoteContainerServer = serde_json::from_value(machine_value)
        .map_err(|error| format!("Invalid machine config payload: {}", error))?;
    let mut raw_targets_file = read_raw_targets_file()?;
    upsert_machine_into_targets(&mut raw_targets_file, previous_machine_id, machine)?;
    persist_config_document(raw_targets_file)
}

pub(crate) fn import_machine_configs_view(machine_values: Vec<Value>) -> Result<Value, String> {
    let mut raw_targets_file = read_raw_targets_file()?;

    for machine_value in machine_values {
        let machine: RemoteContainerServer = serde_json::from_value(machine_value)
            .map_err(|error| format!("Invalid machine config payload: {}", error))?;
        upsert_machine_into_targets(&mut raw_targets_file, None, machine)?;
    }

    persist_config_document(raw_targets_file)
}

pub(crate) fn delete_machine_config_view(machine_id: &str) -> Result<Value, String> {
    let machine_id = normalize_machine_id(machine_id);
    if machine_id.is_empty() {
        return Err(String::from("Provider ID is required."));
    }
    if machine_id == "local" {
        return Err(String::from("The local provider cannot be deleted."));
    }

    let mut raw_targets_file = read_raw_targets_file()?;
    let original_len = raw_targets_file.machines.len();
    raw_targets_file
        .machines
        .retain(|machine| machine.id != machine_id);

    if raw_targets_file.machines.len() == original_len {
        return Err(format!("Unknown provider: {}", machine_id));
    }

    if raw_targets_file
        .selected_target_id
        .as_deref()
        .is_some_and(|target_id| target_id.starts_with(&format!("{machine_id}::")))
    {
        raw_targets_file.selected_target_id = None;
    }

    persist_config_document(raw_targets_file)
}

pub(crate) fn upsert_profile_config_view(
    profile_id: &str,
    previous_profile_id: Option<&str>,
    profile_value: Value,
) -> Result<Value, String> {
    let profile_id = normalize_profile_id(profile_id);
    if profile_id.is_empty() {
        return Err(String::from("Profile ID is required."));
    }

    let profile: ContainerProfileConfig = serde_json::from_value(profile_value)
        .map_err(|error| format!("Invalid profile config payload: {}", error))?;
    let mut raw_targets_file = read_raw_targets_file()?;

    if raw_targets_file.profiles.contains_key(&profile_id)
        && previous_profile_id != Some(profile_id.as_str())
    {
        return Err(format!("Profile \"{}\" already exists.", profile_id));
    }

    if let Some(previous_profile_id) = previous_profile_id.filter(|value| *value != profile_id) {
        raw_targets_file.profiles.remove(previous_profile_id);
        if raw_targets_file.default_profile.as_deref() == Some(previous_profile_id) {
            raw_targets_file.default_profile = Some(profile_id.clone());
        }
    }

    raw_targets_file.profiles.insert(
        profile_id.clone(),
        ContainerProfileConfig {
            workspace: ContainerWorkspace {
                profile: profile_id,
                ..profile.workspace
            },
            ..profile
        },
    );

    persist_config_document(raw_targets_file)
}

pub(crate) fn update_default_profile_view(profile_id: Option<&str>) -> Result<Value, String> {
    let mut raw_targets_file = read_raw_targets_file()?;
    let next_profile_id = profile_id
        .map(normalize_profile_id)
        .filter(|value| !value.is_empty());

    if let Some(profile_id) = next_profile_id.as_ref() {
        if !raw_targets_file.profiles.contains_key(profile_id) {
            return Err(format!("Unknown profile: {}", profile_id));
        }
    }

    raw_targets_file.default_profile = next_profile_id;
    persist_config_document(raw_targets_file)
}

pub(crate) fn move_machine_config_view(machine_id: &str, direction: i32) -> Result<Value, String> {
    let machine_id = normalize_machine_id(machine_id);
    if machine_id.is_empty() {
        return Err(String::from("Provider ID is required."));
    }
    if direction == 0 {
        return load_machine_config_document_view();
    }

    let mut raw_targets_file = read_raw_targets_file()?;
    let current_index = raw_targets_file
        .machines
        .iter()
        .position(|machine| machine.id == machine_id)
        .ok_or_else(|| format!("Unknown provider: {}", machine_id))?;
    let next_index = current_index as i32 + direction;

    if next_index < 0 || next_index >= raw_targets_file.machines.len() as i32 {
        return load_machine_config_document_view();
    }

    let moved_machine = raw_targets_file.machines.remove(current_index);
    raw_targets_file
        .machines
        .insert(next_index as usize, moved_machine);

    persist_config_document(raw_targets_file)
}
