use super::super::TUNNEL_PROBE_TIMEOUT;
use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
};
use url::Url;

fn browser_probe_path(url: &Url) -> String {
    let mut path = url.path().to_string();

    if path.is_empty() {
        path.push('/');
    }

    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }

    path
}

fn browser_probe_host_header(url: &Url) -> Option<String> {
    let host = url.host_str()?;

    match url.port() {
        Some(port) => Some(format!("{}:{}", host, port)),
        None => Some(host.to_string()),
    }
}

fn browser_probe_addrs(url: &Url) -> Vec<std::net::SocketAddr> {
    let Some(host) = url.host_str() else {
        return Vec::new();
    };

    let Some(port) = url.port_or_known_default() else {
        return Vec::new();
    };

    (host, port)
        .to_socket_addrs()
        .map(|addrs| addrs.collect())
        .unwrap_or_default()
}

fn probe_browser_http(url: &Url) -> bool {
    if url.scheme() != "http" {
        return false;
    }

    let host_header = match browser_probe_host_header(url) {
        Some(value) => value,
        None => return false,
    };

    let request = format!(
        "HEAD {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        browser_probe_path(url),
        host_header
    );

    for addr in browser_probe_addrs(url) {
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, TUNNEL_PROBE_TIMEOUT) else {
            continue;
        };

        let _ = stream.set_read_timeout(Some(TUNNEL_PROBE_TIMEOUT));
        let _ = stream.set_write_timeout(Some(TUNNEL_PROBE_TIMEOUT));

        if stream.write_all(request.as_bytes()).is_err() {
            continue;
        }

        let mut buffer = [0u8; 64];

        match stream.read(&mut buffer) {
            Ok(read_count) if read_count > 0 => {
                let response = String::from_utf8_lossy(&buffer[..read_count]);
                if response.starts_with("HTTP/") {
                    return true;
                }
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::UnexpectedEof
                ) => {}
            Err(_) => {}
        }
    }

    false
}

fn probe_browser_tcp(url: &Url) -> bool {
    browser_probe_addrs(url)
        .into_iter()
        .any(|addr| TcpStream::connect_timeout(&addr, TUNNEL_PROBE_TIMEOUT).is_ok())
}

fn probe_browser_ready(local_url: &str) -> bool {
    let Ok(url) = Url::parse(local_url) else {
        return false;
    };

    if probe_browser_http(&url) {
        return true;
    }

    probe_browser_tcp(&url)
}

pub(super) fn probe_targets_ready(probe_urls: &[String]) -> bool {
    !probe_urls.is_empty() && probe_urls.iter().all(|url| probe_browser_ready(url))
}
