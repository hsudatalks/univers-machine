use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    process::Child,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use univers_ark_russh::{LocalForward, PtySession};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BrowserServiceType {
    #[default]
    Http,
    Vite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DeveloperServiceKind {
    #[serde(alias = "browser")]
    #[default]
    Web,
    Endpoint,
    Command,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EndpointProbeType {
    #[default]
    Http,
    Tcp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MachineTransport {
    Local,
    #[default]
    Ssh,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ManagedContainerKind {
    Host,
    #[default]
    Managed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserSurface {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) service_type: BrowserServiceType,
    #[serde(default)]
    pub(crate) background_prerender: bool,
    pub(crate) tunnel_command: String,
    pub(crate) local_url: String,
    pub(crate) remote_url: String,
    #[serde(default)]
    pub(crate) vite_hmr_tunnel_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeveloperService {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) kind: DeveloperServiceKind,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    #[serde(alias = "browser")]
    pub(crate) web: Option<BrowserSurface>,
    #[serde(default)]
    pub(crate) endpoint: Option<EndpointService>,
    #[serde(default)]
    pub(crate) command: Option<CommandService>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandService {
    #[serde(default)]
    pub(crate) restart: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerWorkspace {
    #[serde(default)]
    pub(crate) profile: String,
    #[serde(default)]
    pub(crate) default_tool: String,
    #[serde(default)]
    pub(crate) project_path: String,
    #[serde(default)]
    pub(crate) files_root: String,
    #[serde(default)]
    #[serde(alias = "primaryBrowserServiceId")]
    pub(crate) primary_web_service_id: String,
    #[serde(default)]
    pub(crate) tmux_command_service_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EndpointService {
    #[serde(default)]
    pub(crate) probe_type: EndpointProbeType,
    #[serde(default)]
    pub(crate) host: String,
    pub(crate) port: u16,
    #[serde(default)]
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeveloperTarget {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) machine_id: String,
    #[serde(default)]
    pub(crate) container_id: String,
    #[serde(default)]
    pub(crate) transport: MachineTransport,
    #[serde(default)]
    pub(crate) container_kind: ManagedContainerKind,
    pub(crate) label: String,
    pub(crate) host: String,
    pub(crate) description: String,
    pub(crate) terminal_command: String,
    #[serde(default)]
    pub(crate) terminal_startup_command: String,
    #[serde(default)]
    pub(crate) notes: Vec<String>,
    #[serde(default)]
    pub(crate) workspace: ContainerWorkspace,
    #[serde(default)]
    pub(crate) services: Vec<DeveloperService>,
    #[serde(default)]
    pub(crate) surfaces: Vec<BrowserSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TargetsFile {
    pub(crate) selected_target_id: Option<String>,
    pub(crate) default_profile: Option<String>,
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
    pub(crate) machines: Vec<ManagedServer>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedContainer {
    pub(crate) server_id: String,
    pub(crate) server_label: String,
    pub(crate) container_id: String,
    pub(crate) kind: ManagedContainerKind,
    pub(crate) transport: MachineTransport,
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
    pub(crate) host_target_id: String,
    pub(crate) label: String,
    pub(crate) transport: MachineTransport,
    pub(crate) host: String,
    pub(crate) description: String,
    pub(crate) state: String,
    pub(crate) message: String,
    pub(crate) containers: Vec<ManagedContainer>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportedMachineJump {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) user: String,
    pub(crate) identity_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MachineImportCandidate {
    pub(crate) import_id: String,
    pub(crate) machine_id: String,
    pub(crate) label: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) ssh_user: String,
    pub(crate) identity_files: Vec<String>,
    pub(crate) jump_chain: Vec<ImportedMachineJump>,
    pub(crate) description: String,
    pub(crate) detail: String,
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
    pub(crate) service_id: String,
    pub(crate) surface_id: String,
    pub(crate) local_url: Option<String>,
    pub(crate) state: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ConnectivitySnapshot {
    pub(crate) state: String,
    pub(crate) message: String,
    pub(crate) reachable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectivityStatusEvent {
    pub(crate) entity: String,
    pub(crate) machine_id: String,
    pub(crate) target_id: Option<String>,
    pub(crate) state: String,
    pub(crate) message: String,
    pub(crate) reachable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServiceStatus {
    pub(crate) target_id: String,
    pub(crate) service_id: String,
    pub(crate) kind: DeveloperServiceKind,
    pub(crate) state: String,
    pub(crate) message: String,
    pub(crate) local_url: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ServiceRegistration {
    pub(crate) kind: DeveloperServiceKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PortRangeDiagnostics {
    pub(crate) start: u16,
    pub(crate) end: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeActivityDiagnostics {
    pub(crate) visible: bool,
    pub(crate) focused: bool,
    pub(crate) online: bool,
    pub(crate) recovering: bool,
    pub(crate) recovery_generation: u64,
    pub(crate) last_recovery_started_at_ms: u64,
    pub(crate) active_machine_id: Option<String>,
    pub(crate) active_target_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchedulerBudgetDiagnostics {
    pub(crate) max_tunnel_reconciles: usize,
    pub(crate) max_connectivity_probes: usize,
    pub(crate) max_dashboard_refreshes: usize,
    pub(crate) next_wake_in_ms: u64,
    pub(crate) last_cycle_started_at_ms: u64,
    pub(crate) last_cycle_finished_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TunnelDiagnostics {
    pub(crate) desired_count: usize,
    pub(crate) session_count: usize,
    pub(crate) ready_session_count: usize,
    pub(crate) local_port_count: usize,
    pub(crate) status_counts: BTreeMap<String, usize>,
    pub(crate) status_events_per_minute: usize,
    pub(crate) status_items_per_minute: usize,
    pub(crate) reconciles_per_minute: usize,
    pub(crate) next_due_in_ms: u64,
    pub(crate) due_now_count: usize,
    pub(crate) waiting_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectivityDiagnostics {
    pub(crate) machine_snapshot_count: usize,
    pub(crate) container_snapshot_count: usize,
    pub(crate) machine_state_counts: BTreeMap<String, usize>,
    pub(crate) container_state_counts: BTreeMap<String, usize>,
    pub(crate) status_events_per_minute: usize,
    pub(crate) status_items_per_minute: usize,
    pub(crate) probes_per_minute: usize,
    pub(crate) next_due_in_ms: u64,
    pub(crate) due_now_count: usize,
    pub(crate) backoff_target_count: usize,
    pub(crate) max_consecutive_failures: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DashboardDiagnostics {
    pub(crate) registered_count: usize,
    pub(crate) updates_per_minute: usize,
    pub(crate) next_due_in_ms: u64,
    pub(crate) due_now_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalDiagnostics {
    pub(crate) session_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppDiagnostics {
    pub(crate) process_id: u32,
    pub(crate) channel: String,
    pub(crate) config_path: String,
    pub(crate) surface_ports: PortRangeDiagnostics,
    pub(crate) internal_tunnel_ports: PortRangeDiagnostics,
    pub(crate) activity: RuntimeActivityDiagnostics,
    pub(crate) scheduler: SchedulerBudgetDiagnostics,
    pub(crate) terminals: TerminalDiagnostics,
    pub(crate) tunnels: TunnelDiagnostics,
    pub(crate) connectivity: ConnectivityDiagnostics,
    pub(crate) dashboards: DashboardDiagnostics,
    pub(crate) secret_management: SecretManagementDiagnostics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretManagementDiagnostics {
    pub(crate) db_path: String,
    pub(crate) store_backend: String,
    pub(crate) provider_count: usize,
    pub(crate) credential_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) audit_event_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SecretAssignmentTargetKind {
    Machine,
    Container,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretProviderRecord {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) provider_kind: String,
    pub(crate) base_url: String,
    pub(crate) description: String,
    pub(crate) created_at_ms: u64,
    pub(crate) updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretCredentialRecord {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) label: String,
    pub(crate) description: String,
    pub(crate) has_secret: bool,
    pub(crate) secret_backend: String,
    pub(crate) created_at_ms: u64,
    pub(crate) updated_at_ms: u64,
    pub(crate) last_rotated_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretAssignmentRecord {
    pub(crate) id: String,
    pub(crate) credential_id: String,
    pub(crate) target_kind: SecretAssignmentTargetKind,
    pub(crate) target_id: String,
    pub(crate) env_var: String,
    pub(crate) file_path: String,
    pub(crate) enabled: bool,
    pub(crate) created_at_ms: u64,
    pub(crate) updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretAuditEventRecord {
    pub(crate) id: i64,
    pub(crate) event_kind: String,
    pub(crate) entity_kind: String,
    pub(crate) entity_id: String,
    pub(crate) detail: String,
    pub(crate) created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretInventory {
    pub(crate) db_path: String,
    pub(crate) store_backend: String,
    pub(crate) providers: Vec<SecretProviderRecord>,
    pub(crate) credentials: Vec<SecretCredentialRecord>,
    pub(crate) assignments: Vec<SecretAssignmentRecord>,
    pub(crate) audit_events: Vec<SecretAuditEventRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretProviderInput {
    #[serde(default)]
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) provider_kind: String,
    #[serde(default)]
    pub(crate) base_url: String,
    #[serde(default)]
    pub(crate) description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretCredentialInput {
    #[serde(default)]
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) secret_value: Option<String>,
    #[serde(default)]
    pub(crate) clear_secret: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecretAssignmentInput {
    #[serde(default)]
    pub(crate) id: String,
    pub(crate) credential_id: String,
    pub(crate) target_kind: SecretAssignmentTargetKind,
    pub(crate) target_id: String,
    #[serde(default)]
    pub(crate) env_var: String,
    #[serde(default)]
    pub(crate) file_path: String,
    #[serde(default = "default_secret_assignment_enabled")]
    pub(crate) enabled: bool,
}

fn default_secret_assignment_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RateWindow {
    pub(crate) entries: VecDeque<(Instant, usize)>,
}

impl RateWindow {
    const WINDOW: Duration = Duration::from_secs(60);

    fn prune(&mut self, now: Instant) {
        while let Some((at, _)) = self.entries.front() {
            if now.saturating_duration_since(*at) <= Self::WINDOW {
                break;
            }
            self.entries.pop_front();
        }
    }

    pub(crate) fn record(&mut self, now: Instant, count: usize) {
        if count == 0 {
            self.prune(now);
            return;
        }
        self.entries.push_back((now, count));
        self.prune(now);
    }

    pub(crate) fn per_minute(&self, now: Instant) -> usize {
        self.entries
            .iter()
            .filter(|(at, _)| now.saturating_duration_since(*at) <= Self::WINDOW)
            .map(|(_, count)| *count)
            .sum()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TunnelTelemetry {
    pub(crate) status_events: RateWindow,
    pub(crate) status_items: RateWindow,
    pub(crate) reconciles: RateWindow,
    pub(crate) next_due_in_ms: u64,
    pub(crate) due_now_count: usize,
    pub(crate) waiting_count: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ConnectivityTelemetry {
    pub(crate) status_events: RateWindow,
    pub(crate) status_items: RateWindow,
    pub(crate) probes: RateWindow,
    pub(crate) next_due_in_ms: u64,
    pub(crate) due_now_count: usize,
    pub(crate) backoff_target_count: usize,
    pub(crate) max_consecutive_failures: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DashboardTelemetry {
    pub(crate) updates: RateWindow,
    pub(crate) next_due_in_ms: u64,
    pub(crate) due_now_count: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SchedulerTelemetry {
    pub(crate) next_wake_in_ms: u64,
    pub(crate) last_cycle_started_at_ms: u64,
    pub(crate) last_cycle_finished_at_ms: u64,
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
pub(crate) struct RusshTerminalSession {
    pub(crate) session: PtySession,
}

#[derive(Clone)]
pub(crate) struct TerminalSession {
    pub(crate) russh: RusshTerminalSession,
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
    pub(crate) service_id: String,
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
    pub(crate) status_snapshots: Arc<Mutex<HashMap<String, TunnelStatus>>>,
    pub(crate) telemetry: Arc<Mutex<TunnelTelemetry>>,
    pub(crate) next_session_id: AtomicU64,
}

#[derive(Clone)]
pub(crate) struct DashboardMonitor {
    pub(crate) refresh_seconds: u64,
}

#[derive(Clone, Default)]
pub(crate) struct DashboardState {
    pub(crate) sessions: Arc<Mutex<HashMap<String, DashboardMonitor>>>,
    pub(crate) telemetry: Arc<Mutex<DashboardTelemetry>>,
}

#[derive(Clone, Default)]
pub(crate) struct ServiceState {
    pub(crate) registrations: Arc<Mutex<HashMap<String, ServiceRegistration>>>,
    pub(crate) statuses: Arc<Mutex<HashMap<String, ServiceStatus>>>,
}

#[derive(Clone, Default)]
pub(crate) struct ConnectivityState {
    pub(crate) machine_snapshots: Arc<Mutex<HashMap<String, ConnectivitySnapshot>>>,
    pub(crate) target_snapshots: Arc<Mutex<HashMap<String, ConnectivitySnapshot>>>,
    pub(crate) telemetry: Arc<Mutex<ConnectivityTelemetry>>,
}

#[derive(Clone)]
pub(crate) struct RuntimeActivityState {
    pub(crate) visible: Arc<AtomicBool>,
    pub(crate) focused: Arc<AtomicBool>,
    pub(crate) online: Arc<AtomicBool>,
    pub(crate) recovering_until_ms: Arc<AtomicU64>,
    pub(crate) last_recovery_started_at_ms: Arc<AtomicU64>,
    pub(crate) recovery_generation: Arc<AtomicU64>,
    pub(crate) active_machine_id: Arc<Mutex<Option<String>>>,
    pub(crate) active_target_id: Arc<Mutex<Option<String>>>,
}

#[derive(Clone, Default)]
pub(crate) struct SchedulerState {
    pub(crate) stop_requested: Arc<AtomicBool>,
    pub(crate) telemetry: Arc<Mutex<SchedulerTelemetry>>,
}

impl Clone for TunnelState {
    fn clone(&self) -> Self {
        Self {
            sessions: self.sessions.clone(),
            desired_tunnels: self.desired_tunnels.clone(),
            local_ports: self.local_ports.clone(),
            status_snapshots: self.status_snapshots.clone(),
            telemetry: self.telemetry.clone(),
            next_session_id: AtomicU64::new(
                self.next_session_id
                    .load(std::sync::atomic::Ordering::Relaxed),
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
            status_snapshots: Arc::new(Mutex::new(HashMap::new())),
            telemetry: Arc::new(Mutex::new(TunnelTelemetry::default())),
            next_session_id: AtomicU64::new(1),
        }
    }
}

impl Default for RuntimeActivityState {
    fn default() -> Self {
        Self {
            visible: Arc::new(AtomicBool::new(true)),
            focused: Arc::new(AtomicBool::new(true)),
            online: Arc::new(AtomicBool::new(true)),
            recovering_until_ms: Arc::new(AtomicU64::new(0)),
            last_recovery_started_at_ms: Arc::new(AtomicU64::new(0)),
            recovery_generation: Arc::new(AtomicU64::new(0)),
            active_machine_id: Arc::new(Mutex::new(None)),
            active_target_id: Arc::new(Mutex::new(None)),
        }
    }
}
