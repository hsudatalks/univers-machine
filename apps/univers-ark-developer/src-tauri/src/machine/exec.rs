use super::{
    chain::resolve_target_ssh_chain,
    inventory::load_inventory,
    repository::read_raw_targets_file,
    resolve_raw_target,
    ssh::{
        build_host_ssh_command, build_ssh_command, run_target_shell_command_internal,
        shell_single_quote,
    },
    RawTargetsFile,
};
use crate::{
    infra::russh::execute_chain_blocking,
    models::{DeveloperTarget, ManagedContainerKind},
};
use std::process::Output;
use univers_ark_russh::{ClientOptions as RusshClientOptions, ExecOutput as RusshExecOutput};

pub(crate) fn execute_target_command_via_russh(
    target_id: &str,
    command: &str,
) -> Result<RusshExecOutput, String> {
    let chain = resolve_target_ssh_chain(target_id)?;
    execute_chain_blocking(&chain, command, &RusshClientOptions::default())
        .map_err(|error| format!("russh exec failed for {target_id}: {error}"))
}

pub(crate) fn run_target_shell_command(
    target_id: &str,
    remote_command: &str,
) -> Result<Output, String> {
    let target: DeveloperTarget = resolve_raw_target(target_id)?;

    let raw_targets_file: RawTargetsFile = read_raw_targets_file()?;
    let server = raw_targets_file
        .machines
        .iter()
        .find(|server| server.id == target.machine_id)
        .ok_or_else(|| format!("Unknown machine for {target_id}"))?;

    if matches!(target.container_kind, ManagedContainerKind::Host) {
        let quoted_remote_command = shell_single_quote(remote_command);
        let ssh_command = build_host_ssh_command(server, &[], Some(&quoted_remote_command));
        return run_target_shell_command_internal(target_id, &ssh_command);
    }

    let inventory = load_inventory(false)?;

    if let Some(container) = inventory
        .servers
        .iter()
        .flat_map(|server| server.containers.iter())
        .find(|container| container.target_id == target_id)
    {
        let quoted_remote_command = shell_single_quote(remote_command);
        let ssh_command = build_ssh_command(
            server,
            &container.ipv4,
            &container.name,
            &container.ssh_user,
            &[],
            Some(&quoted_remote_command),
        );

        return run_target_shell_command_internal(target_id, &ssh_command);
    }

    run_target_shell_command_internal(target_id, remote_command)
}
