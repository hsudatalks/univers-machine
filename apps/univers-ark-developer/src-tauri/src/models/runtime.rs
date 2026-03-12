use super::DeveloperServiceKind;
use serde::Serialize;
use std::{
    collections::VecDeque,
    process::Child,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::{Duration, Instant},
};
use univers_ark_russh::{LocalForward, PtySession};

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
