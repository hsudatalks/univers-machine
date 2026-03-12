mod build;
mod probe;

use crate::infra::shell;

pub(crate) use self::probe::maybe_auto_deploy_target_public_key;
pub(super) use self::build::{
    build_host_ssh_command, build_ssh_command, container_host_key_alias,
    default_terminal_startup_command, expand_home_path, host_terminal_startup_command,
    machine_host_key_alias, managed_container_ssh_user, profile_terminal_startup_command,
    resolved_known_hosts_path, shell_single_quote, ssh_destination, ssh_options_for_context,
    terminal_command_for_server,
};

pub(super) fn run_target_shell_command_internal(
    target_id: &str,
    command: &str,
) -> Result<std::process::Output, String> {
    shell::shell_command(command).output().map_err(|error| {
        format!(
            "Failed to execute shell command for {}: {}",
            target_id, error
        )
    })
}
