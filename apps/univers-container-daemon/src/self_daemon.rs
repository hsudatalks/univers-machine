use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use univers_infra_systemd::{
    SystemdUserServiceManager, UserServiceLogs, UserServiceMutationResult, UserServiceStatus,
    UserServiceUnitFile,
};

static STARTED_AT: OnceLock<String> = OnceLock::new();
static LISTEN_PORT: OnceLock<u16> = OnceLock::new();

const UNIT_NAME: &str = "univers-container-daemon.service";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DaemonInfo {
    pub name: String,
    pub version: String,
    pub pid: u32,
    pub executable_path: String,
    pub started_at: String,
    pub listen_port: Option<u16>,
    pub service: DaemonServiceStatus,
}

pub(crate) type DaemonServiceStatus = UserServiceStatus;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallDaemonServiceRequest {
    pub binary_path: Option<String>,
    pub working_directory: Option<String>,
    pub port: Option<u16>,
    pub log_level: Option<String>,
    pub enable: Option<bool>,
    pub start: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateDaemonServiceRequest {
    pub binary_path: Option<String>,
    pub working_directory: Option<String>,
    pub port: Option<u16>,
    pub log_level: Option<String>,
    pub enable: Option<bool>,
    pub restart: Option<bool>,
}

pub(crate) type DaemonServiceMutationResult = UserServiceMutationResult;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DaemonServiceLogsQuery {
    pub lines: Option<usize>,
}

pub(crate) type DaemonServiceLogs = UserServiceLogs;

pub(crate) type DaemonServiceUnitFile = UserServiceUnitFile;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServiceUnitConfig {
    binary_path: PathBuf,
    working_directory: PathBuf,
    port: u16,
    log_level: String,
}

pub(crate) fn record_process_start(port: u16) {
    let _ = STARTED_AT.get_or_init(|| chrono::Utc::now().to_rfc3339());
    let _ = LISTEN_PORT.set(port);
}

pub(crate) fn collect_daemon_info() -> DaemonInfo {
    let executable_path = std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "unknown".into());

    DaemonInfo {
        name: "univers-container-daemon".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        pid: std::process::id(),
        executable_path,
        started_at: STARTED_AT
            .get()
            .cloned()
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        listen_port: LISTEN_PORT.get().copied(),
        service: collect_service_status(),
    }
}

pub(crate) fn collect_service_status() -> DaemonServiceStatus {
    systemd().status(UNIT_NAME)
}

pub(crate) async fn install_service(
    request: InstallDaemonServiceRequest,
) -> Result<DaemonServiceMutationResult> {
    tokio::task::spawn_blocking(move || install_service_blocking(request)).await?
}

pub(crate) async fn start_service() -> Result<DaemonServiceMutationResult> {
    tokio::task::spawn_blocking(|| run_service_action("start")).await?
}

pub(crate) async fn stop_service() -> Result<DaemonServiceMutationResult> {
    tokio::task::spawn_blocking(|| run_service_action("stop")).await?
}

pub(crate) async fn restart_service() -> Result<DaemonServiceMutationResult> {
    tokio::task::spawn_blocking(|| run_service_action("restart")).await?
}

pub(crate) async fn uninstall_service() -> Result<DaemonServiceMutationResult> {
    tokio::task::spawn_blocking(uninstall_service_blocking).await?
}

pub(crate) async fn update_service(
    request: UpdateDaemonServiceRequest,
) -> Result<DaemonServiceMutationResult> {
    tokio::task::spawn_blocking(move || update_service_blocking(request)).await?
}

pub(crate) async fn collect_service_logs(lines: usize) -> Result<DaemonServiceLogs> {
    tokio::task::spawn_blocking(move || collect_service_logs_blocking(lines)).await?
}

pub(crate) async fn collect_service_unit_file() -> Result<DaemonServiceUnitFile> {
    tokio::task::spawn_blocking(collect_service_unit_file_blocking).await?
}

fn install_service_blocking(
    request: InstallDaemonServiceRequest,
) -> Result<DaemonServiceMutationResult> {
    systemd().ensure_available()?;
    let config = ServiceUnitConfig {
        binary_path: resolve_binary_path(request.binary_path.as_deref())?,
        working_directory: resolve_working_directory(request.working_directory.as_deref())?,
        port: request
            .port
            .or_else(|| LISTEN_PORT.get().copied())
            .unwrap_or(3100),
        log_level: request.log_level.unwrap_or_else(|| "info".into()),
    };
    let enable = request.enable.unwrap_or(true);
    let start = request.start.unwrap_or(true);
    let unit = render_unit_file(
        &config.binary_path,
        &config.working_directory,
        config.port,
        &config.log_level,
    );
    let unit_path = systemd().write_unit_file(UNIT_NAME, &unit)?;
    if enable {
        systemd().set_enabled(UNIT_NAME, true)?;
    }
    if start {
        systemd().run_action(UNIT_NAME, "start")?;
    }

    Ok(DaemonServiceMutationResult {
        action: "install",
        message: format!("Installed {}", unit_path.display()),
        service: collect_service_status(),
    })
}

fn update_service_blocking(
    request: UpdateDaemonServiceRequest,
) -> Result<DaemonServiceMutationResult> {
    systemd().ensure_available()?;

    let unit = collect_service_unit_file_blocking()?;
    if !unit.installed {
        return Err(anyhow!("{} is not installed", UNIT_NAME));
    }

    let current_config = parse_unit_config(
        unit.content
            .as_deref()
            .ok_or_else(|| anyhow!("{} is missing content", UNIT_NAME))?,
    )?;
    let current_status = collect_service_status();
    let binary_path = request
        .binary_path
        .unwrap_or_else(|| current_config.binary_path.display().to_string());
    let working_directory = request
        .working_directory
        .unwrap_or_else(|| current_config.working_directory.display().to_string());

    let config = ServiceUnitConfig {
        binary_path: resolve_binary_path(Some(&binary_path))?,
        working_directory: resolve_working_directory(Some(&working_directory))?,
        port: request.port.unwrap_or(current_config.port),
        log_level: request.log_level.unwrap_or(current_config.log_level),
    };

    let unit_content = render_unit_file(
        &config.binary_path,
        &config.working_directory,
        config.port,
        &config.log_level,
    );
    let unit_file = systemd().write_unit_file(UNIT_NAME, &unit_content)?;

    match request.enable {
        Some(true) => systemd().set_enabled(UNIT_NAME, true)?,
        Some(false) if current_status.enabled => systemd().set_enabled(UNIT_NAME, false)?,
        _ => {}
    }

    let should_restart = request.restart.unwrap_or(current_status.active);
    if should_restart {
        if current_status.active {
            systemd().run_action(UNIT_NAME, "restart")?;
        } else {
            systemd().run_action(UNIT_NAME, "start")?;
        }
    }

    Ok(DaemonServiceMutationResult {
        action: "update",
        message: format!("Updated {}", unit_file.display()),
        service: collect_service_status(),
    })
}

fn uninstall_service_blocking() -> Result<DaemonServiceMutationResult> {
    systemd().uninstall(UNIT_NAME)
}

fn collect_service_logs_blocking(lines: usize) -> Result<DaemonServiceLogs> {
    systemd().logs(UNIT_NAME, lines)
}

fn collect_service_unit_file_blocking() -> Result<DaemonServiceUnitFile> {
    systemd().unit_file(UNIT_NAME)
}

fn run_service_action(action: &'static str) -> Result<DaemonServiceMutationResult> {
    systemd().run_action(UNIT_NAME, action)
}

fn resolve_binary_path(requested: Option<&str>) -> Result<PathBuf> {
    let path = requested
        .map(PathBuf::from)
        .unwrap_or(std::env::current_exe().context("Failed to resolve current executable")?);
    if !path.exists() {
        return Err(anyhow!("Binary path does not exist: {}", path.display()));
    }
    Ok(path)
}

fn resolve_working_directory(requested: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = requested {
        return Ok(PathBuf::from(path));
    }

    if let Some(home) = home_dir() {
        return Ok(home);
    }

    std::env::current_dir().context("Failed to determine working directory")
}

fn render_unit_file(
    binary_path: &Path,
    working_directory: &Path,
    port: u16,
    log_level: &str,
) -> String {
    let exec_start = format!("{} daemon --port {}", quote_systemd_arg(binary_path), port);
    let working_directory = quote_systemd_arg(working_directory);
    let log_level = log_level.replace('\n', " ");

    format!(
        "[Unit]\nDescription=Univers Container Daemon\nAfter=default.target\n\n[Service]\nType=simple\nExecStart={exec_start}\nWorkingDirectory={working_directory}\nRestart=always\nRestartSec=2\nEnvironment=RUST_LOG={log_level}\n\n[Install]\nWantedBy=default.target\n"
    )
}

fn parse_unit_config(content: &str) -> Result<ServiceUnitConfig> {
    let exec_start = unit_value(content, "ExecStart")
        .ok_or_else(|| anyhow!("Unit file is missing ExecStart"))?;
    let working_directory = unit_value(content, "WorkingDirectory")
        .ok_or_else(|| anyhow!("Unit file is missing WorkingDirectory"))?;
    let log_level = unit_value(content, "Environment")
        .and_then(|value| value.strip_prefix("RUST_LOG="))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Unit file is missing Environment=RUST_LOG"))?;

    let (binary_path, port) = parse_exec_start(exec_start)?;

    Ok(ServiceUnitConfig {
        binary_path: PathBuf::from(binary_path),
        working_directory: PathBuf::from(unquote_systemd_value(working_directory)),
        port,
        log_level,
    })
}

fn unit_value<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    content
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .map(str::trim)
}

fn parse_exec_start(value: &str) -> Result<(String, u16)> {
    let suffix = " daemon --port ";
    let (binary, port) = value
        .rsplit_once(suffix)
        .ok_or_else(|| anyhow!("Unsupported ExecStart format: {value}"))?;
    let port = port
        .trim()
        .parse::<u16>()
        .with_context(|| format!("Invalid daemon port in ExecStart: {value}"))?;
    Ok((unquote_systemd_value(binary), port))
}

fn quote_systemd_arg(path: &Path) -> String {
    let raw = path.display().to_string();
    if raw.contains([' ', '\t', '"']) {
        format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        raw
    }
}

fn unquote_systemd_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        trimmed.to_string()
    }
}

fn systemd() -> SystemdUserServiceManager {
    SystemdUserServiceManager::new()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::{parse_unit_config, render_unit_file, ServiceUnitConfig};
    use std::path::Path;

    #[test]
    fn renders_unit_with_expected_exec_start() {
        let unit = render_unit_file(
            Path::new("/opt/univers-container-daemon"),
            Path::new("/home/tester"),
            3100,
            "info",
        );

        assert!(unit.contains("ExecStart=/opt/univers-container-daemon daemon --port 3100"));
        assert!(unit.contains("WorkingDirectory=/home/tester"));
        assert!(unit.contains("Environment=RUST_LOG=info"));
    }

    #[test]
    fn quotes_paths_with_spaces() {
        let unit = render_unit_file(
            Path::new("/tmp/dev builds/univers-container-daemon"),
            Path::new("/tmp/dev builds"),
            3200,
            "debug",
        );

        assert!(unit
            .contains("ExecStart=\"/tmp/dev builds/univers-container-daemon\" daemon --port 3200"));
        assert!(unit.contains("WorkingDirectory=\"/tmp/dev builds\""));
    }

    #[test]
    fn parses_rendered_unit_file() {
        let unit = render_unit_file(
            Path::new("/tmp/dev builds/univers-container-daemon"),
            Path::new("/tmp/dev builds"),
            3300,
            "debug",
        );

        let parsed = parse_unit_config(&unit).expect("expected parsed config");
        assert_eq!(
            parsed,
            ServiceUnitConfig {
                binary_path: "/tmp/dev builds/univers-container-daemon".into(),
                working_directory: "/tmp/dev builds".into(),
                port: 3300,
                log_level: "debug".into(),
            }
        );
    }
}
