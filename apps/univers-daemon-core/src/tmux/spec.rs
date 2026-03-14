use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use tracing::warn;
pub use univers_ark_kernel::workspace::{
    WindowDefinition, WorkspaceDefinition, WorkspaceProfile, WorkspaceSpecRepository,
};
pub use univers_infra_workspace::{
    command_exists, container_tmux_server_name, container_tmux_working_directory,
    discover_servers_config_path, first_existing_directory, machine_tmux_server_name,
    machine_tmux_working_directory,
};

#[derive(Debug, Default)]
pub(crate) struct DefaultWorkspaceSpecRepository;

impl DefaultWorkspaceSpecRepository {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl WorkspaceSpecRepository for DefaultWorkspaceSpecRepository {
    fn list(&self, profile: WorkspaceProfile) -> Vec<WorkspaceDefinition> {
        workspace_definitions(profile)
    }
}

#[derive(Debug, Deserialize)]
struct DevSessionsConfig {
    #[serde(default)]
    sessions: BTreeMap<String, DevSessionConfig>,
}

#[derive(Debug, Deserialize)]
struct DevSessionConfig {
    #[serde(default)]
    ssh_options: Option<String>,
    #[serde(default)]
    disconnect_message: Option<String>,
    #[serde(default)]
    local_window: Option<DevLocalWindowConfig>,
    #[serde(default)]
    servers: BTreeMap<String, DevServerConfig>,
}

#[derive(Debug, Deserialize)]
struct DevServerConfig {
    description: Option<String>,
    host: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DevLocalWindowConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LxdContainerEntry {
    name: String,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MachineTargetKind {
    Lxd,
    OrbStack,
}

#[derive(Debug, Clone)]
struct MachineTarget {
    name: String,
    kind: MachineTargetKind,
}

pub(crate) fn workspace_definitions(profile: WorkspaceProfile) -> Vec<WorkspaceDefinition> {
    let mut workspaces = match profile {
        WorkspaceProfile::Machine => machine_native_workspaces(),
        WorkspaceProfile::Container => container_native_workspaces(),
    };

    if matches!(profile, WorkspaceProfile::Machine) {
        match load_dev_workspaces() {
            Ok(mut dev_workspaces) => workspaces.append(&mut dev_workspaces),
            Err(error) => warn!("Failed to load dev workspaces: {error}"),
        }
    }

    workspaces
}

pub(crate) fn container_native_workspaces() -> Vec<WorkspaceDefinition> {
    let working_directory = container_tmux_working_directory();
    let tmux_server = container_tmux_server_name();
    vec![
        native_workspace(
            "container-desktop-view",
            "Container Desktop Workspace",
            "workspace",
            "native::container-daemon",
            Some(tmux_server.as_str()),
            &working_directory,
            vec![
                window_spec(
                    "workbench",
                    "Workbench",
                    "service",
                    None,
                    Some("nodejs"),
                    vec![],
                ),
                window_spec("operation", "Operation", "shell", None, None, vec![]),
                window_spec("manager", "Manager", "shell", None, None, vec![]),
            ],
        ),
        native_workspace(
            "container-mobile-view",
            "Container Mobile Workspace",
            "workspace",
            "native::container-daemon",
            Some(tmux_server.as_str()),
            &working_directory,
            vec![
                window_spec(
                    "dev",
                    "Dev",
                    "agent",
                    None,
                    Some("claude-code"),
                    vec![String::from("coding"), String::from("terminal")],
                )
                .with_agent("claude-code-dev"),
                window_spec(
                    "service",
                    "Service",
                    "service",
                    None,
                    Some("nodejs"),
                    vec![],
                ),
                window_spec("ops", "Ops", "shell", None, None, vec![]),
                window_spec("manager", "Manager", "shell", None, None, vec![]),
            ],
        ),
    ]
}

fn load_dev_workspaces() -> Result<Vec<WorkspaceDefinition>> {
    let Some(config_path) = discover_servers_config_path() else {
        return Ok(Vec::new());
    };

    let raw = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let config: DevSessionsConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("Failed to parse {}", config_path.display()))?;

    let working_directory = machine_tmux_working_directory();
    let tmux_server = machine_tmux_server_name();
    Ok(config
        .sessions
        .into_iter()
        .map(|(session_id, session)| {
            let primary_description = session
                .servers
                .values()
                .find_map(|server| server.description.clone())
                .unwrap_or_else(|| "Development workspace".to_string());
            let host_suffix = session
                .servers
                .values()
                .filter_map(|server| server.host.clone())
                .collect::<Vec<_>>();
            let title = if host_suffix.is_empty() {
                primary_description
            } else {
                format!("{} · {}", primary_description, host_suffix.join(", "))
            };

            WorkspaceDefinition {
                id: session_id.clone(),
                title,
                category: String::from("workspace"),
                source: format!("native::workspace-config::{}", config_path.display()),
                tmux_server: Some(tmux_server.clone()),
                working_directory: working_directory.clone(),
                windows: build_dev_workspace_windows(
                    &session_id,
                    &session,
                    &tmux_server,
                    &working_directory,
                ),
            }
        })
        .collect())
}

fn machine_native_workspaces() -> Vec<WorkspaceDefinition> {
    let working_directory = machine_tmux_working_directory();
    let tmux_server = machine_tmux_server_name();
    let desktop_windows = machine_view_windows("container-desktop-view", &working_directory);
    let mobile_windows = machine_view_windows("container-mobile-view", &working_directory);

    vec![
        native_workspace(
            "machine-desktop-view",
            "Machine Desktop Workspace",
            "workspace",
            "native::machine-daemon",
            Some(tmux_server.as_str()),
            &working_directory,
            desktop_windows,
        ),
        native_workspace(
            "machine-mobile-view",
            "Machine Mobile Workspace",
            "workspace",
            "native::machine-daemon",
            Some(tmux_server.as_str()),
            &working_directory,
            mobile_windows,
        ),
        native_workspace(
            "univers-machine-manage",
            "Machine Manage Workspace",
            "workspace",
            "native::machine-daemon",
            Some(tmux_server.as_str()),
            &working_directory,
            vec![window_spec("manage", "Manage", "shell", None, None, vec![])],
        ),
    ]
}

fn machine_view_windows(
    container_session: &str,
    working_directory: &Path,
) -> Vec<WindowDefinition> {
    let mut windows = detect_machine_targets()
        .into_iter()
        .map(|target| {
            let title = target
                .name
                .strip_suffix("-dev")
                .unwrap_or(target.name.as_str())
                .to_string();
            window_spec(
                &title,
                &title,
                "remote",
                Some(build_machine_target_attach_command(
                    &target,
                    container_session,
                    working_directory,
                )),
                None,
                vec![],
            )
        })
        .collect::<Vec<_>>();

    windows.push(window_spec(
        "machine",
        "Machine",
        "shell",
        Some(format!(
            "unset TMUX && tmux -L {} attach -t univers-machine-manage 2>/dev/null || ({})",
            shell_single_quote(&machine_tmux_server_name()),
            login_shell_command(working_directory),
        )),
        None,
        vec![],
    ));
    windows
}

fn build_dev_workspace_windows(
    workspace_id: &str,
    session: &DevSessionConfig,
    tmux_server: &str,
    working_directory: &Path,
) -> Vec<WindowDefinition> {
    let ssh_options = session
        .ssh_options
        .as_deref()
        .unwrap_or("-o ConnectTimeout=10 -o ServerAliveInterval=60 -o ServerAliveCountMax=3");
    let disconnect_message = session
        .disconnect_message
        .as_deref()
        .unwrap_or("[Connection lost - Press Enter to reconnect]");

    let mut windows = session
        .servers
        .iter()
        .map(|(window_id, server)| {
            let host = server.host.as_deref().unwrap_or(window_id);
            window_spec(
                window_id,
                window_id,
                "agent",
                Some(build_dev_window_command(
                    host,
                    ssh_options,
                    disconnect_message,
                    working_directory,
                )),
                Some("claude-code"),
                vec![String::from("coding"), String::from("terminal")],
            )
            .with_agent("claude-code-dev")
        })
        .collect::<Vec<_>>();

    if session
        .local_window
        .as_ref()
        .map(|window| window.enabled)
        .unwrap_or(false)
    {
        let title = session
            .local_window
            .as_ref()
            .and_then(|window| window.display_name.as_deref())
            .unwrap_or("local");
        windows.push(window_spec(
            title,
            title,
            "shell",
            Some(build_local_dev_window_command(
                tmux_server,
                workspace_id,
                working_directory,
            )),
            None,
            vec![],
        ));
    }

    windows
}

fn native_workspace(
    id: &str,
    title: &str,
    category: &str,
    source: &str,
    tmux_server: Option<&str>,
    working_directory: &Path,
    windows: Vec<WindowDefinition>,
) -> WorkspaceDefinition {
    WorkspaceDefinition {
        id: id.to_string(),
        title: title.to_string(),
        category: category.to_string(),
        source: source.to_string(),
        tmux_server: tmux_server.map(str::to_string),
        working_directory: working_directory.to_path_buf(),
        windows,
    }
}

fn window_spec(
    id: &str,
    title: &str,
    kind: &str,
    command: Option<String>,
    app_id: Option<&str>,
    skills: Vec<String>,
) -> WindowDefinition {
    WindowDefinition {
        id: id.to_string(),
        title: title.to_string(),
        kind: kind.to_string(),
        agent_id: None,
        app_id: app_id.map(str::to_string),
        skills,
        command,
    }
}

fn detect_machine_targets() -> Vec<MachineTarget> {
    let mut targets = Vec::new();

    if command_exists("lxc") {
        targets.extend(detect_lxd_targets());
    }
    if command_exists("orb") {
        targets.extend(detect_orbstack_targets());
    }

    targets
}

fn detect_lxd_targets() -> Vec<MachineTarget> {
    let Ok(output) = Command::new("lxc").args(["list", "--format=json"]).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let Ok(entries) = serde_json::from_slice::<Vec<LxdContainerEntry>>(&output.stdout) else {
        return Vec::new();
    };

    entries
        .into_iter()
        .filter(|entry| entry.status == "Running")
        .filter(|entry| is_dev_target_name(&entry.name))
        .map(|entry| MachineTarget {
            name: entry.name,
            kind: MachineTargetKind::Lxd,
        })
        .collect()
}

fn detect_orbstack_targets() -> Vec<MachineTarget> {
    let Ok(output) = Command::new("orb").arg("list").output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?.trim();
            if !is_dev_target_name(name) || !line.contains(" running ") {
                return None;
            }
            Some(MachineTarget {
                name: name.to_string(),
                kind: MachineTargetKind::OrbStack,
            })
        })
        .collect()
}

fn is_dev_target_name(name: &str) -> bool {
    name.ends_with("-dev") && name != "ubuntu"
}

fn build_machine_target_attach_command(
    target: &MachineTarget,
    container_workspace: &str,
    working_directory: &Path,
) -> String {
    let fallback_shell = format!("({})", login_shell_command(working_directory));
    let inner_attach = format!(
        "unset TMUX && tmux -L {} attach -d -t {} 2>/dev/null || {}",
        shell_single_quote(&container_tmux_server_name()),
        shell_single_quote(container_workspace),
        fallback_shell
    );

    match target.kind {
        MachineTargetKind::Lxd => format!(
            "lxc exec {} -- su - ubuntu -c {}",
            shell_single_quote(&target.name),
            shell_single_quote(&format!("bash -lc {}", shell_single_quote(&inner_attach))),
        ),
        MachineTargetKind::OrbStack => format!(
            "ssh {}@orb {}",
            shell_single_quote(&target.name),
            shell_single_quote(&format!("bash -lc {}", shell_single_quote(&inner_attach))),
        ),
    }
}

fn build_dev_window_command(
    host: &str,
    ssh_options: &str,
    disconnect_message: &str,
    working_directory: &Path,
) -> String {
    let remote_attach = format!(
        "unset TMUX && tmux -L {} attach -t machine-mobile-view 2>/dev/null || ({})",
        shell_single_quote(&machine_tmux_server_name()),
        login_shell_command(working_directory)
    );
    format!(
        "while true; do ssh {} {} -t {} 2>&1; printf '\\n%s\\n' {}; read; done",
        ssh_options,
        shell_single_quote(host),
        shell_single_quote(&format!("bash -lc {}", shell_single_quote(&remote_attach))),
        escape_printf_literal(disconnect_message),
    )
}

fn build_local_dev_window_command(
    tmux_server: &str,
    workspace_id: &str,
    working_directory: &Path,
) -> String {
    let fallback_shell = format!("({})", login_shell_command(working_directory));
    format!(
        "tmux -L {} attach -t machine-mobile-view 2>/dev/null || {{ echo \"machine-mobile-view not found for {} ({})\"; {}; }}",
        shell_single_quote(&machine_tmux_server_name()),
        escape_double_quoted(workspace_id),
        escape_double_quoted(tmux_server),
        fallback_shell,
    )
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn escape_printf_literal(value: &str) -> String {
    shell_single_quote(&value.replace('%', "%%"))
}

fn escape_double_quoted(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn login_shell_command(working_directory: &Path) -> String {
    format!(
        "cd {} && exec bash -l",
        shell_single_quote(&working_directory.display().to_string())
    )
}
