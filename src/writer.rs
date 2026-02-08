//! Streaming `Writer` class for batched inserts into a table.
//!
//! Usage:
//!     with conn.writer("sensors") as w:
//!         w.insert({"ts": 1, "value": 42.0})
//!         w.insert({"ts": 2, "value": 43.0})
//!     # auto-flushed on exit

use parking_lot::Mutex;
use pyo3::prelude::*;

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
        self.check_closed()?;
        let batches = conversion::python_to_batches(py, data, None)?;
        py.allow_threads(|| {
            let mut guard = self.inner.lock();
            let writer = guard.as_mut().unwrap();
            for batch in batches {
                writer.write(batch).into_pyresult()?;
            }
            Ok(())
        })
    }

    /// Flush the writer buffer. Returns 0 (flush has no row count).
    fn flush(&self, py: Python<'_>) -> PyResult<u64> {
        self.check_closed()?;
        py.allow_threads(|| {
            let mut guard = self.inner.lock();
            let writer = guard.as_mut().unwrap();
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
            py.allow_threads(|| {
                w.close().into_pyresult()
            })?;
        }
        Ok(())
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

    fn check_closed(&self) -> PyResult<()> {
        let guard = self.inner.lock();
        if guard.is_none() {
            Err(IngestionError::new_err("Writer is closed"))
        } else {
            Ok(())
        }
    }
}
