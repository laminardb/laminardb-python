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
        py.allow_threads(|| {
            let _rt = crate::async_support::runtime().enter();
            let mut guard = self.inner.lock();
            match guard.as_mut() {
                Some(stream) => match stream.try_next().into_pyresult()? {
                    Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                    None => Ok(None),
                },
                None => Ok(None),
            }
        })
    }

    fn __repr__(&self) -> String {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(s) if s.is_active() => "Subscription(active)".to_owned(),
            Some(_) => "Subscription(finished)".to_owned(),
            None => "Subscription(cancelled)".to_owned(),
        }
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<QueryResult> {
        py.allow_threads(|| {
            let _rt = crate::async_support::runtime().enter();
            let mut guard = self.inner.lock();
            match guard.as_mut() {
                Some(stream) if stream.is_active() => match stream.next().into_pyresult()? {
                    Some(batch) => Ok(QueryResult::from_batch(batch)),
                    None => Err(PyStopIteration::new_err(())),
                },
                _ => Err(PyStopIteration::new_err(())),
            }
        })
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.cancel(py)?;
        Ok(false)
    }

    fn __del__(&self) {
        let mut guard = self.inner.lock();
        if let Some(stream) = guard.as_mut() {
            stream.cancel();
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
