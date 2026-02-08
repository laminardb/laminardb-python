//! Streaming `Writer` class for batched inserts into a table.
//!
//! Usage:
//!     with conn.writer("sensors") as w:
//!         w.insert({"ts": 1, "value": 42.0})
//!         w.insert({"ts": 2, "value": 43.0})
//!     # auto-flushed on exit

use std::sync::Arc;

use arrow_array::RecordBatch;
use parking_lot::Mutex;
use pyo3::prelude::*;

use crate::conversion;
use crate::error::{IngestionError, IntoPyResult};

/// A streaming writer for batched inserts into a table.
#[pyclass(name = "Writer")]
pub struct Writer {
    conn: Arc<laminardb_core::Connection>,
    table: String,
    buffer: Mutex<Vec<RecordBatch>>,
    closed: bool,
}

// Safety: all fields are Send + Sync
unsafe impl Send for Writer {}
unsafe impl Sync for Writer {}

#[pymethods]
impl Writer {
    /// Add data to the write buffer.
    fn insert(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<()> {
        self.check_closed()?;
        let batches = conversion::python_to_batches(py, data, None)?;
        let mut buf = self.buffer.lock();
        buf.extend(batches);
        Ok(())
    }

    /// Flush the buffer to the database. Returns the number of rows written.
    fn flush(&self, py: Python<'_>) -> PyResult<u64> {
        self.check_closed()?;
        let batches: Vec<RecordBatch> = {
            let mut buf = self.buffer.lock();
            std::mem::take(&mut *buf)
        };

        if batches.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.clone();
        let table = self.table.clone();
        py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            let mut total = 0u64;
            for batch in &batches {
                total += rt.block_on(conn.insert(&table, batch)).into_pyresult()?;
            }
            Ok(total)
        })
    }

    /// Flush remaining data and close the writer.
    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        if !self.closed {
            self.flush(py)?;
            self.closed = true;
        }
        Ok(())
    }

    // Context manager support
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
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
    pub fn new(conn: Arc<laminardb_core::Connection>, table: String) -> Self {
        Self {
            conn,
            table,
            buffer: Mutex::new(Vec::new()),
            closed: false,
        }
    }

    fn check_closed(&self) -> PyResult<()> {
        if self.closed {
            Err(IngestionError::new_err("Writer is closed"))
        } else {
            Ok(())
        }
    }
}
