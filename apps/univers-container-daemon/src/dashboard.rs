use crate::container::ContainerRuntimeInfo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io,
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::task;
use univers_daemon_core::agent::{event::SessionSnapshot, state::AgentState};

const DEFAULT_PROJECT_PATH: &str = "~/repos";
const DEFAULT_HTTP_PROBE_TIMEOUT_SECS: u64 = 1;
const DEFAULT_TCP_PROBE_TIMEOUT_SECS: u64 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardRequest {
    #[serde(default)]
    pub(crate) project_path: Option<String>,
    #[serde(default)]
    pub(crate) declared_services: Vec<DashboardDeclaredService>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardDeclaredService {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) probe_type: DashboardProbeType,
    pub(crate) host: String,
    pub(crate) port: u16,
    #[serde(default)]
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) url: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DashboardProbeType {
    Http,
    Tcp,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerDashboard {
    pub(crate) project: DashboardProjectInfo,
    pub(crate) runtime: ContainerRuntimeInfo,
    pub(crate) services: Vec<DashboardServiceInfo>,
    pub(crate) agent: DashboardAgentInfo,
    pub(crate) tmux: DashboardTmuxInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardProjectInfo {
    pub(crate) project_path: String,
    pub(crate) repo_found: bool,
    pub(crate) branch: Option<String>,
    pub(crate) is_dirty: bool,
    pub(crate) changed_files: u64,
    pub(crate) head_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardServiceInfo {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) detail: String,
    pub(crate) url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardAgentInfo {
    pub(crate) active_agent: String,
    pub(crate) source: String,
    pub(crate) last_activity: Option<String>,
    pub(crate) latest_report: Option<String>,
    pub(crate) latest_report_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardTmuxSessionInfo {
    pub(crate) server: String,
    pub(crate) name: String,
    pub(crate) windows: u64,
    pub(crate) attached: bool,
    pub(crate) active_command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardTmuxInfo {
    pub(crate) installed: bool,
    pub(crate) server_running: bool,
    pub(crate) session_count: u64,
    pub(crate) attached_count: u64,
    pub(crate) active_session: Option<String>,
    pub(crate) active_command: Option<String>,
    pub(crate) sessions: Vec<DashboardTmuxSessionInfo>,
}

#[derive(Debug, Clone)]
struct LatestWorkspaceEntry {
    relative_path: String,
    modified_at: SystemTime,
}

#[derive(Debug, Clone)]
struct AgentWorkspaceHints {
    latest_marker: Option<(String, String, SystemTime)>,
    latest_report: Option<LatestWorkspaceEntry>,
}

pub(crate) async fn collect_dashboard(
    request: DashboardRequest,
    agent_state: Arc<AgentState>,
) -> anyhow::Result<ContainerDashboard> {
    let project_path = resolve_project_path(request.project_path.as_deref());
    let project_path_for_project = project_path.clone();
    let project_path_for_tmux = project_path.clone();
    let project_path_for_agent = project_path.clone();
    let declared_services = request.declared_services;

    let project_task = task::spawn_blocking(move || collect_project_info(&project_path_for_project));
    let runtime_task = task::spawn_blocking(ContainerRuntimeInfo::collect);
    let tmux_task = task::spawn_blocking(move || collect_tmux_info(&project_path_for_tmux));
    let hints_task =
        task::spawn_blocking(move || collect_agent_workspace_hints(&project_path_for_agent));

    let sessions = agent_state.list_sessions(true).await;

    let project = project_task.await??;
    let runtime = runtime_task.await?;
    let tmux = tmux_task.await??;
    let hints = hints_task.await??;
    let services = collect_service_info(&project.project_path, declared_services).await;
    let agent = collect_agent_info(&project.project_path, &tmux, &sessions, hints);

    Ok(ContainerDashboard {
        project,
        runtime,
        services,
        agent,
        tmux,
    })
}

fn resolve_project_path(requested: Option<&str>) -> PathBuf {
    let raw = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_PROJECT_PATH);
    expand_home_path(raw)
}

fn expand_home_path(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir();
    }
    if let Some(suffix) = path.strip_prefix("~/") {
        return home_dir().join(suffix);
    }
    PathBuf::from(path)
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

fn collect_project_info(project_path: &Path) -> anyhow::Result<DashboardProjectInfo> {
    let repo_found = project_path.is_dir();
    let git_dir = project_path.join(".git");

    let (branch, is_dirty, changed_files, head_summary) = if repo_found && git_dir.is_dir() {
        let branch = run_git(project_path, &["rev-parse", "--abbrev-ref", "HEAD"]);
        let (is_dirty, changed_files) = git_dirty_state(project_path);
        let head_summary = git_head_summary(project_path);
        (branch, is_dirty, changed_files, head_summary)
    } else {
        (None, false, 0, None)
    };

    Ok(DashboardProjectInfo {
        project_path: project_path.display().to_string(),
        repo_found,
        branch,
        is_dirty,
        changed_files,
        head_summary,
    })
}

fn git_dirty_state(project_path: &Path) -> (bool, u64) {
    let Some(stdout) = run_git(project_path, &["status", "--porcelain"]) else {
        return (false, 0);
    };
    let changed_files = stdout.lines().filter(|line| !line.trim().is_empty()).count() as u64;
    (changed_files > 0, changed_files)
}

fn git_head_summary(project_path: &Path) -> Option<String> {
    run_git(project_path, &["log", "-1", "--pretty=%h %s"])
}

fn run_git(project_path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!stdout.is_empty()).then_some(stdout)
}

async fn collect_service_info(
    project_path: &str,
    declared_services: Vec<DashboardDeclaredService>,
) -> Vec<DashboardServiceInfo> {
    let services = if declared_services.is_empty() {
        default_dashboard_services(project_path)
    } else {
        declared_services
    };

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DEFAULT_HTTP_PROBE_TIMEOUT_SECS))
        .build()
        .ok();
    let mut service_infos = Vec::new();

    for service in services {
        if service.port == 0 {
            continue;
        }
        service_infos.push(probe_service(service, http_client.as_ref()).await);
    }

    service_infos
}

fn default_dashboard_services(project_path: &str) -> Vec<DashboardDeclaredService> {
    let env_path = Path::new(project_path).join(".env");
    let env_values = read_env_file(&env_path);
    let database_url = env_values
        .get("DATABASE_URL")
        .map(String::as_str)
        .unwrap_or("memory")
        .trim()
        .to_string();

    let mut services = Vec::new();
    if database_url.eq_ignore_ascii_case("memory") {
        services.push(DashboardDeclaredService {
            id: String::from("surrealdb"),
            label: String::from("SurrealDB"),
            probe_type: DashboardProbeType::Tcp,
            host: String::from("127.0.0.1"),
            port: 0,
            path: String::new(),
            url: String::new(),
        });
    } else if let Ok(parsed) = url::Url::parse(&database_url) {
        services.push(DashboardDeclaredService {
            id: String::from("surrealdb"),
            label: String::from("SurrealDB"),
            probe_type: DashboardProbeType::Tcp,
            host: parsed.host_str().unwrap_or("127.0.0.1").to_string(),
            port: parsed.port().unwrap_or(8000),
            path: String::new(),
            url: format!(
                "http://{}:{}",
                parsed.host_str().unwrap_or("127.0.0.1"),
                parsed.port().unwrap_or(8000)
            ),
        });
    }

    let server_port = env_values
        .get("SERVER_PORT")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3003);
    let agents_port = env_values
        .get("AGENTS_PORT")
        .or_else(|| env_values.get("COPILOT_PORT"))
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(server_port.saturating_add(1));

    services.extend([
        DashboardDeclaredService {
            id: String::from("workbench-api"),
            label: String::from("Workbench API"),
            probe_type: DashboardProbeType::Http,
            host: String::from("127.0.0.1"),
            port: server_port,
            path: String::from("/health"),
            url: format!("http://127.0.0.1:{server_port}"),
        },
        DashboardDeclaredService {
            id: String::from("agents-api"),
            label: String::from("Agents API"),
            probe_type: DashboardProbeType::Http,
            host: String::from("127.0.0.1"),
            port: agents_port,
            path: String::from("/health"),
            url: format!("http://127.0.0.1:{agents_port}"),
        },
        DashboardDeclaredService {
            id: String::from("web-ui"),
            label: String::from("Web UI"),
            probe_type: DashboardProbeType::Http,
            host: String::from("127.0.0.1"),
            port: 3432,
            path: String::new(),
            url: String::from("http://127.0.0.1:3432"),
        },
    ]);

    services
}

fn read_env_file(path: &Path) -> HashMap<String, String> {
    let Ok(content) = fs::read_to_string(path) else {
        return HashMap::new();
    };

    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let (key, value) = trimmed.split_once('=')?;
            Some((
                key.trim().to_string(),
                value.trim().trim_matches('"').trim_matches('\'').to_string(),
            ))
        })
        .collect()
}

async fn probe_service(
    service: DashboardDeclaredService,
    http_client: Option<&reqwest::Client>,
) -> DashboardServiceInfo {
    if service.port == 0 && service.id == "surrealdb" {
        return DashboardServiceInfo {
            id: service.id,
            label: service.label,
            status: String::from("embedded"),
            detail: String::from("Embedded memory database"),
            url: None,
        };
    }

    let host = if service.host.trim().is_empty() {
        String::from("127.0.0.1")
    } else {
        service.host.clone()
    };
    let tcp_ready = tcp_reachable(&host, service.port, Duration::from_secs(DEFAULT_TCP_PROBE_TIMEOUT_SECS));
    let status = if tcp_ready { "running" } else { "down" }.to_string();
    let path_suffix = if service.path.is_empty() {
        String::new()
    } else {
        service.path.clone()
    };
    let fallback_url = if service.url.trim().is_empty() {
        match service.probe_type {
            DashboardProbeType::Http => Some(format!("http://{}:{}", host, service.port)),
            DashboardProbeType::Tcp => Some(format!("tcp://{}:{}", host, service.port)),
        }
    } else {
        Some(service.url.clone())
    };

    match service.probe_type {
        DashboardProbeType::Tcp => DashboardServiceInfo {
            id: service.id,
            label: service.label,
            status,
            detail: format!("tcp://{}:{}", host, service.port),
            url: fallback_url,
        },
        DashboardProbeType::Http => {
            let mut detail = format!("{}:{}{}", host, service.port, path_suffix);
            if tcp_ready {
                if let Some(client) = http_client {
                    let probe_url = build_http_probe_url(&host, service.port, &service.path);
                    if let Ok(response) = client.get(&probe_url).send().await {
                        detail = format!(
                            "HTTP {} · {}:{}{}",
                            response.status().as_u16(),
                            host,
                            service.port,
                            path_suffix
                        );
                    } else {
                        detail = format!("TCP ready · {}:{}", host, service.port);
                    }
                } else {
                    detail = format!("TCP ready · {}:{}", host, service.port);
                }
            }

            DashboardServiceInfo {
                id: service.id,
                label: service.label,
                status,
                detail,
                url: fallback_url,
            }
        }
    }
}

fn build_http_probe_url(host: &str, port: u16, path: &str) -> String {
    if path.is_empty() {
        return format!("http://{}:{}", host, port);
    }

    if path.starts_with('/') {
        format!("http://{}:{}{}", host, port, path)
    } else {
        format!("http://{}:{}/{}", host, port, path)
    }
}

fn tcp_reachable(host: &str, port: u16, timeout: Duration) -> bool {
    let Ok(addresses) = format!("{host}:{port}").to_socket_addrs() else {
        return false;
    };

    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, timeout).is_ok())
}

fn collect_tmux_info(project_path: &Path) -> anyhow::Result<DashboardTmuxInfo> {
    if !command_exists("tmux") {
        return Ok(DashboardTmuxInfo {
            installed: false,
            server_running: false,
            session_count: 0,
            attached_count: 0,
            active_session: None,
            active_command: None,
            sessions: Vec::new(),
        });
    }

    let default_sessions = collect_tmux_sessions(project_path, "default", &[])?;
    let container_sessions = collect_tmux_sessions(project_path, "container", &["-L", "container"])?;
    let mut sessions = default_sessions;
    sessions.extend(container_sessions);

    let attached_count = sessions.iter().filter(|session| session.attached).count() as u64;
    let preferred_session = sessions
        .iter()
        .find(|session| session.attached && session.server == "default")
        .or_else(|| sessions.iter().find(|session| session.attached))
        .or_else(|| sessions.first())
        .cloned();

    Ok(DashboardTmuxInfo {
        installed: true,
        server_running: !sessions.is_empty(),
        session_count: sessions.len() as u64,
        attached_count,
        active_session: preferred_session
            .as_ref()
            .map(|session| format!("{} · {}", session.server, session.name)),
        active_command: preferred_session.and_then(|session| session.active_command),
        sessions,
    })
}

fn collect_tmux_sessions(
    project_path: &Path,
    server_label: &str,
    extra_args: &[&str],
) -> anyhow::Result<Vec<DashboardTmuxSessionInfo>> {
    let current_dir = if project_path.is_dir() {
        project_path
    } else {
        Path::new("/")
    };
    let session_output = Command::new("tmux")
        .args(extra_args)
        .args([
            "list-sessions",
            "-F",
            "#{session_name}\t#{session_windows}\t#{?session_attached,1,0}",
        ])
        .current_dir(current_dir)
        .output()?;
    if !session_output.status.success() {
        return Ok(Vec::new());
    }

    let pane_output = Command::new("tmux")
        .args(extra_args)
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{session_name}\t#{?pane_active,1,0}\t#{pane_current_command}",
        ])
        .current_dir(current_dir)
        .output()?;

    let pane_commands = if pane_output.status.success() {
        parse_tmux_pane_commands(&String::from_utf8_lossy(&pane_output.stdout))
    } else {
        HashMap::new()
    };

    Ok(String::from_utf8_lossy(&session_output.stdout)
        .lines()
        .filter_map(|line| parse_tmux_session_line(server_label, line, &pane_commands))
        .collect())
}

fn parse_tmux_pane_commands(output: &str) -> HashMap<String, String> {
    let mut pane_by_session = HashMap::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }

        let session_name = parts[0].trim();
        let is_active = parts[1].trim() == "1";
        let current_command = parts[2].trim();
        if session_name.is_empty() || current_command.is_empty() {
            continue;
        }

        if is_active || !pane_by_session.contains_key(session_name) {
            pane_by_session.insert(session_name.to_string(), current_command.to_string());
        }
    }

    pane_by_session
}

fn parse_tmux_session_line(
    server_label: &str,
    line: &str,
    pane_commands: &HashMap<String, String>,
) -> Option<DashboardTmuxSessionInfo> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 3 {
        return None;
    }

    let name = parts[0].trim();
    if name.is_empty() {
        return None;
    }

    Some(DashboardTmuxSessionInfo {
        server: server_label.to_string(),
        name: name.to_string(),
        windows: parts[1].trim().parse::<u64>().ok().unwrap_or(0),
        attached: parts[2].trim() == "1",
        active_command: pane_commands.get(name).cloned(),
    })
}

fn command_exists(command: &str) -> bool {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|path| path.join(command))
        .any(|candidate| candidate.exists())
}

fn collect_agent_workspace_hints(project_path: &Path) -> anyhow::Result<AgentWorkspaceHints> {
    let latest_marker = collect_latest_agent_marker(project_path)?;
    let latest_report = collect_latest_report(project_path)?;

    Ok(AgentWorkspaceHints {
        latest_marker,
        latest_report,
    })
}

fn collect_latest_agent_marker(
    project_path: &Path,
) -> anyhow::Result<Option<(String, String, SystemTime)>> {
    let markers = [
        (".claude", "claude"),
        (".codex", "codex"),
        (".agents", "agent"),
        (".agent", "agent"),
    ];
    let mut latest = None;

    for (relative_path, agent_name) in markers {
        let marker_path = project_path.join(relative_path);
        let Ok(metadata) = fs::metadata(&marker_path) else {
            continue;
        };
        let Ok(modified_at) = metadata.modified() else {
            continue;
        };
        let should_replace = latest
            .as_ref()
            .map(|(_, _, current)| modified_at > *current)
            .unwrap_or(true);
        if should_replace {
            latest = Some((relative_path.to_string(), agent_name.to_string(), modified_at));
        }
    }

    Ok(latest)
}

fn collect_latest_report(project_path: &Path) -> anyhow::Result<Option<LatestWorkspaceEntry>> {
    if !project_path.is_dir() {
        return Ok(None);
    }

    let mut stack = vec![project_path.to_path_buf()];
    let mut latest = None;

    while let Some(path) = stack.pop() {
        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => continue,
            Err(error) => return Err(error.into()),
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            if entry_path.is_dir() {
                if matches!(
                    file_name.as_ref(),
                    ".git" | "node_modules" | "dist" | "build" | "target"
                ) {
                    continue;
                }
                stack.push(entry_path);
                continue;
            }

            let upper_name = file_name.to_ascii_uppercase();
            let lower_name = file_name.to_ascii_lowercase();
            if !upper_name.contains("REPORT")
                || !matches!(
                    lower_name.as_str(),
                    name if name.ends_with(".md")
                        || name.ends_with(".markdown")
                        || name.ends_with(".txt")
                )
            {
                continue;
            }

            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified_at) = metadata.modified() else {
                continue;
            };
            let relative_path = entry_path
                .strip_prefix(project_path)
                .unwrap_or(&entry_path)
                .display()
                .to_string();
            let should_replace = latest
                .as_ref()
                .map(|current: &LatestWorkspaceEntry| modified_at > current.modified_at)
                .unwrap_or(true);
            if should_replace {
                latest = Some(LatestWorkspaceEntry {
                    relative_path,
                    modified_at,
                });
            }
        }
    }

    Ok(latest)
}

fn collect_agent_info(
    project_path: &str,
    tmux: &DashboardTmuxInfo,
    sessions: &[SessionSnapshot],
    hints: AgentWorkspaceHints,
) -> DashboardAgentInfo {
    let scoped_sessions = select_scoped_sessions(project_path, sessions);
    let latest_active_session = scoped_sessions
        .iter()
        .find(|session| session.status != "ended")
        .cloned();
    let latest_session = scoped_sessions.first().cloned();
    let tmux_command = tmux
        .active_command
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    let latest_report = hints.latest_report.as_ref().map(|entry| entry.relative_path.clone());
    let latest_report_updated_at = hints
        .latest_report
        .as_ref()
        .map(|entry| format_system_time(entry.modified_at));

    if tmux_command.contains("claude") && tmux_command.contains("codex") {
        return DashboardAgentInfo {
            active_agent: String::from("mixed"),
            source: String::from("tmux"),
            last_activity: tmux
                .active_session
                .as_ref()
                .zip(tmux.active_command.as_ref())
                .map(|(session, command)| format!("tmux · {} · {}", session, command)),
            latest_report,
            latest_report_updated_at,
        };
    }

    if tmux_command.contains("claude") || tmux_command.contains("codex") {
        return DashboardAgentInfo {
            active_agent: if tmux_command.contains("claude") {
                String::from("claude")
            } else {
                String::from("codex")
            },
            source: String::from("tmux"),
            last_activity: tmux
                .active_session
                .as_ref()
                .zip(tmux.active_command.as_ref())
                .map(|(session, command)| format!("tmux · {} · {}", session, command)),
            latest_report,
            latest_report_updated_at,
        };
    }

    if let Some(session) = latest_active_session.or(latest_session.clone()) {
        let inferred_agent = hints
            .latest_marker
            .as_ref()
            .map(|(_, agent_name, _)| agent_name.clone())
            .unwrap_or_else(|| String::from("agent"));
        return DashboardAgentInfo {
            active_agent: inferred_agent,
            source: String::from("daemon"),
            last_activity: Some(format!(
                "session · {} · {}",
                session.last_event.as_deref().unwrap_or("activity"),
                session.updated_at
            )),
            latest_report,
            latest_report_updated_at,
        };
    }

    if let Some((marker_path, agent_name, modified_at)) = hints.latest_marker {
        return DashboardAgentInfo {
            active_agent: agent_name,
            source: String::from("workspace"),
            last_activity: Some(format!(
                "{} · {}",
                marker_path,
                format_system_time(modified_at)
            )),
            latest_report,
            latest_report_updated_at,
        };
    }

    if latest_report.is_some() {
        return DashboardAgentInfo {
            active_agent: String::from("agent"),
            source: String::from("report"),
            last_activity: latest_report.clone(),
            latest_report,
            latest_report_updated_at,
        };
    }

    DashboardAgentInfo {
        active_agent: String::from("unknown"),
        source: String::from("none"),
        last_activity: None,
        latest_report: None,
        latest_report_updated_at: None,
    }
}

fn select_scoped_sessions(project_path: &str, sessions: &[SessionSnapshot]) -> Vec<SessionSnapshot> {
    let normalized_project = project_path.trim_end_matches('/').to_string();
    let matching_sessions: Vec<SessionSnapshot> = sessions
        .iter()
        .filter(|session| {
            let cwd = session.cwd.trim_end_matches('/');
            cwd == normalized_project || cwd.starts_with(&format!("{}/", normalized_project))
        })
        .cloned()
        .collect();

    if matching_sessions.is_empty() {
        sessions.to_vec()
    } else {
        matching_sessions
    }
}

fn format_system_time(value: SystemTime) -> String {
    let timestamp: DateTime<Utc> = value.into();
    timestamp.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        build_http_probe_url, expand_home_path, parse_tmux_pane_commands, parse_tmux_session_line,
        select_scoped_sessions, DashboardTmuxSessionInfo,
    };
    use univers_daemon_core::agent::event::SessionSnapshot;

    #[test]
    fn expands_home_paths() {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
        assert_eq!(expand_home_path("~"), std::path::PathBuf::from(&home));
        assert_eq!(
            expand_home_path("~/repos"),
            std::path::PathBuf::from(&home).join("repos")
        );
    }

    #[test]
    fn builds_http_probe_urls() {
        assert_eq!(
            build_http_probe_url("127.0.0.1", 3000, "/health"),
            "http://127.0.0.1:3000/health"
        );
        assert_eq!(
            build_http_probe_url("127.0.0.1", 3000, "health"),
            "http://127.0.0.1:3000/health"
        );
    }

    #[test]
    fn parses_tmux_sessions() {
        let panes = parse_tmux_pane_commands("dev\t0\tbash\ndev\t1\tclaude\n");
        let session = parse_tmux_session_line("default", "dev\t3\t1", &panes)
            .expect("expected session");
        assert_eq!(
            session,
            DashboardTmuxSessionInfo {
                server: String::from("default"),
                name: String::from("dev"),
                windows: 3,
                attached: true,
                active_command: Some(String::from("claude")),
            }
        );
    }

    #[test]
    fn scopes_sessions_by_project_path() {
        let sessions = vec![
            SessionSnapshot {
                session_id: String::from("1"),
                cwd: String::from("/workspace/app"),
                status: String::from("active"),
                last_event: Some(String::from("SessionStart")),
                last_tool: None,
                started_at: String::from("2026-03-12T00:00:00Z"),
                updated_at: String::from("2026-03-12T00:00:00Z"),
            },
            SessionSnapshot {
                session_id: String::from("2"),
                cwd: String::from("/other"),
                status: String::from("active"),
                last_event: Some(String::from("SessionStart")),
                last_tool: None,
                started_at: String::from("2026-03-12T00:00:00Z"),
                updated_at: String::from("2026-03-12T00:00:00Z"),
            },
        ];

        let scoped = select_scoped_sessions("/workspace/app", &sessions);
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].session_id, "1");
    }
}
