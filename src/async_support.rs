//! Async support: Tokio runtime management and async Python classes.
//!
//! Provides:
//! - Lazy-initialized global Tokio runtime
//! - `AsyncSubscription` class for async iteration

use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;

use crate::error::{IntoPyResult, SubscriptionError};
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
///
///     async for result in await conn.subscribe_async("SELECT ..."):
///         df = result.to_pandas()
#[pyclass(name = "AsyncSubscription")]
pub struct AsyncSubscription {
    inner: Arc<Mutex<laminardb_core::Subscription>>,
    active: Arc<std::sync::atomic::AtomicBool>,
}

unsafe impl Send for AsyncSubscription {}
unsafe impl Sync for AsyncSubscription {}

#[pymethods]
impl AsyncSubscription {
    /// Whether the subscription is still active.
    #[getter]
    fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Cancel the subscription.
    fn cancel<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        let active = self.active.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            if active.swap(false, std::sync::atomic::Ordering::Relaxed) {
                let sub = inner.lock();
                sub.cancel().await.into_pyresult()?;
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

        let inner = self.inner.clone();
        let active = self.active.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let result = {
                let mut sub = inner.lock();
                sub.next().await
            };

            match result {
                Some(Ok(result)) => Ok(QueryResult::from_core(result)),
                Some(Err(e)) => Err(SubscriptionError::new_err(e.to_string())),
                None => {
                    active.store(false, std::sync::atomic::Ordering::Relaxed);
                    Err(PyStopAsyncIteration::new_err(()))
                }
            }
        })
    }
}

impl AsyncSubscription {
    pub fn from_core(sub: laminardb_core::Subscription) -> Self {
        Self {
            inner: Arc::new(Mutex::new(sub)),
            active: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }
}
