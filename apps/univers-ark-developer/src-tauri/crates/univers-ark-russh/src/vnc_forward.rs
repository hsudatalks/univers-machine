use std::{
    io::ErrorKind,
    sync::{Arc, Mutex},
};

use futures_util::{SinkExt, StreamExt};
use russh::{client::Handle, Disconnect};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};
use tokio_tungstenite::tungstenite::Message;

use crate::{
    connection::{connect_chain, ClientHandler},
    ssh_config::ResolvedEndpointChain,
    types::{ClientOptions, RusshError, VncForward, VncForwardInner},
};

pub async fn start_vnc_ws_forward_chain(
    chain: &ResolvedEndpointChain,
    remote_host: &str,
    remote_port: u16,
    options: &ClientOptions,
) -> Result<VncForward, RusshError> {
    // Connect SSH first
    let client = connect_chain(chain, options).await?;
    let handle = Arc::new(client.handle);

    // Bind listener on the current runtime (Tauri's tokio runtime)
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
    let inner = Arc::new(VncForwardInner {
        local_port: local_addr.port(),
        shutdown: Mutex::new(Some(shutdown_tx)),
        running: std::sync::atomic::AtomicBool::new(true),
        error: Mutex::new(None),
    });

    let remote_host = remote_host.to_string();
    let inner_clone = inner.clone();

    // Spawn the accept loop on the SAME runtime (Tauri's tokio runtime)
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _)) => {
                            let handle = handle.clone();
                            let remote_host = remote_host.clone();
                            let inner = inner_clone.clone();
                            tokio::spawn(async move {
                                if let Err(error) = handle_vnc_ws_connection(
                                    stream, &handle, &remote_host, remote_port,
                                ).await {
                                    if !is_benign_error(&error) {
                                        if let Ok(mut stored) = inner.error.lock() {
                                            *stored = Some(format!(
                                                "vnc forward connection failed: {error}"
                                            ));
                                        }
                                    }
                                }
                            });
                        }
                        Err(error) => {
                            if let Ok(mut stored) = inner_clone.error.lock() {
                                *stored = Some(format!(
                                    "failed to accept vnc ws connection: {error}"
                                ));
                            }
                            break;
                        }
                    }
                }
            }
        }

        inner_clone
            .running
            .store(false, std::sync::atomic::Ordering::Release);
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "English")
            .await;
    });

    Ok(VncForward { inner })
}

async fn handle_vnc_ws_connection(
    tcp_stream: tokio::net::TcpStream,
    handle: &Handle<ClientHandler>,
    remote_host: &str,
    remote_port: u16,
) -> Result<(), RusshError> {
    eprintln!("[vnc] ws connection accepted, upgrading to websocket...");
    let ws_stream = tokio_tungstenite::accept_async(tcp_stream)
        .await
        .map_err(|error| {
            eprintln!("[vnc] websocket handshake failed: {error}");
            RusshError::ForwardTask(format!("websocket handshake failed: {error}"))
        })?;
    eprintln!("[vnc] websocket handshake OK, opening SSH channel to {remote_host}:{remote_port}...");

    let channel = handle
        .channel_open_direct_tcpip(
            remote_host.to_string(),
            remote_port.into(),
            String::from("127.0.0.1"),
            0,
        )
        .await
        .map_err(|error| {
            eprintln!("[vnc] SSH channel_open_direct_tcpip failed: {error}");
            error
        })?;
    eprintln!("[vnc] SSH channel opened OK, starting VNC data relay...");
    let ssh_stream = channel.into_stream();
    let (mut ssh_read, mut ssh_write) = tokio::io::split(ssh_stream);
    let (mut ws_write, mut ws_read) = ws_stream.split();

    tokio::select! {
        result = async {
            while let Some(msg) = ws_read.next().await {
                match msg {
                    Ok(Message::Binary(data)) => {
                        ssh_write.write_all(&data).await.map_err(RusshError::Io)?;
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
            Ok::<_, RusshError>(())
        } => { let _ = result; }
        result = async {
            let mut buf = vec![0u8; 65536];
            loop {
                let n = ssh_read.read(&mut buf).await.map_err(RusshError::Io)?;
                if n == 0 {
                    break;
                }
                ws_write
                    .send(Message::Binary(buf[..n].to_vec().into()))
                    .await
                    .map_err(|error| {
                        RusshError::ForwardTask(format!("ws send failed: {error}"))
                    })?;
            }
            Ok::<_, RusshError>(())
        } => { let _ = result; }
    }

    let _ = ssh_write.shutdown().await;
    Ok(())
}

fn is_benign_error(error: &RusshError) -> bool {
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
        RusshError::ForwardTask(_) => false,
        _ => false,
    }
}
