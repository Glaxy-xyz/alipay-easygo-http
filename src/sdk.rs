//! All SDK calls run on one dedicated OS thread, fed by a bounded queue:
//! the SDK is a stateful singleton with no documented thread safety, and its
//! calls block (upload_order goes over the network), so they must stay off
//! the async runtime. A full queue fails fast instead of piling up latency.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use tokio::sync::oneshot;

use crate::error::ApiError;
use crate::{events, ffi};

type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Clone)]
pub struct Sdk {
    tx: SyncSender<Job>,
    timeout: Duration,
    pub initialized: Arc<AtomicBool>,
}

impl Sdk {
    pub fn start(queue_capacity: usize, timeout: Duration) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Job>(queue_capacity);
        std::thread::Builder::new()
            .name("easygo-sdk".into())
            .spawn(move || {
                for job in rx {
                    if catch_unwind(AssertUnwindSafe(job)).is_err() {
                        tracing::error!("SDK job panicked");
                    }
                }
            })
            .expect("failed to spawn SDK worker thread");

        // SAFETY: the callback is a plain extern "C" fn that never unwinds.
        unsafe { ffi::eg_set_event_callback(Some(events::on_sdk_event)) };

        Self {
            tx,
            timeout,
            initialized: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn call<R, F>(&self, f: F) -> Result<R, ApiError>
    where
        R: Send + 'static,
        F: FnOnce() -> R + Send + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        let job: Job = Box::new(move || {
            let _ = reply_tx.send(f());
        });
        self.tx.try_send(job).map_err(|e| match e {
            TrySendError::Full(_) => {
                ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "SDK queue is full")
            }
            TrySendError::Disconnected(_) => {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "SDK worker has stopped")
            }
        })?;

        match tokio::time::timeout(self.timeout, reply_rx).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(_)) => Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "SDK call panicked",
            )),
            Err(_) => Err(ApiError::new(
                StatusCode::GATEWAY_TIMEOUT,
                "SDK call timed out; it keeps running and blocks the queue until it returns",
            )),
        }
    }
}
