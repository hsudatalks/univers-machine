use std::{
    io::ErrorKind,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use russh::{client::Handle, Disconnect};
use tokio::{
    io::{copy_bidirectional, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use crate::{
    connection::{connect_chain, ClientHandler},
    ssh_config::ResolvedEndpointChain,
    types::{ClientOptions, LocalForward, LocalForwardInner, RusshError},
};

pub async fn start_local_forward_chain(
    chain: &ResolvedEndpointChain,
    local_bind_addr: &str,
    remote_host: &str,
    remote_port: u16,
    options: &ClientOptions,
) -> Result<LocalForward, RusshError> {
    let chain = chain.clone();
    let remote_host = remote_host.to_string();
    let options = options.clone();
    let local_bind_addr = local_bind_addr.to_string();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<LocalForward, RusshError>>(1);

    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = ready_tx.send(Err(RusshError::ForwardTask(format!(
                    "failed to build forward runtime: {error}"
                ))));
                return;
            }
        };

        runtime.block_on(async move {
            let listener = match TcpListener::bind(&local_bind_addr).await {
                Ok(listener) => listener,
                Err(error) => {
                    let _ = ready_tx.send(Err(RusshError::Io(error)));
                    return;
                }
            };
            let local_addr = match listener.local_addr() {
                Ok(addr) => addr,
                Err(error) => {
                    let _ = ready_tx.send(Err(RusshError::Io(error)));
                    return;
                }
            };

            let client = match connect_chain(&chain, &options).await {
                Ok(client) => client,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            let handle = Arc::new(client.handle);

            let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
            let inner = Arc::new(LocalForwardInner {
                local_addr,
                shutdown: Mutex::new(Some(shutdown_tx)),
                running: std::sync::atomic::AtomicBool::new(true),
                error: Mutex::new(None),
            });

            let _ = ready_tx.send(Ok(LocalForward {
                inner: inner.clone(),
            }));

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => {
                        match accepted {
                            Ok((socket, _)) => {
                                let remote_host = remote_host.clone();
                                let handle = handle.clone();
                                let inner = inner.clone();
                                tokio::spawn(async move {
                                    if let Err(error) =
                                        forward_connection(socket, &handle, &remote_host, remote_port).await
                                    {
                                        if is_benign_forward_error(&error) {
                                            return;
                                        }
                                        if let Ok(mut stored) = inner.error.lock() {
                                            *stored = Some(format!(
                                                "forward connection failed: {error}"
                                            ));
                                        }
                                        if let Ok(mut shutdown) = inner.shutdown.lock() {
                                            if let Some(sender) = shutdown.take() {
                                                let _ = sender.send(());
                                            }
                                        }
                                    }
                                });
                            }
                            Err(error) => {
                                if let Ok(mut stored) = inner.error.lock() {
                                    *stored = Some(format!("failed to accept local forward connection: {error}"));
                                }
                                break;
                            }
                        }
                    }
                }
            }

            inner
                .running
                .store(false, std::sync::atomic::Ordering::Release);
            let _ = handle
                .disconnect(Disconnect::ByApplication, "", "English")
                .await;
        });
    });

    ready_rx
        .recv()
        .map_err(|error| RusshError::ForwardTask(format!("failed to receive local forward startup: {error}")))?
}

async fn forward_connection(
    mut inbound: TcpStream,
    handle: &Handle<ClientHandler>,
    remote_host: &str,
    remote_port: u16,
) -> Result<(), RusshError> {
    let channel = handle
        .channel_open_direct_tcpip(
            remote_host.to_string(),
            remote_port.into(),
            String::from("127.0.0.1"),
            0,
        )
        .await?;
    let mut outbound = channel.into_stream();

    let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
    let _ = outbound.shutdown().await;

    Ok(())
}

fn is_benign_forward_error(error: &RusshError) -> bool {
    match error {
        RusshError::Io(source) => matches!(
            source.kind(),
            ErrorKind::BrokenPipe
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::UnexpectedEof
                | ErrorKind::NotConnected
        ),
        RusshError::Russh(source) => {
            let message = source.to_string().to_ascii_lowercase();
            message.contains("channel closed")
                || message.contains("channel eof")
                || message.contains("connection reset")
                || message.contains("send error")
        }
        _ => false,
    }
}
