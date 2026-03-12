mod connection;
mod discovery;
mod inventory;
mod profiles;
mod ssh;
mod store;

use crate::models::{
    BrowserSurface, ContainerWorkspace, DeveloperService, DeveloperTarget, MachineTransport,
    ManagedContainerKind, ManagedServer, TargetsFile,
};
use serde::Deserialize;
use std::collections::HashMap;

pub(crate) use self::{
    connection::{execute_target_command_via_russh, resolve_target_ssh_chain, run_target_shell_command},
    inventory::{
        read_bootstrap_data, read_server_inventory, read_targets_file, resolve_raw_target,
        scan_and_store_server_inventory,
    },
    store::{
        initialize_targets_file_path, read_targets_config, save_targets_config, targets_file_path,
        univers_config_dir,
    },
};

use self::{
    profiles::ContainerProfileConfig,
    ssh::{build_host_ssh_command, shell_single_quote},
    store::read_raw_targets_file,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTargetsFile {
    selected_target_id: Option<String>,
    default_profile: Option<String>,
    #[serde(default)]
    profiles: HashMap<String, ContainerProfileConfig>,
    #[serde(default)]
    machines: Vec<RemoteContainerServer>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(super) enum ContainerManagerType {
    #[default]
    None,
    Lxd,
    Docker,
    Orbstack,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(super) enum ContainerDiscoveryMode {
    #[serde(rename = "host-only", alias = "hostOnly")]
    HostOnly,
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct MachineContainerConfig {
    #[serde(default)]
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) kind: ManagedContainerKind,
    #[serde(default = "default_container_enabled")]
    pub(super) enabled: bool,
    #[serde(default = "default_container_source")]
    pub(super) source: String,
    #[serde(default)]
    pub(super) ssh_user: String,
    #[serde(default)]
    pub(super) ssh_user_candidates: Vec<String>,
    #[serde(default)]
    pub(super) label: String,
    #[serde(default)]
    pub(super) description: String,
    #[serde(default)]
    pub(super) ipv4: String,
    #[serde(default = "default_manual_container_status")]
    pub(super) status: String,
    #[serde(default)]
    pub(super) workspace: ContainerWorkspace,
    #[serde(default)]
    pub(super) services: Vec<DeveloperService>,
    #[serde(default)]
    pub(super) surfaces: Vec<BrowserSurface>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct SshJumpConfig {
    pub(super) host: String,
    #[serde(default = "default_ssh_port")]
    pub(super) port: u16,
    pub(super) user: String,
    #[serde(default)]
    pub(super) identity_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteContainerServer {
    pub(super) id: String,
    pub(super) label: String,
    #[serde(default)]
    pub(super) transport: MachineTransport,
    pub(super) host: String,
    #[serde(default = "default_ssh_port")]
    pub(super) port: u16,
    pub(super) description: String,
    #[serde(default)]
    pub(super) manager_type: ContainerManagerType,
    #[serde(default)]
    pub(super) discovery_mode: ContainerDiscoveryMode,
    #[serde(default)]
    pub(super) discovery_command: String,
    #[serde(default = "default_ssh_user")]
    pub(super) ssh_user: String,
    #[serde(default = "default_container_ssh_user")]
    pub(super) container_ssh_user: String,
    #[serde(default)]
    pub(super) identity_files: Vec<String>,
    #[serde(default)]
    pub(super) jump_chain: Vec<SshJumpConfig>,
    #[serde(default = "default_known_hosts_path")]
    pub(super) known_hosts_path: String,
    #[serde(default = "default_strict_host_key_checking")]
    pub(super) strict_host_key_checking: bool,
    #[serde(default = "default_container_name_suffix")]
    pub(super) container_name_suffix: String,
    #[serde(default)]
    pub(super) include_stopped: bool,
    #[serde(default)]
    pub(super) target_label_template: String,
    #[serde(default)]
    pub(super) target_host_template: String,
    #[serde(default)]
    pub(super) target_description_template: String,
    #[serde(default)]
    pub(super) terminal_command_template: String,
    #[serde(default)]
    pub(super) notes: Vec<String>,
    #[serde(default)]
    pub(super) workspace: ContainerWorkspace,
    #[serde(default)]
    pub(super) services: Vec<DeveloperService>,
    #[serde(default)]
    pub(super) surfaces: Vec<BrowserSurface>,
    #[serde(default)]
    pub(super) containers: Vec<MachineContainerConfig>,
}

#[derive(Debug, Clone)]
pub(super) struct DiscoveredContainer {
    pub(super) id: String,
    pub(super) kind: ManagedContainerKind,
    pub(super) name: String,
    pub(super) source: String,
    pub(super) ssh_user: String,
    pub(super) ssh_user_candidates: Vec<String>,
    pub(super) status: String,
    pub(super) ipv4: String,
    pub(super) label: Option<String>,
    pub(super) description: Option<String>,
    pub(super) workspace: Option<ContainerWorkspace>,
    pub(super) services: Vec<DeveloperService>,
    pub(super) surfaces: Vec<BrowserSurface>,
}

pub(super) struct RemoteContainerContext<'a> {
    pub(super) container_ip: &'a str,
    pub(super) container_label: &'a str,
    pub(super) container_name: &'a str,
    pub(super) ssh_user: &'a str,
    pub(super) server: &'a RemoteContainerServer,
}

#[derive(Clone)]
struct ResolvedInventory {
    targets_file: TargetsFile,
    servers: Vec<ManagedServer>,
}

#[derive(Clone)]
struct CachedResolvedInventory {
    inventory: ResolvedInventory,
}

pub(super) struct DiscoveredServerInventory {
    pub(super) server: ManagedServer,
    pub(super) available_targets: Vec<DeveloperTarget>,
}

fn default_ssh_port() -> u16 {
    22
}

fn default_ssh_user() -> String {
    String::from("ubuntu")
}

fn default_container_ssh_user() -> String {
    String::new()
}

fn default_container_name_suffix() -> String {
    String::from("-dev")
}

fn default_known_hosts_path() -> String {
    String::from("~/.univers/known_hosts")
}

fn default_strict_host_key_checking() -> bool {
    true
}

fn default_manual_container_status() -> String {
    String::from("RUNNING")
}

fn default_container_enabled() -> bool {
    true
}

fn default_container_source() -> String {
    String::from("unknown")
}

fn current_username() -> Option<String> {
    std::env::var(if cfg!(windows) { "USERNAME" } else { "USER" })
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn manager_priority(server: &RemoteContainerServer) -> Vec<ContainerManagerType> {
    match server.manager_type {
        ContainerManagerType::None => vec![
            ContainerManagerType::Orbstack,
            ContainerManagerType::Docker,
            ContainerManagerType::Lxd,
        ],
        manager_type => vec![manager_type],
    }
}

const LOCAL_MACHINE_ID: &str = "local";
const LOCAL_MACHINE_HOST: &str = "127.0.0.1";
const LOCAL_MACHINE_LABEL: &str = "Local";
const LOCAL_MACHINE_DESCRIPTION: &str = "Local machine.";

pub(super) fn targets_file_name() -> &'static str {
    if cfg!(debug_assertions) {
        "univers-ark-developer.dev.json"
    } else {
        "univers-ark-developer.json"
    }
}

pub(crate) fn restart_container(server_id: &str, container_name: &str) -> Result<(), String> {
    let raw_targets_file = read_raw_targets_file()?;
    let server = raw_targets_file
        .machines
        .iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| format!("Unknown machine: {}", server_id))?;
    if matches!(server.transport, MachineTransport::Local) {
        return Err(String::from(
            "Local host container cannot be restarted from machine inventory.",
        ));
    }

    let mut errors = Vec::new();

    for manager_type in manager_priority(server) {
        let restart_command = match manager_type {
            ContainerManagerType::Orbstack => build_host_ssh_command(
                server,
                &[],
                Some(&shell_single_quote(&format!(
                    "/opt/homebrew/bin/orb restart {}",
                    container_name
                ))),
            ),
            ContainerManagerType::Docker => build_host_ssh_command(
                server,
                &[],
                Some(&shell_single_quote(&format!(
                    "docker restart {}",
                    container_name
                ))),
            ),
            ContainerManagerType::Lxd => build_host_ssh_command(
                server,
                &[],
                Some(&shell_single_quote(&format!(
                    "lxc restart {} --force",
                    container_name
                ))),
            ),
            ContainerManagerType::None => continue,
        };

        let output = crate::shell::shell_command(&restart_command)
            .output()
            .map_err(|error| {
                format!(
                    "Failed to restart container {} on {}: {}",
                    container_name, server.host, error
                )
            })?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!(
                "Failed to restart container {} on {}: exit code {}",
                container_name, server.host, output.status
            )
        };

        let normalized = detail.to_ascii_lowercase();
        if normalized.contains("command not found")
            || normalized.contains("no such file or directory")
            || normalized.contains("not found")
        {
            continue;
        }

        errors.push(detail);
    }

    if errors.is_empty() {
        Err(format!(
            "Failed to restart container {} on {} with supported container managers.",
            container_name, server.host
        ))
    } else {
        Err(errors.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::discovery::{
        build_target_from_container, parse_discovered_containers, server_state_for_containers,
    };
    use super::ssh::ssh_destination;
    use super::*;
    use crate::models::{
        BrowserServiceType, MachineTransport, ManagedContainer, ManagedContainerKind,
    };

    fn fixture_server() -> RemoteContainerServer {
        RemoteContainerServer {
            id: String::from("mechanism-dev"),
            label: String::from("Mechanism"),
            transport: MachineTransport::Ssh,
            host: String::from("mechanism-dev"),
            port: 22,
            description: String::from("Mechanism development server."),
            manager_type: ContainerManagerType::Lxd,
            discovery_mode: ContainerDiscoveryMode::Auto,
            discovery_command: String::new(),
            ssh_user: String::from("david"),
            container_ssh_user: String::from("ubuntu"),
            identity_files: vec![],
            jump_chain: vec![],
            known_hosts_path: String::new(),
            strict_host_key_checking: false,
            container_name_suffix: String::from("-dev"),
            include_stopped: false,
            target_label_template: String::new(),
            target_host_template: String::from("{serverHost}"),
            target_description_template: String::new(),
            terminal_command_template: String::new(),
            notes: vec![String::from(
                "SSH target: {sshUser}@{containerIp} via {serverHost}.",
            )],
            workspace: ContainerWorkspace::default(),
            services: vec![],
            surfaces: vec![BrowserSurface {
                id: String::from("development"),
                label: String::from("Development"),
                service_type: BrowserServiceType::Vite,
                background_prerender: true,
                tunnel_command: String::from(
                    "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3432 -J {serverHost} {sshUser}@{containerIp}",
                ),
                local_url: String::from("http://127.0.0.1:{localPort}/"),
                remote_url: String::from("http://127.0.0.1:3432/"),
                vite_hmr_tunnel_command: String::from(
                    "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3433 -J {serverHost} {sshUser}@{containerIp}",
                ),
            }],
            containers: vec![],
        }
    }

    #[test]
    fn parses_running_dev_containers_from_lxd_csv() {
        let server = fixture_server();
        let discovery_output = "\
automation-dev,RUNNING,10.211.82.78 (eth0)\n\
runtime-dev,RUNNING,10.211.82.38 (eth0)\n\
tooling,STOPPED,\n\
workflow-dev,RUNNING,10.211.82.202 (eth0)\n";

        let containers = parse_discovered_containers(&server, discovery_output).unwrap();

        assert_eq!(containers.len(), 3);
        assert_eq!(containers[0].name, "automation-dev");
        assert_eq!(containers[0].ipv4, "10.211.82.78");
        assert_eq!(containers[1].name, "runtime-dev");
        assert_eq!(containers[2].name, "workflow-dev");
    }

    #[test]
    fn prefers_eth0_address_from_multiline_csv_field() {
        let server = fixture_server();
        let discovery_output = "\
env-dev,RUNNING,\"172.17.0.1 (docker0)\n\
10.197.97.142 (eth0)\"\n";

        let containers = parse_discovered_containers(&server, discovery_output).unwrap();

        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].name, "env-dev");
        assert_eq!(containers[0].ipv4, "10.197.97.142");
    }

    #[test]
    fn renders_terminal_and_tunnel_commands_for_discovered_container() {
        let server = fixture_server();
        let container = DiscoveredContainer {
            id: String::from("workflow-dev"),
            kind: ManagedContainerKind::Managed,
            name: String::from("workflow-dev"),
            source: String::from("lxd"),
            ssh_user: String::from("ubuntu"),
            ssh_user_candidates: vec![String::from("ubuntu"), String::from("root")],
            status: String::from("RUNNING"),
            ipv4: String::from("10.211.82.202"),
            label: None,
            description: None,
            workspace: None,
            services: vec![],
            surfaces: vec![],
        };

        let target = build_target_from_container(&server, &container);

        assert_eq!(target.id, "mechanism-dev::workflow-dev");
        assert_eq!(target.label, "Workflow");
        assert_eq!(target.host, "mechanism-dev");
        let home = if cfg!(windows) {
            std::env::var("USERPROFILE").unwrap().replace('\\', "/")
        } else {
            std::env::var("HOME").unwrap()
        };
        let expected_known_hosts_file = format!("{}/.ssh/univers-ark-developer-known_hosts", home);
        let expected_terminal_command = format!(
            "ssh -o UserKnownHostsFile={kh} -o HostKeyAlias=univers-ark-developer--mechanism-dev--workflow-dev -o StrictHostKeyChecking=no -tt -J david@mechanism-dev -p 22 ubuntu@10.211.82.202 'exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l'",
            kh = expected_known_hosts_file
        );
        assert_eq!(target.terminal_command, expected_terminal_command);
        assert_eq!(
            target.surfaces[0].tunnel_command,
            format!(
                "ssh -o UserKnownHostsFile={} -o HostKeyAlias=univers-ark-developer--mechanism-dev--workflow-dev -o StrictHostKeyChecking=no -NT -L {{localPort}}:127.0.0.1:3432 -J mechanism-dev ubuntu@10.211.82.202",
                expected_known_hosts_file
            )
        );
    }

    #[test]
    fn uses_tmux_startup_for_ark_workbench_container_profiles() {
        let mut server = fixture_server();
        server.workspace.profile = String::from("ark-workbench");
        let container = DiscoveredContainer {
            id: String::from("workflow-dev"),
            kind: ManagedContainerKind::Managed,
            name: String::from("workflow-dev"),
            source: String::from("lxd"),
            ssh_user: String::from("ubuntu"),
            ssh_user_candidates: vec![String::from("ubuntu"), String::from("root")],
            status: String::from("RUNNING"),
            ipv4: String::from("10.211.82.202"),
            label: None,
            description: None,
            workspace: None,
            services: vec![],
            surfaces: vec![],
        };

        let target = build_target_from_container(&server, &container);

        assert_eq!(
            target.terminal_startup_command,
            "tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l"
        );
        assert!(target.terminal_command.contains("tmux-mobile-view attach"));
    }

    #[test]
    fn scan_prefers_detected_container_user_over_stale_saved_user() {
        let server = fixture_server();
        let discovered = DiscoveredContainer {
            id: String::from("automation-dev"),
            kind: ManagedContainerKind::Managed,
            name: String::from("automation-dev"),
            source: String::from("lxd"),
            ssh_user: String::from("ubuntu"),
            ssh_user_candidates: vec![String::from("ubuntu"), String::from("root")],
            status: String::from("RUNNING"),
            ipv4: String::from("10.211.82.78"),
            label: None,
            description: None,
            workspace: None,
            services: vec![],
            surfaces: vec![],
        };
        let existing = MachineContainerConfig {
            id: String::from("automation-dev"),
            name: String::from("automation-dev"),
            kind: ManagedContainerKind::Managed,
            enabled: true,
            source: String::from("lxd"),
            ssh_user: String::from("david"),
            ssh_user_candidates: vec![String::from("david"), String::from("ubuntu")],
            label: String::new(),
            description: String::new(),
            ipv4: String::from("10.211.82.78"),
            status: String::from("RUNNING"),
            workspace: ContainerWorkspace::default(),
            services: vec![],
            surfaces: vec![],
        };

        let value =
            super::inventory::discovered_container_to_manual_value(&server, &discovered, Some(&existing));

        assert_eq!(value.get("sshUser").and_then(Value::as_str), Some("ubuntu"));
        assert_eq!(
            value
                .get("sshUserCandidates")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>()
                }),
            Some(vec![
                String::from("ubuntu"),
                String::from("root"),
                String::from("david"),
            ])
        );
    }

    #[test]
    fn builds_ready_server_state_from_reachable_containers() {
        let containers = vec![
            ManagedContainer {
                server_id: String::from("mechanism-dev"),
                server_label: String::from("Mechanism"),
                container_id: String::from("automation-dev"),
                kind: ManagedContainerKind::Managed,
                transport: MachineTransport::Ssh,
                target_id: String::from("mechanism-dev::automation-dev"),
                name: String::from("automation-dev"),
                label: String::from("Automation"),
                status: String::from("RUNNING"),
                ipv4: String::from("10.211.82.78"),
                ssh_user: String::from("ubuntu"),
                ssh_destination: String::from("ubuntu@10.211.82.78"),
                ssh_command: String::from("ssh -J mechanism-dev ubuntu@10.211.82.78"),
                ssh_state: String::from("ready"),
                ssh_message: String::from("SSH ready via mechanism-dev."),
                ssh_reachable: true,
            },
            ManagedContainer {
                server_id: String::from("mechanism-dev"),
                server_label: String::from("Mechanism"),
                container_id: String::from("runtime-dev"),
                kind: ManagedContainerKind::Managed,
                transport: MachineTransport::Ssh,
                target_id: String::from("mechanism-dev::runtime-dev"),
                name: String::from("runtime-dev"),
                label: String::from("Runtime"),
                status: String::from("RUNNING"),
                ipv4: String::from("10.211.82.38"),
                ssh_user: String::from("ubuntu"),
                ssh_destination: String::from("ubuntu@10.211.82.38"),
                ssh_command: String::from("ssh -J mechanism-dev ubuntu@10.211.82.38"),
                ssh_state: String::from("ready"),
                ssh_message: String::from("SSH ready via mechanism-dev."),
                ssh_reachable: true,
            },
        ];

        let (state, message) = server_state_for_containers(&containers);

        assert_eq!(state, "ready");
        assert!(message.contains("2 development container(s)"));
    }

    #[test]
    fn computes_ssh_destination() {
        assert_eq!(ssh_destination("10.1.2.3", "ubuntu"), "ubuntu@10.1.2.3");
    }
}
