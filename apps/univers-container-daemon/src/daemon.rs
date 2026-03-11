use axum::routing::get;
use axum::{extract::State, response::Json, Router};
use serde::Serialize;
use std::sync::Arc;
use tracing::info;
use univers_daemon_core::agent::state::AgentState;
use univers_daemon_core::api::response::ApiResponse;
use univers_daemon_core::api::routes::{shared_routes, legacy_compat_routes, DaemonState};
use univers_daemon_core::installer::InstallerRegistry;
use univers_daemon_core::tmux::service::TmuxServiceManager;

/// Container-specific info.
#[derive(Debug, Clone, Serialize)]
struct ContainerInfo {
    container_id: Option<String>,
    image: Option<String>,
    hostname: String,
    mounts: Vec<String>,
}

pub async fn run_daemon(port: u16) -> anyhow::Result<()> {
    let agent_state = AgentState::new();
    let tmux_manager = Arc::new(TmuxServiceManager::new());
    let installer_registry = Arc::new(InstallerRegistry::with_defaults());

    let daemon_state = Arc::new(DaemonState {
        agent_state,
        tmux_manager,
        installer_registry,
    });

    let app = Router::new()
        // Container-specific route
        .route("/api/container", get(get_container_info))
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
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| {
        sysinfo::System::host_name().unwrap_or_else(|| "unknown".into())
    });

    // Try to detect container ID from cgroup
    let container_id = std::fs::read_to_string("/proc/1/cgroup")
        .ok()
        .and_then(|cgroup| {
            for line in cgroup.lines() {
                if let Some(id) = line.rsplit('/').next() {
                    if id.len() >= 12 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Some(id[..12].to_string());
                    }
                }
            }
            None
        });

    let image = std::env::var("CONTAINER_IMAGE").ok();

    // Detect mounts from /proc/mounts
    let mounts = std::fs::read_to_string("/proc/mounts")
        .ok()
        .map(|content| {
            content
                .lines()
                .filter(|line| {
                    // Filter to interesting mounts (bind mounts, volumes)
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let fstype = parts[2];
                        // Skip pseudo filesystems
                        !matches!(
                            fstype,
                            "proc" | "sysfs" | "devpts" | "tmpfs" | "cgroup" | "cgroup2"
                                | "mqueue" | "devtmpfs" | "securityfs" | "debugfs"
                                | "pstore" | "fusectl" | "hugetlbfs" | "bpf"
                        )
                    } else {
                        false
                    }
                })
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        Some(parts[1].to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Json(ApiResponse::ok(ContainerInfo {
        container_id,
        image,
        hostname,
        mounts,
    }))
}
