use super::{
    RawTargetsFile, RemoteContainerServer,
    inventory::load_inventory,
    repository::read_raw_targets_file,
    resolve_raw_target,
    ssh::{container_host_key_alias, machine_host_key_alias, resolved_known_hosts_path},
};
use crate::models::{MachineTransport, ManagedContainerKind};
use std::path::PathBuf;
use univers_ark_russh::{ResolvedEndpoint, ResolvedEndpointChain};

fn identity_paths(paths: &[String]) -> Vec<PathBuf> {
    paths.iter().map(PathBuf::from).collect()
}

fn resolved_machine_chain(server: &RemoteContainerServer) -> Result<ResolvedEndpointChain, String> {
    if matches!(server.transport, MachineTransport::Local) {
        return Err(format!("Machine {} uses local transport", server.id));
    }

    let known_hosts_path = resolved_known_hosts_path(server);
    let mut hops = server
        .jump_chain
        .iter()
        .enumerate()
        .map(|(index, jump)| {
            ResolvedEndpoint::new(
                format!("{}::jump-{}", server.id, index + 1),
                jump.host.clone(),
                jump.user.clone(),
                jump.port,
                identity_paths(&jump.identity_files),
            )
            .with_known_hosts(
                known_hosts_path.clone(),
                jump.host.clone(),
                server.strict_host_key_checking,
            )
        })
        .collect::<Vec<_>>();
    hops.push(
        ResolvedEndpoint::new(
            server.id.clone(),
            server.host.clone(),
            server.ssh_user.clone(),
            server.port,
            identity_paths(&server.identity_files),
        )
        .with_known_hosts(
            known_hosts_path,
            machine_host_key_alias(server),
            server.strict_host_key_checking,
        ),
    );

    Ok(ResolvedEndpointChain::from_hops(hops))
}

pub(crate) fn resolve_target_ssh_chain(target_id: &str) -> Result<ResolvedEndpointChain, String> {
    let target = resolve_raw_target(target_id)?;
    if matches!(target.transport, MachineTransport::Local) {
        return Err(format!("Target {} uses local transport", target_id));
    }

    let raw_targets_file: RawTargetsFile = read_raw_targets_file()?;
    let server = raw_targets_file
        .machines
        .iter()
        .find(|server| server.id == target.machine_id)
        .ok_or_else(|| format!("Unknown machine for {}", target_id))?;

    if matches!(target.container_kind, ManagedContainerKind::Host) {
        return resolved_machine_chain(server);
    }

    let inventory = load_inventory(false)?;

    if let Some(container) = inventory
        .servers
        .iter()
        .flat_map(|server| server.containers.iter())
        .find(|container| container.target_id == target_id)
    {
        let mut chain = resolved_machine_chain(server)?;
        chain.push(
            ResolvedEndpoint::new(
                format!("{}::{}", server.id, container.name),
                container.ipv4.clone(),
                container.ssh_user.clone(),
                22,
                Vec::new(),
            )
            .with_known_hosts(
                resolved_known_hosts_path(server),
                container_host_key_alias(server, &container.name),
                server.strict_host_key_checking,
            ),
        );

        return Ok(chain);
    }

    Err(format!("Unknown machine inventory for {}", target_id))
}
