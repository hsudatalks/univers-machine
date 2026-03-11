use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Json, Response};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tracing::info;
use univers_daemon_core::agent::state::AgentState;
use univers_daemon_core::api::response::ApiResponse;
use univers_daemon_core::api::routes::{shared_routes, DaemonState};
use univers_daemon_core::installer::InstallerRegistry;
use univers_daemon_core::tmux::service::TmuxServiceManager;

use crate::machine::{MachineInfo, NetworkInterface};

/// Extended state for machine-daemon (includes auth token).
#[allow(dead_code)]
pub struct MachineDaemonState {
    pub daemon: Arc<DaemonState>,
    pub auth_token: Option<String>,
}

pub async fn run_daemon(port: u16, auth_token: Option<String>) -> anyhow::Result<()> {
    let agent_state = AgentState::new();
    let tmux_manager = Arc::new(TmuxServiceManager::new());
    let installer_registry = Arc::new(InstallerRegistry::with_defaults());

    let daemon_state = Arc::new(DaemonState {
        agent_state,
        tmux_manager,
        installer_registry,
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

async fn auth_middleware(
    req: Request<Body>,
    next: Next,
    expected_token: Arc<String>,
) -> Response {
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
            let body = serde_json::to_string(&ApiResponse::<()>::err("Invalid token"))
                .unwrap_or_default();
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
    Json(ApiResponse::ok(MachineInfo::collect()))
}

async fn get_network_interfaces(
    State(_state): State<Arc<MachineDaemonState>>,
) -> Json<ApiResponse<Vec<NetworkInterface>>> {
    Json(ApiResponse::ok(NetworkInterface::list()))
}
