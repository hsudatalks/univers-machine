use std::path::PathBuf;

use super::super::{RemoteContainerContext, RemoteContainerServer};

pub(crate) fn managed_container_ssh_user(server: &RemoteContainerServer) -> &str {
    if server.container_ssh_user.trim().is_empty() {
        &server.ssh_user
    } else {
        &server.container_ssh_user
    }
}

pub(crate) fn managed_known_hosts_file() -> String {
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
            format!("{normalized}/.ssh/univers-ark-developer-known_hosts")
        }
        _ => String::from("~/.ssh/univers-ark-developer-known_hosts"),
    }
}

pub(crate) fn expand_home_path(path: &str) -> String {
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

pub(crate) fn resolved_known_hosts_path(server: &RemoteContainerServer) -> PathBuf {
    if server.known_hosts_path.trim().is_empty() {
        PathBuf::from(managed_known_hosts_file())
    } else {
        PathBuf::from(expand_home_path(&server.known_hosts_path))
    }
}

pub(crate) fn container_host_key_alias(
    server: &RemoteContainerServer,
    container_name: &str,
) -> String {
    format!("univers-ark-developer--{}--{}", server.id, container_name)
}

pub(crate) fn machine_host_key_alias(server: &RemoteContainerServer) -> String {
    format!("univers-ark-developer--{}--host", server.id)
}

fn base_ssh_flags(server: &RemoteContainerServer, host_key_alias: &str) -> Vec<String> {
    let mut flags = Vec::new();
    let known_hosts_file = resolved_known_hosts_path(server)
        .to_string_lossy()
        .to_string();

    flags.push(format!("-o UserKnownHostsFile={known_hosts_file}"));
    flags.push(format!("-o HostKeyAlias={host_key_alias}"));
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

pub(crate) fn ssh_options_for_context(
    server: &RemoteContainerServer,
    container_name: &str,
) -> String {
    let host_key_alias = container_host_key_alias(server, container_name);
    base_ssh_flags(server, &host_key_alias).join(" ")
}

pub(crate) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(crate) fn ssh_destination(container_ip: &str, ssh_user: &str) -> String {
    format!("{ssh_user}@{container_ip}")
}

pub(crate) fn default_terminal_startup_command() -> String {
    String::from("exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l")
}

pub(crate) fn host_terminal_startup_command(server: &RemoteContainerServer) -> String {
    if server.host_terminal_startup_command.trim().is_empty() {
        default_terminal_startup_command()
    } else {
        server.host_terminal_startup_command.trim().to_string()
    }
}

pub(crate) fn profile_terminal_startup_command(profile: &str) -> String {
    if profile.trim() == "ark-workbench" {
        return String::from(
            "tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l",
        );
    }

    default_terminal_startup_command()
}

pub(crate) fn build_ssh_command(
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

pub(crate) fn build_host_ssh_command(
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

pub(crate) fn terminal_command_for_server(
    server: &RemoteContainerServer,
    context: &RemoteContainerContext<'_>,
    startup_command: &str,
) -> String {
    super::super::discovery::render_template(&server.terminal_command_template, context, || {
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

/// Expose base ssh flags for the host to the probe module (auto-deploy).
pub(super) fn base_ssh_flags_for_server(server: &RemoteContainerServer) -> Vec<String> {
    base_ssh_flags(server, &machine_host_key_alias(server))
}

/// Expose proxy_jump_for_host to the probe module (auto-deploy).
pub(super) fn proxy_jump_for_host_pub(server: &RemoteContainerServer) -> Option<String> {
    proxy_jump_for_host(server)
}
