mod discovery;
mod ssh;

use crate::models::{BrowserSurface, DeveloperTarget, ManagedServer, TargetsFile};
use serde::Deserialize;
use std::{
    fs,
    path::PathBuf,
    process::Output,
    sync::{Mutex, OnceLock},
};
use tauri::{path::BaseDirectory, AppHandle, Manager, Runtime};

use self::{
    discovery::discover_remote_server_inventory,
    ssh::{build_ssh_command, run_target_shell_command_internal, shell_single_quote},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTargetsFile {
    selected_target_id: Option<String>,
    #[serde(default)]
    targets: Vec<DeveloperTarget>,
    #[serde(default)]
    remote_servers: Vec<RemoteContainerServer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteContainerServer {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) host: String,
    pub(super) description: String,
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
    pub(super) surfaces: Vec<BrowserSurface>,
}

#[derive(Debug, Clone)]
pub(super) struct DiscoveredContainer {
    pub(super) name: String,
    pub(super) status: String,
    pub(super) ipv4: String,
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

const TARGETS_FILE_NAME: &str = "developer-targets.json";
const SERVER_TERMINAL_TARGET_PREFIX: &str = "server-host::";

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
        .unwrap_or_else(|| app_root().join(TARGETS_FILE_NAME))
}

fn bundled_targets_file_path<R: Runtime>(app_handle: &AppHandle<R>) -> PathBuf {
    app_handle
        .path()
        .resolve(TARGETS_FILE_NAME, BaseDirectory::Resource)
        .unwrap_or_else(|_| app_root().join(TARGETS_FILE_NAME))
}

pub(crate) fn initialize_targets_file_path<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> Result<PathBuf, String> {
    let app_config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|error| format!("Failed to resolve app config directory: {}", error))?;

    fs::create_dir_all(&app_config_dir).map_err(|error| {
        format!(
            "Failed to create app config directory {}: {}",
            app_config_dir.display(),
            error
        )
    })?;

    let writable_targets_path = app_config_dir.join(TARGETS_FILE_NAME);

    if !writable_targets_path.exists() {
        let bundled_path = bundled_targets_file_path(app_handle);

        fs::copy(&bundled_path, &writable_targets_path).map_err(|error| {
            format!(
                "Failed to copy bundled targets file from {} to {}: {}",
                bundled_path.display(),
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

    let raw_targets_file = read_raw_targets_file()?;
    let mut targets = raw_targets_file.targets;
    let mut servers = Vec::new();

    let discovered: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = raw_targets_file
            .remote_servers
            .iter()
            .map(|server| scope.spawn(|| discover_remote_server_inventory(server)))
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
        notes: vec![format!("Server shell for {}.", server.host)],
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

    let is_orbstack = server.discovery_command.contains("orb");

    let restart_command = if is_orbstack {
        format!("ssh {} 'orb restart {}'", server.host, container_name)
    } else {
        format!(
            "ssh {} 'lxc restart {} --force'",
            server.host, container_name
        )
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
    use crate::models::ManagedContainer;
    use super::discovery::{
        build_target_from_container, parse_discovered_containers, server_state_for_containers,
    };

    fn fixture_server() -> RemoteContainerServer {
        RemoteContainerServer {
            id: String::from("mechanism-dev"),
            label: String::from("Mechanism"),
            host: String::from("mechanism-dev"),
            description: String::from("Mechanism development server."),
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
            surfaces: vec![BrowserSurface {
                id: String::from("development"),
                label: String::from("Development"),
                tunnel_command: String::from(
                    "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3432 -J {serverHost} {sshUser}@{containerIp}",
                ),
                local_url: String::from("http://127.0.0.1:{localPort}/"),
                remote_url: String::from("http://127.0.0.1:3432/"),
                vite_hmr_tunnel_command: String::from(
                    "ssh {sshOptions} -NT -L {localPort}:127.0.0.1:3433 -J {serverHost} {sshUser}@{containerIp}",
                ),
            }],
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
