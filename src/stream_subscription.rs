//! True stream subscription bindings wrapping `laminar_db::api::ArrowSubscription`.
//!
//! Unlike `Subscription` / `AsyncSubscription` (which wrap `QueryStream` from
//! `query_stream(sql)`), these classes wrap `ArrowSubscription` obtained from
//! `Connection::subscribe(stream_name)` — a true continuous subscription to a
//! named stream created via `CREATE STREAM ... AS SELECT ...`.

use std::time::Duration;

use parking_lot::Mutex;
use pyo3::exceptions::{PyStopAsyncIteration, PyStopIteration};
use pyo3::prelude::*;
use pyo3_arrow::PySchema;

use crate::async_support::runtime;
use crate::error::IntoPyResult;
use crate::query::QueryResult;

// ---------------------------------------------------------------------------
// Synchronous StreamSubscription
// ---------------------------------------------------------------------------

/// A synchronous subscription to a named stream.
///
/// Wraps `laminar_db::api::ArrowSubscription` as a Python iterator.
/// Created via `Connection.subscribe_stream(name)`.
#[pyclass(name = "StreamSubscription")]
pub struct StreamSubscription {
    inner: Mutex<Option<laminar_db::api::ArrowSubscription>>,
}

// Safety: ArrowSubscription is Send. We protect shared access with Mutex.
unsafe impl Send for StreamSubscription {}
unsafe impl Sync for StreamSubscription {}

#[pymethods]
impl StreamSubscription {
    /// Whether the subscription is still active.
    #[getter]
    fn is_active(&self) -> bool {
        let guard = self.inner.lock();
        guard.as_ref().is_some_and(|s| s.is_active())
    }

    /// The schema of the subscription as a PyArrow Schema.
    #[getter]
    fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(sub) => {
                let schema = sub.schema();
                let py_schema = PySchema::from(schema);
                let obj = py_schema.into_pyarrow(py)?;
                Ok(obj.into_pyobject(py)?.into_any().unbind())
            }
            None => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "StreamSubscription has been cancelled",
            )),
        }
    }

    /// Blocking wait for the next batch.
    fn next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        let has_sub = self.inner.lock().is_some();
        if !has_sub {
            return Ok(None);
        }

        py.allow_threads(|| {
            let _rt = runtime().enter();
            let mut guard = self.inner.lock();
            let sub = guard.as_mut().unwrap();
            match sub.next().into_pyresult()? {
                Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                None => Ok(None),
            }
        })
    }

    /// Blocking wait for the next batch with a timeout in milliseconds.
    ///
    /// Returns None if the timeout expires without data.
    fn next_timeout(&self, py: Python<'_>, timeout_ms: u64) -> PyResult<Option<QueryResult>> {
        let has_sub = self.inner.lock().is_some();
        if !has_sub {
            return Ok(None);
        }

        let timeout = Duration::from_millis(timeout_ms);
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let mut guard = self.inner.lock();
            let sub = guard.as_mut().unwrap();
            match sub.next_timeout(timeout).into_pyresult()? {
                Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                None => Ok(None),
            }
        })
    }

    /// Non-blocking poll for the next batch.
    fn try_next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        let has_sub = self.inner.lock().is_some();
        if !has_sub {
            return Ok(None);
        }

        py.allow_threads(|| {
            let _rt = runtime().enter();
            let mut guard = self.inner.lock();
            let sub = guard.as_mut().unwrap();
            match sub.try_next().into_pyresult()? {
                Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                None => Ok(None),
            }
        })
    }

    /// Cancel the subscription.
    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            let mut guard = self.inner.lock();
            if let Some(sub) = guard.as_mut() {
                sub.cancel();
            }
            // Take the option to mark as cancelled (distinguishes from finished)
            let _ = guard.take();
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(s) if s.is_active() => "StreamSubscription(active)".to_owned(),
            Some(_) => "StreamSubscription(finished)".to_owned(),
            None => "StreamSubscription(cancelled)".to_owned(),
        }
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
            let _rt = runtime().enter();
            let mut guard = self.inner.lock();
            let sub = guard.as_mut().unwrap();
            sub.next().into_pyresult()
        })?;

        match result {
            Some(batch) => Ok(QueryResult::from_batch(batch)),
            None => Err(PyStopIteration::new_err(())),
        }
    }
}

impl StreamSubscription {
    pub fn from_core(sub: laminar_db::api::ArrowSubscription) -> Self {
        Self {
            inner: Mutex::new(Some(sub)),
        }
    }
}

// ---------------------------------------------------------------------------
// Asynchronous AsyncStreamSubscription
// ---------------------------------------------------------------------------

/// An asynchronous subscription to a named stream.
///
/// Wraps `laminar_db::api::ArrowSubscription` as a Python async iterator.
/// Created via `Connection.subscribe_stream_async(name)`.
#[pyclass(name = "AsyncStreamSubscription")]
pub struct AsyncStreamSubscription {
    inner: Mutex<Option<laminar_db::api::ArrowSubscription>>,
}

// Safety: ArrowSubscription is Send. We protect shared access with Mutex.
unsafe impl Send for AsyncStreamSubscription {}
unsafe impl Sync for AsyncStreamSubscription {}

#[pymethods]
impl AsyncStreamSubscription {
    /// Whether the subscription is still active.
    #[getter]
    fn is_active(&self) -> bool {
        let guard = self.inner.lock();
        guard.as_ref().is_some_and(|s| s.is_active())
    }

    /// The schema of the subscription as a PyArrow Schema.
    #[getter]
    fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(sub) => {
                let schema = sub.schema();
                let py_schema = PySchema::from(schema);
                let obj = py_schema.into_pyarrow(py)?;
                Ok(obj.into_pyobject(py)?.into_any().unbind())
            }
            None => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "AsyncStreamSubscription has been cancelled",
            )),
        }
    }

    /// Blocking wait for the next batch.
    fn next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        let has_sub = self.inner.lock().is_some();
        if !has_sub {
            return Ok(None);
        }

        py.allow_threads(|| {
            let _rt = runtime().enter();
            let mut guard = self.inner.lock();
            let sub = guard.as_mut().unwrap();
            match sub.next().into_pyresult()? {
                Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                None => Ok(None),
            }
        })
    }

    /// Blocking wait for the next batch with a timeout in milliseconds.
    ///
    /// Returns None if the timeout expires without data.
    fn next_timeout(&self, py: Python<'_>, timeout_ms: u64) -> PyResult<Option<QueryResult>> {
        let has_sub = self.inner.lock().is_some();
        if !has_sub {
            return Ok(None);
        }

        let timeout = Duration::from_millis(timeout_ms);
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let mut guard = self.inner.lock();
            let sub = guard.as_mut().unwrap();
            match sub.next_timeout(timeout).into_pyresult()? {
                Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                None => Ok(None),
            }
        })
    }

    /// Non-blocking poll for the next batch.
    fn try_next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        let has_sub = self.inner.lock().is_some();
        if !has_sub {
            return Ok(None);
        }

        py.allow_threads(|| {
            let _rt = runtime().enter();
            let mut guard = self.inner.lock();
            let sub = guard.as_mut().unwrap();
            match sub.try_next().into_pyresult()? {
                Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                None => Ok(None),
            }
        })
    }

    /// Cancel the subscription.
    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            let mut guard = self.inner.lock();
            if let Some(sub) = guard.as_mut() {
                sub.cancel();
            }
            let _ = guard.take();
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(s) if s.is_active() => "AsyncStreamSubscription(active)".to_owned(),
            Some(_) => "AsyncStreamSubscription(finished)".to_owned(),
            None => "AsyncStreamSubscription(cancelled)".to_owned(),
        }
    }

    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if !self.is_active() {
            return Err(PyStopAsyncIteration::new_err(()));
        }

        // ArrowSubscription.next() is blocking, so we do it under the lock
        // then wrap the result in a future for async Python.
        pyo3_async_runtimes::tokio::future_into_py(py, {
            let result = {
                let mut guard = self.inner.lock();
                match guard.as_mut() {
                    Some(sub) => sub.next().into_pyresult(),
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

impl AsyncStreamSubscription {
    pub fn from_core(sub: laminar_db::api::ArrowSubscription) -> Self {
        Self {
            inner: Mutex::new(Some(sub)),
        }
    }
}
