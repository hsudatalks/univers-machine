use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Json, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tracing::info;
use univers_daemon_shared::agent::repository::SessionRepository;
use univers_daemon_shared::agents::AgentCatalog;
use univers_daemon_shared::api::response::ApiResponse;
use univers_daemon_shared::api::routes::{shared_routes, DaemonState};
use univers_daemon_shared::app::AppCatalog;
use univers_daemon_shared::application::agent::AgentApplicationService;
use univers_daemon_shared::application::agent_session::AgentSessionApplicationService;
use univers_daemon_shared::application::catalog::CatalogQueryService;
use univers_daemon_shared::application::installer::InstallerApplicationService;
use univers_daemon_shared::application::workspace::WorkspaceApplicationService;
use univers_daemon_shared::installer::InstallerRegistry;
use univers_daemon_shared::tmux::workspace::WorkspaceManager;
use univers_infra_sqlite::SqliteSessionRepository;

use crate::application::machine::MachineApplicationService;
use crate::machine::{MachineInfo, NetworkInterface};

/// Extended state for machine-daemon (includes auth token).
#[allow(dead_code)]
pub struct MachineDaemonState {
    pub daemon: Arc<DaemonState>,
    pub auth_token: Option<String>,
}

pub async fn run_daemon(port: u16, auth_token: Option<String>) -> anyhow::Result<()> {
    let session_repository: Arc<dyn SessionRepository> = Arc::new(SqliteSessionRepository::new());
    let agent_sessions = Arc::new(AgentSessionApplicationService::new(session_repository));
    let workspace_manager = Arc::new(WorkspaceManager::for_machine());
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

    let machine_state = Arc::new(MachineDaemonState {
        daemon: daemon_state.clone(),
        auth_token: auth_token.clone(),
    });

    let mut app = Router::new()
        // Machine-specific routes
        .route("/api/machine", get(get_machine_info))
        .route("/api/machine/network", get(get_network_interfaces))
        .with_state(machine_state)
        // Shared routes from core
        .merge(shared_routes().with_state(daemon_state));

    // Add auth middleware if token is configured
    if let Some(token) = auth_token {
        let token = Arc::new(token);
        app = app.layer(middleware::from_fn(move |req, next| {
            let token = token.clone();
            auth_middleware(req, next, token)
        }));
        info!("Bearer token authentication enabled");
    } else {
        info!("WARNING: No auth token configured. API is open to all.");
    }

    let addr = format!("0.0.0.0:{port}");
    info!("univers-machine-daemon listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn auth_middleware(req: Request<Body>, next: Next, expected_token: Arc<String>) -> Response {
    // Allow health checks without auth
    if req.uri().path() == "/health" {
        return next.run(req).await;
    }

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(value) if value.starts_with("Bearer ") => {
            let token = &value[7..];
            if token == expected_token.as_str() {
                return next.run(req).await;
            }
            let body =
                serde_json::to_string(&ApiResponse::<()>::err("Invalid token")).unwrap_or_default();
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap()
        }
        _ => {
            let body = serde_json::to_string(&ApiResponse::<()>::err(
                "Missing Authorization header. Use: Bearer <token>",
            ))
            .unwrap_or_default();
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap()
        }
    }
}

async fn get_machine_info(
    State(_state): State<Arc<MachineDaemonState>>,
) -> Json<ApiResponse<MachineInfo>> {
    let service = MachineApplicationService::new();
    Json(ApiResponse::ok(service.info()))
}

async fn get_network_interfaces(
    State(_state): State<Arc<MachineDaemonState>>,
) -> Json<ApiResponse<Vec<NetworkInterface>>> {
    let service = MachineApplicationService::new();
    Json(ApiResponse::ok(service.network_interfaces()))
}
