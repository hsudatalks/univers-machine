mod discovery;
mod profiles;
mod ssh;

use crate::models::{
    BrowserSurface, ContainerWorkspace, DeveloperService, DeveloperTarget, MachineTransport,
    ManagedContainerKind, ManagedServer, TargetsFile,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    process::Output,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tauri::{path::BaseDirectory, AppHandle, Manager, Runtime};
use univers_ark_russh::{
    execute_chain, ClientOptions as RusshClientOptions, ExecOutput as RusshExecOutput,
    ResolvedEndpoint, ResolvedEndpointChain,
};

use self::{
    discovery::{
        cached_remote_server_inventory, discover_remote_server_inventory,
        inventory_from_scanned_containers, scan_server_containers,
    },
    profiles::{
        apply_profile_defaults_to_remote_server, ContainerProfileConfig, ContainerProfiles,
    },
    ssh::{
        build_host_ssh_command, build_ssh_command, container_host_key_alias,
        machine_host_key_alias, resolved_known_hosts_path, run_target_shell_command_internal,
        shell_single_quote,
    },
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

fn preferred_local_profile(raw_targets_file: &RawTargetsFile) -> String {
    if raw_targets_file.profiles.contains_key("ark-workbench") {
        return String::from("ark-workbench");
    }

    raw_targets_file.default_profile.clone().unwrap_or_default()
}

fn local_machine_server(ssh_user: &str, profile: &str) -> RemoteContainerServer {
    RemoteContainerServer {
        id: String::from(LOCAL_MACHINE_ID),
        label: String::from(LOCAL_MACHINE_LABEL),
        transport: MachineTransport::Ssh,
        host: String::from(LOCAL_MACHINE_HOST),
        port: 22,
        description: String::from(LOCAL_MACHINE_DESCRIPTION),
        manager_type: ContainerManagerType::None,
        discovery_mode: ContainerDiscoveryMode::HostOnly,
        discovery_command: String::new(),
        ssh_user: ssh_user.to_string(),
        container_ssh_user: ssh_user.to_string(),
        identity_files: vec![],
        jump_chain: vec![],
        known_hosts_path: default_known_hosts_path(),
        strict_host_key_checking: true,
        container_name_suffix: String::new(),
        include_stopped: false,
        target_label_template: String::new(),
        target_host_template: String::from("{machineHost}"),
        target_description_template: String::new(),
        terminal_command_template: String::new(),
        notes: vec![],
        workspace: ContainerWorkspace {
            profile: profile.to_string(),
            ..ContainerWorkspace::default()
        },
        services: vec![],
        surfaces: vec![],
        containers: vec![],
    }
}

fn local_machine_template(raw_targets_file: &RawTargetsFile, ssh_user: &str) -> Value {
    let profile = preferred_local_profile(raw_targets_file);
    json!({
        "id": LOCAL_MACHINE_ID,
        "label": LOCAL_MACHINE_LABEL,
        "transport": "ssh",
        "host": LOCAL_MACHINE_HOST,
        "port": 22,
        "description": LOCAL_MACHINE_DESCRIPTION,
        "managerType": "none",
        "discoveryMode": "host-only",
        "discoveryCommand": "",
        "sshUser": ssh_user,
        "containerSshUser": ssh_user,
        "identityFiles": [],
        "jumpChain": [],
        "knownHostsPath": default_known_hosts_path(),
        "strictHostKeyChecking": true,
        "containerNameSuffix": "",
        "includeStopped": false,
        "targetLabelTemplate": "",
        "targetHostTemplate": "{machineHost}",
        "targetDescriptionTemplate": "",
        "terminalCommandTemplate": "",
        "notes": [],
        "workspace": {
            "profile": profile,
            "defaultTool": "dashboard",
            "projectPath": "",
            "filesRoot": "",
            "primaryWebServiceId": "",
            "tmuxCommandServiceId": ""
        },
        "services": [],
        "surfaces": [],
        "containers": []
    })
}

fn local_machine_probe_chain(ssh_user: &str) -> ResolvedEndpointChain {
    let server = local_machine_server(ssh_user, "");
    let endpoint = ResolvedEndpoint::new(
        LOCAL_MACHINE_ID,
        LOCAL_MACHINE_HOST,
        ssh_user,
        22,
        Vec::new(),
    )
    .with_known_hosts(
        resolved_known_hosts_path(&server),
        machine_host_key_alias(&server),
        true,
    );

    ResolvedEndpointChain::from_hops(vec![endpoint])
}

fn local_machine_available() -> bool {
    let Some(ssh_user) = current_username() else {
        return false;
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return false,
    };

    let options = RusshClientOptions {
        connect_timeout: Duration::from_secs(2),
        inactivity_timeout: Some(Duration::from_secs(2)),
        keepalive_interval: None,
        keepalive_max: 0,
    };

    runtime
        .block_on(execute_chain(
            &local_machine_probe_chain(&ssh_user),
            "printf univers-ark-local-ready",
            &options,
        ))
        .map(|output| output.exit_status == 0)
        .unwrap_or(false)
}

fn sync_local_machine_config(config_path: &PathBuf) -> Result<(), String> {
    let raw_content = fs::read_to_string(config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;
    let sanitized_content = sanitize_targets_json_content(&raw_content)?;
    let raw_targets_file: RawTargetsFile = serde_json::from_str(&sanitized_content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))?;
    let mut raw_json: Value = serde_json::from_str(&sanitized_content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))?;
    let Some(machines) = raw_json.get_mut("machines").and_then(Value::as_array_mut) else {
        return Err(String::from("Config is missing machines."));
    };

    let local_index = machines
        .iter()
        .position(|machine| machine.get("id").and_then(Value::as_str) == Some(LOCAL_MACHINE_ID));
    let local_available = local_machine_available();
    let mut changed = false;

    if local_available {
        let Some(ssh_user) = current_username() else {
            return Ok(());
        };
        let local_machine = local_machine_template(&raw_targets_file, &ssh_user);
        if let Some(index) = local_index {
            if machines[index] != local_machine {
                machines[index] = local_machine;
                changed = true;
            }
        } else {
            machines.insert(0, local_machine);
            if raw_json
                .get("selectedTargetId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                raw_json["selectedTargetId"] = Value::String(String::from("local::host"));
            }
            changed = true;
        }
    } else if let Some(index) = local_index {
        machines.remove(index);
        let selected_target_id = raw_json
            .get("selectedTargetId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if selected_target_id == "local::host" {
            raw_json["selectedTargetId"] = Value::Null;
        }
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let next_content = serde_json::to_string_pretty(&raw_json)
        .map_err(|error| format!("Failed to serialize updated config: {}", error))?;
    fs::write(config_path, next_content)
        .map_err(|error| format!("Failed to write {}: {}", config_path.display(), error))?;
    if let Ok(mut cache) = targets_cache().lock() {
        *cache = None;
    }
    Ok(())
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

const BUNDLED_TARGETS_TEMPLATE_NAME: &str = "developer-targets.json";
const LOCAL_MACHINE_ID: &str = "local";
const LOCAL_MACHINE_HOST: &str = "127.0.0.1";
const LOCAL_MACHINE_LABEL: &str = "Local";
const LOCAL_MACHINE_DESCRIPTION: &str = "Local machine.";

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
    configured_targets_path().get().cloned().unwrap_or_else(|| {
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
    sync_local_machine_config(&writable_targets_path)?;

    Ok(writable_targets_path)
}

fn targets_cache() -> &'static Mutex<Option<CachedResolvedInventory>> {
    static TARGETS_CACHE: OnceLock<Mutex<Option<CachedResolvedInventory>>> = OnceLock::new();

    TARGETS_CACHE.get_or_init(|| Mutex::new(None))
}

fn sanitize_workspace_aliases(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(legacy_value) = map.remove("primaryBrowserServiceId") {
                map.entry(String::from("primaryWebServiceId"))
                    .or_insert(legacy_value);
            }

            map.values_mut().for_each(sanitize_workspace_aliases);
        }
        Value::Array(items) => items.iter_mut().for_each(sanitize_workspace_aliases),
        _ => {}
    }
}

fn sanitize_targets_json_content(content: &str) -> Result<String, String> {
    let mut value: Value =
        serde_json::from_str(content).map_err(|error| format!("Invalid config JSON: {}", error))?;
    sanitize_workspace_aliases(&mut value);
    serde_json::to_string_pretty(&value)
        .map_err(|error| format!("Failed to serialize sanitized config JSON: {}", error))
}

fn read_raw_targets_file() -> Result<RawTargetsFile, String> {
    let config_path = targets_file_path();
    let content = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;

    let sanitized = sanitize_targets_json_content(&content)?;
    serde_json::from_str::<RawTargetsFile>(&sanitized)
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
    let mut targets = Vec::new();
    let mut servers = Vec::new();

    raw_targets_file.machines.iter_mut().for_each(|server| {
        apply_profile_defaults_to_remote_server(server, &profiles, default_profile.as_deref())
    });

    let discovered: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = raw_targets_file
            .machines
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

        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
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
    server: &RemoteContainerServer,
    container: &DiscoveredContainer,
    existing: Option<&MachineContainerConfig>,
) -> Value {
    let id = if container.id.trim().is_empty() {
        existing
            .map(|item| item.id.clone())
            .unwrap_or_else(|| container.name.clone())
    } else {
        container.id.clone()
    };
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
    let source = if matches!(container.kind, ManagedContainerKind::Host) {
        String::from("host")
    } else if let Some(existing) = existing {
        if existing.source.trim().is_empty() || existing.source == "unknown" {
            container.source.clone()
        } else {
            existing.source.clone()
        }
    } else {
        container.source.clone()
    };
    let enabled = if matches!(container.kind, ManagedContainerKind::Host) {
        true
    } else if let Some(existing) = existing {
        existing.enabled
    } else if server.container_name_suffix.trim().is_empty() {
        true
    } else {
        container.name.ends_with(&server.container_name_suffix)
    };
    let workspace = existing
        .map(|item| serde_json::to_value(&item.workspace).unwrap_or_else(|_| json!({})))
        .unwrap_or_else(|| json!({}));
    let services = existing
        .map(|item| serde_json::to_value(&item.services).unwrap_or_else(|_| json!([])))
        .unwrap_or_else(|| json!([]));
    let surfaces = existing
        .map(|item| serde_json::to_value(&item.surfaces).unwrap_or_else(|_| json!([])))
        .unwrap_or_else(|| json!([]));
    let ssh_user = if matches!(container.kind, ManagedContainerKind::Host) {
        server.ssh_user.clone()
    } else if !container.ssh_user.trim().is_empty() {
        container.ssh_user.clone()
    } else if let Some(existing) = existing {
        if !existing.ssh_user.trim().is_empty() {
            existing.ssh_user.clone()
        } else if !server.container_ssh_user.trim().is_empty() {
            server.container_ssh_user.clone()
        } else {
            server.ssh_user.clone()
        }
    } else if !server.container_ssh_user.trim().is_empty() {
        server.container_ssh_user.clone()
    } else {
        server.ssh_user.clone()
    };
    let mut ssh_user_candidates = Vec::new();
    if !ssh_user.trim().is_empty() {
        ssh_user_candidates.push(ssh_user.clone());
    }
    ssh_user_candidates.extend(container.ssh_user_candidates.iter().cloned());
    if let Some(existing) = existing {
        ssh_user_candidates.extend(existing.ssh_user_candidates.iter().cloned());
    }
    let mut seen_ssh_users = std::collections::HashSet::new();
    ssh_user_candidates.retain(|candidate| {
        let candidate = candidate.trim();
        !candidate.is_empty() && seen_ssh_users.insert(candidate.to_string())
    });

    json!({
        "id": id,
        "name": container.name,
        "kind": container.kind,
        "enabled": enabled,
        "source": source,
        "sshUser": ssh_user,
        "sshUserCandidates": ssh_user_candidates,
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
    let sanitized_content = sanitize_targets_json_content(&raw_content)?;
    let mut raw_json: Value = serde_json::from_str(&sanitized_content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))?;
    let mut raw_targets_file: RawTargetsFile = serde_json::from_str(&sanitized_content)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))?;

    let profiles: ContainerProfiles = raw_targets_file.profiles.clone();
    let default_profile = raw_targets_file.default_profile.clone();
    raw_targets_file.machines.iter_mut().for_each(|server| {
        apply_profile_defaults_to_remote_server(server, &profiles, default_profile.as_deref())
    });

    let Some(server_index) = raw_targets_file
        .machines
        .iter()
        .position(|server| server.id == server_id)
    else {
        return Err(format!("Unknown server: {}", server_id));
    };

    let server = raw_targets_file.machines[server_index].clone();
    let discovered = scan_server_containers(&server)?;
    let inventory = inventory_from_scanned_containers(&server, discovered.clone());
    let existing_manual = raw_targets_file.machines[server_index].containers.clone();
    let manual_values = discovered
        .iter()
        .map(|container| {
            let existing = existing_manual
                .iter()
                .find(|item| item.name == container.name);
            discovered_container_to_manual_value(&server, container, existing)
        })
        .collect::<Vec<_>>();

    let Some(remote_servers) = raw_json.get_mut("machines").and_then(Value::as_array_mut) else {
        return Err(String::from("Config is missing machines."));
    };

    let Some(server_json) = remote_servers
        .iter_mut()
        .find(|server_json| server_json.get("id").and_then(Value::as_str) == Some(server_id))
    else {
        return Err(format!("Unknown server: {}", server_id));
    };

    server_json["containers"] = Value::Array(manual_values);
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

pub(crate) fn resolve_raw_target(target_id: &str) -> Result<DeveloperTarget, String> {
    let targets_file = read_targets_file()?;

    if let Some(target) = targets_file
        .targets
        .into_iter()
        .find(|target| target.id == target_id)
    {
        return Ok(target);
    }
    Err(format!("Unknown target: {}", target_id))
}

fn identity_paths(paths: &[String]) -> Vec<PathBuf> {
    paths.iter().map(PathBuf::from).collect()
}

fn resolved_machine_chain(server: &RemoteContainerServer) -> Result<ResolvedEndpointChain, String> {
    if matches!(server.transport, MachineTransport::Local) {
        return Err(format!("Machine {} uses local transport", server.id));
    }

    let known_hosts_path = resolved_known_hosts_path(server);
    let mut hops = server
        .jump_chain
        .iter()
        .enumerate()
        .map(|(index, jump)| {
            ResolvedEndpoint::new(
                format!("{}::jump-{}", server.id, index + 1),
                jump.host.clone(),
                jump.user.clone(),
                jump.port,
                identity_paths(&jump.identity_files),
            )
            .with_known_hosts(
                known_hosts_path.clone(),
                jump.host.clone(),
                server.strict_host_key_checking,
            )
        })
        .collect::<Vec<_>>();
    hops.push(
        ResolvedEndpoint::new(
            server.id.clone(),
            server.host.clone(),
            server.ssh_user.clone(),
            server.port,
            identity_paths(&server.identity_files),
        )
        .with_known_hosts(
            known_hosts_path,
            machine_host_key_alias(server),
            server.strict_host_key_checking,
        ),
    );

    Ok(ResolvedEndpointChain::from_hops(hops))
}

pub(crate) fn resolve_target_ssh_chain(target_id: &str) -> Result<ResolvedEndpointChain, String> {
    let target = resolve_raw_target(target_id)?;
    if matches!(target.transport, MachineTransport::Local) {
        return Err(format!("Target {} uses local transport", target_id));
    }
    let raw_targets_file = read_raw_targets_file()?;
    let server = raw_targets_file
        .machines
        .iter()
        .find(|server| server.id == target.machine_id)
        .ok_or_else(|| format!("Unknown machine for {}", target_id))?;

    if matches!(target.container_kind, ManagedContainerKind::Host) {
        return resolved_machine_chain(server);
    }

    let inventory = load_inventory(false)?;

    if let Some(container) = inventory
        .servers
        .iter()
        .flat_map(|server| server.containers.iter())
        .find(|container| container.target_id == target_id)
    {
        let mut chain = resolved_machine_chain(server)?;
        chain.push(
            ResolvedEndpoint::new(
                format!("{}::{}", server.id, container.name),
                container.ipv4.clone(),
                container.ssh_user.clone(),
                22,
                Vec::new(),
            )
            .with_known_hosts(
                resolved_known_hosts_path(server),
                container_host_key_alias(server, &container.name),
                server.strict_host_key_checking,
            ),
        );

        return Ok(chain);
    }

    Err(format!("Unknown machine inventory for {}", target_id))
}

pub(crate) fn execute_target_command_via_russh(
    target_id: &str,
    command: &str,
) -> Result<RusshExecOutput, String> {
    let chain = resolve_target_ssh_chain(target_id)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Failed to build russh runtime: {}", error))?;

    runtime
        .block_on(execute_chain(
            &chain,
            command,
            &RusshClientOptions::default(),
        ))
        .map_err(|error| format!("russh exec failed for {}: {}", target_id, error))
}

pub(crate) fn run_target_shell_command(
    target_id: &str,
    remote_command: &str,
) -> Result<Output, String> {
    let target = resolve_raw_target(target_id)?;

    let raw_targets_file = read_raw_targets_file()?;
    let server = raw_targets_file
        .machines
        .iter()
        .find(|server| server.id == target.machine_id)
        .ok_or_else(|| format!("Unknown machine for {}", target_id))?;

    if matches!(target.container_kind, ManagedContainerKind::Host) {
        let quoted_remote_command = shell_single_quote(remote_command);
        let ssh_command = build_host_ssh_command(server, &[], Some(&quoted_remote_command));
        return run_target_shell_command_internal(target_id, &ssh_command);
    }

    let inventory = load_inventory(false)?;

    if let Some(container) = inventory
        .servers
        .iter()
        .flat_map(|server| server.containers.iter())
        .find(|container| container.target_id == target_id)
    {
        let quoted_remote_command = shell_single_quote(remote_command);
        let ssh_command = build_ssh_command(
            server,
            &container.ipv4,
            &container.name,
            &container.ssh_user,
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

pub(crate) fn read_targets_config() -> Result<String, String> {
    let config_path = targets_file_path();
    let content = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;

    sanitize_targets_json_content(&content)
}

pub(crate) fn save_targets_config(content: &str) -> Result<(), String> {
    let sanitized_content = sanitize_targets_json_content(content)?;
    serde_json::from_str::<RawTargetsFile>(&sanitized_content)
        .map_err(|error| format!("Invalid config JSON: {}", error))?;

    let config_path = targets_file_path();
    fs::write(&config_path, sanitized_content)
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

        let value = discovered_container_to_manual_value(&server, &discovered, Some(&existing));

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
