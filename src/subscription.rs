//! Synchronous `Subscription` class for continuous queries.
//!
//! Usage:
//!     sub = conn.subscribe("SELECT * FROM sensors WHERE value > 100")
//!     for result in sub:
//!         print(result.to_pandas())

use std::sync::Arc;

use parking_lot::Mutex;
use pyo3::exceptions::PyStopIteration;
use pyo3::prelude::*;

use crate::error::{IntoPyResult, SubscriptionError};
use crate::query::QueryResult;

/// A synchronous subscription to a continuous query.
#[pyclass(name = "Subscription")]
pub struct Subscription {
    inner: Arc<Mutex<laminardb_core::Subscription>>,
    active: Arc<std::sync::atomic::AtomicBool>,
}

unsafe impl Send for Subscription {}
unsafe impl Sync for Subscription {}

#[pymethods]
impl Subscription {
    /// Whether the subscription is still active.
    #[getter]
    fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Cancel the subscription.
    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        if self.active.swap(false, std::sync::atomic::Ordering::Relaxed) {
            let inner = self.inner.clone();
            py.allow_threads(|| {
                let rt = crate::async_support::runtime();
                let sub = inner.lock();
                rt.block_on(sub.cancel()).into_pyresult()
            })?;
        }
        Ok(())
    }

    /// Non-blocking poll for the next result.
    fn try_next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        if !self.is_active() {
            return Ok(None);
        }

        let inner = self.inner.clone();
        py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            let mut sub = inner.lock();
            match rt.block_on(sub.try_next()) {
                Ok(Some(result)) => Ok(Some(QueryResult::from_core(result))),
                Ok(None) => Ok(None),
                Err(e) => Err(SubscriptionError::new_err(e.to_string())),
            }
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<QueryResult> {
        if !self.is_active() {
            return Err(PyStopIteration::new_err(()));
        }

        let inner = self.inner.clone();
        let result = py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            let mut sub = inner.lock();
            rt.block_on(sub.next())
        });

        match result {
            Some(Ok(result)) => Ok(QueryResult::from_core(result)),
            Some(Err(e)) => Err(SubscriptionError::new_err(e.to_string())),
            None => {
                self.active.store(false, std::sync::atomic::Ordering::Relaxed);
                Err(PyStopIteration::new_err(()))
            }
        }
    }
}

impl Subscription {
    pub fn from_core(sub: laminardb_core::Subscription) -> Self {
        Self {
            inner: Arc::new(Mutex::new(sub)),
            active: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }
}
