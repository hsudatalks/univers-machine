use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserServiceStatus {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserServiceMutationResult {
    pub action: &'static str,
    pub message: String,
    pub service: UserServiceStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserServiceLogs {
    pub unit_name: String,
    pub lines: usize,
    pub manager_available: bool,
    pub user_session_available: bool,
    pub logs_available: bool,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserServiceUnitFile {
    pub unit_name: String,
    pub unit_path: String,
    pub installed: bool,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SystemdUserServiceManager;

impl SystemdUserServiceManager {
    pub fn new() -> Self {
        Self
    }

    pub fn status(&self, unit_name: &str) -> UserServiceStatus {
        let unit_path = service_unit_path(unit_name);
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
            match systemctl_bool(["is-active", unit_name]) {
                Ok(value) => active = value,
                Err(error) => last_error = Some(error.to_string()),
            }

            match systemctl_bool(["is-enabled", unit_name]) {
                Ok(value) => enabled = value,
                Err(error) if last_error.is_none() => last_error = Some(error.to_string()),
                Err(_) => {}
            }
        } else if installed && manager_available && !user_session_available {
            last_error =
                Some("systemctl --user is installed but no user service manager is available".into());
        }

        UserServiceStatus {
            manager: "systemd-user",
            manager_available,
            user_session_available,
            unit_name: unit_name.into(),
            unit_path: unit_path.display().to_string(),
            installed,
            active,
            enabled,
            last_error,
        }
    }

    pub fn ensure_available(&self) -> Result<()> {
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

    pub fn logs(&self, unit_name: &str, lines: usize) -> Result<UserServiceLogs> {
        let lines = lines.clamp(1, 500);
        let manager_available = journalctl_exists();
        let user_session_available = if manager_available {
            systemctl_user_available()
        } else {
            false
        };

        if !manager_available {
            return Ok(UserServiceLogs {
                unit_name: unit_name.into(),
                lines,
                manager_available,
                user_session_available,
                logs_available: false,
                entries: vec![],
            });
        }

        if !user_session_available {
            return Err(anyhow!(
                "journalctl is installed but no user service manager is available"
            ));
        }

        let output = Command::new("journalctl")
            .arg("--user")
            .arg("-u")
            .arg(unit_name)
            .arg("-n")
            .arg(lines.to_string())
            .arg("--no-pager")
            .arg("-o")
            .arg("short-iso")
            .output()
            .context("Failed to execute journalctl")?;

        if !output.status.success() {
            return Err(anyhow!(stderr_string(&output)));
        }

        let entries = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty() && *line != "-- No entries --")
            .map(ToOwned::to_owned)
            .collect();

        Ok(UserServiceLogs {
            unit_name: unit_name.into(),
            lines,
            manager_available,
            user_session_available,
            logs_available: true,
            entries,
        })
    }

    pub fn unit_file(&self, unit_name: &str) -> Result<UserServiceUnitFile> {
        let unit_path = service_unit_path(unit_name);
        let installed = unit_path.exists();
        let content = if installed {
            Some(
                std::fs::read_to_string(&unit_path)
                    .with_context(|| format!("Failed to read {}", unit_path.display()))?,
            )
        } else {
            None
        };

        Ok(UserServiceUnitFile {
            unit_name: unit_name.into(),
            unit_path: unit_path.display().to_string(),
            installed,
            content,
        })
    }

    pub fn write_unit_file(&self, unit_name: &str, content: &str) -> Result<PathBuf> {
        let unit_path = service_unit_path(unit_name);
        let unit_dir = unit_path
            .parent()
            .ok_or_else(|| anyhow!("Invalid unit file path"))?;
        std::fs::create_dir_all(unit_dir)
            .with_context(|| format!("Failed to create {}", unit_dir.display()))?;
        std::fs::write(&unit_path, content)
            .with_context(|| format!("Failed to write {}", unit_path.display()))?;
        self.daemon_reload()?;
        Ok(unit_path)
    }

    pub fn set_enabled(&self, unit_name: &str, enabled: bool) -> Result<()> {
        if enabled {
            run_systemctl(["enable", unit_name])
        } else {
            run_systemctl(["disable", unit_name])
        }
    }

    pub fn run_action(&self, unit_name: &str, action: &'static str) -> Result<UserServiceMutationResult> {
        self.ensure_available()?;

        if !service_unit_path(unit_name).exists() {
            return Err(anyhow!("{unit_name} is not installed"));
        }

        run_systemctl([action, unit_name])?;
        Ok(UserServiceMutationResult {
            action,
            message: format!("{action} {unit_name}"),
            service: self.status(unit_name),
        })
    }

    pub fn uninstall(&self, unit_name: &str) -> Result<UserServiceMutationResult> {
        self.ensure_available()?;

        let unit_path = service_unit_path(unit_name);
        if unit_path.exists() {
            let _ = run_systemctl(["disable", "--now", unit_name]);
            std::fs::remove_file(&unit_path)
                .with_context(|| format!("Failed to remove {}", unit_path.display()))?;
            self.daemon_reload()?;
        }

        Ok(UserServiceMutationResult {
            action: "uninstall",
            message: format!("Removed {}", unit_path.display()),
            service: self.status(unit_name),
        })
    }

    pub fn daemon_reload(&self) -> Result<()> {
        run_systemctl(["daemon-reload"])
    }
}

fn service_unit_path(unit_name: &str) -> PathBuf {
    let base = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
    };
    base.join("systemd/user").join(unit_name)
}

fn systemctl_exists() -> bool {
    Command::new("systemctl")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn journalctl_exists() -> bool {
    Command::new("journalctl")
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
