mod cleanup;
mod forwarding;
mod proxy;
mod registry;
mod session;
mod status;
mod supervisor;

use crate::constants::{TUNNEL_PROBE_INTERVAL, TUNNEL_PROBE_MESSAGE_DELAY, TUNNEL_PROBE_TIMEOUT};
use std::time::Duration;

pub(crate) use self::{
    cleanup::cleanup_stale_ssh_tunnels,
    forwarding::start_tunnel,
    registry::{register_desired_tunnel, sync_desired_tunnels},
    session::{remove_tunnel_session_if_current, stop_tunnel_session, tunnel_session_is_alive},
    status::{
        active_tunnel_status, direct_tunnel_status, emit_tunnel_status_updates,
        starting_tunnel_status,
    },
    supervisor::{
        reconcile_registered_tunnel, run_tunnel_supervisor_cycle, stop_all_tunnels,
        TunnelSupervisorState,
    },
};

pub(super) const TUNNEL_STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const TUNNEL_SUPERVISOR_ACTIVE_SLEEP: Duration = Duration::from_millis(200);
const TUNNEL_SUPERVISOR_MAX_SLEEP: Duration = Duration::from_secs(2);
const TUNNEL_RETRY_INTERVAL: Duration = Duration::from_secs(2);
pub(super) const TUNNEL_READY_PROBE_INTERVAL: Duration = Duration::from_millis(1500);
const TUNNEL_RECOVERY_STAGGER_STEP: Duration = Duration::from_millis(250);
