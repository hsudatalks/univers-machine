pub mod claude_code;
pub mod common;
pub mod nodejs;
pub mod opencode;
pub mod surrealdb;

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for software installers.
#[async_trait]
pub trait Installer: Send + Sync {
    /// Unique identifier (e.g., "claude-code").
    fn name(&self) -> &str;
    /// Human-readable name (e.g., "Claude Code").
    fn display_name(&self) -> &str;
    /// Short description.
    fn description(&self) -> &str;
    /// Check if the software is installed.
    async fn is_installed(&self) -> bool;
    /// Get the installed version, if any.
    async fn installed_version(&self) -> Option<String>;
    /// Perform the installation.
    async fn install(&self) -> Result<InstallResult>;
}

/// Summary info for listing installers.
#[derive(Debug, Clone, Serialize)]
pub struct InstallerInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
}

/// Status check result.
#[derive(Debug, Clone, Serialize)]
pub struct InstallerStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
}

/// Installation result.
#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub success: bool,
    pub version: Option<String>,
    pub message: String,
}

/// Registry of available installers.
pub struct InstallerRegistry {
    installers: HashMap<String, Arc<dyn Installer>>,
}

impl InstallerRegistry {
    pub fn new() -> Self {
        Self {
            installers: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all built-in installers.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(claude_code::ClaudeCodeInstaller));
        registry.register(Arc::new(opencode::OpenCodeInstaller));
        registry.register(Arc::new(surrealdb::SurrealDbInstaller));
        registry.register(Arc::new(nodejs::NodeJsInstaller));
        registry
    }

    pub fn register(&mut self, installer: Arc<dyn Installer>) {
        self.installers
            .insert(installer.name().to_string(), installer);
    }

    pub async fn list_infos(&self) -> Vec<InstallerInfo> {
        let mut infos: Vec<InstallerInfo> = self
            .installers
            .values()
            .map(|i| InstallerInfo {
                name: i.name().to_string(),
                display_name: i.display_name().to_string(),
                description: i.description().to_string(),
            })
            .collect();
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        infos
    }

    pub async fn check_status(&self, name: &str) -> Result<InstallerStatus> {
        let installer = self
            .installers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown installer: {name}"))?;
        let installed = installer.is_installed().await;
        let version = installer.installed_version().await;
        Ok(InstallerStatus {
            name: name.to_string(),
            installed,
            version,
        })
    }

    pub async fn install(&self, name: &str) -> Result<InstallResult> {
        let installer = self
            .installers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown installer: {name}"))?;
        installer.install().await
    }
}
