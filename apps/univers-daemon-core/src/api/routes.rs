use crate::agent::event::{HookEvent, SessionSnapshot};
use crate::agents::AgentStatus;
use crate::api::response::ApiResponse;
use crate::app::{AppSpec, AppStatus};
use crate::application::agent::{AgentApplicationService, AgentLaunchRequest, AgentRuntimeView};
use crate::application::agent_session::AgentSessionApplicationService;
use crate::application::catalog::CatalogQueryService;
use crate::application::installer::InstallerApplicationService;
use crate::application::workspace::WorkspaceApplicationService;
use crate::system::SystemInfo;
use crate::tmux::workspace::{WindowStatus, WorkspaceStatus};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

/// Shared daemon state used by all route handlers.
pub struct DaemonState {
    pub agent_sessions: Arc<AgentSessionApplicationService>,
    pub workspace_service: Arc<WorkspaceApplicationService>,
    pub catalog_service: Arc<CatalogQueryService>,
    pub agent_service: Arc<AgentApplicationService>,
    pub installer_service: Arc<InstallerApplicationService>,
}

/// Build the shared routes that both container-daemon and machine-daemon mount.
pub fn shared_routes() -> Router<Arc<DaemonState>> {
    Router::new()
        .route("/health", get(health))
        .route("/api/system", get(get_system_info))
        .route("/api/apps", get(get_apps))
        .route("/api/apps/catalog", get(get_app_specs))
        .route("/api/apps/:id", get(get_app_status))
        .route("/api/agents/catalog", get(get_agents))
        .route("/api/agents/catalog/:id", get(get_agent_status))
        .route("/api/agents/catalog/:id/runtime", get(get_agent_runtime))
        .route("/api/agents/catalog/:id/launch", post(launch_agent))
        .route("/api/agents/catalog/:id/stop", post(stop_agent))
        // Agent routes
        .route("/api/agents/event", post(handle_agent_event))
        .route("/api/agents/sessions", get(get_agent_sessions))
        .route("/api/agents/sessions/all", get(get_agent_sessions_all))
        // Workspace routes
        .route("/api/workspaces", get(get_workspaces))
        .route("/api/workspaces/:workspace_id/start", post(workspace_start))
        .route("/api/workspaces/:workspace_id/stop", post(workspace_stop))
        .route(
            "/api/workspaces/:workspace_id/restart",
            post(workspace_restart),
        )
        .route("/api/workspaces/:workspace_id/logs", get(workspace_logs))
        .route(
            "/api/workspaces/:workspace_id/windows",
            get(get_workspace_windows),
        )
        .route(
            "/api/workspaces/:workspace_id/windows/:window_id/start",
            post(workspace_window_start),
        )
        .route(
            "/api/workspaces/:workspace_id/windows/:window_id/stop",
            post(workspace_window_stop),
        )
        .route(
            "/api/workspaces/:workspace_id/windows/:window_id/restart",
            post(workspace_window_restart),
        )
        .route(
            "/api/workspaces/:workspace_id/windows/:window_id/logs",
            get(workspace_window_logs),
        )
        // Installer routes
        .route("/api/installers", get(get_installers))
        .route("/api/installers/:name/status", get(get_installer_status))
        .route("/api/installers/:name/install", post(run_installer))
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

// ── Apps / Agents catalog ───────────────────────────────────────────────────

async fn get_app_specs(State(state): State<Arc<DaemonState>>) -> Json<ApiResponse<Vec<AppSpec>>> {
    Json(ApiResponse::ok(state.catalog_service.list_app_specs()))
}

async fn get_apps(State(state): State<Arc<DaemonState>>) -> Json<ApiResponse<Vec<AppStatus>>> {
    let statuses = state.catalog_service.list_apps().await;
    Json(ApiResponse::ok(statuses))
}

async fn get_app_status(
    State(state): State<Arc<DaemonState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<AppStatus>> {
    match state.catalog_service.get_app(&id).await {
        Ok(status) => Json(ApiResponse::ok(status)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_agents(State(state): State<Arc<DaemonState>>) -> Json<ApiResponse<Vec<AgentStatus>>> {
    let statuses = state.catalog_service.list_agents().await;
    Json(ApiResponse::ok(statuses))
}

async fn get_agent_status(
    State(state): State<Arc<DaemonState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<AgentStatus>> {
    match state.catalog_service.get_agent(&id).await {
        Ok(status) => Json(ApiResponse::ok(status)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn get_agent_runtime(
    State(state): State<Arc<DaemonState>>,
    Path(id): Path<String>,
) -> Json<ApiResponse<AgentRuntimeView>> {
    match state.agent_service.runtime(&id).await {
        Ok(runtime) => Json(ApiResponse::ok(runtime)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn launch_agent(
    State(state): State<Arc<DaemonState>>,
    Path(id): Path<String>,
    Json(request): Json<AgentLaunchRequest>,
) -> Json<ApiResponse<AgentRuntimeView>> {
    match state.agent_service.launch(&id, request).await {
        Ok(runtime) => Json(ApiResponse::ok(runtime)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

async fn stop_agent(
    State(state): State<Arc<DaemonState>>,
    Path(id): Path<String>,
    Json(request): Json<AgentLaunchRequest>,
) -> Json<ApiResponse<AgentRuntimeView>> {
    match state.agent_service.stop(&id, request).await {
        Ok(runtime) => Json(ApiResponse::ok(runtime)),
        Err(error) => Json(ApiResponse::err(error.to_string())),
    }
}

// ── Agent (new API) ──────────────────────────────────────────────────────────

async fn handle_agent_event(
    State(state): State<Arc<DaemonState>>,
    Json(ev): Json<HookEvent>,
) -> Json<ApiResponse<()>> {
    state.agent_sessions.process_event(ev).await;
    Json(ApiResponse::ok(()))
}

async fn get_agent_sessions(
    State(state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<SessionSnapshot>>> {
    Json(ApiResponse::ok(state.agent_sessions.list_sessions(false).await))
}

async fn get_agent_sessions_all(
    State(state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<SessionSnapshot>>> {
    Json(ApiResponse::ok(state.agent_sessions.list_sessions(true).await))
}

// ── Agent (legacy compat) ────────────────────────────────────────────────────

async fn handle_agent_event_legacy(
    State(state): State<Arc<DaemonState>>,
    Json(ev): Json<HookEvent>,
) -> StatusCode {
    state.agent_sessions.process_event(ev).await;
    StatusCode::OK
}

async fn get_agent_sessions_legacy(
    State(state): State<Arc<DaemonState>>,
) -> Json<Vec<SessionSnapshot>> {
    Json(state.agent_sessions.list_sessions(false).await)
}

async fn get_agent_sessions_all_legacy(
    State(state): State<Arc<DaemonState>>,
) -> Json<Vec<SessionSnapshot>> {
    Json(state.agent_sessions.list_sessions(true).await)
}

// ── Tmux ─────────────────────────────────────────────────────────────────────

async fn get_workspaces(
    State(state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<WorkspaceStatus>>> {
    let workspaces = state.workspace_service.list_workspaces().await;
    Json(ApiResponse::ok(workspaces))
}

async fn get_workspace_windows(
    State(state): State<Arc<DaemonState>>,
    Path(workspace_id): Path<String>,
) -> Json<ApiResponse<Vec<WindowStatus>>> {
    match state.workspace_service.list_windows(&workspace_id).await {
        Ok(windows) => Json(ApiResponse::ok(windows)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn workspace_start(
    State(state): State<Arc<DaemonState>>,
    Path(workspace_id): Path<String>,
) -> Json<ApiResponse<String>> {
    match state.workspace_service.start_workspace(&workspace_id).await {
        Ok(()) => Json(ApiResponse::ok(format!(
            "Workspace '{workspace_id}' started"
        ))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn workspace_stop(
    State(state): State<Arc<DaemonState>>,
    Path(workspace_id): Path<String>,
) -> Json<ApiResponse<String>> {
    match state.workspace_service.stop_workspace(&workspace_id).await {
        Ok(()) => Json(ApiResponse::ok(format!(
            "Workspace '{workspace_id}' stopped"
        ))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn workspace_restart(
    State(state): State<Arc<DaemonState>>,
    Path(workspace_id): Path<String>,
) -> Json<ApiResponse<String>> {
    match state
        .workspace_service
        .restart_workspace(&workspace_id)
        .await
    {
        Ok(()) => Json(ApiResponse::ok(format!(
            "Workspace '{workspace_id}' restarted"
        ))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn workspace_logs(
    State(state): State<Arc<DaemonState>>,
    Path(workspace_id): Path<String>,
) -> Json<ApiResponse<String>> {
    match state
        .workspace_service
        .capture_workspace_logs(&workspace_id)
        .await
    {
        Ok(logs) => Json(ApiResponse::ok(logs)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn workspace_window_start(
    State(state): State<Arc<DaemonState>>,
    Path((workspace_id, window_id)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    match state
        .workspace_service
        .start_window(&workspace_id, &window_id)
        .await
    {
        Ok(()) => Json(ApiResponse::ok(format!(
            "Window '{window_id}' in workspace '{workspace_id}' started"
        ))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn workspace_window_stop(
    State(state): State<Arc<DaemonState>>,
    Path((workspace_id, window_id)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    match state
        .workspace_service
        .stop_window(&workspace_id, &window_id)
        .await
    {
        Ok(()) => Json(ApiResponse::ok(format!(
            "Window '{window_id}' in workspace '{workspace_id}' stopped"
        ))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn workspace_window_restart(
    State(state): State<Arc<DaemonState>>,
    Path((workspace_id, window_id)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    match state
        .workspace_service
        .restart_window(&workspace_id, &window_id)
        .await
    {
        Ok(()) => Json(ApiResponse::ok(format!(
            "Window '{window_id}' in workspace '{workspace_id}' restarted"
        ))),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn workspace_window_logs(
    State(state): State<Arc<DaemonState>>,
    Path((workspace_id, window_id)): Path<(String, String)>,
) -> Json<ApiResponse<String>> {
    match state
        .workspace_service
        .capture_window_logs(&workspace_id, &window_id)
        .await
    {
        Ok(logs) => Json(ApiResponse::ok(logs)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

// ── Installers ───────────────────────────────────────────────────────────────

async fn get_installers(
    State(state): State<Arc<DaemonState>>,
) -> Json<ApiResponse<Vec<crate::installer::InstallerInfo>>> {
    let infos = state.installer_service.list_installers().await;
    Json(ApiResponse::ok(infos))
}

async fn get_installer_status(
    State(state): State<Arc<DaemonState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<crate::installer::InstallerStatus>> {
    match state.installer_service.installer_status(&name).await {
        Ok(status) => Json(ApiResponse::ok(status)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}

async fn run_installer(
    State(state): State<Arc<DaemonState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<crate::installer::InstallResult>> {
    match state.installer_service.install(&name).await {
        Ok(result) => Json(ApiResponse::ok(result)),
        Err(e) => Json(ApiResponse::err(e.to_string())),
    }
}
