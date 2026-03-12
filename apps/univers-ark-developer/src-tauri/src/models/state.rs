use super::{
    ConnectivitySnapshot, ConnectivityTelemetry, DashboardTelemetry, SchedulerTelemetry,
    ServiceRegistration, ServiceStatus, TerminalSession, TunnelRegistration, TunnelSession,
    TunnelStatus, TunnelTelemetry,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, Mutex,
    },
};

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
