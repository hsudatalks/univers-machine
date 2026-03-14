use super::{
    inventory::{read_bootstrap_data, read_server_inventory, scan_and_store_server_inventory},
    targets_file_path,
};
use crate::{
    models::{AppBootstrap, ConnectivityState, ManagedServer, TunnelState},
    runtime::connectivity::apply_connectivity_snapshots,
    services::runtime::read_runtime_targets_file,
};

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
