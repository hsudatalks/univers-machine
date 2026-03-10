use std::{
    io::ErrorKind,
    net::{TcpStream, ToSocketAddrs},
};

use russh::{ChannelMsg, Disconnect};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    connection::connect_chain,
    ssh_config::ResolvedEndpointChain,
    types::{ClientOptions, ExecOutput, HttpProbeOutput, PtyShellProbeOutput, RusshError},
};

pub async fn execute_chain(
    chain: &ResolvedEndpointChain,
    command: &str,
    options: &ClientOptions,
) -> Result<ExecOutput, RusshError> {
    let client = connect_chain(chain, options).await?;
    let mut channel = client.handle.channel_open_session().await?;
    channel.exec(true, command).await?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_status = None;

    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
            ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
            ChannelMsg::ExitStatus {
                exit_status: status,
            } => exit_status = Some(status),
            ChannelMsg::Eof => break,
            _ => {}
        }
    }

    let _ = client
        .handle
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;

    Ok(ExecOutput {
        exit_status: exit_status.unwrap_or_default(),
        stdout,
        stderr,
    })
}

pub async fn probe_http_chain(
    chain: &ResolvedEndpointChain,
    remote_host: &str,
    remote_port: u16,
    path: &str,
    options: &ClientOptions,
) -> Result<HttpProbeOutput, RusshError> {
    let client = connect_chain(chain, options).await?;
    let channel = client
        .handle
        .channel_open_direct_tcpip(
            remote_host.to_string(),
            remote_port.into(),
            String::from("127.0.0.1"),
            0,
        )
        .await?;

    let mut stream = channel.into_stream();
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, remote_host
    );
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;

    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        response.extend_from_slice(&buffer[..bytes_read]);
        let header_complete = response.windows(4).any(|window| window == b"\r\n\r\n");
        if header_complete && response.len() >= 512 {
            break;
        }
    }

    let _ = client
        .handle
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;

    let response = String::from_utf8_lossy(&response).to_string();
    let mut lines = response.lines();
    let status_line = lines.next().unwrap_or_default().to_string();
    let body_preview = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.chars().take(240).collect::<String>())
        .unwrap_or_default();

    Ok(HttpProbeOutput {
        status_line,
        body_preview,
    })
}

pub async fn probe_pty_shell_chain(
    chain: &ResolvedEndpointChain,
    command: &str,
    options: &ClientOptions,
) -> Result<PtyShellProbeOutput, RusshError> {
    let client = connect_chain(chain, options).await?;
    let mut channel = client.handle.channel_open_session().await?;
    channel
        .request_pty(true, "xterm-256color", 120, 32, 0, 0, &[])
        .await?;
    channel.request_shell(true).await?;
    channel.window_change(132, 36, 0, 0).await?;

    let marker = "__UA_RUSSH_DONE__";
    let shell_command = format!("{command}\nprintf '\\n{marker}:%s\\n' $? \nexit\n");
    let mut writer = channel.make_writer();
    writer.write_all(shell_command.as_bytes()).await?;
    writer.flush().await?;
    drop(writer);

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut marker_found = false;

    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => {
                stdout.extend_from_slice(&data);
                if !marker_found
                    && stdout
                        .windows(marker.len())
                        .any(|window| window == marker.as_bytes())
                {
                    marker_found = true;
                }
            }
            ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
            ChannelMsg::Close | ChannelMsg::Eof => break,
            _ => {}
        }
    }

    let _ = client
        .handle
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;

    Ok(PtyShellProbeOutput {
        marker_found,
        stdout,
        stderr,
    })
}

#[allow(dead_code)]
fn _probe_tcp_ready(host: &str, port: u16, timeout: std::time::Duration) -> bool {
    (host, port)
        .to_socket_addrs()
        .map(|addrs| addrs.into_iter().any(|addr| TcpStream::connect_timeout(&addr, timeout).is_ok()))
        .unwrap_or(false)
}

#[allow(dead_code)]
fn _is_timeout_like(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::UnexpectedEof
    )
}
