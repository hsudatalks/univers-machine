use crate::models::{
    ContainerDashboard, DeveloperServiceKind, DeveloperTarget, ServiceStatus, TunnelStatus,
};
use std::collections::HashMap;

pub(crate) fn tunnel_service_status(status: &TunnelStatus) -> ServiceStatus {
    ServiceStatus {
        target_id: status.target_id.clone(),
        service_id: status.service_id.clone(),
        kind: DeveloperServiceKind::Web,
        state: status.state.clone(),
        message: status.message.clone(),
        local_url: status.local_url.clone(),
    }
}

pub(crate) fn command_service_status(
    target_id: &str,
    service_id: &str,
    state: &str,
    message: impl Into<String>,
) -> ServiceStatus {
    ServiceStatus {
        target_id: target_id.to_string(),
        service_id: service_id.to_string(),
        kind: DeveloperServiceKind::Command,
        state: state.to_string(),
        message: message.into(),
        local_url: None,
    }
}

pub(crate) fn dashboard_service_statuses(
    target_id: &str,
    dashboard: &ContainerDashboard,
    target: Option<&DeveloperTarget>,
    existing_statuses: &HashMap<String, ServiceStatus>,
) -> Vec<ServiceStatus> {
    dashboard
        .services
        .iter()
        .map(|service| {
            let kind = target
                .and_then(|target| {
                    target
                        .services
                        .iter()
                        .find(|candidate| candidate.id == service.id)
                })
                .map(|service| service.kind)
                .unwrap_or(DeveloperServiceKind::Endpoint);

            let local_url = if kind == DeveloperServiceKind::Web {
                existing_statuses
                    .get(&service.id)
                    .and_then(|status| status.local_url.clone())
            } else {
                service.url.clone()
            };

            ServiceStatus {
                target_id: target_id.to_string(),
                service_id: service.id.clone(),
                kind,
                state: service.status.clone(),
                message: service.detail.clone(),
                local_url,
            }
        })
        .collect()
}
