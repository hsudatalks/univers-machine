use std::time::Duration;

pub(crate) const OUTPUT_BUFFER_LIMIT: usize = 128 * 1024;

#[cfg(debug_assertions)]
pub(crate) const SURFACE_PORT_START: u16 = 43000;
#[cfg(debug_assertions)]
pub(crate) const SURFACE_PORT_END: u16 = 43999;
#[cfg(debug_assertions)]
pub(crate) const INTERNAL_TUNNEL_PORT_START: u16 = 44000;
#[cfg(debug_assertions)]
pub(crate) const INTERNAL_TUNNEL_PORT_END: u16 = 44999;

#[cfg(not(debug_assertions))]
pub(crate) const SURFACE_PORT_START: u16 = 45000;
#[cfg(not(debug_assertions))]
pub(crate) const SURFACE_PORT_END: u16 = 45999;
#[cfg(not(debug_assertions))]
pub(crate) const INTERNAL_TUNNEL_PORT_START: u16 = 46000;
#[cfg(not(debug_assertions))]
pub(crate) const INTERNAL_TUNNEL_PORT_END: u16 = 46999;

pub(crate) const SURFACE_HOST: &str = "127.0.0.1";
pub(crate) const TUNNEL_PROBE_INTERVAL: Duration = Duration::from_millis(300);
pub(crate) const TUNNEL_PROBE_TIMEOUT: Duration = Duration::from_millis(700);
pub(crate) const TUNNEL_PROBE_MESSAGE_DELAY: Duration = Duration::from_secs(2);
pub(crate) const PROXY_ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(60);
pub(crate) const PROXY_CONNECT_TIMEOUT: Duration = Duration::from_millis(900);
pub(crate) const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
