use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use url::Url;

use super::commands::dispatch_invoke;
use super::events::{latest_web_event_sequence, wait_for_web_events};
use super::static_files::{content_type_for_path, resolve_static_path};
use super::{
    get_actual_port, DEFAULT_WEB_CONSOLE_PORT, INDEX_HTML, MAX_HTTP_REQUEST_BYTES,
    REQUEST_READ_TIMEOUT,
};

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    query: Option<String>,
    body: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct InvokeRequest {
    cmd: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Serialize)]
struct InvokeResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

pub(super) async fn handle_connection(
    mut stream: TcpStream,
    dist_root: PathBuf,
) -> Result<(), String> {
    let Some(request) = read_http_request(&mut stream).await? else {
        return Ok(());
    };

    if request.method == "OPTIONS" {
        return write_response(
            &mut stream,
            204,
            "No Content",
            "text/plain; charset=utf-8",
            b"",
        )
        .await;
    }

    if request.method == "POST" && request.path == "/__cockpit_web__/invoke" {
        return handle_invoke_request(&mut stream, &request).await;
    }

    if request.method == "GET" && request.path == "/__cockpit_web__/events" {
        return handle_event_poll_request(&mut stream, &request).await;
    }

    if request.method == "GET" && request.path == "/__cockpit_web__/health" {
        let body = json!({
            "ok": true,
            "port": get_actual_port(),
            "version": env!("CARGO_PKG_VERSION"),
        });
        let body = serde_json::to_vec(&body).map_err(|err| err.to_string())?;
        return write_response(
            &mut stream,
            200,
            "OK",
            "application/json; charset=utf-8",
            &body,
        )
        .await;
    }

    if request.method != "GET" && request.method != "HEAD" {
        return write_response(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"method not allowed",
        )
        .await;
    }

    let file_path = resolve_static_path(&dist_root, &request.path)?;
    let (file_path, content_type) = if file_path.exists() && file_path.is_file() {
        let content_type = content_type_for_path(&file_path);
        (file_path, content_type)
    } else {
        (dist_root.join(INDEX_HTML), "text/html; charset=utf-8")
    };

    let body = tokio::fs::read(&file_path)
        .await
        .map_err(|err| format!("read {} failed: {}", file_path.display(), err))?;
    if request.method == "HEAD" {
        return write_response(&mut stream, 200, "OK", content_type, b"").await;
    }
    write_response(&mut stream, 200, "OK", content_type, &body).await
}

async fn handle_invoke_request(
    stream: &mut TcpStream,
    request: &HttpRequest,
) -> Result<(), String> {
    let invoke: InvokeRequest =
        serde_json::from_slice(&request.body).map_err(|err| format!("invalid JSON: {}", err))?;
    let response = match dispatch_invoke(&invoke.cmd, &invoke.args).await {
        Ok(value) => InvokeResponse {
            ok: true,
            value: Some(value),
            error: None,
        },
        Err(error) => InvokeResponse {
            ok: false,
            value: None,
            error: Some(Value::String(error)),
        },
    };
    let status = if response.ok { 200 } else { 400 };
    let body = serde_json::to_vec(&response).map_err(|err| err.to_string())?;
    write_response(
        stream,
        status,
        if status == 200 { "OK" } else { "Bad Request" },
        "application/json; charset=utf-8",
        &body,
    )
    .await
}

async fn handle_event_poll_request(
    stream: &mut TcpStream,
    request: &HttpRequest,
) -> Result<(), String> {
    let after = query_param(request.query.as_deref(), "after")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let client_id = query_param(request.query.as_deref(), "clientId");
    let events = wait_for_web_events(client_id.as_deref(), after).await;
    let body = json!({
        "events": events,
        "latestSequence": latest_web_event_sequence(),
    });
    let body = serde_json::to_vec(&body).map_err(|err| err.to_string())?;
    write_response(stream, 200, "OK", "application/json; charset=utf-8", &body).await
}

async fn read_http_request(stream: &mut TcpStream) -> Result<Option<HttpRequest>, String> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 4096];
    let header_end = loop {
        let read = timeout(REQUEST_READ_TIMEOUT, stream.read(&mut temp))
            .await
            .map_err(|_| "request read timed out".to_string())?
            .map_err(|err| err.to_string())?;
        if read == 0 {
            if buffer.is_empty() {
                return Ok(None);
            }
            return Err("connection closed before headers completed".to_string());
        }
        buffer.extend_from_slice(&temp[..read]);
        if buffer.len() > MAX_HTTP_REQUEST_BYTES {
            return Err("request too large".to_string());
        }
        if let Some(pos) = find_header_end(&buffer) {
            break pos;
        }
    };

    let header_text =
        String::from_utf8(buffer[..header_end].to_vec()).map_err(|err| err.to_string())?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or("").to_string();
    let raw_path = request_parts.next().unwrap_or("/");
    let (path, query) = normalize_request_path(raw_path)?;
    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if name == "content-length" {
                content_length = value
                    .parse::<usize>()
                    .map_err(|_| "invalid content-length".to_string())?;
            }
        }
    }

    if content_length > MAX_HTTP_REQUEST_BYTES {
        return Err("request body too large".to_string());
    }

    let body_start = header_end + 4;
    let mut body = buffer.get(body_start..).unwrap_or_default().to_vec();
    while body.len() < content_length {
        let read = timeout(REQUEST_READ_TIMEOUT, stream.read(&mut temp))
            .await
            .map_err(|_| "request body read timed out".to_string())?
            .map_err(|err| err.to_string())?;
        if read == 0 {
            return Err("connection closed before body completed".to_string());
        }
        body.extend_from_slice(&temp[..read]);
        if body.len() > MAX_HTTP_REQUEST_BYTES {
            return Err("request body too large".to_string());
        }
    }
    body.truncate(content_length);

    Ok(Some(HttpRequest {
        method,
        path,
        query,
        body,
    }))
}

fn normalize_request_path(raw_path: &str) -> Result<(String, Option<String>), String> {
    let url = Url::parse(&format!("http://127.0.0.1{}", raw_path))
        .map_err(|err| format!("invalid request path: {}", err))?;
    Ok((url.path().to_string(), url.query().map(str::to_string)))
}

fn query_param(query: Option<&str>, name: &str) -> Option<String> {
    let query = query?;
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let headers = format!(
        "HTTP/1.1 {} {}\r\ncontent-type: {}\r\ncontent-length: {}\r\ncache-control: no-store\r\nx-content-type-options: nosniff\r\naccess-control-allow-origin: http://127.0.0.1:{}\r\naccess-control-allow-methods: GET,POST,OPTIONS\r\naccess-control-allow-headers: content-type\r\nconnection: close\r\n\r\n",
        status,
        reason,
        content_type,
        body.len(),
        get_actual_port().unwrap_or(DEFAULT_WEB_CONSOLE_PORT)
    );
    stream
        .write_all(headers.as_bytes())
        .await
        .map_err(|err| err.to_string())?;
    stream
        .write_all(body)
        .await
        .map_err(|err| err.to_string())?;
    stream.shutdown().await.map_err(|err| err.to_string())
}
