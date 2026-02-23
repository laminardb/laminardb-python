//! Callback-based push subscription API.
//!
//! `CallbackSubscription` runs a background thread that polls for new batches
//! and invokes a user-provided Python callable for each one.  This provides a
//! push-based alternative to the pull-based iterator subscriptions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use parking_lot::Mutex;
use pyo3::prelude::*;

use crate::async_support::runtime;
use crate::query::QueryResult;

// ---------------------------------------------------------------------------
// SubscriptionKind — unifies QueryStream and ArrowSubscription
// ---------------------------------------------------------------------------

enum SubscriptionKind {
    Query(laminar_db::api::QueryStream),
    Stream(laminar_db::api::ArrowSubscription),
}

impl SubscriptionKind {
    fn try_next(&mut self) -> Result<Option<arrow_array::RecordBatch>, laminar_db::api::ApiError> {
        match self {
            Self::Query(s) => s.try_next(),
            Self::Stream(s) => s.try_next(),
        }
    }

    fn is_active(&self) -> bool {
        match self {
            Self::Query(s) => s.is_active(),
            Self::Stream(s) => s.is_active(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Query(s) => s.cancel(),
            Self::Stream(s) => s.cancel(),
        }
    }
}

// Safety: The inner types are Send (protected by being owned by a single thread).
unsafe impl Send for SubscriptionKind {}

// ---------------------------------------------------------------------------
// CallbackSubscription pyclass
// ---------------------------------------------------------------------------

/// A push-based subscription that calls a Python function for each batch.
///
/// Created via `Connection.subscribe_callback(sql, on_data)` or
/// `Connection.subscribe_stream_callback(name, on_data)`.
///
/// The subscription runs on a background thread and invokes `on_data`
/// with a `QueryResult` for each batch received.  Call `cancel()` to
/// stop the subscription and `wait()` to block until the background
/// thread exits.
#[pyclass(name = "CallbackSubscription")]
pub struct CallbackSubscription {
    cancelled: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    kind: String,
}

unsafe impl Send for CallbackSubscription {}
unsafe impl Sync for CallbackSubscription {}

#[pymethods]
impl CallbackSubscription {
    /// Whether the background thread is still running.
    #[getter]
    fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Stop the subscription.  Safe to call multiple times.
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Block until the background thread exits.  Releases the GIL.
    fn wait(&self, py: Python<'_>) -> PyResult<()> {
        let handle = self.thread.lock().take();
        if let Some(h) = handle {
            py.allow_threads(|| {
                let _ = h.join();
            });
        }
        Ok(())
    }

    fn __repr__(&self) -> String {
        let state = if self.active.load(Ordering::Relaxed) {
            "active"
        } else {
            "finished"
        };
        format!("CallbackSubscription({}, {})", self.kind, state)
    }

    fn __del__(&self) {
        // Set the cancel flag so the background thread will exit.
        // Do NOT join here — the background thread may be inside
        // Python::with_gil() while the GC holds the GIL, which
        // would deadlock.
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Factory methods (pub(crate), not exposed to Python)
// ---------------------------------------------------------------------------

impl CallbackSubscription {
    pub(crate) fn from_query_stream(
        stream: laminar_db::api::QueryStream,
        on_data: PyObject,
        on_error: Option<PyObject>,
    ) -> Self {
        Self::spawn(SubscriptionKind::Query(stream), on_data, on_error, "query")
    }

    pub(crate) fn from_arrow_subscription(
        sub: laminar_db::api::ArrowSubscription,
        on_data: PyObject,
        on_error: Option<PyObject>,
    ) -> Self {
        Self::spawn(SubscriptionKind::Stream(sub), on_data, on_error, "stream")
    }

    fn spawn(
        mut sub: SubscriptionKind,
        on_data: PyObject,
        on_error: Option<PyObject>,
        kind: &str,
    ) -> Self {
        let cancelled = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(true));
        let cancelled2 = cancelled.clone();
        let active2 = active.clone();
        let kind_str = kind.to_owned();

        let handle = thread::Builder::new()
            .name(format!("laminardb-callback-{}", kind))
            .spawn(move || {
                let _rt = runtime().enter();
                callback_thread_loop(&mut sub, &cancelled2, &on_data, &on_error);
                active2.store(false, Ordering::Relaxed);
            })
            .expect("failed to spawn callback thread");

        Self {
            cancelled,
            active,
            thread: Mutex::new(Some(handle)),
            kind: kind_str,
        }
    }
}

// ---------------------------------------------------------------------------
// Background thread loop
// ---------------------------------------------------------------------------

fn callback_thread_loop(
    sub: &mut SubscriptionKind,
    cancelled: &AtomicBool,
    on_data: &PyObject,
    on_error: &Option<PyObject>,
) {
    loop {
        if cancelled.load(Ordering::Relaxed) {
            sub.cancel();
            break;
        }

        if !sub.is_active() {
            break;
        }

        match sub.try_next() {
            Ok(Some(batch)) => {
                let result = QueryResult::from_batch(batch);
                let should_stop = Python::with_gil(|py| match on_data.call1(py, (result,)) {
                    Ok(_) => false,
                    Err(e) => handle_callback_error(py, e, on_error),
                });
                if should_stop {
                    sub.cancel();
                    break;
                }
            }
            Ok(None) => {
                // No data ready yet — sleep briefly to avoid busy-spin
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let msg = format!("{}", e);
                let should_stop = Python::with_gil(|py| {
                    let py_err = pyo3::exceptions::PyRuntimeError::new_err(msg);
                    handle_callback_error(py, py_err, on_error)
                });
                if should_stop {
                    sub.cancel();
                    break;
                }
            }
        }
    }
}

/// Route an error to on_error if provided; returns `true` if the loop should stop.
fn handle_callback_error(py: Python<'_>, error: PyErr, on_error: &Option<PyObject>) -> bool {
    match on_error {
        Some(handler) => {
            let msg = error.to_string();
            match handler.call1(py, (msg,)) {
                Ok(_) => false, // on_error handled it, continue
                Err(e) => {
                    eprintln!("laminardb: on_error callback raised: {}", e);
                    true
                }
            }
        }
        None => {
            eprintln!("laminardb: callback error (no on_error handler): {}", error);
            true
        }
    }
}
