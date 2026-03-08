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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserSurface {
    pub(crate) id: String,
    pub(crate) label: String,
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
    pub(crate) proxy: Option<LocalProxyHandle>,
    pub(crate) ready: Arc<AtomicBool>,
}

#[derive(Clone, Default)]
pub(crate) struct TerminalState {
    pub(crate) sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
}

pub(crate) struct TunnelState {
    pub(crate) sessions: Arc<Mutex<HashMap<String, TunnelSession>>>,
    pub(crate) local_ports: Arc<Mutex<HashMap<String, u16>>>,
    pub(crate) next_session_id: AtomicU64,
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
            local_ports: Arc::new(Mutex::new(HashMap::new())),
            next_session_id: AtomicU64::new(1),
        }
    }
}
