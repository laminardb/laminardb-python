//! Synchronous `Subscription` class for continuous queries.
//!
//! Wraps `laminar_db::api::QueryStream` as a Python iterator.

use parking_lot::Mutex;
use pyo3::exceptions::PyStopIteration;
use pyo3::prelude::*;

use crate::error::IntoPyResult;
use crate::query::QueryResult;

/// A synchronous subscription to a continuous query.
#[pyclass(name = "Subscription")]
pub struct Subscription {
    inner: Mutex<Option<laminar_db::api::QueryStream>>,
}

unsafe impl Send for Subscription {}
unsafe impl Sync for Subscription {}

#[pymethods]
impl Subscription {
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

    /// Non-blocking poll for the next result.
    fn try_next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        let guard = self.inner.lock();
        if guard.is_none() {
            return Ok(None);
        }
        drop(guard);

        py.allow_threads(|| {
            let mut guard = self.inner.lock();
            let stream = guard.as_mut().unwrap();
            match stream.try_next().into_pyresult()? {
                Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                None => Ok(None),
            }
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<QueryResult> {
        let guard = self.inner.lock();
        if guard.as_ref().is_none_or(|s| !s.is_active()) {
            return Err(PyStopIteration::new_err(()));
        }
        drop(guard);

        let result = py.allow_threads(|| {
            let mut guard = self.inner.lock();
            let stream = guard.as_mut().unwrap();
            stream.next().into_pyresult()
        })?;

        match result {
            Some(batch) => Ok(QueryResult::from_batch(batch)),
            None => Err(PyStopIteration::new_err(())),
        }
    }
}

impl Subscription {
    pub fn from_core(stream: laminar_db::api::QueryStream) -> Self {
        Self {
            inner: Mutex::new(Some(stream)),
        }
    }
}
