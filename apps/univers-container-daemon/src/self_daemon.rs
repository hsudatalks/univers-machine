use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DaemonServiceStatus {
    pub manager: &'static str,
    pub manager_available: bool,
    pub user_session_available: bool,
    pub unit_name: String,
    pub unit_path: String,
    pub installed: bool,
    pub active: bool,
    pub enabled: bool,
    pub last_error: Option<String>,
}

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DaemonServiceMutationResult {
    pub action: &'static str,
    pub message: String,
    pub service: DaemonServiceStatus,
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
    let unit_path = service_unit_path();
    let installed = unit_path.exists();
    let manager_available = systemctl_exists();
    let user_session_available = if manager_available {
        systemctl_user_available()
    } else {
        false
    };

    let mut active = false;
    let mut enabled = false;
    let mut last_error = None;

    if installed && manager_available && user_session_available {
        match systemctl_bool(["is-active", UNIT_NAME]) {
            Ok(value) => active = value,
            Err(error) => last_error = Some(error.to_string()),
        }

        match systemctl_bool(["is-enabled", UNIT_NAME]) {
            Ok(value) => enabled = value,
            Err(error) if last_error.is_none() => last_error = Some(error.to_string()),
            Err(_) => {}
        }
    } else if installed && manager_available && !user_session_available {
        last_error =
            Some("systemctl --user is installed but no user service manager is available".into());
    }

    DaemonServiceStatus {
        manager: "systemd-user",
        manager_available,
        user_session_available,
        unit_name: UNIT_NAME.into(),
        unit_path: unit_path.display().to_string(),
        installed,
        active,
        enabled,
        last_error,
    }
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

fn install_service_blocking(
    request: InstallDaemonServiceRequest,
) -> Result<DaemonServiceMutationResult> {
    ensure_user_systemd()?;

    let unit_path = service_unit_path();
    let unit_dir = unit_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid unit file path"))?;
    std::fs::create_dir_all(unit_dir)
        .with_context(|| format!("Failed to create {}", unit_dir.display()))?;

    let binary_path = resolve_binary_path(request.binary_path.as_deref())?;
    let working_directory = resolve_working_directory(request.working_directory.as_deref())?;
    let port = request
        .port
        .or_else(|| LISTEN_PORT.get().copied())
        .unwrap_or(3100);
    let log_level = request.log_level.as_deref().unwrap_or("info");

    let unit = render_unit_file(&binary_path, &working_directory, port, log_level);
    std::fs::write(&unit_path, unit)
        .with_context(|| format!("Failed to write {}", unit_path.display()))?;

    run_systemctl(["daemon-reload"])?;

    let enable = request.enable.unwrap_or(true);
    let start = request.start.unwrap_or(true);

    if enable {
        run_systemctl(["enable", UNIT_NAME])?;
    }

    if start {
        run_systemctl(["start", UNIT_NAME])?;
    }

    Ok(DaemonServiceMutationResult {
        action: "install",
        message: format!("Installed {}", unit_path.display()),
        service: collect_service_status(),
    })
}

fn uninstall_service_blocking() -> Result<DaemonServiceMutationResult> {
    ensure_user_systemd()?;

    let unit_path = service_unit_path();
    if unit_path.exists() {
        let _ = run_systemctl(["disable", "--now", UNIT_NAME]);
        std::fs::remove_file(&unit_path)
            .with_context(|| format!("Failed to remove {}", unit_path.display()))?;
        run_systemctl(["daemon-reload"])?;
    }

    Ok(DaemonServiceMutationResult {
        action: "uninstall",
        message: format!("Removed {}", unit_path.display()),
        service: collect_service_status(),
    })
}

fn run_service_action(action: &'static str) -> Result<DaemonServiceMutationResult> {
    ensure_user_systemd()?;

    if !service_unit_path().exists() {
        return Err(anyhow!("{} is not installed", UNIT_NAME));
    }

    run_systemctl([action, UNIT_NAME])?;
    Ok(DaemonServiceMutationResult {
        action,
        message: format!("{action} {}", UNIT_NAME),
        service: collect_service_status(),
    })
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

fn quote_systemd_arg(path: &Path) -> String {
    let raw = path.display().to_string();
    if raw.contains([' ', '\t', '"']) {
        format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        raw
    }
}

fn service_unit_path() -> PathBuf {
    let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
    };
    base.join("systemd/user").join(UNIT_NAME)
}

fn ensure_user_systemd() -> Result<()> {
    if !systemctl_exists() {
        return Err(anyhow!("systemctl is not installed"));
    }
    if !systemctl_user_available() {
        return Err(anyhow!(
            "systemctl --user is installed but no user service manager is available"
        ));
    }
    Ok(())
}

fn systemctl_exists() -> bool {
    Command::new("systemctl")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn systemctl_user_available() -> bool {
    Command::new("systemctl")
        .arg("--user")
        .arg("show-environment")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn systemctl_bool<I, S>(args: I) -> Result<bool>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .context("Failed to execute systemctl")?;

    if output.status.success() {
        return Ok(true);
    }

    match output.status.code() {
        Some(1..=4) => Ok(false),
        _ => Err(anyhow!(stderr_string(&output))),
    }
}

fn run_systemctl<I, S>(args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .context("Failed to execute systemctl")?;

    if output.status.success() {
        return Ok(());
    }

    Err(anyhow!(stderr_string(&output)))
}

fn stderr_string(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        "systemctl command failed".into()
    } else {
        stderr
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::render_unit_file;
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
}
