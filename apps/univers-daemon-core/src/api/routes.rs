use crate::agent::event::{HookEvent, SessionSnapshot};
use crate::agent::state::AgentState;
use crate::api::response::ApiResponse;
use crate::installer::InstallerRegistry;
use crate::system::SystemInfo;
use crate::tmux::service::TmuxServiceManager;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

/// Shared daemon state used by all route handlers.
pub struct DaemonState {
    pub agent_state: Arc<AgentState>,
    pub tmux_manager: Arc<TmuxServiceManager>,
    pub installer_registry: Arc<InstallerRegistry>,
}

/// Build the shared routes that both container-daemon and machine-daemon mount.
pub fn shared_routes() -> Router<Arc<DaemonState>> {
    Router::new()
        .route("/health", get(health))
        .route("/api/system", get(get_system_info))
        // Agent routes
        .route("/api/agents/event", post(handle_agent_event))
        .route("/api/agents/sessions", get(get_agent_sessions))
        .route("/api/agents/sessions/all", get(get_agent_sessions_all))
        // Tmux routes
        .route("/api/tmux/services", get(get_tmux_services))
        .route(
            "/api/tmux/services/:name/start",
            post(tmux_service_start),
        )
        .route(
            "/api/tmux/services/:name/stop",
            post(tmux_service_stop),
        )
        .route(
            "/api/tmux/services/:name/restart",
            post(tmux_service_restart),
        )
        .route(
            "/api/tmux/services/:name/logs",
            get(tmux_service_logs),
        )
        // Installer routes
        .route("/api/installers", get(get_installers))
        .route(
            "/api/installers/:name/status",
            get(get_installer_status),
        )
        .route(
            "/api/installers/:name/install",
            post(run_installer),
        )
}

/// Build backward-compatible routes for container-daemon (legacy API).
pub fn legacy_compat_routes() -> Router<Arc<DaemonState>> {
    Router::new()
        .route("/event", post(handle_agent_event_legacy))
        .route("/status", get(get_agent_sessions_legacy))
        .route("/status/all", get(get_agent_sessions_all_legacy))
}

// ── Health ───────────────────────────────────────────────────────────────────

async fn health() -> &'static str {
    "ok"
}

// ── System ───────────────────────────────────────────────────────────────────

async fn get_system_info() -> Json<ApiResponse<SystemInfo>> {
    Json(ApiResponse::ok(SystemInfo::collect()))
}

// ── Agent (new API) ──────────────────────────────────────────────────────────

async fn handle_agent_event(
    State(state): State<Arc<DaemonState>>,
    Json(ev): Json<HookEvent>,
) -> Json<ApiResponse<()>> {
    state.agent_state.process_event(ev).await;
    Json(ApiResponse::ok(()))
}

async fn get_agent_sessions(
    State(state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<SessionSnapshot>>> {
    Json(ApiResponse::ok(
        state.agent_state.list_sessions(false).await,
    ))
}

async fn get_agent_sessions_all(
    State(state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<SessionSnapshot>>> {
    Json(ApiResponse::ok(
        state.agent_state.list_sessions(true).await,
    ))
}

// ── Agent (legacy compat) ────────────────────────────────────────────────────

async fn handle_agent_event_legacy(
    State(state): State<Arc<DaemonState>>,
    Json(ev): Json<HookEvent>,
) -> StatusCode {
    state.agent_state.process_event(ev).await;
    StatusCode::OK
}

async fn get_agent_sessions_legacy(
    State(state): State<Arc<DaemonState>>,
) -> Json<Vec<SessionSnapshot>> {
    Json(state.agent_state.list_sessions(false).await)
}

async fn get_agent_sessions_all_legacy(
    State(state): State<Arc<DaemonState>>,
) -> Json<Vec<SessionSnapshot>> {
    Json(state.agent_state.list_sessions(true).await)
}

// ── Tmux ─────────────────────────────────────────────────────────────────────

async fn get_tmux_services(
    State(state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<crate::tmux::service::TmuxServiceStatus>>> {
    let statuses = state.tmux_manager.list_statuses().await;
    Json(ApiResponse::ok(statuses))
}

async fn tmux_service_start(
    State(state): State<Arc<DaemonState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<String>> {
    match state.tmux_manager.start_service(&name).await {
        Ok(()) => Json(ApiResponse::ok(format!("Service '{name}' started"))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn tmux_service_stop(
    State(state): State<Arc<DaemonState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<String>> {
    match state.tmux_manager.stop_service(&name).await {
        Ok(()) => Json(ApiResponse::ok(format!("Service '{name}' stopped"))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn tmux_service_restart(
    State(state): State<Arc<DaemonState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<String>> {
    match state.tmux_manager.restart_service(&name).await {
        Ok(()) => Json(ApiResponse::ok(format!("Service '{name}' restarted"))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn tmux_service_logs(
    State(state): State<Arc<DaemonState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<String>> {
    match state.tmux_manager.capture_logs(&name).await {
        Ok(logs) => Json(ApiResponse::ok(logs)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

// ── Installers ───────────────────────────────────────────────────────────────

async fn get_installers(
    State(state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<crate::installer::InstallerInfo>>> {
    let infos = state.installer_registry.list_infos().await;
    Json(ApiResponse::ok(infos))
}

async fn get_installer_status(
    State(state): State<Arc<DaemonState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<crate::installer::InstallerStatus>> {
    match state.installer_registry.check_status(&name).await {
        Ok(status) => Json(ApiResponse::ok(status)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn run_installer(
    State(state): State<Arc<DaemonState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<crate::installer::InstallResult>> {
    match state.installer_registry.install(&name).await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}
