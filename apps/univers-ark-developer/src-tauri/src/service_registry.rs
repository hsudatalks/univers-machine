use crate::{
    config::resolve_raw_target,
    models::{
        ContainerDashboard, DeveloperServiceKind, ServiceRegistration, ServiceState,
        ServiceStatus, TunnelStatus,
    },
    runtime::service_key,
};
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub(crate) const SERVICE_STATUS_EVENT: &str = "service-status";

fn upsert_service_status<R: Runtime>(app: &AppHandle<R>, status: ServiceStatus) {
    if let Some(statuses) = app
        .try_state::<ServiceState>()
        .map(|service_state| service_state.statuses.clone())
    {
        if let Ok(mut statuses) = statuses.lock() {
            statuses.insert(service_key(&status.target_id, &status.service_id), status.clone());
        }
    }

    let _ = app.emit(SERVICE_STATUS_EVENT, status);
}

fn existing_service_status<R: Runtime>(
    app: &AppHandle<R>,
    target_id: &str,
    service_id: &str,
) -> Option<ServiceStatus> {
    let statuses = app
        .try_state::<ServiceState>()
        .map(|service_state| service_state.statuses.clone())?;
    let statuses = statuses.lock().ok()?;

    statuses.get(&service_key(target_id, service_id)).cloned()
}

fn register_service<R: Runtime>(
    app: &AppHandle<R>,
    target_id: &str,
    service_id: &str,
    kind: DeveloperServiceKind,
) {
    if let Some(registrations) = app
        .try_state::<ServiceState>()
        .map(|service_state| service_state.registrations.clone())
    {
        if let Ok(mut registrations) = registrations.lock() {
            registrations.insert(
                service_key(target_id, service_id),
                ServiceRegistration {
                    kind,
                },
            );
        }
    }
}

pub(crate) fn sync_registered_web_services<R: Runtime>(
    app: &AppHandle<R>,
    requests: &[(String, String)],
) {
    let Some(registrations) = app
        .try_state::<ServiceState>()
        .map(|service_state| service_state.registrations.clone())
    else {
        return;
    };

    let keep_keys = requests
        .iter()
        .map(|(target_id, service_id)| service_key(target_id, service_id))
        .collect::<std::collections::HashSet<_>>();

    let Ok(mut registrations_guard) = registrations.lock() else {
        return;
    };

    registrations_guard.retain(|key, registration| {
        if registration.kind != DeveloperServiceKind::Web {
            return true;
        }

        keep_keys.contains(key)
    });

    for (target_id, service_id) in requests {
        registrations_guard.insert(
            service_key(target_id, service_id),
            ServiceRegistration {
                kind: DeveloperServiceKind::Web,
            },
        );
    }
}

pub(crate) fn emit_tunnel_service_status<R: Runtime>(app: &AppHandle<R>, status: &TunnelStatus) {
    register_service(app, &status.target_id, &status.service_id, DeveloperServiceKind::Web);
    upsert_service_status(
        app,
        ServiceStatus {
            target_id: status.target_id.clone(),
            service_id: status.service_id.clone(),
            kind: DeveloperServiceKind::Web,
            state: status.state.clone(),
            message: status.message.clone(),
            local_url: status.local_url.clone(),
        },
    );
}

pub(crate) fn emit_command_service_status<R: Runtime>(
    app: &AppHandle<R>,
    target_id: &str,
    service_id: &str,
    state: &str,
    message: impl Into<String>,
) {
    register_service(app, target_id, service_id, DeveloperServiceKind::Command);
    upsert_service_status(
        app,
        ServiceStatus {
            target_id: target_id.to_string(),
            service_id: service_id.to_string(),
            kind: DeveloperServiceKind::Command,
            state: state.to_string(),
            message: message.into(),
            local_url: None,
        },
    );
}

pub(crate) fn emit_dashboard_service_statuses<R: Runtime>(
    app: &AppHandle<R>,
    target_id: &str,
    dashboard: &ContainerDashboard,
) {
    let target = resolve_raw_target(target_id).ok();

    for service in &dashboard.services {
        let kind = target
            .as_ref()
            .and_then(|target| target.services.iter().find(|candidate| candidate.id == service.id))
            .map(|service| service.kind)
            .unwrap_or(DeveloperServiceKind::Endpoint);

        register_service(app, target_id, &service.id, kind);
        let existing_status = existing_service_status(app, target_id, &service.id);
        upsert_service_status(
            app,
            ServiceStatus {
                target_id: target_id.to_string(),
                service_id: service.id.clone(),
                kind,
                state: service.status.clone(),
                message: service.detail.clone(),
                local_url: if kind == DeveloperServiceKind::Web {
                    existing_status.and_then(|status| status.local_url)
                } else {
                    service.url.clone()
                },
            },
        );
    }
}
