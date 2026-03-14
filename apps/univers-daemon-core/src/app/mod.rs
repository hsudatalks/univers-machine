use crate::installer::InstallerRegistry;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSpec {
    pub id: String,
    pub title: String,
    pub description: String,
    pub command: String,
    pub installer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub id: String,
    pub title: String,
    pub description: String,
    pub command: String,
    pub installer_id: Option<String>,
    pub installed: bool,
    pub version: Option<String>,
    pub executable_path: Option<String>,
}

#[derive(Debug, Default)]
pub struct AppCatalog;

impl AppCatalog {
    pub fn new() -> Self {
        Self
    }

    pub fn list_specs(&self) -> Vec<AppSpec> {
        builtin_apps()
    }

    pub async fn list_statuses(&self, installers: &InstallerRegistry) -> Vec<AppStatus> {
        let mut statuses = Vec::new();
        for spec in self.list_specs() {
            match self.status_for(&spec.id, installers).await {
                Ok(status) => statuses.push(status),
                Err(_) => statuses.push(spec_to_status(spec, false, None, None)),
            }
        }
        statuses
    }

    pub async fn status_for(&self, id: &str, installers: &InstallerRegistry) -> Result<AppStatus> {
        let spec = self
            .list_specs()
            .into_iter()
            .find(|spec| spec.id == id)
            .ok_or_else(|| anyhow!("Unknown app '{id}'"))?;

        let executable_path =
            resolve_command_path(&spec.command).map(|path| path.display().to_string());

        if let Some(installer_id) = spec.installer_id.as_deref() {
            let status = installers.check_status(installer_id).await?;
            return Ok(spec_to_status(
                spec,
                status.installed,
                status.version,
                executable_path,
            ));
        }

        let installed = executable_path.is_some();
        let version = if installed {
            command_version(&spec.command)
        } else {
            None
        };
        Ok(spec_to_status(spec, installed, version, executable_path))
    }
}

fn spec_to_status(
    spec: AppSpec,
    installed: bool,
    version: Option<String>,
    executable_path: Option<String>,
) -> AppStatus {
    AppStatus {
        id: spec.id,
        title: spec.title,
        description: spec.description,
        command: spec.command,
        installer_id: spec.installer_id,
        installed,
        version,
        executable_path,
    }
}

fn builtin_apps() -> Vec<AppSpec> {
    vec![
        AppSpec {
            id: String::from("claude-code"),
            title: String::from("Claude Code"),
            description: String::from("Claude Code terminal agent runtime."),
            command: String::from("claude"),
            installer_id: Some(String::from("claude-code")),
        },
        AppSpec {
            id: String::from("opencode"),
            title: String::from("OpenCode"),
            description: String::from("OpenCode terminal coding agent runtime."),
            command: String::from("opencode"),
            installer_id: Some(String::from("opencode")),
        },
        AppSpec {
            id: String::from("codex-cli"),
            title: String::from("Codex CLI"),
            description: String::from("Codex CLI terminal agent runtime."),
            command: String::from("codex"),
            installer_id: None,
        },
        AppSpec {
            id: String::from("nodejs"),
            title: String::from("Node.js"),
            description: String::from("Node.js runtime used by frontend and service windows."),
            command: String::from("node"),
            installer_id: Some(String::from("nodejs")),
        },
        AppSpec {
            id: String::from("surrealdb"),
            title: String::from("SurrealDB"),
            description: String::from("SurrealDB database runtime."),
            command: String::from("surreal"),
            installer_id: Some(String::from("surrealdb")),
        },
    ]
}

fn resolve_command_path(command: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|path| path.join(command))
            .find(|path| is_executable(path))
    })
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn command_version(command: &str) -> Option<String> {
    let output = std::process::Command::new(command)
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout.lines().next().unwrap_or_default().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    (!stderr.is_empty()).then_some(stderr.lines().next().unwrap_or_default().to_string())
}
