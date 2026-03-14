use crate::application::container_runtime::ContainerRuntimeApplicationService;
use crate::application::daemon_service::DaemonServiceApplicationService;
use crate::application::dashboard::ContainerDashboardApplicationService;
use crate::container::{
    ContainerInfo, ContainerPortInfo, ContainerProcessesInfo, ContainerRuntimeInfo,
};
use crate::dashboard::{
    ContainerDashboard, DashboardAgentInfo, DashboardProjectInfo, DashboardRequest,
    DashboardServiceInfo, DashboardTmuxInfo,
};
use crate::self_daemon::{
    record_process_start, DaemonInfo, DaemonServiceLogs, DaemonServiceLogsQuery,
    DaemonServiceMutationResult, DaemonServiceStatus, DaemonServiceUnitFile,
    InstallDaemonServiceRequest, UpdateDaemonServiceRequest,
};
use axum::routing::{get, post};
use axum::{
    extract::{Query, State},
    response::Json,
    Router,
};
use std::sync::Arc;
use tracing::info;
use univers_daemon_shared::agent::repository::SessionRepository;
use univers_daemon_shared::agents::AgentCatalog;
use univers_daemon_shared::api::response::ApiResponse;
use univers_daemon_shared::api::routes::{legacy_compat_routes, shared_routes, DaemonState};
use univers_daemon_shared::app::AppCatalog;
use univers_daemon_shared::application::agent::AgentApplicationService;
use univers_daemon_shared::application::agent_session::AgentSessionApplicationService;
use univers_daemon_shared::application::catalog::CatalogQueryService;
use univers_daemon_shared::application::installer::InstallerApplicationService;
use univers_daemon_shared::application::workspace::WorkspaceApplicationService;
use univers_daemon_shared::installer::InstallerRegistry;
use univers_daemon_shared::tmux::workspace::WorkspaceManager;
use univers_infra_sqlite::SqliteSessionRepository;

pub async fn run_daemon(port: u16) -> anyhow::Result<()> {
    record_process_start(port);

    let session_repository: Arc<dyn SessionRepository> = Arc::new(SqliteSessionRepository::new());
    let agent_sessions = Arc::new(AgentSessionApplicationService::new(session_repository));
    let workspace_manager = Arc::new(WorkspaceManager::for_container());
    let app_catalog = Arc::new(AppCatalog::new());
    let agent_catalog = Arc::new(AgentCatalog::new());
    let installer_registry = Arc::new(InstallerRegistry::with_defaults());
    let workspace_service = Arc::new(WorkspaceApplicationService::new(workspace_manager));
    let catalog_service = Arc::new(CatalogQueryService::new(
        app_catalog,
        agent_catalog,
        installer_registry.clone(),
        agent_sessions.clone(),
    ));
    let agent_service = Arc::new(AgentApplicationService::new(
        catalog_service.clone(),
        workspace_service.clone(),
    ));
    let installer_service = Arc::new(InstallerApplicationService::new(installer_registry.clone()));

    let daemon_state = Arc::new(DaemonState {
        agent_sessions,
        workspace_service,
        catalog_service,
        agent_service,
        installer_service,
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
        .route(
            "/api/daemon/service/unit",
            get(get_daemon_service_unit_file),
        )
        .route("/api/daemon/service/install", post(install_daemon_service))
        .route("/api/daemon/service/update", post(update_daemon_service))
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
    let service = ContainerRuntimeApplicationService::new();
    Json(ApiResponse::ok(service.info()))
}

async fn get_container_runtime(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<ContainerRuntimeInfo>> {
    let service = ContainerRuntimeApplicationService::new();
    Json(ApiResponse::ok(service.runtime()))
}

async fn get_container_processes(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<ContainerProcessesInfo>> {
    let service = ContainerRuntimeApplicationService::new();
    Json(ApiResponse::ok(service.processes()))
}

async fn get_container_ports(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<ContainerPortInfo>>> {
    let service = ContainerRuntimeApplicationService::new();
    Json(ApiResponse::ok(service.ports()))
}

async fn get_container_dashboard(
    State(state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<ContainerDashboard>> {
    let service = ContainerDashboardApplicationService::new(
        state.agent_sessions.clone(),
        state.workspace_service.clone(),
    );
    match service.dashboard(request).await {
        Ok(dashboard) => Json(ApiResponse::ok(dashboard)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_container_project(
    State(state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<DashboardProjectInfo>> {
    let service = ContainerDashboardApplicationService::new(
        state.agent_sessions.clone(),
        state.workspace_service.clone(),
    );
    match service.project(request).await {
        Ok(project) => Json(ApiResponse::ok(project)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_container_services(
    State(state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<Vec<DashboardServiceInfo>>> {
    let service = ContainerDashboardApplicationService::new(
        state.agent_sessions.clone(),
        state.workspace_service.clone(),
    );
    match service.services(request).await {
        Ok(services) => Json(ApiResponse::ok(services)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_container_agent(
    State(state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<DashboardAgentInfo>> {
    let service = ContainerDashboardApplicationService::new(
        state.agent_sessions.clone(),
        state.workspace_service.clone(),
    );
    match service.agent(request).await {
        Ok(agent) => Json(ApiResponse::ok(agent)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_container_tmux(
    State(state): State<Arc<DaemonState>>,
    Json(request): Json<DashboardRequest>,
) -> Json<ApiResponse<DashboardTmuxInfo>> {
    let service = ContainerDashboardApplicationService::new(
        state.agent_sessions.clone(),
        state.workspace_service.clone(),
    );
    match service.tmux(request).await {
        Ok(tmux) => Json(ApiResponse::ok(tmux)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_daemon_info(State(_state): State<Arc<DaemonState>>) -> Json<ApiResponse<DaemonInfo>> {
    let service = DaemonServiceApplicationService::new();
    Json(ApiResponse::ok(service.info()))
}

async fn get_daemon_service_status(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceStatus>> {
    let service = DaemonServiceApplicationService::new();
    Json(ApiResponse::ok(service.service_status()))
}

async fn get_daemon_service_logs(
    State(_state): State<Arc<DaemonState>>,
    Query(query): Query<DaemonServiceLogsQuery>,
) -> Json<ApiResponse<DaemonServiceLogs>> {
    let lines = query.lines.unwrap_or(100);
    let service = DaemonServiceApplicationService::new();
    match service.service_logs(lines).await {
        Ok(logs) => Json(ApiResponse::ok(logs)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_daemon_service_unit_file(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceUnitFile>> {
    let service = DaemonServiceApplicationService::new();
    match service.service_unit_file().await {
        Ok(unit) => Json(ApiResponse::ok(unit)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn install_daemon_service(
    State(_state): State<Arc<DaemonState>>,
    Json(request): Json<InstallDaemonServiceRequest>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    let service = DaemonServiceApplicationService::new();
    match service.install_service(request).await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn update_daemon_service(
    State(_state): State<Arc<DaemonState>>,
    Json(request): Json<UpdateDaemonServiceRequest>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    let service = DaemonServiceApplicationService::new();
    match service.update_service(request).await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn start_daemon_service(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    let service = DaemonServiceApplicationService::new();
    match service.start_service().await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn stop_daemon_service(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    let service = DaemonServiceApplicationService::new();
    match service.stop_service().await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn restart_daemon_service(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    let service = DaemonServiceApplicationService::new();
    match service.restart_service().await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn uninstall_daemon_service(
    State(_state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<DaemonServiceMutationResult>> {
    let service = DaemonServiceApplicationService::new();
    match service.uninstall_service().await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}
