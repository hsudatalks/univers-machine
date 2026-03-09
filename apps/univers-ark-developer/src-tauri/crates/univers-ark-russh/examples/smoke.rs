use std::{env, process::ExitCode};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use univers_ark_russh::{
    execute_alias, list_directory_alias, probe_http_alias, probe_pty_shell_alias,
    read_file_preview_alias, start_local_forward_alias, ClientOptions, SshConfigResolver,
};

#[tokio::main]
async fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(mode) = args.next() else {
        eprintln!("usage:");
        eprintln!("  smoke exec <destination> <command...>");
        eprintln!("  smoke http-probe <destination> <remote-port> [path]");
        eprintln!("  smoke local-forward-self-test <destination> <remote-port> [path]");
        eprintln!("  smoke pty-shell-probe <destination> <command...>");
        eprintln!("  smoke list-dir <destination> [path]");
        eprintln!("  smoke preview-file <destination> <path>");
        return ExitCode::from(2);
    };

    let resolver = match SshConfigResolver::from_default_path() {
        Ok(resolver) => resolver,
        Err(error) => {
            eprintln!("failed to load ssh config: {error}");
            return ExitCode::from(1);
        }
    };

    let options = ClientOptions::default();

    match mode.as_str() {
        "exec" => {
            let Some(destination) = args.next() else {
                eprintln!("missing destination");
                return ExitCode::from(2);
            };
            let command = args.collect::<Vec<_>>().join(" ");
            if command.trim().is_empty() {
                eprintln!("missing command");
                return ExitCode::from(2);
            }

            match execute_alias(&resolver, &destination, &command, &options).await {
                Ok(output) => {
                    println!("exit_status={}", output.exit_status);
                    if !output.stdout.is_empty() {
                        println!("--- stdout ---");
                        print!("{}", String::from_utf8_lossy(&output.stdout));
                    }
                    if !output.stderr.is_empty() {
                        println!("--- stderr ---");
                        eprint!("{}", String::from_utf8_lossy(&output.stderr));
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("exec failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        "http-probe" => {
            let Some(destination) = args.next() else {
                eprintln!("missing destination");
                return ExitCode::from(2);
            };
            let Some(remote_port) = args.next() else {
                eprintln!("missing remote port");
                return ExitCode::from(2);
            };
            let remote_port = match remote_port.parse::<u16>() {
                Ok(port) => port,
                Err(error) => {
                    eprintln!("invalid remote port: {error}");
                    return ExitCode::from(2);
                }
            };
            let path = args.next().unwrap_or_else(|| String::from("/"));

            match probe_http_alias(
                &resolver,
                &destination,
                "127.0.0.1",
                remote_port,
                &path,
                &options,
            )
            .await
            {
                Ok(result) => {
                    println!("status={}", result.status_line);
                    if !result.body_preview.is_empty() {
                        println!("body_preview={}", result.body_preview);
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("http probe failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        "local-forward-self-test" => {
            let Some(destination) = args.next() else {
                eprintln!("missing destination");
                return ExitCode::from(2);
            };
            let Some(remote_port) = args.next() else {
                eprintln!("missing remote port");
                return ExitCode::from(2);
            };
            let remote_port = match remote_port.parse::<u16>() {
                Ok(port) => port,
                Err(error) => {
                    eprintln!("invalid remote port: {error}");
                    return ExitCode::from(2);
                }
            };
            let path = args.next().unwrap_or_else(|| String::from("/"));

            let forward = match start_local_forward_alias(
                &resolver,
                &destination,
                "127.0.0.1:0",
                "127.0.0.1",
                remote_port,
                &options,
            )
            .await
            {
                Ok(forward) => forward,
                Err(error) => {
                    eprintln!("failed to start local forward: {error}");
                    return ExitCode::from(1);
                }
            };

            let local_addr = forward.local_addr();
            println!("local_addr={local_addr}");

            let mut socket = match TcpStream::connect(local_addr).await {
                Ok(socket) => socket,
                Err(error) => {
                    let _ = forward.stop().await;
                    eprintln!("failed to connect to local forward: {error}");
                    return ExitCode::from(1);
                }
            };

            let request = format!(
                "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                path
            );
            if let Err(error) = socket.write_all(request.as_bytes()).await {
                let _ = forward.stop().await;
                eprintln!("failed to write request: {error}");
                return ExitCode::from(1);
            }

            let mut response = Vec::new();
            let mut buffer = [0_u8; 8192];
            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(bytes_read) => response.extend_from_slice(&buffer[..bytes_read]),
                    Err(error) => {
                        let _ = forward.stop().await;
                        eprintln!("failed to read response: {error}");
                        return ExitCode::from(1);
                    }
                }
            }

            let response = String::from_utf8_lossy(&response).to_string();
            let status_line = response.lines().next().unwrap_or_default();
            println!("status={status_line}");
            if let Some((_, body)) = response.split_once("\r\n\r\n") {
                let preview = body.chars().take(240).collect::<String>();
                if !preview.is_empty() {
                    println!("body_preview={preview}");
                }
            }

            match forward.stop().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("failed to stop local forward: {error}");
                    ExitCode::from(1)
                }
            }
        }
        "pty-shell-probe" => {
            let Some(destination) = args.next() else {
                eprintln!("missing destination");
                return ExitCode::from(2);
            };
            let command = args.collect::<Vec<_>>().join(" ");
            if command.trim().is_empty() {
                eprintln!("missing command");
                return ExitCode::from(2);
            }

            match probe_pty_shell_alias(&resolver, &destination, &command, &options).await {
                Ok(output) => {
                    println!("marker_found={}", output.marker_found);
                    if !output.stdout.is_empty() {
                        println!("--- stdout ---");
                        print!("{}", String::from_utf8_lossy(&output.stdout));
                    }
                    if !output.stderr.is_empty() {
                        println!("--- stderr ---");
                        eprint!("{}", String::from_utf8_lossy(&output.stderr));
                    }
                    if output.marker_found {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => {
                    eprintln!("pty shell probe failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        "list-dir" => {
            let Some(destination) = args.next() else {
                eprintln!("missing destination");
                return ExitCode::from(2);
            };
            let path = args.next();

            match list_directory_alias(&resolver, &destination, path.as_deref(), &options).await {
                Ok(listing) => {
                    println!("path={}", listing.path);
                    if let Some(parent_path) = listing.parent_path {
                        println!("parent_path={parent_path}");
                    }
                    for entry in listing.entries.into_iter().take(20) {
                        println!(
                            "{}\t{}\t{}\t{}",
                            entry.kind,
                            entry.size,
                            if entry.is_hidden { "hidden" } else { "visible" },
                            entry.path
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("list-dir failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        "preview-file" => {
            let Some(destination) = args.next() else {
                eprintln!("missing destination");
                return ExitCode::from(2);
            };
            let Some(path) = args.next() else {
                eprintln!("missing path");
                return ExitCode::from(2);
            };

            match read_file_preview_alias(&resolver, &destination, &path, &options).await {
                Ok(preview) => {
                    println!("path={}", preview.path);
                    println!("is_binary={}", preview.is_binary);
                    println!("truncated={}", preview.truncated);
                    if !preview.content.is_empty() {
                        println!("--- content ---");
                        print!("{}", preview.content.chars().take(400).collect::<String>());
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("preview-file failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        other => {
            eprintln!("unknown mode: {other}");
            ExitCode::from(2)
        }
    }
}
