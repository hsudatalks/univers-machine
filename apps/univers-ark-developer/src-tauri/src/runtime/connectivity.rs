mod cycle;
mod monitor;
mod probe;
mod status;

use std::time::Duration;

pub(crate) use self::cycle::{apply_connectivity_snapshots, run_connectivity_scheduler_cycle};
pub(crate) use self::monitor::ConnectivitySchedulerState;

const CONNECTIVITY_MONITOR_TICK: Duration = Duration::from_secs(2);
const CONNECTIVITY_INVENTORY_REFRESH_INTERVAL: Duration = Duration::from_secs(10);
pub(super) const CONNECTIVITY_ACTIVE_RECHECK_INTERVAL: Duration = Duration::from_secs(15);
pub(super) const CONNECTIVITY_READY_RECHECK_INTERVAL: Duration = Duration::from_secs(60);
pub(super) const CONNECTIVITY_CHECKING_RETRY_INTERVAL: Duration = Duration::from_secs(5);
pub(super) const CONNECTIVITY_ERROR_BACKOFF_BASE: Duration = Duration::from_secs(10);
pub(super) const CONNECTIVITY_ERROR_BACKOFF_MAX: Duration = Duration::from_secs(300);
pub(super) const CONNECTIVITY_PROBE_COMMAND: &str = "uname -s";
