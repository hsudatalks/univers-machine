use crate::container::{
    collect_ports, ContainerInfo, ContainerProcessesInfo, ContainerRuntimeInfo,
};
use crate::dashboard::{collect_dashboard, ContainerDashboard, DashboardRequest};
use axum::routing::{get, post};
use axum::{extract::State, response::Json, Router};
use std::sync::Arc;
use tracing::info;
use univers_daemon_core::agent::state::AgentState;
use univers_daemon_core::api::response::ApiResponse;
use univers_daemon_core::api::routes::{shared_routes, legacy_compat_routes, DaemonState};
use univers_daemon_core::installer::InstallerRegistry;
use univers_daemon_core::tmux::service::TmuxServiceManager;

pub async fn run_daemon(port: u16) -> anyhow::Result<()> {
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
        .route("/api/container/dashboard", post(get_container_dashboard))
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
