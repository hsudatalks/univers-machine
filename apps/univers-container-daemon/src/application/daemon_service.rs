use crate::self_daemon::{
    collect_daemon_info, collect_service_logs, collect_service_status, collect_service_unit_file,
    install_service, restart_service, start_service, stop_service, uninstall_service,
    update_service, DaemonInfo, DaemonServiceLogs, DaemonServiceMutationResult,
    DaemonServiceStatus, DaemonServiceUnitFile, InstallDaemonServiceRequest,
    UpdateDaemonServiceRequest,
};
use anyhow::Result;

#[derive(Debug, Clone, Default)]
pub(crate) struct DaemonServiceApplicationService;

impl DaemonServiceApplicationService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn info(&self) -> DaemonInfo {
        collect_daemon_info()
    }

    pub(crate) fn service_status(&self) -> DaemonServiceStatus {
        collect_service_status()
    }

    pub(crate) async fn service_logs(&self, lines: usize) -> Result<DaemonServiceLogs> {
        collect_service_logs(lines).await
    }

    pub(crate) async fn service_unit_file(&self) -> Result<DaemonServiceUnitFile> {
        collect_service_unit_file().await
    }

    pub(crate) async fn install_service(
        &self,
        request: InstallDaemonServiceRequest,
    ) -> Result<DaemonServiceMutationResult> {
        install_service(request).await
    }

    pub(crate) async fn update_service(
        &self,
        request: UpdateDaemonServiceRequest,
    ) -> Result<DaemonServiceMutationResult> {
        update_service(request).await
    }

    pub(crate) async fn start_service(&self) -> Result<DaemonServiceMutationResult> {
        start_service().await
    }

    pub(crate) async fn stop_service(&self) -> Result<DaemonServiceMutationResult> {
        stop_service().await
    }

    pub(crate) async fn restart_service(&self) -> Result<DaemonServiceMutationResult> {
        restart_service().await
    }

    pub(crate) async fn uninstall_service(&self) -> Result<DaemonServiceMutationResult> {
        uninstall_service().await
    }
}
