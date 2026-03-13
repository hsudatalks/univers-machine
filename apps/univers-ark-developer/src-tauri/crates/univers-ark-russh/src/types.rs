use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use thiserror::Error;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub connect_timeout: Duration,
    pub inactivity_timeout: Option<Duration>,
    pub keepalive_interval: Option<Duration>,
    pub keepalive_max: usize,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(12),
            inactivity_timeout: None,
            keepalive_interval: Some(Duration::from_secs(15)),
            keepalive_max: 3,
        }
    }
}

#[derive(Debug)]
pub struct ExecOutput {
    pub exit_status: u32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub struct HttpProbeOutput {
    pub status_line: String,
    pub body_preview: String,
}

#[derive(Debug)]
pub struct PtyShellProbeOutput {
    pub marker_found: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub enum PtySessionEvent {
    Output(Vec<u8>),
    Exit(String),
}

#[derive(Debug)]
pub(crate) enum PtySessionCommand {
    Write(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Stop,
}

#[derive(Clone)]
pub struct PtySession {
    pub(crate) control_tx: tokio_mpsc::UnboundedSender<PtySessionCommand>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) error: Arc<Mutex<Option<String>>>,
}

impl PtySession {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn last_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|error| error.clone())
    }

    pub fn write(&self, data: impl Into<Vec<u8>>) -> Result<(), RusshError> {
        self.control_tx
            .send(PtySessionCommand::Write(data.into()))
            .map_err(|_| RusshError::ForwardTask(String::from("terminal session is not available")))
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), RusshError> {
        self.control_tx
            .send(PtySessionCommand::Resize { cols, rows })
            .map_err(|_| RusshError::ForwardTask(String::from("terminal session is not available")))
    }

    pub fn request_stop(&self) {
        let _ = self.control_tx.send(PtySessionCommand::Stop);
    }

    pub fn wait_stopped(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while self.is_running() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }

        !self.is_running()
    }
}

#[derive(Debug)]
pub struct RemoteDirectoryListing {
    pub path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<RemoteFileEntry>,
}

#[derive(Debug)]
pub struct RemoteFileEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub is_hidden: bool,
}

#[derive(Debug)]
pub struct RemoteFilePreview {
    pub path: String,
    pub content: String,
    pub is_binary: bool,
    pub truncated: bool,
}

#[derive(Debug, Error)]
pub enum RusshError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to resolve ssh destination {0}")]
    ResolveDestination(String),
    #[error("russh error: {0}")]
    Russh(#[from] russh::Error),
    #[error("ssh key error: {0}")]
    Key(#[from] russh::keys::Error),
    #[error("authentication failed for {0}@{1}")]
    Auth(String, String),
    #[error("missing identity file for {0}")]
    MissingIdentity(String),
    #[error("forward task failed: {0}")]
    ForwardTask(String),
    #[error("sftp error: {0}")]
    Sftp(String),
}

#[derive(Clone)]
pub struct LocalForward {
    pub(crate) inner: Arc<LocalForwardInner>,
}

impl LocalForward {
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.local_addr
    }

    pub fn is_running(&self) -> bool {
        self.inner.running.load(Ordering::Acquire)
    }

    pub fn last_error(&self) -> Option<String> {
        self.inner.error.lock().ok().and_then(|error| error.clone())
    }

    pub fn request_stop(&self) {
        if let Ok(mut shutdown) = self.inner.shutdown.lock() {
            if let Some(sender) = shutdown.take() {
                let _ = sender.send(());
            }
        }
    }

    pub fn wait_stopped(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while self.is_running() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }

        !self.is_running()
    }
}

pub(crate) struct LocalForwardInner {
    pub(crate) local_addr: SocketAddr,
    pub(crate) shutdown: Mutex<Option<oneshot::Sender<()>>>,
    pub(crate) running: AtomicBool,
    pub(crate) error: Mutex<Option<String>>,
}
