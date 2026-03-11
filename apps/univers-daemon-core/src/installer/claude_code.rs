use super::common::{command_exists, get_version, run_cmd};
use super::{InstallResult, Installer};
use anyhow::Result;
use async_trait::async_trait;

pub struct ClaudeCodeInstaller;

#[async_trait]
impl Installer for ClaudeCodeInstaller {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn display_name(&self) -> &str {
        "Claude Code"
    }

    fn description(&self) -> &str {
        "Anthropic's CLI for Claude (npm install -g @anthropic-ai/claude-code)"
    }

    async fn is_installed(&self) -> bool {
        command_exists("claude").await
    }

    async fn installed_version(&self) -> Option<String> {
        get_version("claude", &["--version"]).await
    }

    async fn install(&self) -> Result<InstallResult> {
        // Requires npm
        if !command_exists("npm").await {
            return Ok(InstallResult {
                success: false,
                version: None,
                message: "npm is not installed. Install Node.js first.".into(),
            });
        }

        match run_cmd("npm", &["install", "-g", "@anthropic-ai/claude-code"]).await {
            Ok(_) => {
                let version = self.installed_version().await;
                Ok(InstallResult {
                    success: true,
                    version,
                    message: "Claude Code installed successfully".into(),
                })
            }
            Err(e) => Ok(InstallResult {
                success: false,
                version: None,
                message: format!("Installation failed: {e}"),
            }),
        }
    }
}
