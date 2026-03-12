use crate::shell;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::Stdio,
};

use super::{ContainerManagerType, RemoteContainerContext, RemoteContainerServer};

pub(super) fn managed_container_ssh_user(server: &RemoteContainerServer) -> &str {
    if server.container_ssh_user.trim().is_empty() {
        &server.ssh_user
    } else {
        &server.container_ssh_user
    }
}

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

pub(super) fn expand_home_path(path: &str) -> String {
    if let Some(stripped) = path.trim().strip_prefix("~/") {
        let home = if cfg!(windows) {
            std::env::var("USERPROFILE").ok()
        } else {
            std::env::var("HOME").ok()
        };

        if let Some(home) = home {
            return format!("{}/{}", home.replace('\\', "/"), stripped);
        }
    }

    path.trim().to_string()
}

pub(super) fn resolved_known_hosts_path(server: &RemoteContainerServer) -> PathBuf {
    if server.known_hosts_path.trim().is_empty() {
        PathBuf::from(managed_known_hosts_file())
    } else {
        PathBuf::from(expand_home_path(&server.known_hosts_path))
    }
}

pub(super) fn container_host_key_alias(
    server: &RemoteContainerServer,
    container_name: &str,
) -> String {
    format!("univers-ark-developer--{}--{}", server.id, container_name)
}

pub(super) fn machine_host_key_alias(server: &RemoteContainerServer) -> String {
    format!("univers-ark-developer--{}--host", server.id)
}

fn base_ssh_flags(server: &RemoteContainerServer, host_key_alias: &str) -> Vec<String> {
    let mut flags = Vec::new();
    let known_hosts_file = resolved_known_hosts_path(server)
        .to_string_lossy()
        .to_string();

    flags.push(format!("-o UserKnownHostsFile={}", known_hosts_file));
    flags.push(format!("-o HostKeyAlias={}", host_key_alias));
    flags.push(format!(
        "-o StrictHostKeyChecking={}",
        if server.strict_host_key_checking {
            "accept-new"
        } else {
            "no"
        }
    ));

    if !server.identity_files.is_empty() {
        flags.push(String::from("-o IdentitiesOnly=yes"));
        for identity_file in &server.identity_files {
            flags.push(format!("-i {}", expand_home_path(identity_file)));
        }
    }

    flags
}

fn proxy_jump_for_host(server: &RemoteContainerServer) -> Option<String> {
    let jumps = server
        .jump_chain
        .iter()
        .map(|jump| {
            if jump.port == 22 {
                format!("{}@{}", jump.user, jump.host)
            } else {
                format!("{}@{}:{}", jump.user, jump.host, jump.port)
            }
        })
        .collect::<Vec<_>>();

    if jumps.is_empty() {
        None
    } else {
        Some(jumps.join(","))
    }
}

fn proxy_jump_for_container(server: &RemoteContainerServer) -> String {
    let mut jumps = server
        .jump_chain
        .iter()
        .map(|jump| {
            if jump.port == 22 {
                format!("{}@{}", jump.user, jump.host)
            } else {
                format!("{}@{}:{}", jump.user, jump.host, jump.port)
            }
        })
        .collect::<Vec<_>>();
    let machine_hop = if server.port == 22 {
        format!("{}@{}", server.ssh_user, server.host)
    } else {
        format!("{}@{}:{}", server.ssh_user, server.host, server.port)
    };
    jumps.push(machine_hop);
    jumps.join(",")
}

pub(super) fn ssh_options_for_context(
    server: &RemoteContainerServer,
    container_name: &str,
) -> String {
    let host_key_alias = container_host_key_alias(server, container_name);
    base_ssh_flags(server, &host_key_alias).join(" ")
}

pub(super) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn ssh_destination(container_ip: &str, ssh_user: &str) -> String {
    format!("{}@{}", ssh_user, container_ip)
}

pub(super) fn default_terminal_startup_command() -> String {
    String::from("exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l")
}

pub(super) fn profile_terminal_startup_command(profile: &str) -> String {
    if profile.trim() == "ark-workbench" {
        return String::from(
            "tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l",
        );
    }

    default_terminal_startup_command()
}

pub(super) fn build_ssh_command(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
    ssh_user: &str,
    extra_options: &[&str],
    remote_command: Option<&str>,
) -> String {
    let ssh_options = base_ssh_flags(server, &container_host_key_alias(server, container_name));
    let mut command = String::from("ssh");

    for option in &ssh_options {
        command.push(' ');
        command.push_str(option);
    }

    for extra_option in extra_options {
        command.push(' ');
        command.push_str(extra_option);
    }

    command.push_str(" -J ");
    command.push_str(&proxy_jump_for_container(server));
    command.push(' ');
    command.push_str("-p 22 ");
    command.push_str(&ssh_destination(container_ip, ssh_user));

    if let Some(remote_command) = remote_command {
        command.push(' ');
        command.push_str(remote_command);
    }

    command
}

pub(super) fn build_host_ssh_command(
    server: &RemoteContainerServer,
    extra_options: &[&str],
    remote_command: Option<&str>,
) -> String {
    let ssh_options = base_ssh_flags(server, &machine_host_key_alias(server));
    let mut command = String::from("ssh");

    for option in &ssh_options {
        command.push(' ');
        command.push_str(option);
    }

    for extra_option in extra_options {
        command.push(' ');
        command.push_str(extra_option);
    }

    if let Some(proxy_jump) = proxy_jump_for_host(server) {
        command.push_str(" -J ");
        command.push_str(&proxy_jump);
    }

    command.push(' ');
    command.push_str(&format!(
        "-p {} {}@{}",
        server.port, server.ssh_user, server.host
    ));

    if let Some(remote_command) = remote_command {
        command.push(' ');
        command.push_str(remote_command);
    }

    command
}

pub(super) fn terminal_command_for_server(
    server: &RemoteContainerServer,
    context: &RemoteContainerContext<'_>,
    startup_command: &str,
) -> String {
    super::discovery::render_template(&server.terminal_command_template, context, || {
        build_ssh_command(
            server,
            context.container_ip,
            context.container_name,
            context.ssh_user,
            &["-tt"],
            Some(&shell_single_quote(startup_command)),
        )
    })
}

fn ssh_probe_command_for_server(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
    ssh_user: &str,
) -> String {
    build_ssh_command(
        server,
        container_ip,
        container_name,
        ssh_user,
        &[
            "-o BatchMode=yes",
            "-o ConnectTimeout=4",
            "-o ConnectionAttempts=1",
        ],
        Some("true"),
    )
}

fn ssh_probe_command_for_machine_host(server: &RemoteContainerServer) -> String {
    build_host_ssh_command(
        server,
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
    ssh_user: &str,
) -> (bool, String, String) {
    let command = ssh_probe_command_for_server(server, container_ip, container_name, ssh_user);
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

pub(super) fn probe_machine_host_ssh(server: &RemoteContainerServer) -> (bool, String, String) {
    let command = ssh_probe_command_for_machine_host(server);
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
    manager_type: ContainerManagerType,
    container_name: &str,
    ssh_user: &str,
) -> String {
    let user_home = format!("/home/{}", ssh_user);
    let inject_script = format!(
        "mkdir -p {home}/.ssh && chmod 700 {home}/.ssh && cat >> {home}/.ssh/authorized_keys && chmod 600 {home}/.ssh/authorized_keys",
        home = user_home,
    );

    let is_orbstack = matches!(manager_type, ContainerManagerType::Orbstack);

    if is_orbstack {
        format!(
            "{} \"orb run -m {} -u {} bash -c {}\"",
            build_host_ssh_command(server, &[], None),
            container_name,
            ssh_user,
            shell_single_quote(&inject_script),
        )
    } else {
        format!(
            "{} \"lxc exec {} -- bash -c {}\"",
            build_host_ssh_command(server, &[], None),
            container_name,
            shell_single_quote(&inject_script),
        )
    }
}

fn deploy_key_managers(server: &RemoteContainerServer) -> Vec<ContainerManagerType> {
    match server.manager_type {
        ContainerManagerType::Orbstack => vec![ContainerManagerType::Orbstack],
        ContainerManagerType::Lxd => vec![ContainerManagerType::Lxd],
        ContainerManagerType::Docker => vec![],
        ContainerManagerType::None => {
            vec![ContainerManagerType::Orbstack, ContainerManagerType::Lxd]
        }
    }
}

fn auto_deploy_public_key(
    server: &RemoteContainerServer,
    container_name: &str,
    ssh_user: &str,
) -> Option<String> {
    let public_key = local_public_key()?;
    for manager_type in deploy_key_managers(server) {
        let command = deploy_key_command(server, manager_type, container_name, ssh_user);
        let Ok(mut child) = shell::shell_command(&command)
            .stdin(Stdio::piped())
            .spawn()
        else {
            continue;
        };

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(format!("{}\n", public_key).as_bytes());
        }

        let Ok(output) = child.wait_with_output() else {
            continue;
        };

        if output.status.success() {
            return Some(format!(
                "Automatically deployed local SSH public key to {} via {}.",
                container_name, server.host
            ));
        }
    }

    None
}

pub(super) fn probe_managed_container_ssh(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
    ssh_user: &str,
) -> (bool, String, String) {
    let (mut ssh_reachable, mut ssh_state, mut ssh_message) =
        probe_container_ssh(server, container_ip, container_name, ssh_user);

    if !ssh_reachable
        && (ssh_message.contains("Permission denied")
            || ssh_message.contains("authentication failed"))
    {
        if let Some(deploy_message) = auto_deploy_public_key(server, container_name, ssh_user) {
            let (retry_reachable, retry_state, retry_message) =
                probe_container_ssh(server, container_ip, container_name, ssh_user);

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
    shell::shell_command(command).output().map_err(|error| {
        format!(
            "Failed to execute shell command for {}: {}",
            target_id, error
        )
    })
}
