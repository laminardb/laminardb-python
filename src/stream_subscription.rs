//! Named-stream subscription bindings for the framed LaminarDB subscription API.
//!
//! The v0.30 core emits both data batches and durable checkpoint barriers. The
//! frame methods expose that protocol directly. Existing batch-only methods and
//! iterator behavior remain as conveniences and skip barrier frames.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use pyo3::exceptions::{PyStopAsyncIteration, PyStopIteration};
use pyo3::prelude::*;
use pyo3_arrow::PySchema;

use crate::async_support::{SyncCell, runtime};
use crate::error::IntoPyResult;
use crate::query::QueryResult;

type CoreFrame = laminar_db::api::ArrowSubscriptionFrame;
type CoreSubscription = laminar_db::api::ArrowSubscription;

/// A data or durable-progress frame from a named stream subscription.
#[pyclass(name = "SubscriptionFrame", frozen)]
pub struct SubscriptionFrame {
    kind: &'static str,
    sequence: u64,
    batch: Option<QueryResult>,
    epoch: Option<u64>,
    checkpoint_id: Option<u64>,
    through_sequence: Option<u64>,
}

#[pymethods]
impl SubscriptionFrame {
    /// `"batch"` for rows or `"barrier"` for durable progress.
    #[getter]
    fn kind(&self) -> &'static str {
        self.kind
    }

    /// Portal-local delivery sequence; it is not durable or cluster-global.
    #[getter]
    fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Rows carried by a batch frame, otherwise `None`.
    #[getter]
    fn batch(&self) -> Option<QueryResult> {
        self.batch.clone()
    }

    /// Checkpoint epoch for a barrier frame, otherwise `None`.
    #[getter]
    fn epoch(&self) -> Option<u64> {
        self.epoch
    }

    /// Checkpoint identifier for a barrier frame, otherwise `None`.
    #[getter]
    fn checkpoint_id(&self) -> Option<u64> {
        self.checkpoint_id
    }

    /// Local-log cut represented by a barrier frame, otherwise `None`.
    #[getter]
    fn through_sequence(&self) -> Option<u64> {
        self.through_sequence
    }

    #[getter]
    fn is_batch(&self) -> bool {
        self.kind == "batch"
    }

    #[getter]
    fn is_barrier(&self) -> bool {
        self.kind == "barrier"
    }

    fn __repr__(&self) -> String {
        match &self.batch {
            Some(batch) => format!(
                "SubscriptionFrame(kind='batch', sequence={}, rows={})",
                self.sequence,
                batch.row_count()
            ),
            None => format!(
                "SubscriptionFrame(kind='barrier', sequence={}, epoch={}, checkpoint_id={}, through_sequence={})",
                self.sequence,
                self.epoch.unwrap_or_default(),
                self.checkpoint_id.unwrap_or_default(),
                self.through_sequence.unwrap_or_default()
            ),
        }
    }
}

impl SubscriptionFrame {
    fn from_core(frame: CoreFrame) -> Self {
        match frame {
            CoreFrame::Batch {
                batch,
                sequence,
                lease,
            } => Self {
                kind: "batch",
                sequence,
                batch: Some(QueryResult::from_subscription_batch(batch, lease)),
                epoch: None,
                checkpoint_id: None,
                through_sequence: None,
            },
            CoreFrame::Barrier {
                sequence,
                epoch,
                checkpoint_id,
                through_sequence,
            } => Self {
                kind: "barrier",
                sequence,
                batch: None,
                epoch: Some(epoch),
                checkpoint_id: Some(checkpoint_id),
                through_sequence: Some(through_sequence),
            },
        }
    }
}

fn frame_batch(frame: CoreFrame) -> Option<QueryResult> {
    match frame {
        CoreFrame::Batch { batch, lease, .. } => {
            Some(QueryResult::from_subscription_batch(batch, lease))
        }
        CoreFrame::Barrier { .. } => None,
    }
}

fn next_batch(
    sub: &mut CoreSubscription,
) -> Result<Option<QueryResult>, laminar_db::api::ApiError> {
    loop {
        let Some(frame) = sub.next_frame()? else {
            return Ok(None);
        };
        if let Some(batch) = frame_batch(frame) {
            return Ok(Some(batch));
        }
    }
}

async fn next_batch_async(
    sub: &mut CoreSubscription,
) -> Result<Option<QueryResult>, laminar_db::api::ApiError> {
    loop {
        let Some(frame) = sub.next_frame_async().await? else {
            return Ok(None);
        };
        if let Some(batch) = frame_batch(frame) {
            return Ok(Some(batch));
        }
    }
}

fn try_next_batch(
    sub: &mut CoreSubscription,
) -> Result<Option<QueryResult>, laminar_db::api::ApiError> {
    loop {
        let Some(frame) = sub.try_next_frame()? else {
            return Ok(None);
        };
        if let Some(batch) = frame_batch(frame) {
            return Ok(Some(batch));
        }
    }
}

fn next_frame_timeout(
    sub: &mut CoreSubscription,
    timeout: Duration,
) -> Result<Option<CoreFrame>, laminar_db::api::ApiError> {
    runtime().block_on(async {
        tokio::time::timeout(timeout, sub.next_frame_async())
            .await
            .unwrap_or_else(|_| Err(laminar_db::api::ApiError::subscription_timeout()))
    })
}

fn next_batch_timeout(
    sub: &mut CoreSubscription,
    timeout: Duration,
) -> Result<Option<QueryResult>, laminar_db::api::ApiError> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Some(frame) = next_frame_timeout(sub, remaining)? else {
            return Ok(None);
        };
        if let Some(batch) = frame_batch(frame) {
            return Ok(Some(batch));
        }
    }
}

/// A synchronous subscription to a named stream.
#[pyclass(name = "StreamSubscription")]
pub struct StreamSubscription {
    inner: Mutex<Option<CoreSubscription>>,
}

// SAFETY: CoreSubscription is Send and all shared access is serialized.
unsafe impl Send for StreamSubscription {}
unsafe impl Sync for StreamSubscription {}

#[pymethods]
impl StreamSubscription {
    #[getter]
    fn is_active(&self) -> bool {
        let guard = self.inner.lock();
        guard.as_ref().is_some_and(CoreSubscription::is_active)
    }

    #[getter]
    fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(sub) => {
                let obj = PySchema::from(sub.schema()).into_pyarrow(py)?;
                Ok(obj.into_pyobject(py)?.into_any().unbind())
            }
            None => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "StreamSubscription has been cancelled",
            )),
        }
    }

    /// Blocking wait for the next data or checkpoint frame.
    fn next_frame(&self, py: Python<'_>) -> PyResult<Option<SubscriptionFrame>> {
        py.detach(|| {
            let mut guard = self.inner.lock();
            match guard.as_mut() {
                Some(sub) => Ok(sub
                    .next_frame()
                    .into_pyresult()?
                    .map(SubscriptionFrame::from_core)),
                None => Ok(None),
            }
        })
    }

    /// Non-blocking poll for the next data or checkpoint frame.
    fn try_next_frame(&self, py: Python<'_>) -> PyResult<Option<SubscriptionFrame>> {
        py.detach(|| {
            let mut guard = self.inner.lock();
            match guard.as_mut() {
                Some(sub) => Ok(sub
                    .try_next_frame()
                    .into_pyresult()?
                    .map(SubscriptionFrame::from_core)),
                None => Ok(None),
            }
        })
    }

    /// Wait for a frame for at most `timeout_ms` milliseconds.
    fn next_frame_timeout(
        &self,
        py: Python<'_>,
        timeout_ms: u64,
    ) -> PyResult<Option<SubscriptionFrame>> {
        py.detach(|| {
            let mut guard = self.inner.lock();
            match guard.as_mut() {
                Some(sub) => Ok(next_frame_timeout(sub, Duration::from_millis(timeout_ms))
                    .into_pyresult()?
                    .map(SubscriptionFrame::from_core)),
                None => Ok(None),
            }
        })
    }

    /// Blocking wait for the next data batch, skipping barrier frames.
    fn next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        py.detach(|| {
            let mut guard = self.inner.lock();
            match guard.as_mut() {
                Some(sub) => next_batch(sub).into_pyresult(),
                None => Ok(None),
            }
        })
    }

    /// Wait for a data batch for at most `timeout_ms`, skipping barriers.
    fn next_timeout(&self, py: Python<'_>, timeout_ms: u64) -> PyResult<Option<QueryResult>> {
        py.detach(|| {
            let mut guard = self.inner.lock();
            match guard.as_mut() {
                Some(sub) => {
                    next_batch_timeout(sub, Duration::from_millis(timeout_ms)).into_pyresult()
                }
                None => Ok(None),
            }
        })
    }

    /// Non-blocking poll for the next data batch, skipping barriers.
    fn try_next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        py.detach(|| {
            let mut guard = self.inner.lock();
            match guard.as_mut() {
                Some(sub) => try_next_batch(sub).into_pyresult(),
                None => Ok(None),
            }
        })
    }

    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let mut guard = self.inner.lock();
            if let Some(sub) = guard.as_mut() {
                sub.cancel();
            }
            guard.take();
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(sub) if sub.is_active() => "StreamSubscription(active)".to_owned(),
            Some(_) => "StreamSubscription(finished)".to_owned(),
            None => "StreamSubscription(cancelled)".to_owned(),
        }
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<QueryResult> {
        self.next(py)?.ok_or_else(|| PyStopIteration::new_err(()))
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
        if let Some(sub) = guard.as_mut() {
            sub.cancel();
        }
    }
}

impl StreamSubscription {
    pub fn from_core(sub: CoreSubscription) -> Self {
        Self {
            inner: Mutex::new(Some(sub)),
        }
    }
}

/// An asynchronous subscription to a named stream.
#[pyclass(name = "AsyncStreamSubscription")]
pub struct AsyncStreamSubscription {
    inner: Arc<SyncCell<CoreSubscription>>,
}

#[pymethods]
impl AsyncStreamSubscription {
    #[getter]
    fn is_active(&self) -> bool {
        let guard = self.inner.0.lock();
        guard.as_ref().is_some_and(CoreSubscription::is_active)
    }

    #[getter]
    fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let guard = self.inner.0.lock();
        match guard.as_ref() {
            Some(sub) => {
                let obj = PySchema::from(sub.schema()).into_pyarrow(py)?;
                Ok(obj.into_pyobject(py)?.into_any().unbind())
            }
            None => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "AsyncStreamSubscription has been cancelled",
            )),
        }
    }

    /// Await the next data or checkpoint frame.
    fn next_frame<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let cell = Arc::clone(&self.inner);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let result = tokio::task::spawn_blocking(move || {
                let mut guard = cell.0.lock();
                match guard.as_mut() {
                    Some(sub) => {
                        futures::executor::block_on(sub.next_frame_async()).into_pyresult()
                    }
                    None => Ok(None),
                }
            })
            .await
            .map_err(join_error)??;
            Ok(result.map(SubscriptionFrame::from_core))
        })
    }

    /// Non-blocking poll for the next data or checkpoint frame.
    fn try_next_frame(&self, py: Python<'_>) -> PyResult<Option<SubscriptionFrame>> {
        py.detach(|| {
            let mut guard = self.inner.0.lock();
            match guard.as_mut() {
                Some(sub) => Ok(sub
                    .try_next_frame()
                    .into_pyresult()?
                    .map(SubscriptionFrame::from_core)),
                None => Ok(None),
            }
        })
    }

    /// Blocking compatibility method returning only the next data batch.
    fn next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        py.detach(|| {
            let mut guard = self.inner.0.lock();
            match guard.as_mut() {
                Some(sub) => next_batch(sub).into_pyresult(),
                None => Ok(None),
            }
        })
    }

    /// Blocking compatibility timeout returning only a data batch.
    fn next_timeout(&self, py: Python<'_>, timeout_ms: u64) -> PyResult<Option<QueryResult>> {
        py.detach(|| {
            let mut guard = self.inner.0.lock();
            match guard.as_mut() {
                Some(sub) => {
                    next_batch_timeout(sub, Duration::from_millis(timeout_ms)).into_pyresult()
                }
                None => Ok(None),
            }
        })
    }

    /// Non-blocking compatibility poll returning only a data batch.
    fn try_next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        py.detach(|| {
            let mut guard = self.inner.0.lock();
            match guard.as_mut() {
                Some(sub) => try_next_batch(sub).into_pyresult(),
                None => Ok(None),
            }
        })
    }

    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let mut guard = self.inner.0.lock();
            if let Some(sub) = guard.as_mut() {
                sub.cancel();
            }
            guard.take();
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        let guard = self.inner.0.lock();
        match guard.as_ref() {
            Some(sub) if sub.is_active() => "AsyncStreamSubscription(active)".to_owned(),
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

        let cell = Arc::clone(&self.inner);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let result = tokio::task::spawn_blocking(move || {
                let mut guard = cell.0.lock();
                match guard.as_mut() {
                    Some(sub) => futures::executor::block_on(next_batch_async(sub)).into_pyresult(),
                    None => Ok(None),
                }
            })
            .await
            .map_err(join_error)??;

            result.ok_or_else(|| PyStopAsyncIteration::new_err(()))
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

    fn __aenter__(slf: Py<Self>, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        let obj: Py<PyAny> = slf.into_any();
        pyo3_async_runtimes::tokio::future_into_py(py, async move { Ok(obj) })
    }

    fn __aexit__<'py>(
        &self,
        py: Python<'py>,
        _exc_type: Option<&Bound<'py, PyAny>>,
        _exc_val: Option<&Bound<'py, PyAny>>,
        _exc_tb: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.cancel(py)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async { Ok(false) })
    }

    fn __del__(&self) {
        let mut guard = self.inner.0.lock();
        if let Some(sub) = guard.as_mut() {
            sub.cancel();
        }
    }
}

impl AsyncStreamSubscription {
    pub fn from_core(sub: CoreSubscription) -> Self {
        Self {
            inner: Arc::new(SyncCell(Mutex::new(Some(sub)))),
        }
    }
}

fn join_error(error: tokio::task::JoinError) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(format!("Task join error: {error}"))
}
