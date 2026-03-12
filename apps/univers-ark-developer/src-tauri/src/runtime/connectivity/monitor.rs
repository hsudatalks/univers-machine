use super::{
    probe::{prune_probe_schedules, ProbeSchedule},
    status::{prune_snapshots, seed_missing_snapshots},
    CONNECTIVITY_INVENTORY_REFRESH_INTERVAL,
};
use crate::{
    machine::read_server_inventory,
    models::{ConnectivityState, ConnectivityStatusEvent, ManagedServer},
};
use std::{collections::HashMap, time::Instant};

#[derive(Clone)]
pub(super) struct InventoryCache {
    pub(super) machines: Vec<ManagedServer>,
    pub(super) refreshed_at: Instant,
}

#[derive(Default)]
pub(super) struct ConnectivityMonitorState {
    pub(super) inventory_cache: Option<InventoryCache>,
    pub(super) probe_schedules: HashMap<String, ProbeSchedule>,
}

pub(crate) struct ConnectivitySchedulerState {
    pub(super) monitor_state: ConnectivityMonitorState,
    pub(super) last_tick_at: Instant,
    pub(super) last_recovery_generation: u64,
}

impl Default for ConnectivitySchedulerState {
    fn default() -> Self {
        Self {
            monitor_state: ConnectivityMonitorState::default(),
            last_tick_at: Instant::now(),
            last_recovery_generation: 0,
        }
    }
}

pub(super) fn load_inventory(
    connectivity_state: &ConnectivityState,
    monitor_state: &mut ConnectivityMonitorState,
    now: Instant,
    pending_events: &mut Vec<ConnectivityStatusEvent>,
) -> Option<Vec<ManagedServer>> {
    let should_refresh = monitor_state
        .inventory_cache
        .as_ref()
        .map(|cache| {
            now.duration_since(cache.refreshed_at) >= CONNECTIVITY_INVENTORY_REFRESH_INTERVAL
        })
        .unwrap_or(true);

    if should_refresh {
        match read_server_inventory(false) {
            Ok(machines) => {
                seed_missing_snapshots(connectivity_state, &machines, pending_events);
                prune_snapshots(connectivity_state, &machines);
                prune_probe_schedules(monitor_state, &machines);
                monitor_state.inventory_cache = Some(InventoryCache {
                    machines: machines.clone(),
                    refreshed_at: now,
                });
            }
            Err(error) => {
                eprintln!(
                    "Failed to load machine inventory for connectivity monitor: {}",
                    error
                );
            }
        }
    }

    monitor_state
        .inventory_cache
        .as_ref()
        .map(|cache| cache.machines.clone())
}
