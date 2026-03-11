use super::common::{command_exists, get_version, run_shell};
use super::{InstallResult, Installer};
use anyhow::Result;
use async_trait::async_trait;

pub struct NodeJsInstaller;

#[async_trait]
impl Installer for NodeJsInstaller {
    fn name(&self) -> &str {
        "nodejs"
    }

    fn display_name(&self) -> &str {
        "Node.js"
    }

    fn description(&self) -> &str {
        "Node.js JavaScript runtime (via nvm or binary)"
    }

    async fn is_installed(&self) -> bool {
        command_exists("node").await
    }

    async fn installed_version(&self) -> Option<String> {
        get_version("node", &["--version"]).await
    }

    async fn install(&self) -> Result<InstallResult> {
        // Try nvm if available
        if nvm_available().await {
            match run_shell("bash -c 'source $NVM_DIR/nvm.sh && nvm install --lts'").await {
                Ok(_) => {
                    let version = self.installed_version().await;
                    return Ok(InstallResult {
                        success: true,
                        version,
                        message: "Node.js installed via nvm".into(),
                    });
                }
                Err(e) => {
                    tracing::warn!("nvm install failed: {e}");
                }
            }
        }

        // Install nvm first, then node
        match run_shell(
            "curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash && \
             export NVM_DIR=\"$HOME/.nvm\" && \
             . \"$NVM_DIR/nvm.sh\" && \
             nvm install --lts",
        )
        .await
        {
            Ok(_) => {
                let version = self.installed_version().await;
                Ok(InstallResult {
                    success: true,
                    version,
                    message: "Node.js installed via nvm (freshly installed)".into(),
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

async fn nvm_available() -> bool {
    std::env::var("NVM_DIR").is_ok()
        && run_shell("bash -c 'source $NVM_DIR/nvm.sh && nvm --version'")
            .await
            .is_ok()
}
