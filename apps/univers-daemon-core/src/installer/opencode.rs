use super::common::{command_exists, get_version, run_shell};
use super::{InstallResult, Installer};
use anyhow::Result;
use async_trait::async_trait;

pub struct OpenCodeInstaller;

#[async_trait]
impl Installer for OpenCodeInstaller {
    fn name(&self) -> &str {
        "opencode"
    }

    fn display_name(&self) -> &str {
        "OpenCode"
    }

    fn description(&self) -> &str {
        "OpenCode CLI tool (go install or binary download)"
    }

    async fn is_installed(&self) -> bool {
        command_exists("opencode").await
    }

    async fn installed_version(&self) -> Option<String> {
        get_version("opencode", &["version"]).await
    }

    async fn install(&self) -> Result<InstallResult> {
        // Try go install first
        if command_exists("go").await {
            match run_shell("go install github.com/opencode-ai/opencode@latest").await {
                Ok(_) => {
                    let version = self.installed_version().await;
                    return Ok(InstallResult {
                        success: true,
                        version,
                        message: "OpenCode installed via go install".into(),
                    });
                }
                Err(e) => {
                    tracing::warn!("go install failed, trying binary download: {e}");
                }
            }
        }

        // Fallback: curl install script
        match run_shell("curl -fsSL https://opencode.ai/install | sh").await {
            Ok(_) => {
                let version = self.installed_version().await;
                Ok(InstallResult {
                    success: true,
                    version,
                    message: "OpenCode installed via install script".into(),
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
