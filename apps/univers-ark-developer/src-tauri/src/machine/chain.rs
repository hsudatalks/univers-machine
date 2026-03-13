use super::{
    inventory::load_inventory,
    repository::read_raw_targets_file,
    resolve_raw_target,
    ssh::{
        container_host_key_alias, expand_home_path, machine_host_key_alias,
        resolved_known_hosts_path,
    },
    RawTargetsFile, RemoteContainerServer,
};
use crate::models::{MachineTransport, ManagedContainerKind};
use crate::secrets::load_secret_credential_value;
use std::path::PathBuf;
use univers_ark_russh::{ResolvedEndpoint, ResolvedEndpointChain};

fn identity_paths(paths: &[String]) -> Vec<PathBuf> {
    paths.iter()
        .map(|path| PathBuf::from(expand_home_path(path)))
        .collect()
}

fn apply_identity_sources(
    mut endpoint: ResolvedEndpoint,
    alias: &str,
    identity_files: &[String],
    ssh_credential_id: &str,
) -> Result<ResolvedEndpoint, String> {
    endpoint
        .identity_files
        .extend(identity_paths(identity_files));

    if !ssh_credential_id.trim().is_empty() {
        endpoint = endpoint.with_inline_identity(
            format!("{alias}::credential"),
            load_secret_credential_value(ssh_credential_id)?,
        );
    }

    Ok(endpoint)
}

fn accept_new_host_keys(server: &RemoteContainerServer) -> bool {
    server.strict_host_key_checking
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
            let alias = format!("{}::jump-{}", server.id, index + 1);
            apply_identity_sources(
                ResolvedEndpoint::new(
                    alias.clone(),
                    jump.host.clone(),
                    jump.user.clone(),
                    jump.port,
                    Vec::new(),
                ),
                &alias,
                &jump.identity_files,
                &jump.ssh_credential_id,
            )
            .map(|endpoint| {
                endpoint.with_known_hosts(
                    known_hosts_path.clone(),
                    jump.host.clone(),
                    accept_new_host_keys(server),
                )
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    hops.push(
        apply_identity_sources(
            ResolvedEndpoint::new(
                server.id.clone(),
                server.host.clone(),
                server.ssh_user.clone(),
                server.port,
                Vec::new(),
            ),
            &server.id,
            &server.identity_files,
            &server.ssh_credential_id,
        )?
        .with_known_hosts(
            known_hosts_path,
            machine_host_key_alias(server),
            accept_new_host_keys(server),
        ),
    );

    Ok(ResolvedEndpointChain::from_hops(hops))
}

pub(crate) fn resolve_target_ssh_chain(target_id: &str) -> Result<ResolvedEndpointChain, String> {
    let target = resolve_raw_target(target_id)?;
    if matches!(target.transport, MachineTransport::Local) {
        return Err(format!("Target {target_id} uses local transport"));
    }

    let raw_targets_file: RawTargetsFile = read_raw_targets_file()?;
    let server = raw_targets_file
        .machines
        .iter()
        .find(|server| server.id == target.machine_id)
        .ok_or_else(|| format!("Unknown machine for {target_id}"))?;

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
                accept_new_host_keys(server),
            ),
        );

        return Ok(chain);
    }

    Err(format!("Unknown machine inventory for {target_id}"))
}

#[cfg(test)]
mod tests {
    use super::{accept_new_host_keys, identity_paths};
    use crate::machine::{ContainerDiscoveryMode, ContainerManagerType, RemoteContainerServer};
    use crate::models::{ContainerWorkspace, MachineTransport};
    use std::path::PathBuf;

    fn test_server(strict_host_key_checking: bool) -> RemoteContainerServer {
        RemoteContainerServer {
            id: String::from("infra-dev"),
            label: String::from("Infra Dev"),
            transport: MachineTransport::Ssh,
            host: String::from("legion-ubuntu.mink-ph.ts.net"),
            port: 22,
            description: String::new(),
            manager_type: ContainerManagerType::None,
            discovery_mode: ContainerDiscoveryMode::HostOnly,
            discovery_command: String::new(),
            ssh_user: String::from("david"),
            container_ssh_user: String::from("david"),
            identity_files: vec![],
            ssh_credential_id: String::new(),
            jump_chain: vec![],
            known_hosts_path: String::from("~/.univers/known_hosts"),
            strict_host_key_checking,
            container_name_suffix: String::new(),
            include_stopped: false,
            target_label_template: String::new(),
            target_host_template: String::from("{machineHost}"),
            target_description_template: String::new(),
            host_terminal_startup_command: String::new(),
            terminal_command_template: String::new(),
            notes: vec![],
            workspace: ContainerWorkspace::default(),
            services: vec![],
            surfaces: vec![],
            containers: vec![],
        }
    }

    #[test]
    fn relaxed_host_key_checking_accepts_new_keys() {
        assert!(!accept_new_host_keys(&test_server(false)));
        assert!(accept_new_host_keys(&test_server(true)));
    }

    #[test]
    fn identity_paths_expand_home_prefix() {
        let home = std::env::var("HOME").expect("HOME must be set for tests");
        let paths = identity_paths(&[String::from("~/.ssh/id_ed25519")]);
        assert_eq!(paths, vec![PathBuf::from(format!("{home}/.ssh/id_ed25519"))]);
    }
}
