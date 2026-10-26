//! Bridges SDK callbacks (fired on SDK-owned threads) into a broadcast channel
//! for SSE subscribers, and keeps the latest payload per topic so clients that
//! connect after a push (config/price arrive right after init) can catch up.

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct SdkEvent {
    pub topic: String,
    pub data: String,
}

#[derive(Clone, Serialize)]
pub struct LatestEvent {
    pub data: serde_json::Value,
    pub received_at: i64,
}

pub struct EventBus {
    tx: broadcast::Sender<SdkEvent>,
    latest: Mutex<HashMap<String, LatestEvent>>,
}

static BUS: OnceLock<EventBus> = OnceLock::new();

impl EventBus {
    pub fn install(capacity: usize) -> &'static EventBus {
        BUS.get_or_init(|| EventBus {
            tx: broadcast::channel(capacity).0,
            latest: Mutex::new(HashMap::new()),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SdkEvent> {
        self.tx.subscribe()
    }

    pub fn latest(&self) -> HashMap<String, LatestEvent> {
        self.latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn publish(&self, topic: String, data: String) {
        let parsed =
            serde_json::from_str(&data).unwrap_or_else(|_| serde_json::Value::String(data.clone()));
        self.latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                topic.clone(),
                LatestEvent {
                    data: parsed,
                    received_at: crate::now_ms(),
                },
            );
        // No subscribers is not an error.
        let _ = self.tx.send(SdkEvent { topic, data });
    }
}

/// Registered with the shim; must never unwind into C++ or block SDK threads.
pub unsafe extern "C" fn on_sdk_event(topic: *const c_char, json: *const c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if topic.is_null() || json.is_null() {
            return;
        }
        // SAFETY: the shim passes NUL-terminated strings valid for this call.
        let topic = unsafe { CStr::from_ptr(topic) }
            .to_string_lossy()
            .into_owned();
        let data = unsafe { CStr::from_ptr(json) }
            .to_string_lossy()
            .into_owned();
        tracing::info!(%topic, bytes = data.len(), "sdk event");
        if let Some(bus) = BUS.get() {
            bus.publish(topic, data);
        }
    }));
}
