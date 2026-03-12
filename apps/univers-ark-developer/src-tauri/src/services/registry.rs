use crate::{
    machine::resolve_raw_target,
    models::{
        ContainerDashboard, DeveloperServiceKind, ServiceRegistration, ServiceState, ServiceStatus,
        TunnelStatus,
    },
    services::{
        projection::{
            command_service_status, dashboard_service_statuses, tunnel_service_status,
        },
        runtime::service_key,
    },
};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub(crate) const SERVICE_STATUS_EVENT: &str = "service-status";

fn upsert_service_status<R: Runtime>(app: &AppHandle<R>, status: ServiceStatus) {
    if let Some(statuses) = app
        .try_state::<ServiceState>()
        .map(|service_state| service_state.statuses.clone())
    {
        if let Ok(mut statuses) = statuses.lock() {
            statuses.insert(
                service_key(&status.target_id, &status.service_id),
                status.clone(),
            );
        }
    }

    let _ = app.emit(SERVICE_STATUS_EVENT, status);
}

fn existing_service_statuses_for_target<R: Runtime>(
    app: &AppHandle<R>,
    target_id: &str,
) -> HashMap<String, ServiceStatus> {
    let statuses = app
        .try_state::<ServiceState>()
        .map(|service_state| service_state.statuses.clone());
    let Some(statuses) = statuses else {
        return HashMap::new();
    };
    let Ok(statuses) = statuses.lock() else {
        return HashMap::new();
    };

    statuses
        .values()
        .filter(|status| status.target_id == target_id)
        .map(|status| (status.service_id.clone(), status.clone()))
        .collect()
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
                ServiceRegistration { kind },
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
    register_service(
        app,
        &status.target_id,
        &status.service_id,
        DeveloperServiceKind::Web,
    );
    upsert_service_status(app, tunnel_service_status(status));
}

pub(crate) fn emit_command_service_status<R: Runtime>(
    app: &AppHandle<R>,
    target_id: &str,
    service_id: &str,
    state: &str,
    message: impl Into<String>,
) {
    register_service(app, target_id, service_id, DeveloperServiceKind::Command);
    upsert_service_status(app, command_service_status(target_id, service_id, state, message));
}

pub(crate) fn emit_dashboard_service_statuses<R: Runtime>(
    app: &AppHandle<R>,
    target_id: &str,
    dashboard: &ContainerDashboard,
) {
    let target = resolve_raw_target(target_id).ok();
    let existing_statuses = existing_service_statuses_for_target(app, target_id);
    let statuses =
        dashboard_service_statuses(target_id, dashboard, target.as_ref(), &existing_statuses);

    for status in statuses {
        register_service(app, target_id, &status.service_id, status.kind);
        upsert_service_status(app, status);
    }
}
