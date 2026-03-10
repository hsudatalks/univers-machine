use portable_pty::MasterPty;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Write,
    process::Child,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, Mutex,
    },
    time::Instant,
};
use univers_ark_russh::LocalForward;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BrowserServiceType {
    #[default]
    Http,
    Vite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserSurface {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) service_type: BrowserServiceType,
    pub(crate) tunnel_command: String,
    pub(crate) local_url: String,
    pub(crate) remote_url: String,
    #[serde(default)]
    pub(crate) vite_hmr_tunnel_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeveloperTarget {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) host: String,
    pub(crate) description: String,
    pub(crate) terminal_command: String,
    #[serde(default)]
    pub(crate) notes: Vec<String>,
    pub(crate) surfaces: Vec<BrowserSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TargetsFile {
    pub(crate) selected_target_id: Option<String>,
    pub(crate) targets: Vec<DeveloperTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) theme_mode: String,
    pub(crate) dashboard_refresh_seconds: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: String::from("system"),
            dashboard_refresh_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppBootstrap {
    pub(crate) app_name: String,
    pub(crate) config_path: String,
    pub(crate) selected_target_id: Option<String>,
    pub(crate) targets: Vec<DeveloperTarget>,
    pub(crate) servers: Vec<ManagedServer>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedContainer {
    pub(crate) server_id: String,
    pub(crate) server_label: String,
    pub(crate) target_id: String,
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) ipv4: String,
    pub(crate) ssh_user: String,
    pub(crate) ssh_destination: String,
    pub(crate) ssh_command: String,
    pub(crate) ssh_state: String,
    pub(crate) ssh_message: String,
    pub(crate) ssh_reachable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedServer {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) host: String,
    pub(crate) description: String,
    pub(crate) state: String,
    pub(crate) message: String,
    pub(crate) containers: Vec<ManagedContainer>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteFileEntry {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) size: u64,
    pub(crate) is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteDirectoryListing {
    pub(crate) target_id: String,
    pub(crate) path: String,
    pub(crate) parent_path: Option<String>,
    pub(crate) entries: Vec<RemoteFileEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteFilePreview {
    pub(crate) target_id: String,
    pub(crate) path: String,
    pub(crate) content: String,
    pub(crate) is_binary: bool,
    pub(crate) truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerProjectInfo {
    pub(crate) project_path: String,
    pub(crate) repo_found: bool,
    pub(crate) branch: Option<String>,
    pub(crate) is_dirty: bool,
    pub(crate) changed_files: u64,
    pub(crate) head_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerRuntimeInfo {
    pub(crate) hostname: String,
    pub(crate) uptime_seconds: u64,
    pub(crate) process_count: u64,
    pub(crate) load_average_1m: f64,
    pub(crate) load_average_5m: f64,
    pub(crate) load_average_15m: f64,
    pub(crate) memory_total_bytes: u64,
    pub(crate) memory_used_bytes: u64,
    pub(crate) disk_total_bytes: u64,
    pub(crate) disk_used_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerServiceInfo {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) status: String,
    pub(crate) detail: String,
    pub(crate) url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerAgentInfo {
    pub(crate) active_agent: String,
    pub(crate) source: String,
    pub(crate) last_activity: Option<String>,
    pub(crate) latest_report: Option<String>,
    pub(crate) latest_report_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerTmuxSessionInfo {
    pub(crate) server: String,
    pub(crate) name: String,
    pub(crate) windows: u64,
    pub(crate) attached: bool,
    pub(crate) active_command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerTmuxInfo {
    pub(crate) installed: bool,
    pub(crate) server_running: bool,
    pub(crate) session_count: u64,
    pub(crate) attached_count: u64,
    pub(crate) active_session: Option<String>,
    pub(crate) active_command: Option<String>,
    pub(crate) sessions: Vec<ContainerTmuxSessionInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerDashboard {
    pub(crate) target_id: String,
    pub(crate) project: ContainerProjectInfo,
    pub(crate) runtime: ContainerRuntimeInfo,
    pub(crate) services: Vec<ContainerServiceInfo>,
    pub(crate) agent: ContainerAgentInfo,
    pub(crate) tmux: ContainerTmuxInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerDashboardUpdate {
    pub(crate) target_id: String,
    pub(crate) dashboard: Option<ContainerDashboard>,
    pub(crate) error: Option<String>,
    pub(crate) refreshed_at_ms: u64,
    pub(crate) refresh_seconds: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalSnapshot {
    pub(crate) target_id: String,
    pub(crate) output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalOutputEvent {
    pub(crate) target_id: String,
    pub(crate) data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalExitEvent {
    pub(crate) target_id: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TunnelStatus {
    pub(crate) target_id: String,
    pub(crate) surface_id: String,
    pub(crate) local_url: Option<String>,
    pub(crate) state: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequestSummary {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) author_login: String,
    pub(crate) head_ref_name: String,
    pub(crate) is_draft: bool,
    pub(crate) state: String,
    pub(crate) review_decision: Option<String>,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubRepositoryStatus {
    pub(crate) name_with_owner: String,
    pub(crate) description: String,
    pub(crate) url: String,
    pub(crate) default_branch: String,
    pub(crate) viewer_login: String,
    pub(crate) local_repo_path: Option<String>,
    pub(crate) local_branch: Option<String>,
    pub(crate) local_status_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubProjectState {
    pub(crate) repository: GithubRepositoryStatus,
    pub(crate) current_branch_pr: Option<GithubPullRequestSummary>,
    pub(crate) my_open_prs: Vec<GithubPullRequestSummary>,
    pub(crate) open_prs: Vec<GithubPullRequestSummary>,
    pub(crate) closed_prs: Vec<GithubPullRequestSummary>,
    pub(crate) merged_prs: Vec<GithubPullRequestSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequestFile {
    pub(crate) path: String,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequestReview {
    pub(crate) author_login: String,
    pub(crate) state: String,
    pub(crate) body: String,
    pub(crate) submitted_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubStatusCheck {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) conclusion: Option<String>,
    pub(crate) workflow_name: Option<String>,
    pub(crate) details_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubPullRequestDetail {
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) url: String,
    pub(crate) author_login: String,
    pub(crate) head_ref_name: String,
    pub(crate) base_ref_name: String,
    pub(crate) is_draft: bool,
    pub(crate) state: String,
    pub(crate) review_decision: Option<String>,
    pub(crate) updated_at: String,
    pub(crate) merge_state_status: String,
    pub(crate) mergeable: String,
    pub(crate) changed_files: u64,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
    pub(crate) files: Vec<GithubPullRequestFile>,
    pub(crate) latest_reviews: Vec<GithubPullRequestReview>,
    pub(crate) status_checks: Vec<GithubStatusCheck>,
}

#[derive(Clone)]
pub(crate) struct TerminalSession {
    pub(crate) master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    pub(crate) writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub(crate) output: Arc<Mutex<String>>,
}

#[derive(Clone)]
pub(crate) struct TunnelProcess {
    pub(crate) label: String,
    pub(crate) child: Arc<Mutex<Child>>,
    pub(crate) output: Arc<Mutex<String>>,
}

#[derive(Clone)]
pub(crate) struct RusshTunnelForward {
    pub(crate) label: String,
    pub(crate) forward: LocalForward,
}

#[derive(Clone)]
pub(crate) struct LocalProxyHandle {
    pub(crate) stop_requested: Arc<AtomicBool>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) error: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
pub(crate) struct TunnelSession {
    pub(crate) session_id: u64,
    pub(crate) started_at: Instant,
    pub(crate) processes: Vec<TunnelProcess>,
    pub(crate) russh_forwards: Vec<RusshTunnelForward>,
    pub(crate) proxy: Option<LocalProxyHandle>,
    pub(crate) ready: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(crate) struct TunnelRegistration {
    pub(crate) target_id: String,
    pub(crate) surface_id: String,
    pub(crate) next_attempt_at: Instant,
}

#[derive(Clone, Default)]
pub(crate) struct TerminalState {
    pub(crate) sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
}

pub(crate) struct TunnelState {
    pub(crate) sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    pub(crate) desired_tunnels: Arc<Mutex<HashMap<String, TunnelRegistration>>>,
    pub(crate) local_ports: Arc<Mutex<HashMap<String, u16>>>,
    pub(crate) next_session_id: AtomicU64,
}

#[derive(Clone)]
pub(crate) struct DashboardMonitor {
    pub(crate) stop_requested: Arc<AtomicBool>,
}

#[derive(Clone, Default)]
pub(crate) struct DashboardState {
    pub(crate) sessions: Arc<Mutex<HashMap<String, DashboardMonitor>>>,
}

impl Clone for TunnelState {
    fn clone(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
            desired_tunnels: self.desired_tunnels.clone(),
            local_ports: self.local_ports.clone(),
            next_session_id: AtomicU64::new(
                self.next_session_id.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ManagedTunnelSignature {
    pub(crate) ssh_destination: String,
    pub(crate) remote_host: String,
    pub(crate) remote_port: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct ObservedTunnelProcess {
    pub(crate) pid: u32,
    pub(crate) local_port: u16,
    pub(crate) ssh_destination: String,
    pub(crate) remote_host: String,
    pub(crate) remote_port: u16,
}

impl Default for TunnelState {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            desired_tunnels: Arc::new(Mutex::new(HashMap::new())),
            local_ports: Arc::new(Mutex::new(HashMap::new())),
            next_session_id: AtomicU64::new(1),
        }
    }
}
