mod discovery;
mod profiles;
mod ssh;

use crate::models::{
    BrowserSurface, ContainerWorkspace, DeveloperService, DeveloperTarget, ManagedServer,
    TargetsFile,
};
use serde_json::{Value, json};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    process::Output,
    sync::{Mutex, OnceLock},
};
use tauri::{path::BaseDirectory, AppHandle, Manager, Runtime};

use self::{
    discovery::{cached_remote_server_inventory, discover_remote_server_inventory, scan_server_containers},
    profiles::{
        ContainerProfileConfig, ContainerProfiles, apply_profile_defaults_to_remote_server,
        apply_profile_defaults_to_target,
    },
    ssh::{build_ssh_command, run_target_shell_command_internal, shell_single_quote},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTargetsFile {
    selected_target_id: Option<String>,
    default_profile: Option<String>,
    #[serde(default)]
    profiles: HashMap<String, ContainerProfileConfig>,
    #[serde(default)]
    targets: Vec<DeveloperTarget>,
    #[serde(default)]
    remote_servers: Vec<RemoteContainerServer>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(super) enum ContainerManagerType {
    #[default]
    Lxd,
    Docker,
    Orbstack,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(super) enum ContainerDiscoveryMode {
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManualContainerConfig {
    pub(super) name: String,
    #[serde(default)]
    pub(super) label: String,
    #[serde(default)]
    pub(super) description: String,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteContainerServer {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) host: String,
    pub(super) description: String,
    #[serde(default)]
    pub(super) manager_type: ContainerManagerType,
    #[serde(default)]
    pub(super) discovery_mode: ContainerDiscoveryMode,
    #[serde(default)]
    pub(super) discovery_command: String,
    pub(super) ssh_user: String,
    #[serde(default = "default_remote_server_ssh_options")]
    pub(super) ssh_options: String,
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
    pub(super) manual_containers: Vec<ManualContainerConfig>,
}

#[derive(Debug, Clone)]
pub(super) struct DiscoveredContainer {
    pub(super) name: String,
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

fn default_remote_server_ssh_options() -> String {
    String::from("-o StrictHostKeyChecking=accept-new")
}

fn default_container_name_suffix() -> String {
    String::from("-dev")
}

fn default_manual_container_status() -> String {
    String::from("RUNNING")
}

const BUNDLED_TARGETS_TEMPLATE_NAME: &str = "developer-targets.json";
const SERVER_TERMINAL_TARGET_PREFIX: &str = "server-host::";

fn targets_file_name() -> &'static str {
    if cfg!(debug_assertions) {
        "univers-ark-developer.dev.json"
    } else {
        "univers-ark-developer.json"
    }
}

pub(crate) fn univers_config_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| String::from("Failed to resolve user home directory"))?;

    Ok(home.join(".univers"))
}

fn configured_targets_path() -> &'static OnceLock<PathBuf> {
    static CONFIGURED_TARGETS_PATH: OnceLock<PathBuf> = OnceLock::new();
    &CONFIGURED_TARGETS_PATH
}

pub(crate) fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub(crate) fn targets_file_path() -> PathBuf {
    configured_targets_path()
        .get()
        .cloned()
        .unwrap_or_else(|| {
            univers_config_dir()
                .map(|dir| dir.join(targets_file_name()))
                .unwrap_or_else(|_| app_root().join(targets_file_name()))
        })
}

fn bundled_targets_file_path<R: Runtime>(app_handle: &AppHandle<R>) -> PathBuf {
    app_handle
        .path()
        .resolve(BUNDLED_TARGETS_TEMPLATE_NAME, BaseDirectory::Resource)
        .unwrap_or_else(|_| app_root().join(BUNDLED_TARGETS_TEMPLATE_NAME))
}

fn legacy_targets_file_path<R: Runtime>(app_handle: &AppHandle<R>) -> Option<PathBuf> {
    app_handle
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(BUNDLED_TARGETS_TEMPLATE_NAME))
        .filter(|path| path.exists())
}

pub(crate) fn initialize_targets_file_path<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> Result<PathBuf, String> {
    let app_config_dir = univers_config_dir()?;

    fs::create_dir_all(&app_config_dir).map_err(|error| {
        format!(
            "Failed to create config directory {}: {}",
            app_config_dir.display(),
            error
        )
    })?;

    let writable_targets_path = app_config_dir.join(targets_file_name());

    if !writable_targets_path.exists() {
        let source_path = legacy_targets_file_path(app_handle)
            .unwrap_or_else(|| bundled_targets_file_path(app_handle));

        fs::copy(&source_path, &writable_targets_path).map_err(|error| {
            format!(
                "Failed to initialize targets file from {} to {}: {}",
                source_path.display(),
                writable_targets_path.display(),
                error
            )
        })?;
    }

    let _ = configured_targets_path().set(writable_targets_path.clone());

    Ok(writable_targets_path)
}

fn targets_cache() -> &'static Mutex<Option<CachedResolvedInventory>> {
    static TARGETS_CACHE: OnceLock<Mutex<Option<CachedResolvedInventory>>> = OnceLock::new();

    TARGETS_CACHE.get_or_init(|| Mutex::new(None))
}

fn read_raw_targets_file() -> Result<RawTargetsFile, String> {
    let config_path = targets_file_path();
    let content = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;

    serde_json::from_str::<RawTargetsFile>(&content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))
}

fn load_inventory(force_refresh: bool) -> Result<ResolvedInventory, String> {
    if !force_refresh {
        if let Ok(cache) = targets_cache().lock() {
            if let Some(cached) = cache.as_ref() {
                return Ok(cached.inventory.clone());
            }
        }
    }

    let mut raw_targets_file = read_raw_targets_file()?;
    let profiles: ContainerProfiles = raw_targets_file.profiles.clone();
    let default_profile = raw_targets_file.default_profile.clone();
    let mut targets = raw_targets_file.targets;
    targets
        .iter_mut()
        .for_each(|target| {
            apply_profile_defaults_to_target(target, &profiles, default_profile.as_deref())
        });
    let mut servers = Vec::new();

    raw_targets_file
        .remote_servers
        .iter_mut()
        .for_each(|server| {
            apply_profile_defaults_to_remote_server(server, &profiles, default_profile.as_deref())
        });

    let discovered: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = raw_targets_file
            .remote_servers
            .iter()
            .map(|server| {
                scope.spawn(|| {
                    if force_refresh {
                        discover_remote_server_inventory(server)
                    } else {
                        cached_remote_server_inventory(server)
                    }
                })
            })
            .collect();

        handles.into_iter().map(|handle| handle.join().unwrap()).collect()
    });

    for inventory in discovered {
        targets.extend(inventory.available_targets);
        servers.push(inventory.server);
    }

    let inventory = ResolvedInventory {
        targets_file: TargetsFile {
            selected_target_id: raw_targets_file.selected_target_id,
            default_profile,
            targets,
        },
        servers,
    };

    if let Ok(mut cache) = targets_cache().lock() {
        *cache = Some(CachedResolvedInventory {
            inventory: inventory.clone(),
        });
    }

    Ok(inventory)
}

fn discovered_container_to_manual_value(
    container: &DiscoveredContainer,
    existing: Option<&ManualContainerConfig>,
) -> Value {
    let label = container
        .label
        .as_ref()
        .cloned()
        .or_else(|| existing.map(|item| item.label.clone()))
        .unwrap_or_default();
    let description = container
        .description
        .as_ref()
        .cloned()
        .or_else(|| existing.map(|item| item.description.clone()))
        .unwrap_or_default();
    let workspace = existing
        .map(|item| serde_json::to_value(&item.workspace).unwrap_or_else(|_| json!({})))
        .unwrap_or_else(|| json!({}));
    let services = existing
        .map(|item| serde_json::to_value(&item.services).unwrap_or_else(|_| json!([])))
        .unwrap_or_else(|| json!([]));
    let surfaces = existing
        .map(|item| serde_json::to_value(&item.surfaces).unwrap_or_else(|_| json!([])))
        .unwrap_or_else(|| json!([]));

    json!({
        "name": container.name,
        "label": label,
        "description": description,
        "ipv4": container.ipv4,
        "status": container.status,
        "workspace": workspace,
        "services": services,
        "surfaces": surfaces
    })
}

pub(crate) fn scan_and_store_server_inventory(server_id: &str) -> Result<ManagedServer, String> {
    let config_path = targets_file_path();
    let raw_content = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;
    let mut raw_json: Value = serde_json::from_str(&raw_content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))?;
    let mut raw_targets_file: RawTargetsFile = serde_json::from_str(&raw_content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))?;

    let profiles: ContainerProfiles = raw_targets_file.profiles.clone();
    let default_profile = raw_targets_file.default_profile.clone();
    raw_targets_file
        .remote_servers
        .iter_mut()
        .for_each(|server| {
            apply_profile_defaults_to_remote_server(server, &profiles, default_profile.as_deref())
        });

    let Some(server_index) = raw_targets_file
        .remote_servers
        .iter()
        .position(|server| server.id == server_id) else {
        return Err(format!("Unknown server: {}", server_id));
    };

    let server = raw_targets_file.remote_servers[server_index].clone();
    let discovered = scan_server_containers(&server)?;
    let inventory = discover_remote_server_inventory(&server);
    let existing_manual = raw_targets_file.remote_servers[server_index]
        .manual_containers
        .clone();
    let manual_values = discovered
        .iter()
        .map(|container| {
            let existing = existing_manual
                .iter()
                .find(|item| item.name == container.name);
            discovered_container_to_manual_value(container, existing)
        })
        .collect::<Vec<_>>();

    let Some(remote_servers) = raw_json
        .get_mut("remoteServers")
        .and_then(Value::as_array_mut) else {
        return Err(String::from("Config is missing remoteServers."));
    };

    let Some(server_json) = remote_servers
        .iter_mut()
        .find(|server_json| server_json.get("id").and_then(Value::as_str) == Some(server_id))
    else {
        return Err(format!("Unknown server: {}", server_id));
    };

    server_json["manualContainers"] = Value::Array(manual_values);
    let next_content = serde_json::to_string_pretty(&raw_json)
        .map_err(|error| format!("Failed to serialize updated config: {}", error))?;
    save_targets_config(&next_content)?;

    Ok(inventory.server)
}

pub(crate) fn read_server_inventory(force_refresh: bool) -> Result<Vec<ManagedServer>, String> {
    load_inventory(force_refresh).map(|inventory| inventory.servers)
}

pub(crate) fn read_targets_file() -> Result<TargetsFile, String> {
    load_inventory(false).map(|inventory| inventory.targets_file)
}

fn resolve_server_terminal_target(target_id: &str) -> Result<Option<DeveloperTarget>, String> {
    let Some(server_id) = target_id.strip_prefix(SERVER_TERMINAL_TARGET_PREFIX) else {
        return Ok(None);
    };

    let raw_targets_file = read_raw_targets_file()?;
    let Some(server) = raw_targets_file
        .remote_servers
        .into_iter()
        .find(|server| server.id == server_id)
    else {
        return Ok(None);
    };

    Ok(Some(DeveloperTarget {
        id: target_id.to_string(),
        label: format!("{} host", server.label),
        host: server.host.clone(),
        description: format!("Interactive shell on {}.", server.host),
        terminal_command: format!("ssh {}", server.host),
        terminal_startup_command: String::new(),
        notes: vec![format!("Server shell for {}.", server.host)],
        workspace: ContainerWorkspace::default(),
        services: vec![],
        surfaces: vec![],
    }))
}

pub(crate) fn resolve_raw_target(target_id: &str) -> Result<DeveloperTarget, String> {
    let targets_file = read_targets_file()?;

    if let Some(target) = targets_file
        .targets
        .into_iter()
        .find(|target| target.id == target_id)
    {
        return Ok(target);
    }

    resolve_server_terminal_target(target_id)?
        .ok_or_else(|| format!("Unknown target: {}", target_id))
}

pub(crate) fn run_target_shell_command(
    target_id: &str,
    remote_command: &str,
) -> Result<Output, String> {
    let inventory = load_inventory(false)?;

    if let Some(container) = inventory
        .servers
        .iter()
        .flat_map(|server| server.containers.iter())
        .find(|container| container.target_id == target_id)
    {
        let raw_targets_file = read_raw_targets_file()?;
        let server = raw_targets_file
            .remote_servers
            .iter()
            .find(|server| server.id == container.server_id)
            .ok_or_else(|| format!("Unknown remote server for {}", target_id))?;
        let quoted_remote_command = shell_single_quote(remote_command);
        let ssh_command = build_ssh_command(
            server,
            &container.ipv4,
            &container.name,
            &[],
            Some(&quoted_remote_command),
        );

        return run_target_shell_command_internal(target_id, &ssh_command);
    }

    run_target_shell_command_internal(target_id, remote_command)
}

pub(crate) fn restart_container(server_id: &str, container_name: &str) -> Result<(), String> {
    let raw_targets_file = read_raw_targets_file()?;
    let server = raw_targets_file
        .remote_servers
        .iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| format!("Unknown remote server: {}", server_id))?;

    let restart_command = match server.manager_type {
        ContainerManagerType::Orbstack => {
            format!("ssh {} '/opt/homebrew/bin/orb restart {}'", server.host, container_name)
        }
        ContainerManagerType::Docker => {
            format!("ssh {} 'docker restart {}'", server.host, container_name)
        }
        ContainerManagerType::Lxd => {
            format!("ssh {} 'lxc restart {} --force'", server.host, container_name)
        }
    };

    let output = crate::shell::shell_command(&restart_command)
        .output()
        .map_err(|error| {
            format!(
                "Failed to restart container {} on {}: {}",
                container_name, server.host, error
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!(
                "Failed to restart container {} on {}: exit code {}",
                container_name,
                server.host,
                output.status
            )
        } else {
            stderr
        });
    }

    Ok(())
}

pub(crate) fn read_targets_config() -> Result<String, String> {
    let config_path = targets_file_path();
    fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))
}

pub(crate) fn save_targets_config(content: &str) -> Result<(), String> {
    // Validate JSON parses correctly before writing
    serde_json::from_str::<RawTargetsFile>(content)
        .map_err(|error| format!("Invalid config JSON: {}", error))?;

    let config_path = targets_file_path();
    fs::write(&config_path, content)
        .map_err(|error| format!("Failed to write {}: {}", config_path.display(), error))?;

    // Invalidate inventory cache so next load picks up changes
    if let Ok(mut cache) = targets_cache().lock() {
        *cache = None;
    }

    Ok(())
}

pub(crate) fn read_bootstrap_data(
    force_refresh: bool,
) -> Result<(TargetsFile, Vec<ManagedServer>), String> {
    let inventory = load_inventory(force_refresh)?;
    Ok((inventory.targets_file, inventory.servers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::ssh::ssh_destination;
    use crate::models::ManagedContainer;
    use crate::models::BrowserServiceType;
    use super::discovery::{
        build_target_from_container, parse_discovered_containers, server_state_for_containers,
    };

    fn fixture_server() -> RemoteContainerServer {
        RemoteContainerServer {
            id: String::from("mechanism-dev"),
            label: String::from("Mechanism"),
            host: String::from("mechanism-dev"),
            description: String::from("Mechanism development server."),
            manager_type: ContainerManagerType::Lxd,
            discovery_mode: ContainerDiscoveryMode::Auto,
            discovery_command: String::new(),
            ssh_user: String::from("ubuntu"),
            ssh_options: String::from("-o StrictHostKeyChecking=accept-new"),
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
                tunnel_command: String::from(
                    "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3432 -J {serverHost} {sshUser}@{containerIp}",
                ),
                local_url: String::from("http://127.0.0.1:{localPort}/"),
                remote_url: String::from("http://127.0.0.1:3432/"),
                vite_hmr_tunnel_command: String::from(
                    "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3433 -J {serverHost} {sshUser}@{containerIp}",
                ),
            }],
            manual_containers: vec![],
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
            name: String::from("workflow-dev"),
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
        let expected_known_hosts_file = format!(
            "{}/.ssh/univers-ark-developer-known_hosts",
            home
        );
        let expected_terminal_command = format!(
            "ssh -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile={kh} -o HostKeyAlias=univers-ark-developer--mechanism-dev--workflow-dev -tt -J mechanism-dev ubuntu@10.211.82.202 'tmux-mobile-view attach || exec /bin/zsh -l || exec /bin/bash -l || exec /bin/sh -l'",
            kh = expected_known_hosts_file
        );
        assert_eq!(target.terminal_command, expected_terminal_command);
        assert_eq!(
            target.surfaces[0].tunnel_command,
            format!(
                "ssh -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile={} -o HostKeyAlias=univers-ark-developer--mechanism-dev--workflow-dev -NT -L {{localPort}}:127.0.0.1:3432 -J mechanism-dev ubuntu@10.211.82.202",
                expected_known_hosts_file
            )
        );
    }

    #[test]
    fn builds_ready_server_state_from_reachable_containers() {
        let containers = vec![
            ManagedContainer {
                server_id: String::from("mechanism-dev"),
                server_label: String::from("Mechanism"),
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
        let server = fixture_server();
        assert_eq!(ssh_destination(&server, "10.1.2.3"), "ubuntu@10.1.2.3");
    }
}
