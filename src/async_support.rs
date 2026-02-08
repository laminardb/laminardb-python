//! Async support: Tokio runtime management and async Python classes.

use std::sync::OnceLock;

use parking_lot::Mutex;
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;

use crate::error::IntoPyResult;
use crate::query::QueryResult;

// ---------------------------------------------------------------------------
// Global Tokio runtime
// ---------------------------------------------------------------------------

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Get or create the global Tokio runtime.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("laminardb-worker")
            .build()
            .expect("Failed to create Tokio runtime")
    })
}

// ---------------------------------------------------------------------------
// Async subscription
// ---------------------------------------------------------------------------

/// An asynchronous subscription to a continuous query.
#[pyclass(name = "AsyncSubscription")]
pub struct AsyncSubscription {
    inner: Mutex<Option<laminar_db::api::QueryStream>>,
}

unsafe impl Send for AsyncSubscription {}
unsafe impl Sync for AsyncSubscription {}

#[pymethods]
impl AsyncSubscription {
    /// Whether the subscription is still active.
    #[getter]
    fn is_active(&self) -> bool {
        let guard = self.inner.lock();
        guard.as_ref().is_some_and(|s| s.is_active())
    }

    /// Cancel the subscription.
    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            let mut guard = self.inner.lock();
            if let Some(stream) = guard.as_mut() {
                stream.cancel();
            }
            Ok(())
        })
    }

    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if !self.is_active() {
            return Err(PyStopAsyncIteration::new_err(()));
        }

        // For async iteration we still use allow_threads + future_into_py
        // since QueryStream.next() is a blocking call
        let is_active = self.is_active();
        if !is_active {
            return Err(PyStopAsyncIteration::new_err(()));
        }

        // We need to move the check into the future
        // Use a simple wrapper that polls in the tokio runtime
        pyo3_async_runtimes::tokio::future_into_py(py, {
            // Can't move Mutex guard into async, so we do blocking poll
            let result = {
                let mut guard = self.inner.lock();
                match guard.as_mut() {
                    Some(stream) => stream.next().into_pyresult(),
                    None => Ok(None),
                }
            };
            async move {
                match result? {
                    Some(batch) => Ok(QueryResult::from_batch(batch)),
                    None => Err(PyStopAsyncIteration::new_err(())),
                }
            }
        })
    }
}

impl AsyncSubscription {
    pub fn from_core(stream: laminar_db::api::QueryStream) -> Self {
        Self {
            inner: Mutex::new(Some(stream)),
        }
    }
}
