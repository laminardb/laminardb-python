//! Streaming `Writer` class for batched inserts into a table.
//!
//! Usage:
//!     with conn.writer("sensors") as w:
//!         w.insert({"ts": 1, "value": 42.0})
//!         w.insert({"ts": 2, "value": 43.0})
//!     # auto-flushed on exit

use parking_lot::Mutex;
use pyo3::prelude::*;
use pyo3_arrow::PySchema;

use crate::conversion;
use crate::error::{IngestionError, IntoPyResult};

/// A streaming writer for batched inserts into a table.
#[pyclass(name = "Writer")]
pub struct Writer {
    inner: Mutex<Option<laminar_db::api::Writer>>,
}

unsafe impl Send for Writer {}
unsafe impl Sync for Writer {}

#[pymethods]
impl Writer {
    /// Add data to the writer (writes through immediately).
    fn insert(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<()> {
        let schema = {
            let guard = self.inner.lock();
            guard
                .as_ref()
                .ok_or_else(|| IngestionError::new_err("Writer is closed"))?
                .schema()
        };
        let batches = conversion::python_to_batches(py, data, Some(schema.as_ref()))?;
        py.detach(|| {
            let mut guard = self.inner.lock();
            let writer = guard
                .as_mut()
                .ok_or_else(|| IngestionError::new_err("Writer is closed"))?;
            for batch in batches {
                writer.write(batch).into_pyresult()?;
            }
            Ok(())
        })
    }

    /// Flush the writer buffer. Returns 0 (flush has no row count).
    fn flush(&self, py: Python<'_>) -> PyResult<u64> {
        py.detach(|| {
            let mut guard = self.inner.lock();
            let writer = guard
                .as_mut()
                .ok_or_else(|| IngestionError::new_err("Writer is closed"))?;
            writer.flush().into_pyresult()?;
            Ok(0)
        })
    }

    /// Flush remaining data and close the writer.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        let writer = {
            let mut guard = self.inner.lock();
            guard.take()
        };
        if let Some(w) = writer {
            py.detach(|| w.close().into_pyresult())?;
        }
        Ok(())
    }

    /// The name of the source this writer is writing to.
    #[getter]
    fn name(&self) -> PyResult<String> {
        let guard = self.inner.lock();
        let writer = guard
            .as_ref()
            .ok_or_else(|| IngestionError::new_err("Writer is closed"))?;
        Ok(writer.name().to_owned())
    }

    /// The schema of the source as a PyArrow Schema.
    #[getter]
    fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let schema_ref = {
            let guard = self.inner.lock();
            let writer = guard
                .as_ref()
                .ok_or_else(|| IngestionError::new_err("Writer is closed"))?;
            writer.schema()
        };
        let py_schema = PySchema::from(schema_ref);
        let obj = py_schema.into_pyarrow(py)?;
        Ok(obj.into_pyobject(py)?.into_any().unbind())
    }

    /// Emit a watermark timestamp.
    ///
    /// Watermarks indicate that all events with timestamps <= the watermark
    /// have been seen.
    fn watermark(&self, timestamp: i64) -> PyResult<()> {
        let guard = self.inner.lock();
        let writer = guard
            .as_ref()
            .ok_or_else(|| IngestionError::new_err("Writer is closed"))?;
        writer.watermark(timestamp);
        Ok(())
    }

    /// Get the current watermark value.
    #[getter]
    fn current_watermark(&self) -> PyResult<i64> {
        let guard = self.inner.lock();
        let writer = guard
            .as_ref()
            .ok_or_else(|| IngestionError::new_err("Writer is closed"))?;
        Ok(writer.current_watermark())
    }

    fn __repr__(&self) -> String {
        let guard = self.inner.lock();
        match guard.as_ref() {
            Some(w) => format!("Writer(source='{}', open)", w.name()),
            None => "Writer(closed)".to_owned(),
        }
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
        self.close(py)?;
        Ok(false)
    }
}

impl Writer {
    pub fn from_core(writer: laminar_db::api::Writer) -> Self {
        Self {
            inner: Mutex::new(Some(writer)),
        }
    }
}
