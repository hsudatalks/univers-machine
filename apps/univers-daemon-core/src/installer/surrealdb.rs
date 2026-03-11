use super::common::{command_exists, get_version, run_shell};
use super::{InstallResult, Installer};
use anyhow::Result;
use async_trait::async_trait;

pub struct SurrealDbInstaller;

#[async_trait]
impl Installer for SurrealDbInstaller {
    fn name(&self) -> &str {
        "surrealdb"
    }

    fn display_name(&self) -> &str {
        "SurrealDB"
    }

    fn description(&self) -> &str {
        "SurrealDB multi-model database (curl install script)"
    }

    async fn is_installed(&self) -> bool {
        command_exists("surreal").await
    }

    async fn installed_version(&self) -> Option<String> {
        get_version("surreal", &["version"]).await
    }

    async fn install(&self) -> Result<InstallResult> {
        match run_shell("curl -sSf https://install.surrealdb.com | sh").await {
            Ok(_) => {
                let version = self.installed_version().await;
                Ok(InstallResult {
                    success: true,
                    version,
                    message: "SurrealDB installed successfully".into(),
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
