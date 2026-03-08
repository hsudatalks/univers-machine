use crate::shell;
use std::{fs, path::PathBuf};

use super::{RemoteContainerContext, RemoteContainerServer};

pub(super) fn managed_known_hosts_file() -> String {
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE").ok()
    } else {
        std::env::var("HOME").ok()
    };

    match home {
        Some(home) if !home.trim().is_empty() => {
            let normalized = if cfg!(windows) {
                home.replace('\\', "/")
            } else {
                home
            };
            format!("{}/.ssh/univers-ark-developer-known_hosts", normalized)
        }
        _ => String::from("~/.ssh/univers-ark-developer-known_hosts"),
    }
}

pub(super) fn container_host_key_alias(
    server: &RemoteContainerServer,
    container_name: &str,
) -> String {
    format!("univers-ark-developer--{}--{}", server.id, container_name)
}

pub(super) fn ssh_options_for_context(
    server: &RemoteContainerServer,
    container_name: &str,
) -> String {
    let base_options = server.ssh_options.trim();
    let host_key_alias = container_host_key_alias(server, container_name);
    let known_hosts_file = managed_known_hosts_file();
    let managed_known_hosts_option = format!("-o UserKnownHostsFile={}", known_hosts_file);

    if base_options.is_empty() {
        format!(
            "{} -o HostKeyAlias={}",
            managed_known_hosts_option, host_key_alias
        )
    } else {
        format!(
            "{} {} -o HostKeyAlias={}",
            base_options, managed_known_hosts_option, host_key_alias
        )
    }
}

pub(super) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn ssh_destination(server: &RemoteContainerServer, container_ip: &str) -> String {
    format!("{}@{}", server.ssh_user, container_ip)
}

fn default_container_terminal_remote_command() -> String {
    String::from(
        "tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l",
    )
}

pub(super) fn build_ssh_command(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
    extra_options: &[&str],
    remote_command: Option<&str>,
) -> String {
    let ssh_options = ssh_options_for_context(server, container_name);
    let mut command = String::from("ssh");

    if !ssh_options.is_empty() {
        command.push(' ');
        command.push_str(&ssh_options);
    }

    for extra_option in extra_options {
        command.push(' ');
        command.push_str(extra_option);
    }

    command.push_str(" -J ");
    command.push_str(&server.host);
    command.push(' ');
    command.push_str(&ssh_destination(server, container_ip));

    if let Some(remote_command) = remote_command {
        command.push(' ');
        command.push_str(remote_command);
    }

    command
}

pub(super) fn terminal_command_for_server(
    server: &RemoteContainerServer,
    context: &RemoteContainerContext<'_>,
) -> String {
    super::discovery::render_template(&server.terminal_command_template, context, || {
        let remote_command = default_container_terminal_remote_command();

        build_ssh_command(
            server,
            context.container_ip,
            context.container_name,
            &["-tt"],
            Some(&shell_single_quote(&remote_command)),
        )
    })
}

fn ssh_probe_command_for_server(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
) -> String {
    build_ssh_command(
        server,
        container_ip,
        container_name,
        &[
            "-o BatchMode=yes",
            "-o ConnectTimeout=4",
            "-o ConnectionAttempts=1",
        ],
        Some("true"),
    )
}

pub(super) fn probe_container_ssh(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
) -> (bool, String, String) {
    let command = ssh_probe_command_for_server(server, container_ip, container_name);
    let output = shell::shell_command(&command).output();

    match output {
        Ok(output) if output.status.success() => (
            true,
            String::from("ready"),
            format!("SSH ready via {}.", server.host),
        ),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("Probe command exited with {}", output.status)
            };

            (false, String::from("error"), detail)
        }
        Err(error) => (
            false,
            String::from("error"),
            format!("Failed to run SSH probe: {}", error),
        ),
    }
}

fn local_public_key() -> Option<String> {
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE").ok()
    } else {
        std::env::var("HOME").ok()
    };

    let home = home?;
    let ssh_dir = PathBuf::from(&home).join(".ssh");

    for key_name in &["id_ed25519.pub", "id_rsa.pub"] {
        let path = ssh_dir.join(key_name);
        if let Ok(content) = fs::read_to_string(&path) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }

    None
}

fn deploy_key_command(
    server: &RemoteContainerServer,
    container_name: &str,
    public_key: &str,
) -> String {
    let user_home = format!("/home/{}", server.ssh_user);
    let inject_script = format!(
        "mkdir -p {home}/.ssh && chmod 700 {home}/.ssh && echo {key} >> {home}/.ssh/authorized_keys && chmod 600 {home}/.ssh/authorized_keys",
        home = user_home,
        key = shell_single_quote(public_key),
    );

    let is_orbstack = server
        .discovery_command
        .to_ascii_lowercase()
        .contains("orb");

    if is_orbstack {
        format!(
            "ssh {} \"orb run -m {} -u {} bash -c {}\"",
            server.host,
            container_name,
            server.ssh_user,
            shell_single_quote(&inject_script),
        )
    } else {
        format!(
            "ssh {} \"lxc exec {} -- bash -c {}\"",
            server.host,
            container_name,
            shell_single_quote(&inject_script),
        )
    }
}

fn auto_deploy_public_key(
    server: &RemoteContainerServer,
    container_name: &str,
) -> Option<String> {
    let public_key = local_public_key()?;
    let command = deploy_key_command(server, container_name, &public_key);
    let output = shell::shell_command(&command).output().ok()?;

    if output.status.success() {
        Some(format!(
            "Automatically deployed local SSH public key to {} via {}.",
            container_name, server.host
        ))
    } else {
        None
    }
}

pub(super) fn probe_managed_container_ssh(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
) -> (bool, String, String) {
    let (mut ssh_reachable, mut ssh_state, mut ssh_message) =
        probe_container_ssh(server, container_ip, container_name);

    if !ssh_reachable && ssh_message.contains("Permission denied") {
        if let Some(deploy_message) = auto_deploy_public_key(server, container_name) {
            let (retry_reachable, retry_state, retry_message) =
                probe_container_ssh(server, container_ip, container_name);

            if retry_reachable {
                ssh_reachable = true;
                ssh_state = retry_state;
                ssh_message = format!("{} {}", retry_message, deploy_message);
            } else {
                ssh_message = format!("Key deployed but SSH still failed: {}", retry_message);
            }
        }
    }

    (ssh_reachable, ssh_state, ssh_message)
}

pub(super) fn run_target_shell_command_internal(
    target_id: &str,
    command: &str,
) -> Result<std::process::Output, String> {
    shell::shell_command(command)
        .output()
        .map_err(|error| format!("Failed to execute shell command for {}: {}", target_id, error))
}
