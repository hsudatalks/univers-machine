use crate::models::{
    BrowserSurface, ContainerWorkspace, DeveloperService, DeveloperTarget, MachineTransport,
    ManagedContainerKind, ManagedServer, TargetsFile,
};
use serde::Deserialize;
use std::collections::HashMap;

use super::profiles::ContainerProfileConfig;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawTargetsFile {
    pub(super) selected_target_id: Option<String>,
    pub(super) default_profile: Option<String>,
    #[serde(default)]
    pub(super) profiles: HashMap<String, ContainerProfileConfig>,
    #[serde(default)]
    pub(super) machines: Vec<RemoteContainerServer>,
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
    #[serde(default)]
    pub(super) ssh_credential_id: String,
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
    pub(super) ssh_credential_id: String,
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
    pub(super) host_terminal_startup_command: String,
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
pub(super) struct ResolvedInventory {
    pub(super) targets_file: TargetsFile,
    pub(super) servers: Vec<ManagedServer>,
}

#[derive(Clone)]
pub(super) struct CachedResolvedInventory {
    pub(super) inventory: ResolvedInventory,
}

pub(super) struct DiscoveredServerInventory {
    pub(super) server: ManagedServer,
    pub(super) available_targets: Vec<DeveloperTarget>,
}

pub(super) fn default_ssh_port() -> u16 {
    22
}

pub(super) fn default_ssh_user() -> String {
    String::from("ubuntu")
}

pub(super) fn default_container_ssh_user() -> String {
    String::new()
}

pub(super) fn default_container_name_suffix() -> String {
    String::from("-dev")
}

pub(super) fn default_known_hosts_path() -> String {
    String::from("~/.univers/known_hosts")
}

pub(super) fn default_strict_host_key_checking() -> bool {
    true
}

pub(super) fn default_manual_container_status() -> String {
    String::from("RUNNING")
}

pub(super) fn default_container_enabled() -> bool {
    true
}

pub(super) fn default_container_source() -> String {
    String::from("unknown")
}

pub(super) fn current_username() -> Option<String> {
    std::env::var(if cfg!(windows) { "USERNAME" } else { "USER" })
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn manager_priority(server: &RemoteContainerServer) -> Vec<ContainerManagerType> {
    match server.manager_type {
        ContainerManagerType::None => vec![
            ContainerManagerType::Orbstack,
            ContainerManagerType::Docker,
            ContainerManagerType::Lxd,
        ],
        manager_type => vec![manager_type],
    }
}

pub(super) const LOCAL_MACHINE_ID: &str = "local";
pub(super) const LOCAL_MACHINE_HOST: &str = "127.0.0.1";
pub(super) const LOCAL_MACHINE_LABEL: &str = "Local";
pub(super) const LOCAL_MACHINE_DESCRIPTION: &str = "Local machine.";

pub(super) fn targets_file_name() -> &'static str {
    if cfg!(debug_assertions) {
        "univers-ark-developer.dev.json"
    } else {
        "univers-ark-developer.json"
    }
}
