//! Local browser console for the full Cockpit Tools UI.
//! This is intentionally separate from `web_report`, which remains the tokened report endpoint.

mod commands;
mod events;
mod http;
mod static_files;

use std::sync::{OnceLock, RwLock};
use tokio::net::TcpListener;
use tokio::time::Duration;

use super::config::PORT_RANGE;

pub(super) const DEFAULT_WEB_CONSOLE_PORT: u16 = 18181;
pub(super) const MAX_HTTP_REQUEST_BYTES: usize = 2 * 1024 * 1024;
pub(super) const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(8);
pub(super) const EVENT_POLL_TIMEOUT: Duration = Duration::from_secs(25);
pub(super) const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(150);
pub(super) const MAX_EVENT_QUEUE_LEN: usize = 1024;
pub(super) const INDEX_HTML: &str = "index.html";

static ACTUAL_WEB_CONSOLE_PORT: OnceLock<RwLock<Option<u16>>> = OnceLock::new();

fn web_console_port_state() -> &'static RwLock<Option<u16>> {
    ACTUAL_WEB_CONSOLE_PORT.get_or_init(|| RwLock::new(None))
}

fn set_actual_port(port: Option<u16>) {
    if let Ok(mut guard) = web_console_port_state().write() {
        *guard = port;
    }
}

pub fn get_actual_port() -> Option<u16> {
    web_console_port_state()
        .read()
        .ok()
        .and_then(|guard| *guard)
}

pub async fn start_server() {
    let Some(dist_root) = static_files::find_frontend_dist() else {
        set_actual_port(None);
        super::logger::log_warn("[WebConsole] frontend dist directory not found, skip startup");
        return;
    };

    let mut port = DEFAULT_WEB_CONSOLE_PORT;
    let mut listener = None;
    for attempt in 0..PORT_RANGE {
        let addr = format!("127.0.0.1:{}", port);
        match TcpListener::bind(&addr).await {
            Ok(bound) => {
                listener = Some(bound);
                if attempt > 0 {
                    super::logger::log_info(&format!(
                        "[WebConsole] preferred port {} is busy, switched to {}",
                        DEFAULT_WEB_CONSOLE_PORT, port
                    ));
                }
                break;
            }
            Err(err) => {
                super::logger::log_warn(&format!(
                    "[WebConsole] failed to bind 127.0.0.1:{}: {}",
                    port, err
                ));
                port = port.saturating_add(1);
            }
        }
    }

    let Some(listener) = listener else {
        set_actual_port(None);
        super::logger::log_error("[WebConsole] no available local port");
        return;
    };

    set_actual_port(Some(port));
    super::logger::log_info(&format!(
        "[WebConsole] serving full UI at http://127.0.0.1:{}/",
        port
    ));

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let dist_root = dist_root.clone();
                tokio::spawn(async move {
                    if let Err(err) = http::handle_connection(stream, dist_root).await {
                        super::logger::log_warn(&format!("[WebConsole] request failed: {}", err));
                    }
                });
            }
            Err(err) => {
                super::logger::log_warn(&format!("[WebConsole] accept failed: {}", err));
            }
        }
    }
}

pub(super) fn app_handle() -> Result<tauri::AppHandle, String> {
    crate::get_app_handle()
        .cloned()
        .ok_or_else(|| "App runtime is not available".to_string())
}
