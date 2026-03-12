use super::{
    current_username, default_known_hosts_path, targets_file_name, RawTargetsFile,
    RemoteContainerServer,
    LOCAL_MACHINE_DESCRIPTION, LOCAL_MACHINE_HOST, LOCAL_MACHINE_ID, LOCAL_MACHINE_LABEL,
};
use crate::machine::inventory::clear_targets_cache;
use crate::machine::ssh::{machine_host_key_alias, resolved_known_hosts_path};
use crate::models::{ContainerWorkspace, MachineTransport};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::OnceLock, time::Duration};
use tauri::{AppHandle, Manager, Runtime, path::BaseDirectory};
use univers_ark_russh::{
    ClientOptions as RusshClientOptions, ResolvedEndpoint, ResolvedEndpointChain, execute_chain,
};

const BUNDLED_TARGETS_TEMPLATE_NAME: &str = "developer-targets.json";

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
        manager_type: super::ContainerManagerType::None,
        discovery_mode: super::ContainerDiscoveryMode::HostOnly,
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
    clear_targets_cache();
    Ok(())
}

fn configured_targets_path() -> &'static OnceLock<PathBuf> {
    static CONFIGURED_TARGETS_PATH: OnceLock<PathBuf> = OnceLock::new();
    &CONFIGURED_TARGETS_PATH
}

pub(crate) fn univers_config_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| String::from("Failed to resolve user home directory"))?;

    Ok(home.join(".univers"))
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

pub(super) fn sanitize_targets_json_content(content: &str) -> Result<String, String> {
    let mut value: Value =
        serde_json::from_str(content).map_err(|error| format!("Invalid config JSON: {}", error))?;
    sanitize_workspace_aliases(&mut value);
    serde_json::to_string_pretty(&value)
        .map_err(|error| format!("Failed to serialize sanitized config JSON: {}", error))
}

pub(super) fn read_raw_targets_file() -> Result<RawTargetsFile, String> {
    let config_path = targets_file_path();
    let content = fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;

    let sanitized = sanitize_targets_json_content(&content)?;
    serde_json::from_str::<RawTargetsFile>(&sanitized)
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))
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

    clear_targets_cache();
    Ok(())
}
