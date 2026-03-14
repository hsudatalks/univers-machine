use super::{
    inventory::{read_bootstrap_data, read_server_inventory, scan_and_store_server_inventory},
    repository::{read_raw_targets_file, save_raw_targets_file},
    targets_file_path,
    RawTargetsFile, RemoteContainerServer,
};
use crate::{
    models::{AppBootstrap, ConnectivityState, ManagedServer, TunnelState},
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
    let next_machine_id = normalize_machine_id(&machine.id);

    if next_machine_id.is_empty() {
        return Err(String::from("Provider ID is required."));
    }

    if raw_targets_file.machines.iter().any(|entry| {
        entry.id == next_machine_id
            && Some(entry.id.as_str()) != previous_machine_id
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

    if let Some(previous_machine_id) = previous_machine_id.filter(|machine_id| *machine_id != next_machine_id) {
        if raw_targets_file
            .selected_target_id
            .as_deref()
            .is_some_and(|target_id| target_id.starts_with(&format!("{previous_machine_id}::")))
        {
            raw_targets_file.selected_target_id = raw_targets_file.selected_target_id.map(|target_id| {
                target_id.replacen(
                    &format!("{previous_machine_id}::"),
                    &format!("{next_machine_id}::"),
                    1,
                )
            });
        }
    }

    save_raw_targets_file(&raw_targets_file)?;
    config_document_value(raw_targets_file)
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

    save_raw_targets_file(&raw_targets_file)?;
    config_document_value(raw_targets_file)
}
