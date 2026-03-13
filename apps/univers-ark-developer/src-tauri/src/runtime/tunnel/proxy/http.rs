use crate::constants::{MAX_HTTP_HEADER_BYTES, PROXY_CONNECT_TIMEOUT, SURFACE_HOST};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use super::socket_addr_for_local_port;

const ARK_BROWSER_BRIDGE_MARKER: &str = "ark-browser-bridge";
const ARK_BROWSER_BRIDGE_SNIPPET: &str = r#"<script>(function(){const SOURCE="ark-browser-bridge";const payload=()=>({href:window.location.href,path:window.location.pathname+window.location.search+window.location.hash,title:document.title||""});const emit=(type,data)=>{try{window.parent.postMessage({source:SOURCE,mode:"proxy-injected",type,payload:data},"*");}catch(_error){}};const emitNavigation=(type)=>emit(type,payload());const wrap=(name)=>{const original=history[name];if(typeof original!=="function"){return;}history[name]=function(){const result=original.apply(this,arguments);emitNavigation("navigation");return result;};};const resolveTargetUrl=(element,selector,attribute)=>{const matched=element&&typeof element.closest==="function"?element.closest(selector):null;const value=matched&&typeof matched.getAttribute==="function"?matched.getAttribute(attribute):null;if(!value){return null;}try{return new URL(value,window.location.href).href;}catch(_error){return value;}};wrap("pushState");wrap("replaceState");window.addEventListener("popstate",()=>emitNavigation("navigation"));window.addEventListener("hashchange",()=>emitNavigation("navigation"));window.addEventListener("load",()=>emitNavigation("ready"));window.addEventListener("contextmenu",(event)=>{if(event.shiftKey){return;}const target=event.target instanceof Element?event.target:null;const selection=typeof window.getSelection==="function"?window.getSelection()?.toString().trim()||"":"";event.preventDefault();emit("contextmenu",{...payload(),imageUrl:resolveTargetUrl(target,"img[src]","src"),linkUrl:resolveTargetUrl(target,"a[href]","href"),selectionText:selection||null,x:event.clientX,y:event.clientY});},true);const titleObserver=new MutationObserver(()=>emitNavigation("navigation"));const titleElement=document.querySelector("title");if(titleElement){titleObserver.observe(titleElement,{childList:true,subtree:true,characterData:true});}emitNavigation("ready");})();</script>"#;

type HttpHeaders = Vec<(String, String)>;

struct ParsedHttpRequestHead {
    request_bytes: Vec<u8>,
    body_offset: usize,
    method: String,
    path: String,
    version: String,
    headers: HttpHeaders,
}

struct ParsedHttpResponseHead {
    status_line: String,
    headers: HttpHeaders,
    body_offset: usize,
}

fn find_header_terminator(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn read_http_request_head(
    stream: &mut TcpStream,
) -> Result<ParsedHttpRequestHead, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];

    let _ = stream.set_read_timeout(Some(PROXY_CONNECT_TIMEOUT));

    loop {
        let read_count = stream
            .read(&mut chunk)
            .map_err(|error| format!("Failed to read proxy request: {error}"))?;

        if read_count == 0 {
            return Err(String::from(
                "Proxy request closed before the HTTP headers were complete.",
            ));
        }

        buffer.extend_from_slice(&chunk[..read_count]);

        if buffer.len() > MAX_HTTP_HEADER_BYTES {
            return Err(String::from(
                "Proxy request headers exceeded the configured limit.",
            ));
        }

        if let Some(header_end) = find_header_terminator(&buffer) {
            let head = String::from_utf8(buffer[..header_end].to_vec())
                .map_err(|_| String::from("Proxy request headers were not valid UTF-8."))?;
            let mut lines = head.split("\r\n");
            let request_line = lines
                .next()
                .ok_or_else(|| String::from("Proxy request line was missing."))?;
            let mut parts = request_line.split_whitespace();
            let method = parts
                .next()
                .ok_or_else(|| String::from("Proxy request method was missing."))?;
            let path = parts
                .next()
                .ok_or_else(|| String::from("Proxy request path was missing."))?;
            let version = parts
                .next()
                .ok_or_else(|| String::from("Proxy request version was missing."))?;
            let headers = lines
                .filter(|line| !line.is_empty())
                .filter_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    Some((name.trim().to_string(), value.trim().to_string()))
                })
                .collect::<Vec<_>>();

            return Ok(ParsedHttpRequestHead {
                request_bytes: buffer,
                body_offset: header_end + 4,
                method: method.to_string(),
                path: path.to_string(),
                version: version.to_string(),
                headers,
            });
        }
    }
}

fn is_websocket_request(headers: &[(String, String)]) -> bool {
    headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("upgrade") && value.eq_ignore_ascii_case("websocket")
    })
}

fn rebuild_http_request(
    method: &str,
    path: &str,
    version: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Vec<u8> {
    let mut request = format!("{method} {path} {version}\r\n");

    for (name, value) in headers {
        if name.eq_ignore_ascii_case("accept-encoding")
            || name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("proxy-connection")
            || name.eq_ignore_ascii_case("if-none-match")
        {
            continue;
        }

        request.push_str(name);
        request.push_str(": ");
        request.push_str(value);
        request.push_str("\r\n");
    }

    request.push_str("Connection: close\r\n");
    request.push_str("Accept-Encoding: identity\r\n");
    request.push_str("\r\n");

    let mut request_bytes = request.into_bytes();
    request_bytes.extend_from_slice(body);
    request_bytes
}

fn parse_http_response_head(response: &[u8]) -> Result<ParsedHttpResponseHead, String> {
    let header_end = find_header_terminator(response)
        .ok_or_else(|| String::from("Proxy response was missing an HTTP header terminator."))?;
    let head = String::from_utf8(response[..header_end].to_vec())
        .map_err(|_| String::from("Proxy response headers were not valid UTF-8."))?;
    let mut lines = head.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| String::from("Proxy response status line was missing."))?
        .to_string();
    let headers = lines
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect::<Vec<_>>();

    Ok(ParsedHttpResponseHead {
        status_line,
        headers,
        body_offset: header_end + 4,
    })
}

fn response_header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn decode_chunked_body(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoded = Vec::new();
    let mut index = 0usize;

    loop {
        let size_line_end = body[index..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|offset| index + offset)
            .ok_or_else(|| String::from("Invalid chunked response framing."))?;
        let size_line = std::str::from_utf8(&body[index..size_line_end])
            .map_err(|_| String::from("Chunked response size line was not valid UTF-8."))?;
        let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| String::from("Chunked response size could not be parsed."))?;
        index = size_line_end + 2;

        if size == 0 {
            break;
        }

        let chunk_end = index + size;
        if chunk_end + 2 > body.len() {
            return Err(String::from("Chunked response ended unexpectedly."));
        }

        decoded.extend_from_slice(&body[index..chunk_end]);
        index = chunk_end + 2;
    }

    Ok(decoded)
}

fn replace_js_statement(script: &str, prefix: &str, replacement: &str) -> String {
    let Some(start) = script.find(prefix) else {
        return script.to_string();
    };
    let Some(relative_end) = script[start..].find(';') else {
        return script.to_string();
    };
    let end = start + relative_end + 1;

    let mut updated = String::with_capacity(script.len() + replacement.len());
    updated.push_str(&script[..start]);
    updated.push_str(replacement);
    updated.push_str(&script[end..]);
    updated
}

fn rewrite_vite_client_script(script: &str, public_port: u16) -> String {
    let script = replace_js_statement(
        script,
        "const hmrPort = ",
        &format!("const hmrPort = {public_port};"),
    );

    replace_js_statement(
        &script,
        "const directSocketHost = ",
        &format!(
            "const directSocketHost = \"{SURFACE_HOST}:{public_port}/\";"
        ),
    )
}

fn build_rewritten_http_response(
    status_line: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Vec<u8> {
    let mut response = String::new();
    response.push_str(status_line);
    response.push_str("\r\n");

    for (name, value) in headers {
        if name.eq_ignore_ascii_case("content-length")
            || name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("etag")
            || name.eq_ignore_ascii_case("content-encoding")
        {
            continue;
        }

        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }

    response.push_str(&format!("Content-Length: {}\r\n", body.len()));
    response.push_str("Connection: close\r\n");
    response.push_str("\r\n");

    let mut response_bytes = response.into_bytes();
    response_bytes.extend_from_slice(body);
    response_bytes
}

fn decode_http_response_body(
    headers: &[(String, String)],
    response: &[u8],
    body_offset: usize,
) -> Result<Vec<u8>, String> {
    if response_header_value(headers, "transfer-encoding")
        .map(|value| value.eq_ignore_ascii_case("chunked"))
        .unwrap_or(false)
    {
        decode_chunked_body(&response[body_offset..])
    } else {
        Ok(response[body_offset..].to_vec())
    }
}

fn is_html_response(headers: &[(String, String)]) -> bool {
    response_header_value(headers, "content-type")
        .map(|value| value.to_ascii_lowercase().starts_with("text/html"))
        .unwrap_or(false)
}

fn inject_browser_bridge_into_html(html: &str) -> String {
    if html.contains(ARK_BROWSER_BRIDGE_MARKER) {
        return html.to_string();
    }

    let lower = html.to_ascii_lowercase();

    for needle in ["</head>", "</body>"] {
        if let Some(index) = lower.rfind(needle) {
            let mut updated = String::with_capacity(html.len() + ARK_BROWSER_BRIDGE_SNIPPET.len());
            updated.push_str(&html[..index]);
            updated.push_str(ARK_BROWSER_BRIDGE_SNIPPET);
            updated.push_str(&html[index..]);
            return updated;
        }
    }

    let mut updated = String::with_capacity(html.len() + ARK_BROWSER_BRIDGE_SNIPPET.len());
    updated.push_str(ARK_BROWSER_BRIDGE_SNIPPET);
    updated.push_str(html);
    updated
}

fn rewrite_html_navigation_response(response: &[u8]) -> Result<Vec<u8>, String> {
    let ParsedHttpResponseHead {
        status_line,
        headers,
        body_offset,
    } = parse_http_response_head(response)?;

    if !is_html_response(&headers) {
        return Ok(response.to_vec());
    }

    let body = decode_http_response_body(&headers, response, body_offset)?;
    let html = String::from_utf8(body)
        .map_err(|_| String::from("The HTML response body was not valid UTF-8."))?;
    let rewritten = inject_browser_bridge_into_html(&html);

    Ok(build_rewritten_http_response(
        &status_line,
        &headers,
        rewritten.as_bytes(),
    ))
}

fn rewrite_vite_client_response(response: &[u8], public_port: u16) -> Result<Vec<u8>, String> {
    let ParsedHttpResponseHead {
        status_line,
        headers,
        body_offset,
    } = parse_http_response_head(response)?;
    let body = decode_http_response_body(&headers, response, body_offset)?;

    let script = String::from_utf8(body)
        .map_err(|_| String::from("The Vite client response body was not valid UTF-8."))?;
    let rewritten = rewrite_vite_client_script(&script, public_port);

    Ok(build_rewritten_http_response(
        &status_line,
        &headers,
        rewritten.as_bytes(),
    ))
}

fn write_proxy_error_response(stream: &mut TcpStream, status_line: &str, message: &str) {
    let body = message.as_bytes();
    let response = format!(
        "{}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_line,
        body.len(),
        message
    );
    let _ = stream.write_all(response.as_bytes());
}

fn proxy_websocket_connection(
    mut client_stream: TcpStream,
    request_bytes: &[u8],
    upstream_port: u16,
) {
    let Ok(mut upstream_stream) = TcpStream::connect(socket_addr_for_local_port(upstream_port))
    else {
        return;
    };

    let _ = upstream_stream.write_all(request_bytes);

    let Ok(mut client_reader) = client_stream.try_clone() else {
        return;
    };
    let Ok(mut upstream_writer) = upstream_stream.try_clone() else {
        return;
    };

    let forward = std::thread::spawn(move || {
        let _ = std::io::copy(&mut client_reader, &mut upstream_writer);
    });

    let _ = std::io::copy(&mut upstream_stream, &mut client_stream);
    let _ = forward.join();
}

fn proxy_http_connection(
    client_stream: &mut TcpStream,
    request: &ParsedHttpRequestHead,
    upstream_port: u16,
    public_port: u16,
) -> Result<(), String> {
    let mut upstream_stream = TcpStream::connect(socket_addr_for_local_port(upstream_port))
        .map_err(|error| format!("Failed to connect to the upstream dev server: {error}"))?;
    let _ = upstream_stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = upstream_stream.set_write_timeout(Some(Duration::from_secs(10)));

    let upstream_request = rebuild_http_request(
        &request.method,
        &request.path,
        &request.version,
        &request.headers,
        &request.request_bytes[request.body_offset..],
    );
    upstream_stream
        .write_all(&upstream_request)
        .map_err(|error| format!("Failed to forward the proxy request: {error}"))?;

    let mut response = Vec::new();
    upstream_stream
        .read_to_end(&mut response)
        .map_err(|error| format!("Failed to read the upstream response: {error}"))?;

    let response_bytes = if request.path == "/@vite/client" {
        rewrite_vite_client_response(&response, public_port).unwrap_or(response)
    } else {
        rewrite_html_navigation_response(&response).unwrap_or(response)
    };

    client_stream
        .write_all(&response_bytes)
        .map_err(|error| format!("Failed to write the proxy response: {error}"))
}

pub(super) fn handle_vite_proxy_connection(
    mut client_stream: TcpStream,
    public_port: u16,
    upstream_http_port: u16,
    upstream_hmr_port: u16,
) {
    let _ = client_stream.set_nonblocking(false);
    let request = read_http_request_head(&mut client_stream);
    let Ok(request_head) = request else {
        if let Err(error) = request {
            write_proxy_error_response(&mut client_stream, "HTTP/1.1 400 Bad Request", &error);
        }
        return;
    };

    if is_websocket_request(&request_head.headers) {
        proxy_websocket_connection(
            client_stream,
            &request_head.request_bytes,
            upstream_hmr_port,
        );
        return;
    }

    if let Err(error) = proxy_http_connection(
        &mut client_stream,
        &request_head,
        upstream_http_port,
        public_port,
    ) {
        write_proxy_error_response(&mut client_stream, "HTTP/1.1 502 Bad Gateway", &error);
    }
}
