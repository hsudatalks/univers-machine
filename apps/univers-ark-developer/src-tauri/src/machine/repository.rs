use super::{
    current_username, default_known_hosts_path, detect_local_os, ContainerDiscoveryMode,
    ContainerManagerType, DiscoveredContainer, RawTargetsFile, RemoteContainerServer,
    LOCAL_MACHINE_DESCRIPTION, LOCAL_MACHINE_HOST, LOCAL_MACHINE_ID, LOCAL_MACHINE_LABEL,
};
use crate::infra::{
    russh::execute_chain_blocking,
    sqlite::SqliteStore,
    storage_paths::univers_config_dir,
};
use crate::models::{ContainerWorkspace, MachineTransport};
use crate::machine::{
    fs_store::{
        initialize_targets_file_storage, read_targets_file_content, sanitize_targets_json_content,
        targets_file_path, write_targets_file_content,
    },
    inventory::clear_targets_cache,
    ssh::{machine_host_key_alias, resolved_known_hosts_path},
};
use std::{path::PathBuf, time::Duration};
use tauri::{AppHandle, Runtime};
use univers_ark_russh::{
    ClientOptions as RusshClientOptions, ResolvedEndpoint, ResolvedEndpointChain,
};

const MACHINE_CONFIG_DOCUMENT_KEY: &str = "targets_file";

pub(super) trait MachineRepository {
    fn load_targets_file(&self) -> Result<RawTargetsFile, String>;
    fn save_targets_file(&self, targets_file: &RawTargetsFile) -> Result<(), String>;
    fn load_targets_content(&self) -> Result<String, String>;
    fn save_targets_content(&self, content: &str) -> Result<(), String>;
    fn load_inventory_snapshot(&self, machine_id: &str) -> Result<Option<Vec<DiscoveredContainer>>, String>;
    fn save_inventory_snapshot(
        &self,
        machine_id: &str,
        containers: &[DiscoveredContainer],
    ) -> Result<(), String>;
}

pub(super) struct SqliteMachineRepository {
    sqlite: SqliteStore,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn machine_database_file_name() -> &'static str {
    if cfg!(debug_assertions) {
        "univers-ark-developer.dev.machines.db"
    } else {
        "univers-ark-developer.machines.db"
    }
}

fn machine_database_path() -> Result<PathBuf, String> {
    univers_config_dir().map(|dir| dir.join(machine_database_file_name()))
}

fn default_repository() -> Result<SqliteMachineRepository, String> {
    SqliteMachineRepository::new(machine_database_path()?)
}

fn preferred_local_profile(raw_targets_file: &RawTargetsFile) -> String {
    if raw_targets_file.profiles.contains_key("ark-workbench") {
        return String::from("ark-workbench");
    }

    raw_targets_file.default_profile.clone().unwrap_or_default()
}

fn same_machine_config(
    left: &RemoteContainerServer,
    right: &RemoteContainerServer,
) -> Result<bool, String> {
    let left = serde_json::to_value(left)
        .map_err(|error| format!("Failed to serialize local machine config: {}", error))?;
    let right = serde_json::to_value(right)
        .map_err(|error| format!("Failed to serialize local machine config: {}", error))?;
    Ok(left == right)
}

fn local_machine_template(raw_targets_file: &RawTargetsFile, ssh_user: &str) -> RemoteContainerServer {
    RemoteContainerServer {
        id: String::from(LOCAL_MACHINE_ID),
        label: String::from(LOCAL_MACHINE_LABEL),
        transport: MachineTransport::Ssh,
        host: String::from(LOCAL_MACHINE_HOST),
        port: 22,
        description: String::from(LOCAL_MACHINE_DESCRIPTION),
        os: detect_local_os(),
        manager_type: ContainerManagerType::None,
        discovery_mode: ContainerDiscoveryMode::Auto,
        discovery_command: String::new(),
        ssh_user: ssh_user.to_string(),
        container_ssh_user: ssh_user.to_string(),
        identity_files: vec![],
        ssh_credential_id: String::new(),
        jump_chain: vec![],
        known_hosts_path: default_known_hosts_path(),
        strict_host_key_checking: false,
        container_name_suffix: String::new(),
        include_stopped: false,
        target_label_template: String::new(),
        target_host_template: String::from("{machineHost}"),
        target_description_template: String::new(),
        host_terminal_startup_command: String::new(),
        terminal_command_template: String::new(),
        notes: vec![],
        workspace: ContainerWorkspace {
            profile: preferred_local_profile(raw_targets_file),
            default_tool: String::from("dashboard"),
            project_path: String::new(),
            files_root: String::new(),
            primary_web_service_id: String::new(),
            tmux_command_service_id: String::new(),
        },
        services: vec![],
        surfaces: vec![],
        containers: vec![],
    }
}

fn build_local_machine_config(
    raw_targets_file: &RawTargetsFile,
    existing_machine: Option<&RemoteContainerServer>,
) -> RemoteContainerServer {
    let default_ssh_user = current_username().unwrap_or_default();
    let mut next_machine = existing_machine
        .cloned()
        .unwrap_or_else(|| local_machine_template(raw_targets_file, &default_ssh_user));

    next_machine.id = String::from(LOCAL_MACHINE_ID);
    next_machine.transport = MachineTransport::Ssh;
    next_machine.host = String::from(LOCAL_MACHINE_HOST);
    next_machine.os = detect_local_os();

    if next_machine.label.trim().is_empty() {
        next_machine.label = String::from(LOCAL_MACHINE_LABEL);
    }
    if next_machine.description.trim().is_empty() {
        next_machine.description = String::from(LOCAL_MACHINE_DESCRIPTION);
    }
    if next_machine.port == 0 {
        next_machine.port = 22;
    }
    if next_machine.ssh_user.trim().is_empty() {
        next_machine.ssh_user = default_ssh_user.clone();
    }
    if next_machine.container_ssh_user.trim().is_empty() {
        next_machine.container_ssh_user = next_machine.ssh_user.clone();
    }
    if next_machine.known_hosts_path.trim().is_empty() {
        next_machine.known_hosts_path = default_known_hosts_path();
    }
    if next_machine.workspace.profile.trim().is_empty() {
        next_machine.workspace.profile = preferred_local_profile(raw_targets_file);
    }
    if next_machine.workspace.default_tool.trim().is_empty() {
        next_machine.workspace.default_tool = String::from("dashboard");
    }

    next_machine
}

fn local_machine_probe_chain(server: &RemoteContainerServer) -> ResolvedEndpointChain {
    let endpoint = ResolvedEndpoint::new(
        LOCAL_MACHINE_ID,
        server.host.clone(),
        server.ssh_user.clone(),
        server.port,
        server
            .identity_files
            .iter()
            .map(std::path::PathBuf::from)
            .collect(),
    )
    .with_known_hosts(
        resolved_known_hosts_path(server),
        machine_host_key_alias(server),
        server.strict_host_key_checking,
    );

    ResolvedEndpointChain::from_hops(vec![endpoint])
}

fn auto_deploy_local_ssh_key(_ssh_user: &str) {
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE").ok()
    } else {
        std::env::var("HOME").ok()
    };

    let Some(home) = home else {
        return;
    };

    let ssh_dir = std::path::PathBuf::from(&home).join(".ssh");
    let pub_key_content = ["id_ed25519.pub", "id_rsa.pub"]
        .iter()
        .find_map(|name| std::fs::read_to_string(ssh_dir.join(name)).ok())
        .map(|content| content.trim().to_string());

    let Some(pub_key) = pub_key_content.filter(|key| !key.is_empty()) else {
        return;
    };

    let _ = std::fs::create_dir_all(&ssh_dir);
    let auth_keys_path = ssh_dir.join("authorized_keys");
    let already_deployed = std::fs::read_to_string(&auth_keys_path)
        .map(|content| content.contains(&pub_key))
        .unwrap_or(false);
    if !already_deployed {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&auth_keys_path)
        {
            let _ = writeln!(file, "{pub_key}");
        }
    }

    #[cfg(windows)]
    {
        let admin_keys_path =
            std::path::PathBuf::from(r"C:\ProgramData\ssh\administrators_authorized_keys");
        let admin_deployed = std::fs::read_to_string(&admin_keys_path)
            .map(|content| content.contains(&pub_key))
            .unwrap_or(false);
        if !admin_deployed {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&admin_keys_path)
            {
                let _ = writeln!(file, "{pub_key}");
            }

            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("icacls")
                .args([
                    admin_keys_path.to_string_lossy().as_ref(),
                    "/inheritance:r",
                    "/grant",
                    "SYSTEM:(F)",
                    "/grant",
                    "BUILTIN\\Administrators:(F)",
                ])
                .creation_flags(0x08000000)
                .output();
        }
    }
}

fn local_machine_available(server: &RemoteContainerServer) -> bool {
    if server.ssh_user.trim().is_empty() || server.host.trim().is_empty() || server.port == 0 {
        return false;
    }

    auto_deploy_local_ssh_key(&server.ssh_user);

    let options = RusshClientOptions {
        connect_timeout: Duration::from_secs(2),
        inactivity_timeout: Some(Duration::from_secs(2)),
        keepalive_interval: None,
        keepalive_max: 0,
    };

    execute_chain_blocking(
        &local_machine_probe_chain(server),
        "printf univers-ark-local-ready",
        &options,
    )
    .map(|output| output.exit_status == 0)
    .unwrap_or(false)
}

fn sync_local_machine_config(repository: &impl MachineRepository) -> Result<(), String> {
    let mut raw_targets_file = repository.load_targets_file()?;
    let local_index = raw_targets_file
        .machines
        .iter()
        .position(|machine| machine.id == LOCAL_MACHINE_ID);
    let local_machine = build_local_machine_config(
        &raw_targets_file,
        local_index.and_then(|index| raw_targets_file.machines.get(index)),
    );
    let local_available = local_machine_available(&local_machine);
    let mut changed = false;

    if local_available {
        if let Some(index) = local_index {
            if !same_machine_config(&raw_targets_file.machines[index], &local_machine)? {
                raw_targets_file.machines[index] = local_machine;
                changed = true;
            }
        } else {
            raw_targets_file.machines.insert(0, local_machine);
            if raw_targets_file
                .selected_target_id
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                raw_targets_file.selected_target_id = Some(String::from("local::host"));
            }
            changed = true;
        }
    } else if let Some(index) = local_index {
        raw_targets_file.machines.remove(index);
        if raw_targets_file.selected_target_id.as_deref() == Some("local::host") {
            raw_targets_file.selected_target_id = None;
        }
        changed = true;
    }

    if changed {
        repository.save_targets_file(&raw_targets_file)?;
    }

    Ok(())
}

impl SqliteMachineRepository {
    fn new(path: PathBuf) -> Result<Self, String> {
        let repository = Self {
            sqlite: SqliteStore::new(path)?,
        };
        repository.migrate()?;
        Ok(repository)
    }

    fn migrate(&self) -> Result<(), String> {
        self.sqlite.migrate(
            "CREATE TABLE IF NOT EXISTS machine_config_documents (
                document_key  TEXT PRIMARY KEY,
                content       TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS machine_inventory_snapshots (
                machine_id    TEXT PRIMARY KEY,
                content       TEXT NOT NULL,
                scanned_at_ms INTEGER NOT NULL
             );",
        )
    }

    fn read_stored_content(&self) -> Result<Option<String>, String> {
        let connection = self.sqlite.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT content
                 FROM machine_config_documents
                 WHERE document_key = ?1",
            )
            .map_err(|error| format!("Failed to prepare machine config query: {}", error))?;
        let mut rows = statement
            .query([MACHINE_CONFIG_DOCUMENT_KEY])
            .map_err(|error| format!("Failed to query machine config document: {}", error))?;

        let Some(row) = rows
            .next()
            .map_err(|error| format!("Failed to read machine config row: {}", error))?
        else {
            return Ok(None);
        };

        row.get(0)
            .map(Some)
            .map_err(|error| format!("Failed to decode machine config content: {}", error))
    }

    fn write_stored_content(&self, content: &str) -> Result<(), String> {
        let sanitized_content = sanitize_targets_json_content(content)?;
        let connection = self.sqlite.connect()?;
        connection
            .execute(
                "INSERT INTO machine_config_documents (document_key, content, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(document_key) DO UPDATE SET
                   content = excluded.content,
                   updated_at_ms = excluded.updated_at_ms",
                (
                    MACHINE_CONFIG_DOCUMENT_KEY,
                    sanitized_content.as_str(),
                    now_ms(),
                ),
            )
            .map_err(|error| format!("Failed to persist machine config document: {}", error))?;
        write_targets_file_content(&sanitized_content)?;
        clear_targets_cache();
        Ok(())
    }

    fn bootstrap_from_json_mirror_if_needed(&self) -> Result<(), String> {
        if self.read_stored_content()?.is_some() {
            return Ok(());
        }

        let content = read_targets_file_content()?;
        self.write_stored_content(&content)
    }

    fn initialize_targets_file_path<R: Runtime>(
        &self,
        app_handle: &AppHandle<R>,
    ) -> Result<std::path::PathBuf, String> {
        let writable_targets_path = initialize_targets_file_storage(app_handle)?;
        self.bootstrap_from_json_mirror_if_needed()?;
        sync_local_machine_config(self)?;
        let latest_content = self.load_targets_content()?;
        write_targets_file_content(&latest_content)?;
        Ok(writable_targets_path)
    }

    fn prune_inventory_snapshots(&self, targets_file: &RawTargetsFile) -> Result<(), String> {
        let keep_machine_ids = targets_file
            .machines
            .iter()
            .map(|machine| machine.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let connection = self.sqlite.connect()?;
        let mut statement = connection
            .prepare("SELECT machine_id FROM machine_inventory_snapshots")
            .map_err(|error| format!("Failed to prepare machine snapshot prune query: {}", error))?;
        let machine_ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Failed to load machine snapshot ids: {}", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to decode machine snapshot ids: {}", error))?;

        for machine_id in machine_ids {
            if keep_machine_ids.contains(machine_id.as_str()) {
                continue;
            }

            connection
                .execute(
                    "DELETE FROM machine_inventory_snapshots WHERE machine_id = ?1",
                    [&machine_id],
                )
                .map_err(|error| {
                    format!(
                        "Failed to prune stale machine inventory snapshot {}: {}",
                        machine_id, error
                    )
                })?;
        }

        Ok(())
    }
}

impl MachineRepository for SqliteMachineRepository {
    fn load_targets_file(&self) -> Result<RawTargetsFile, String> {
        let content = self.load_targets_content()?;
        serde_json::from_str::<RawTargetsFile>(&content)
            .map_err(|error| format!("Failed to parse {}: {}", targets_file_path().display(), error))
    }

    fn save_targets_file(&self, targets_file: &RawTargetsFile) -> Result<(), String> {
        let content = serde_json::to_string_pretty(targets_file)
            .map_err(|error| format!("Failed to serialize updated config: {}", error))?;
        self.prune_inventory_snapshots(targets_file)?;
        self.write_stored_content(&content)
    }

    fn load_targets_content(&self) -> Result<String, String> {
        if let Some(content) = self.read_stored_content()? {
            return sanitize_targets_json_content(&content);
        }

        let content = read_targets_file_content()?;
        let sanitized = sanitize_targets_json_content(&content)?;
        self.write_stored_content(&sanitized)?;
        Ok(sanitized)
    }

    fn save_targets_content(&self, content: &str) -> Result<(), String> {
        let sanitized = sanitize_targets_json_content(content)?;
        let parsed: RawTargetsFile = serde_json::from_str(&sanitized)
            .map_err(|error| format!("Invalid config JSON: {}", error))?;
        self.save_targets_file(&parsed)
    }

    fn load_inventory_snapshot(&self, machine_id: &str) -> Result<Option<Vec<DiscoveredContainer>>, String> {
        let connection = self.sqlite.connect()?;
        let mut statement = connection
            .prepare(
                "SELECT content
                 FROM machine_inventory_snapshots
                 WHERE machine_id = ?1",
            )
            .map_err(|error| format!("Failed to prepare machine snapshot query: {}", error))?;
        let mut rows = statement
            .query([machine_id])
            .map_err(|error| format!("Failed to query machine inventory snapshot: {}", error))?;

        let Some(row) = rows
            .next()
            .map_err(|error| format!("Failed to read machine inventory snapshot row: {}", error))?
        else {
            return Ok(None);
        };

        let content: String = row
            .get(0)
            .map_err(|error| format!("Failed to decode machine inventory snapshot: {}", error))?;
        let containers = serde_json::from_str::<Vec<DiscoveredContainer>>(&content)
            .map_err(|error| format!("Failed to parse machine inventory snapshot: {}", error))?;
        Ok(Some(containers))
    }

    fn save_inventory_snapshot(
        &self,
        machine_id: &str,
        containers: &[DiscoveredContainer],
    ) -> Result<(), String> {
        let content = serde_json::to_string(containers)
            .map_err(|error| format!("Failed to serialize machine inventory snapshot: {}", error))?;
        let connection = self.sqlite.connect()?;
        connection
            .execute(
                "INSERT INTO machine_inventory_snapshots (machine_id, content, scanned_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(machine_id) DO UPDATE SET
                   content = excluded.content,
                   scanned_at_ms = excluded.scanned_at_ms",
                (machine_id, content.as_str(), now_ms()),
            )
            .map_err(|error| format!("Failed to persist machine inventory snapshot: {}", error))?;
        clear_targets_cache();
        Ok(())
    }
}

impl SqliteMachineRepository {
    fn prune_inventory_snapshots(&self, targets_file: &RawTargetsFile) -> Result<(), String> {
        let keep_machine_ids = targets_file
            .machines
            .iter()
            .map(|machine| machine.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let connection = self.sqlite.connect()?;
        let mut statement = connection
            .prepare("SELECT machine_id FROM machine_inventory_snapshots")
            .map_err(|error| format!("Failed to prepare machine snapshot prune query: {}", error))?;
        let machine_ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Failed to load machine snapshot ids: {}", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Failed to decode machine snapshot ids: {}", error))?;

        for machine_id in machine_ids {
            if keep_machine_ids.contains(machine_id.as_str()) {
                continue;
            }

            connection
                .execute(
                    "DELETE FROM machine_inventory_snapshots WHERE machine_id = ?1",
                    [&machine_id],
                )
                .map_err(|error| {
                    format!(
                        "Failed to prune stale machine inventory snapshot {}: {}",
                        machine_id, error
                    )
                })?;
        }

        Ok(())
    }
}

pub(crate) fn initialize_targets_file_path<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> Result<std::path::PathBuf, String> {
    default_repository()?.initialize_targets_file_path(app_handle)
}

pub(super) fn read_raw_targets_file() -> Result<RawTargetsFile, String> {
    default_repository()?.load_targets_file()
}

pub(super) fn save_raw_targets_file(targets_file: &RawTargetsFile) -> Result<(), String> {
    default_repository()?.save_targets_file(targets_file)
}

pub(super) fn read_machine_inventory_snapshot(
    machine_id: &str,
) -> Result<Option<Vec<DiscoveredContainer>>, String> {
    default_repository()?.load_inventory_snapshot(machine_id)
}

pub(super) fn save_machine_inventory_snapshot(
    machine_id: &str,
    containers: &[DiscoveredContainer],
) -> Result<(), String> {
    default_repository()?.save_inventory_snapshot(machine_id, containers)
}

pub(super) fn read_targets_config_document() -> Result<String, String> {
    default_repository()?.load_targets_content()
}

pub(super) fn save_targets_config_document(content: &str) -> Result<(), String> {
    default_repository()?.save_targets_content(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_targets_file() -> RawTargetsFile {
        RawTargetsFile {
            selected_target_id: Some(String::from("local::host")),
            default_profile: Some(String::from("ark-workbench")),
            profiles: std::collections::HashMap::new(),
            machines: vec![],
        }
    }

    #[test]
    fn local_sync_preserves_user_managed_fields() {
        let raw_targets_file = fixture_targets_file();
        let existing_machine = RemoteContainerServer {
            id: String::from("local"),
            label: String::from("My Local"),
            transport: MachineTransport::Ssh,
            host: String::from("127.0.0.1"),
            port: 2222,
            description: String::from("Customized local provider"),
            os: detect_local_os(),
            manager_type: ContainerManagerType::Orbstack,
            discovery_mode: ContainerDiscoveryMode::Manual,
            discovery_command: String::from("custom-scan"),
            ssh_user: String::from("davidxu"),
            container_ssh_user: String::from("ubuntu"),
            identity_files: vec![String::from("~/.ssh/custom")],
            ssh_credential_id: String::from("ssh-key"),
            jump_chain: vec![],
            known_hosts_path: String::from("~/.ssh/custom_known_hosts"),
            strict_host_key_checking: false,
            container_name_suffix: String::new(),
            include_stopped: true,
            target_label_template: String::new(),
            target_host_template: String::from("{machineHost}"),
            target_description_template: String::new(),
            host_terminal_startup_command: String::from("exec /opt/homebrew/bin/fish -l"),
            terminal_command_template: String::from("ssh {sshOptions} {sshDestination}"),
            notes: vec![String::from("hello")],
            workspace: ContainerWorkspace {
                profile: String::from("custom-profile"),
                default_tool: String::from("services"),
                project_path: String::from("~/repos/demo"),
                files_root: String::from("~/repos/demo"),
                primary_web_service_id: String::from("dev"),
                tmux_command_service_id: String::from("tmux"),
            },
            services: vec![],
            surfaces: vec![],
            containers: vec![],
        };

        let merged = build_local_machine_config(&raw_targets_file, Some(&existing_machine));

        assert_eq!(merged.id, "local");
        assert_eq!(merged.host, "127.0.0.1");
        assert_eq!(merged.transport, MachineTransport::Ssh);
        assert_eq!(merged.label, "My Local");
        assert_eq!(merged.port, 2222);
        assert_eq!(merged.manager_type, ContainerManagerType::Orbstack);
        assert_eq!(merged.discovery_mode, ContainerDiscoveryMode::Manual);
        assert_eq!(
            merged.host_terminal_startup_command,
            "exec /opt/homebrew/bin/fish -l"
        );
        assert_eq!(merged.workspace.profile, "custom-profile");
        assert_eq!(merged.ssh_credential_id, "ssh-key");
    }
}
