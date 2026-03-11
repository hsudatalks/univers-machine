use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::warn;

#[derive(Debug, Clone, Serialize)]
pub struct TmuxServiceStatus {
    pub name: String,
    pub description: String,
    pub category: String,
    pub running: bool,
    pub healthy: bool,
    pub sessions: Vec<String>,
    pub tmux_server: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct TmuxServiceManager {
    repo_root: PathBuf,
}

#[derive(Debug, Clone)]
struct TmuxServiceDefinition {
    name: String,
    description: String,
    category: String,
    source: String,
    tmux_server: Option<String>,
    sessions: Vec<String>,
    control: ServiceControl,
}

#[derive(Debug, Clone)]
enum ServiceControl {
    MachineViews {
        primary_session: String,
    },
    DevSession {
        session_name: String,
        tmux_server: String,
    },
}

#[derive(Debug, Deserialize)]
struct DevSessionsConfig {
    #[serde(default)]
    sessions: BTreeMap<String, DevSessionConfig>,
}

#[derive(Debug, Deserialize)]
struct DevSessionConfig {
    #[serde(default)]
    servers: BTreeMap<String, DevServerConfig>,
}

#[derive(Debug, Deserialize)]
struct DevServerConfig {
    description: Option<String>,
    host: Option<String>,
}

impl TmuxServiceManager {
    pub fn new() -> Self {
        Self {
            repo_root: repo_root(),
        }
    }

    pub async fn list_statuses(&self) -> Vec<TmuxServiceStatus> {
        self.service_definitions()
            .into_iter()
            .map(|definition| self.build_status(definition))
            .collect()
    }

    pub async fn start_service(&self, name: &str) -> Result<()> {
        let definition = self.find_service(name)?;
        self.run_control(definition, "start")
    }

    pub async fn stop_service(&self, name: &str) -> Result<()> {
        let definition = self.find_service(name)?;
        self.run_control(definition, "stop")
    }

    pub async fn restart_service(&self, name: &str) -> Result<()> {
        let definition = self.find_service(name)?;
        self.run_control(definition, "restart")
    }

    pub async fn capture_logs(&self, name: &str) -> Result<String> {
        let definition = self.find_service(name)?;
        match &definition.control {
            ServiceControl::MachineViews { primary_session } => {
                capture_tmux_logs("machine", primary_session)
            }
            ServiceControl::DevSession {
                session_name,
                tmux_server,
            } => capture_tmux_logs(tmux_server, session_name),
        }
    }

    fn find_service(&self, name: &str) -> Result<TmuxServiceDefinition> {
        self.service_definitions()
            .into_iter()
            .find(|service| service.name == name)
            .ok_or_else(|| anyhow!("Unknown tmux service '{name}'"))
    }

    fn build_status(&self, definition: TmuxServiceDefinition) -> TmuxServiceStatus {
        let (running, healthy) = match &definition.control {
            ServiceControl::MachineViews { primary_session } => {
                let running = tmux_session_exists("machine", primary_session);
                (running, running)
            }
            ServiceControl::DevSession {
                session_name,
                tmux_server,
            } => {
                let running = tmux_session_exists(tmux_server, session_name);
                (running, running)
            }
        };

        TmuxServiceStatus {
            name: definition.name,
            description: definition.description,
            category: definition.category,
            running,
            healthy,
            sessions: definition.sessions,
            tmux_server: definition.tmux_server,
            source: definition.source,
        }
    }

    fn run_control(&self, definition: TmuxServiceDefinition, action: &str) -> Result<()> {
        match definition.control {
            ServiceControl::MachineViews { .. } => self.run_bash_script(
                &self
                    .repo_root
                    .join(".claude/skills/machine-manage/scripts/machine-view-manager.sh"),
                &[action],
            ),
            ServiceControl::DevSession { session_name, .. } => self.run_bash_script(
                &self
                    .repo_root
                    .join(".claude/skills/dev-manage/scripts/dev-session-manager.sh"),
                &[&session_name, action],
            ),
        }
    }

    fn run_bash_script(&self, script_path: &Path, args: &[&str]) -> Result<()> {
        if !script_path.exists() {
            return Err(anyhow!("Script not found: {}", script_path.display()));
        }

        let output = Command::new("bash")
            .arg(script_path)
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .with_context(|| format!("Failed to execute {}", script_path.display()))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("{} exited with {}", script_path.display(), output.status)
        };
        Err(anyhow!(message))
    }

    fn service_definitions(&self) -> Vec<TmuxServiceDefinition> {
        let mut services = vec![
            machine_service(
                "machine-desktop-view",
                "Machine desktop aggregate view",
                "Attachs VM/container desktop panes into a local tmux workspace.",
            ),
            machine_service(
                "machine-mobile-view",
                "Machine mobile aggregate view",
                "Aggregates VM/container mobile panes for compact machine-level monitoring.",
            ),
            machine_service(
                "univers-machine-manage",
                "Machine manage control session",
                "Local machine management session created by machine-view-manager.",
            ),
        ];

        match self.load_dev_session_services() {
            Ok(mut dev_services) => services.append(&mut dev_services),
            Err(error) => warn!("Failed to load dev tmux services: {error}"),
        }

        services
    }

    fn load_dev_session_services(&self) -> Result<Vec<TmuxServiceDefinition>> {
        let config_path = self
            .resolve_first_existing(&["config/servers.yaml", "config/servers.yaml.example"])
            .ok_or_else(|| anyhow!("config/servers.yaml not found"))?;

        let raw = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;
        let config: DevSessionsConfig = serde_yaml::from_str(&raw)
            .with_context(|| format!("Failed to parse {}", config_path.display()))?;

        let services = config
            .sessions
            .into_iter()
            .map(|(session_name, session)| {
                let server_count = session.servers.len();
                let hosts = session
                    .servers
                    .values()
                    .filter_map(|server| server.host.clone())
                    .collect::<Vec<_>>();
                let primary_description = session
                    .servers
                    .values()
                    .find_map(|server| server.description.clone())
                    .unwrap_or_else(|| "Development tmux session".to_string());
                let host_suffix = if hosts.is_empty() {
                    String::new()
                } else {
                    format!(" Hosts: {}.", hosts.join(", "))
                };

                TmuxServiceDefinition {
                    name: session_name.clone(),
                    description: format!("{primary_description} ({server_count} windows){host_suffix}"),
                    category: String::from("dev-session"),
                    source: format!("{}::{}", config_path.display(), session_name),
                    tmux_server: Some(dev_tmux_server_name(&session_name)),
                    sessions: vec![session_name.clone()],
                    control: ServiceControl::DevSession {
                        tmux_server: dev_tmux_server_name(&session_name),
                        session_name,
                    },
                }
            })
            .collect();

        Ok(services)
    }

    fn resolve_first_existing(&self, candidates: &[&str]) -> Option<PathBuf> {
        candidates
            .iter()
            .map(|candidate| self.repo_root.join(candidate))
            .find(|path| path.exists())
    }
}

fn machine_service(name: &str, _label: &str, description: &str) -> TmuxServiceDefinition {
    TmuxServiceDefinition {
        name: name.to_string(),
        description: description.to_string(),
        category: String::from("machine-view"),
        source: String::from(".claude/skills/machine-manage/scripts/machine-view-manager.sh"),
        tmux_server: Some(String::from("machine")),
        sessions: vec![name.to_string()],
        control: ServiceControl::MachineViews {
            primary_session: name.to_string(),
        },
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

fn dev_tmux_server_name(session_name: &str) -> String {
    session_name
        .strip_suffix("-dev")
        .unwrap_or(session_name)
        .to_string()
}

fn tmux_session_exists(server: &str, session: &str) -> bool {
    Command::new("tmux")
        .args(["-L", server, "has-session", "-t", session])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn capture_tmux_logs(server: &str, session: &str) -> Result<String> {
    if !tmux_session_exists(server, session) {
        return Err(anyhow!(
            "tmux session '{session}' is not running on server '{server}'"
        ));
    }

    let target = format!("{session}:0");
    let output = Command::new("tmux")
        .args(["-L", server, "capture-pane", "-t", &target, "-p", "-S", "-200"])
        .output()
        .with_context(|| format!("Failed to capture logs for tmux session '{session}'"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!(if stderr.is_empty() {
            format!("tmux capture-pane failed for '{session}'")
        } else {
            stderr
        }));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::dev_tmux_server_name;

    #[test]
    fn strips_dev_suffix_for_tmux_server() {
        assert_eq!(dev_tmux_server_name("ark-dev"), "ark");
        assert_eq!(dev_tmux_server_name("sandbox"), "sandbox");
    }
}
