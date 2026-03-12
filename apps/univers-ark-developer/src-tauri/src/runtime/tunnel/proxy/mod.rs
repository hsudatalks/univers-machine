mod http;

use crate::{constants::PROXY_ACCEPT_POLL_INTERVAL, models::LocalProxyHandle};
use std::{
    io::ErrorKind,
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use self::http::handle_vite_proxy_connection;

fn socket_addr_for_local_port(port: u16) -> std::net::SocketAddr {
    ([127, 0, 0, 1], port).into()
}

pub(crate) fn start_vite_proxy(
    public_port: u16,
    upstream_http_port: u16,
    upstream_hmr_port: u16,
) -> Result<LocalProxyHandle, String> {
    let listener = TcpListener::bind(socket_addr_for_local_port(public_port)).map_err(|error| {
        format!(
            "Failed to bind the local development proxy on {}: {}",
            public_port, error
        )
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure the local development proxy: {}", error))?;

    let stop_requested = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    let error = Arc::new(Mutex::new(None));
    let stop_flag = stop_requested.clone();
    let running_flag = running.clone();
    let error_state = error.clone();

    std::thread::spawn(move || {
        loop {
            if stop_flag.load(Ordering::Acquire) {
                break;
            }

            match listener.accept() {
                Ok((stream, _)) => {
                    std::thread::spawn(move || {
                        handle_vite_proxy_connection(
                            stream,
                            public_port,
                            upstream_http_port,
                            upstream_hmr_port,
                        );
                    });
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(PROXY_ACCEPT_POLL_INTERVAL);
                }
                Err(error) => {
                    if let Ok(mut last_error) = error_state.lock() {
                        *last_error =
                            Some(format!("The local development proxy stopped: {}", error));
                    }
                    break;
                }
            }
        }

        running_flag.store(false, Ordering::Release);
    });

    Ok(LocalProxyHandle {
        stop_requested,
        running,
        error,
    })
}

pub(crate) fn proxy_error_message(proxy: &LocalProxyHandle) -> Option<String> {
    proxy.error.lock().ok().and_then(|message| message.clone())
}
