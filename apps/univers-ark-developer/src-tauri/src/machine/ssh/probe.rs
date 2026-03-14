use crate::machine::{inventory::resolve_raw_target, repository::read_raw_targets_file};
use crate::models::ManagedContainerKind;
use std::{fs, path::PathBuf};

use super::super::{ContainerManagerType, RemoteContainerServer};
use super::shell_single_quote;

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

fn deploy_key_inject_script(ssh_user: &str, public_key: &str) -> String {
    format!(
        "target_home=$(getent passwd {user} 2>/dev/null | cut -d: -f6); \
if [ -z \"$target_home\" ]; then \
  if [ {user} = 'root' ]; then \
    target_home=/root; \
  else \
    target_home=/home/{user}; \
  fi; \
fi; \
mkdir -p \"$target_home/.ssh\" && chmod 700 \"$target_home/.ssh\" && \
touch \"$target_home/.ssh/authorized_keys\" && \
grep -Fqx {key} \"$target_home/.ssh/authorized_keys\" || echo {key} >> \"$target_home/.ssh/authorized_keys\" && \
chmod 600 \"$target_home/.ssh/authorized_keys\"",
        user = shell_single_quote(ssh_user),
        key = shell_single_quote(public_key),
    )
}

fn deploy_key_managers(server: &RemoteContainerServer) -> Vec<ContainerManagerType> {
    match server.manager_type {
        ContainerManagerType::Orbstack => vec![ContainerManagerType::Orbstack],
        ContainerManagerType::Lxd => vec![ContainerManagerType::Lxd],
        ContainerManagerType::Docker | ContainerManagerType::Wsl => vec![],
        ContainerManagerType::None => {
            vec![ContainerManagerType::Orbstack, ContainerManagerType::Lxd]
        }
    }
}

fn deploy_key_remote_command(
    manager_type: ContainerManagerType,
    container_name: &str,
    ssh_user: &str,
    inject_script: &str,
) -> String {
    let is_orbstack = matches!(manager_type, ContainerManagerType::Orbstack);
    if is_orbstack {
        format!(
            "orb run -m {} -u {} bash -c {}",
            container_name,
            ssh_user,
            shell_single_quote(inject_script),
        )
    } else {
        format!(
            "lxc exec {} -- bash -c {}",
            container_name,
            shell_single_quote(inject_script),
        )
    }
}

fn auto_deploy_public_key(
    server: &RemoteContainerServer,
    container_name: &str,
    ssh_user: &str,
) -> Option<String> {
    let public_key = local_public_key()?;
    let inject_script = deploy_key_inject_script(ssh_user, &public_key);

    for manager_type in deploy_key_managers(server) {
        let remote_command =
            deploy_key_remote_command(manager_type, container_name, ssh_user, &inject_script);

        // Build the SSH command directly with proper arg separation to avoid
        // Windows shell tokenization issues with nested quoting.
        let ssh_flags = super::build::base_ssh_flags_for_server(server);
        let mut command = std::process::Command::new("ssh");
        for flag in &ssh_flags {
            command.arg(flag);
        }
        if let Some(proxy_jump) = super::build::proxy_jump_for_host_pub(server) {
            command.arg("-J").arg(&proxy_jump);
        }
        command.arg("-p").arg(server.port.to_string());
        command.arg(format!("{}@{}", server.ssh_user, server.host));
        // Wrap in login shell so PATH includes orb/lxc on macOS (non-interactive SSH
        // doesn't source login profiles where OrbStack adds its PATH entries).
        let wrapped_remote = format!("bash -lc {}", shell_single_quote(&remote_command));
        command.arg(&wrapped_remote);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let Ok(output) = command.output() else {
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

fn looks_like_auth_failure(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("authentication failed")
        || normalized.contains("permission denied")
        || normalized.contains("disconnected")
}

pub(crate) fn maybe_auto_deploy_target_public_key(
    target_id: &str,
    probe_error: &str,
) -> Option<String> {
    if !looks_like_auth_failure(probe_error) {
        return None;
    }

    let target = resolve_raw_target(target_id).ok()?;
    if matches!(target.container_kind, ManagedContainerKind::Host) || target.container_id.is_empty()
    {
        return None;
    }

    let raw_targets_file = read_raw_targets_file().ok()?;
    let server = raw_targets_file
        .machines
        .into_iter()
        .find(|server| server.id == target.machine_id)?;
    let container = server.containers.iter().find(|container| {
        container.id == target.container_id || container.name == target.container_id
    });
    let container_name = container
        .map(|container| container.name.as_str())
        .unwrap_or(target.container_id.as_str());
    let ssh_user = container
        .map(|container| container.ssh_user.trim())
        .filter(|ssh_user| !ssh_user.is_empty())
        .or_else(|| {
            let ssh_user = server.container_ssh_user.trim();
            (!ssh_user.is_empty()).then_some(ssh_user)
        })
        .unwrap_or(server.ssh_user.as_str());

    auto_deploy_public_key(&server, container_name, ssh_user)
}
