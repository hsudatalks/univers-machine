use super::{
    current_username, default_known_hosts_path, RawTargetsFile, RemoteContainerServer,
    LOCAL_MACHINE_DESCRIPTION, LOCAL_MACHINE_HOST, LOCAL_MACHINE_ID, LOCAL_MACHINE_LABEL,
};
use crate::infra::russh::execute_chain_blocking;
use crate::machine::{
    fs_store::{
        initialize_targets_file_storage, read_targets_file_content, sanitize_targets_json_content,
        write_targets_file_content,
    },
    inventory::clear_targets_cache,
    ssh::{machine_host_key_alias, resolved_known_hosts_path},
};
use crate::models::{ContainerWorkspace, MachineTransport};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Runtime};
use univers_ark_russh::{
    ClientOptions as RusshClientOptions, ResolvedEndpoint, ResolvedEndpointChain,
};

pub(super) trait MachineRepository {
    fn read_raw_targets_file(&self) -> Result<RawTargetsFile, String>;
    fn read_targets_config(&self) -> Result<String, String>;
    fn save_targets_config(&self, content: &str) -> Result<(), String>;
}

pub(super) struct FsMachineRepository;

fn default_repository() -> FsMachineRepository {
    FsMachineRepository
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
        manager_type: super::ContainerManagerType::None,
        discovery_mode: super::ContainerDiscoveryMode::Auto,
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
        "discoveryMode": "auto",
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
    let options = RusshClientOptions {
        connect_timeout: Duration::from_secs(2),
        inactivity_timeout: Some(Duration::from_secs(2)),
        keepalive_interval: None,
        keepalive_max: 0,
    };

    execute_chain_blocking(
        &local_machine_probe_chain(&ssh_user),
        "printf univers-ark-local-ready",
        &options,
    )
    .map(|output| output.exit_status == 0)
    .unwrap_or(false)
}

fn sync_local_machine_config() -> Result<(), String> {
    let raw_content = read_targets_file_content()?;
    let sanitized_content = sanitize_targets_json_content(&raw_content)?;
    let raw_targets_file: RawTargetsFile =
        serde_json::from_str(&sanitized_content).map_err(|error| {
            format!(
                "Failed to parse {}: {}",
                super::fs_store::targets_file_path().display(),
                error
            )
        })?;
    let mut raw_json: Value = serde_json::from_str(&sanitized_content).map_err(|error| {
        format!(
            "Failed to parse {}: {}",
            super::fs_store::targets_file_path().display(),
            error
        )
    })?;
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
    write_targets_file_content(&next_content)?;
    clear_targets_cache();
    Ok(())
}

impl FsMachineRepository {
    fn initialize_targets_file_path<R: Runtime>(
        &self,
        app_handle: &AppHandle<R>,
    ) -> Result<std::path::PathBuf, String> {
        let writable_targets_path = initialize_targets_file_storage(app_handle)?;
        sync_local_machine_config()?;
        Ok(writable_targets_path)
    }
}

impl MachineRepository for FsMachineRepository {
    fn read_raw_targets_file(&self) -> Result<RawTargetsFile, String> {
        let content = read_targets_file_content()?;
        let sanitized = sanitize_targets_json_content(&content)?;
        serde_json::from_str::<RawTargetsFile>(&sanitized).map_err(|error| {
            format!(
                "Failed to parse {}: {}",
                super::fs_store::targets_file_path().display(),
                error
            )
        })
    }

    fn read_targets_config(&self) -> Result<String, String> {
        let content = read_targets_file_content()?;
        sanitize_targets_json_content(&content)
    }

    fn save_targets_config(&self, content: &str) -> Result<(), String> {
        let sanitized_content = sanitize_targets_json_content(content)?;
        serde_json::from_str::<RawTargetsFile>(&sanitized_content)
            .map_err(|error| format!("Invalid config JSON: {}", error))?;

        write_targets_file_content(&sanitized_content)?;
        clear_targets_cache();
        Ok(())
    }
}

pub(crate) fn initialize_targets_file_path<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> Result<std::path::PathBuf, String> {
    default_repository().initialize_targets_file_path(app_handle)
}

pub(super) fn read_raw_targets_file() -> Result<RawTargetsFile, String> {
    default_repository().read_raw_targets_file()
}

pub(crate) fn read_targets_config() -> Result<String, String> {
    default_repository().read_targets_config()
}

pub(crate) fn save_targets_config(content: &str) -> Result<(), String> {
    default_repository().save_targets_config(content)
}
