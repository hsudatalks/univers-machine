use crate::container::{
    collect_ports, ContainerInfo, ContainerProcessesInfo, ContainerRuntimeInfo,
};
use crate::dashboard::{
    collect_agent, collect_dashboard, collect_project, collect_services, collect_tmux,
    ContainerDashboard, DashboardAgentInfo, DashboardProjectInfo, DashboardRequest,
    DashboardServiceInfo, DashboardTmuxInfo,
};
use crate::self_daemon::{
    collect_daemon_info, collect_service_logs, collect_service_status, collect_service_unit_file,
    install_service, record_process_start, restart_service, start_service, stop_service,
    uninstall_service, DaemonInfo, DaemonServiceLogs, DaemonServiceLogsQuery,
    DaemonServiceMutationResult, DaemonServiceStatus, DaemonServiceUnitFile,
    InstallDaemonServiceRequest,
};
use axum::routing::{get, post};
use axum::{
    extract::{Query, State},
    response::Json,
    Router,
};
use std::sync::Arc;
use tracing::info;
use univers_daemon_core::agent::state::AgentState;
use univers_daemon_core::api::response::ApiResponse;
use univers_daemon_core::api::routes::{legacy_compat_routes, shared_routes, DaemonState};
use univers_daemon_core::installer::InstallerRegistry;
use univers_daemon_core::tmux::service::TmuxServiceManager;

pub async fn run_daemon(port: u16) -> anyhow::Result<()> {
    record_process_start(port);

    let agent_state = AgentState::new();
    let tmux_manager = Arc::new(TmuxServiceManager::for_container());
    let installer_registry = Arc::new(InstallerRegistry::with_defaults());

    let daemon_state = Arc::new(DaemonState {
        agent_state,
        tmux_manager,
        installer_registry,
    });

    let app = Router::new()
        // Container-specific route
        .route("/api/container", get(get_container_info))
        .route("/api/container/runtime", get(get_container_runtime))
        .route("/api/container/processes", get(get_container_processes))
        .route("/api/container/ports", get(get_container_ports))
        .route("/api/container/project", post(get_container_project))
        .route("/api/container/services", post(get_container_services))
        .route("/api/container/agent", post(get_container_agent))
        .route("/api/container/tmux", post(get_container_tmux))
        .route("/api/container/dashboard", post(get_container_dashboard))
        .route("/api/daemon", get(get_daemon_info))
        .route("/api/daemon/service", get(get_daemon_service_status))
        .route("/api/daemon/service/logs", get(get_daemon_service_logs))
        .route("/api/daemon/service/unit", get(get_daemon_service_unit_file))
        .route("/api/daemon/service/install", post(install_daemon_service))
        .route("/api/daemon/service/start", post(start_daemon_service))
        .route("/api/daemon/service/stop", post(stop_daemon_service))
        .route("/api/daemon/service/restart", post(restart_daemon_service))
        .route(
            "/api/daemon/service/uninstall",
            post(uninstall_daemon_service),
        )
        // Shared routes from core
        .merge(shared_routes())
        // Legacy backward-compatible routes
        .merge(legacy_compat_routes())
        .with_state(daemon_state);

    let addr = format!("0.0.0.0:{port}");
    info!("univers-container-daemon listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn get_container_info(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<ContainerInfo>> {
    Json(ApiResponse::ok(ContainerInfo::collect()))
}

async fn get_container_runtime(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<ContainerRuntimeInfo>> {
    Json(ApiResponse::ok(ContainerRuntimeInfo::collect()))
}

async fn get_container_processes(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<ContainerProcessesInfo>> {
    Json(ApiResponse::ok(ContainerProcessesInfo::collect()))
}

async fn get_container_ports(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<crate::container::ContainerPortInfo>>> {
    Json(ApiResponse::ok(collect_ports()))
}

async fn get_container_dashboard(
    State(state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<ContainerDashboard>> {
    match collect_dashboard(request, state.agent_state.clone()).await {
        Ok(dashboard) => Json(ApiResponse::ok(dashboard)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_container_project(
    State(_state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<DashboardProjectInfo>> {
    match collect_project(request).await {
        Ok(project) => Json(ApiResponse::ok(project)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_container_services(
    State(_state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<Vec<DashboardServiceInfo>>> {
    match collect_services(request).await {
        Ok(services) => Json(ApiResponse::ok(services)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_container_agent(
    State(state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<DashboardAgentInfo>> {
    match collect_agent(request, state.agent_state.clone()).await {
        Ok(agent) => Json(ApiResponse::ok(agent)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_container_tmux(
    State(_state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<DashboardTmuxInfo>> {
    match collect_tmux(request).await {
        Ok(tmux) => Json(ApiResponse::ok(tmux)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_daemon_info(State(_state): State<Arc<DaemonState>>) -> Json<ApiResponse<DaemonInfo>> {
    Json(ApiResponse::ok(collect_daemon_info()))
}

async fn get_daemon_service_status(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceStatus>> {
    Json(ApiResponse::ok(collect_service_status()))
}

async fn get_daemon_service_logs(
    State(_state): State<Arc<DaemonState>>,
    Query(query): Query<DaemonServiceLogsQuery>,
) -> Json<ApiResponse<DaemonServiceLogs>> {
    let lines = query.lines.unwrap_or(100);
    match collect_service_logs(lines).await {
        Ok(logs) => Json(ApiResponse::ok(logs)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_daemon_service_unit_file(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceUnitFile>> {
    match collect_service_unit_file().await {
        Ok(unit) => Json(ApiResponse::ok(unit)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn install_daemon_service(
    State(_state): State<Arc<DaemonState>>,
    Json(request): Json<InstallDaemonServiceRequest>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    match install_service(request).await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn start_daemon_service(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    match start_service().await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn stop_daemon_service(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    match stop_service().await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn restart_daemon_service(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    match restart_service().await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn uninstall_daemon_service(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    match uninstall_service().await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}
