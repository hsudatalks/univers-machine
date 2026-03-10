use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
};

use russh::{ChannelMsg, Disconnect};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc as tokio_mpsc;

use crate::{
    connection::connect_chain,
    ssh_config::ResolvedEndpointChain,
    types::{ClientOptions, PtySession, PtySessionCommand, PtySessionEvent, RusshError},
};

pub fn start_pty_session_chain(
    chain: &ResolvedEndpointChain,
    startup_command: &str,
    cols: u16,
    rows: u16,
    options: &ClientOptions,
) -> Result<(PtySession, mpsc::Receiver<PtySessionEvent>), RusshError> {
    let (control_tx, control_rx) = tokio_mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::channel();
    let running = Arc::new(AtomicBool::new(true));
    let error = Arc::new(Mutex::new(None));

    let session = PtySession {
        control_tx,
        running: running.clone(),
        error: error.clone(),
    };

    let chain = chain.clone();
    let startup_command = startup_command.trim().to_string();
    let options = options.clone();

    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(source) => {
                let message = format!("failed to build russh terminal runtime: {source}");
                if let Ok(mut current_error) = error.lock() {
                    *current_error = Some(message.clone());
                }
                running.store(false, Ordering::Release);
                let _ = event_tx.send(PtySessionEvent::Exit(message));
                return;
            }
        };

        runtime.block_on(async move {
            let result = run_pty_session(
                &chain,
                &startup_command,
                cols,
                rows,
                &options,
                control_rx,
                event_tx.clone(),
            )
            .await;

            if let Err(source) = result {
                let message = source.to_string();
                if let Ok(mut current_error) = error.lock() {
                    *current_error = Some(message.clone());
                }
                let _ = event_tx.send(PtySessionEvent::Exit(message));
            }

            running.store(false, Ordering::Release);
        });
    });

    Ok((session, event_rx))
}

async fn run_pty_session(
    chain: &ResolvedEndpointChain,
    startup_command: &str,
    cols: u16,
    rows: u16,
    options: &ClientOptions,
    mut control_rx: tokio_mpsc::UnboundedReceiver<PtySessionCommand>,
    event_tx: mpsc::Sender<PtySessionEvent>,
) -> Result<(), RusshError> {
    let client = connect_chain(chain, options).await?;
    let mut channel = client.handle.channel_open_session().await?;
    channel
        .request_pty(
            true,
            "xterm-256color",
            cols.max(40).into(),
            rows.max(12).into(),
            0,
            0,
            &[],
        )
        .await?;
    channel.request_shell(true).await?;
    channel
        .window_change(cols.max(40).into(), rows.max(12).into(), 0, 0)
        .await?;

    let mut writer = channel.make_writer();
    if !startup_command.is_empty() {
        let command = format!("{startup_command}\n");
        writer.write_all(command.as_bytes()).await?;
        writer.flush().await?;
    }

    let mut exit_reason = String::from("terminal session closed");

    loop {
        tokio::select! {
            command = control_rx.recv() => {
                match command {
                    Some(PtySessionCommand::Write(data)) => {
                        writer.write_all(&data).await?;
                        writer.flush().await?;
                    }
                    Some(PtySessionCommand::Resize { cols, rows }) => {
                        channel
                            .window_change(cols.max(40).into(), rows.max(12).into(), 0, 0)
                            .await?;
                    }
                    Some(PtySessionCommand::Stop) => {
                        exit_reason = String::from("terminal session stopped");
                        let _ = channel.eof().await;
                        break;
                    }
                    None => {
                        exit_reason = String::from("terminal control channel closed");
                        let _ = channel.eof().await;
                        break;
                    }
                }
            }
            message = channel.wait() => {
                match message {
                    Some(ChannelMsg::Data { data }) => {
                        let _ = event_tx.send(PtySessionEvent::Output(data.to_vec()));
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let _ = event_tx.send(PtySessionEvent::Output(data.to_vec()));
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        exit_reason = format!("terminal exited with status {exit_status}");
                    }
                    Some(ChannelMsg::Close) | Some(ChannelMsg::Eof) => {
                        break;
                    }
                    Some(_) => {}
                    None => {
                        exit_reason = String::from("terminal channel closed");
                        break;
                    }
                }
            }
        }
    }

    let _ = client
        .handle
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;
    let _ = event_tx.send(PtySessionEvent::Exit(exit_reason));

    Ok(())
}
