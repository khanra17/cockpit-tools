use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::{OnceLock, RwLock};
use tauri::{Emitter, Listener};
use tokio::time::sleep;

use super::{app_handle, EVENT_POLL_INTERVAL, EVENT_POLL_TIMEOUT, MAX_EVENT_QUEUE_LEN};

static WEB_EVENT_STATE: OnceLock<RwLock<WebEventState>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WebEventMessage {
    sequence: u64,
    event: String,
    payload: Value,
}

#[derive(Debug, Default)]
struct WebEventState {
    next_listener_id: u32,
    next_sequence: u64,
    browser_listeners: HashMap<u32, WebBrowserEventListener>,
    tauri_listeners: HashMap<String, tauri::EventId>,
    queue: VecDeque<WebEventMessage>,
}

#[derive(Debug, Clone)]
struct WebBrowserEventListener {
    event: String,
    client_id: String,
    registered_after_sequence: u64,
}

fn web_event_state() -> &'static RwLock<WebEventState> {
    WEB_EVENT_STATE.get_or_init(|| RwLock::new(WebEventState::default()))
}

pub(super) fn register_web_event_listener(
    event_name: String,
    client_id: String,
) -> Result<u32, String> {
    let mut state = web_event_state()
        .write()
        .map_err(|_| "web event state is poisoned".to_string())?;

    state.next_listener_id = state.next_listener_id.saturating_add(1).max(1);
    let listener_id = state.next_listener_id;
    let registered_after_sequence = state.next_sequence;
    state.browser_listeners.insert(
        listener_id,
        WebBrowserEventListener {
            event: event_name.clone(),
            client_id,
            registered_after_sequence,
        },
    );

    if !state.tauri_listeners.contains_key(&event_name) {
        let app = app_handle()?;
        let captured_event = event_name.clone();
        let tauri_listener_id = app.listen_any(event_name.clone(), move |event| {
            push_web_event(&captured_event, event.payload());
        });
        state.tauri_listeners.insert(event_name, tauri_listener_id);
    }

    Ok(listener_id)
}

pub(super) fn unregister_web_event_listener(
    listener_id: u32,
    client_id: Option<&str>,
) -> Result<(), String> {
    let tauri_listener_to_remove = {
        let mut state = web_event_state()
            .write()
            .map_err(|_| "web event state is poisoned".to_string())?;
        let Some(listener) = state.browser_listeners.get(&listener_id) else {
            return Ok(());
        };
        if client_id.is_some_and(|expected| expected != listener.client_id) {
            return Ok(());
        }
        let Some(listener) = state.browser_listeners.remove(&listener_id) else {
            return Ok(());
        };
        let event_name = listener.event;

        let still_used = state
            .browser_listeners
            .values()
            .any(|registered_event| registered_event.event == event_name);
        if still_used {
            None
        } else {
            state.tauri_listeners.remove(&event_name)
        }
    };

    if let Some(tauri_listener_id) = tauri_listener_to_remove {
        if let Ok(app) = app_handle() {
            app.unlisten(tauri_listener_id);
        }
    }

    Ok(())
}

pub(super) fn emit_web_event_from_browser(
    event_name: String,
    payload: Option<Value>,
) -> Result<(), String> {
    let payload = payload.unwrap_or(Value::Null);
    app_handle()?
        .emit(event_name.as_str(), payload)
        .map_err(|err| err.to_string())
}

fn push_web_event(event_name: &str, raw_payload: &str) {
    let payload = serde_json::from_str(raw_payload)
        .unwrap_or_else(|_| Value::String(raw_payload.to_string()));
    push_web_event_value(event_name, payload);
}

fn push_web_event_value(event_name: &str, payload: Value) {
    let Ok(mut state) = web_event_state().write() else {
        return;
    };
    state.next_sequence = state.next_sequence.saturating_add(1);
    let sequence = state.next_sequence;
    state.queue.push_back(WebEventMessage {
        sequence,
        event: event_name.to_string(),
        payload,
    });
    while state.queue.len() > MAX_EVENT_QUEUE_LEN {
        state.queue.pop_front();
    }
}

pub(super) async fn wait_for_web_events(
    client_id: Option<&str>,
    after: u64,
) -> Vec<WebEventMessage> {
    let started = std::time::Instant::now();
    loop {
        let events = collect_web_events(client_id, after);
        if !events.is_empty() || started.elapsed() >= EVENT_POLL_TIMEOUT {
            return events;
        }
        sleep(EVENT_POLL_INTERVAL).await;
    }
}

fn collect_web_events(client_id: Option<&str>, after: u64) -> Vec<WebEventMessage> {
    web_event_state()
        .read()
        .map(|state| {
            let active_listeners = state
                .browser_listeners
                .values()
                .filter(|listener| client_id.map_or(true, |id| listener.client_id == id))
                .cloned()
                .collect::<Vec<_>>();

            state
                .queue
                .iter()
                .filter(|event| {
                    event.sequence > after
                        && active_listeners.iter().any(|listener| {
                            listener.event == event.event
                                && event.sequence > listener.registered_after_sequence
                        })
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn latest_web_event_sequence() -> u64 {
    web_event_state()
        .read()
        .map(|state| state.next_sequence)
        .unwrap_or_default()
}
