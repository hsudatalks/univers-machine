use crate::installer::{InstallResult, InstallerInfo, InstallerRegistry, InstallerStatus};
use anyhow::Result;
use std::sync::Arc;

pub struct InstallerApplicationService {
    installer_registry: Arc<InstallerRegistry>,
}

impl InstallerApplicationService {
    pub fn new(installer_registry: Arc<InstallerRegistry>) -> Self {
        Self { installer_registry }
    }

    pub async fn list_installers(&self) -> Vec<InstallerInfo> {
        self.installer_registry.list_infos().await
    }

    pub async fn installer_status(&self, name: &str) -> Result<InstallerStatus> {
        self.installer_registry.check_status(name).await
    }

    pub async fn install(&self, name: &str) -> Result<InstallResult> {
        self.installer_registry.install(name).await
    }
}
