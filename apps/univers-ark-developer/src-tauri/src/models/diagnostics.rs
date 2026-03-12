use serde::Serialize;
use std::collections::BTreeMap;

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
pub(crate) struct SecretManagementDiagnostics {
    pub(crate) db_path: String,
    pub(crate) store_backend: String,
    pub(crate) provider_count: usize,
    pub(crate) credential_count: usize,
    pub(crate) assignment_count: usize,
    pub(crate) audit_event_count: usize,
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
