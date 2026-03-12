use crate::infra::shell;
use std::{fs, path::PathBuf};

use super::super::{ContainerManagerType, RemoteContainerServer};
use super::{build_host_ssh_command, build_ssh_command, shell_single_quote};

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

pub(crate) fn probe_container_ssh(
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

pub(crate) fn probe_machine_host_ssh(server: &RemoteContainerServer) -> (bool, String, String) {
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
    public_key: &str,
) -> String {
    let user_home = format!("/home/{}", ssh_user);
    let inject_script = format!(
        "mkdir -p {home}/.ssh && chmod 700 {home}/.ssh && echo {key} >> {home}/.ssh/authorized_keys && chmod 600 {home}/.ssh/authorized_keys",
        home = user_home,
        key = shell_single_quote(public_key),
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
        let command =
            deploy_key_command(server, manager_type, container_name, ssh_user, &public_key);
        let Ok(output) = shell::shell_command(&command).output() else {
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

pub(crate) fn probe_managed_container_ssh(
    server: &RemoteContainerServer,
    container_ip: &str,
    container_name: &str,
    ssh_user: &str,
) -> (bool, String, String) {
    let (mut ssh_reachable, mut ssh_state, mut ssh_message) =
        probe_container_ssh(server, container_ip, container_name, ssh_user);

    if !ssh_reachable && ssh_message.contains("Permission denied") {
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
