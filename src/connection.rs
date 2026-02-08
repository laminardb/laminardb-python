//! Python `Connection` class wrapping `laminardb_core::Connection`.
//!
//! All blocking operations release the GIL via `py.allow_threads()`.
//! The inner connection is `Arc`-wrapped for safe sharing across threads.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyString;

use crate::conversion;
use crate::error::{ConnectionError, IntoPyResult};
use crate::query::QueryResult;
use crate::subscription::Subscription;
use crate::async_support::AsyncSubscription;
use crate::writer::Writer;

/// A connection to a LaminarDB database.
///
/// Use as a context manager for automatic cleanup:
///
///     with laminardb.open("mydb") as conn:
///         conn.insert("sensors", {"ts": 1, "value": 42.0})
#[pyclass(name = "Connection")]
pub struct PyConnection {
    inner: Arc<laminardb_core::Connection>,
    closed: bool,
}

// Safety: laminardb_core::Connection is Send + Sync
unsafe impl Send for PyConnection {}
unsafe impl Sync for PyConnection {}

#[pymethods]
impl PyConnection {
    /// Insert data into a table. Returns the number of rows inserted.
    ///
    /// Accepts: dict, list[dict], dict of lists, PyArrow RecordBatch/Table,
    /// Pandas DataFrame, or Polars DataFrame.
    fn insert(&self, py: Python<'_>, table: &str, data: &Bound<'_, PyAny>) -> PyResult<u64> {
        self.check_closed()?;
        let batches = conversion::python_to_batches(py, data, None)?;
        let conn = self.inner.clone();
        let table = table.to_owned();
        py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            let mut total = 0u64;
            for batch in &batches {
                total += rt.block_on(conn.insert(&table, batch)).into_pyresult()?;
            }
            Ok(total)
        })
    }

    /// Insert JSON string data into a table.
    fn insert_json(&self, py: Python<'_>, table: &str, data: &str) -> PyResult<u64> {
        self.check_closed()?;
        let batches = conversion::json_str_to_batches(data, None)?;
        let conn = self.inner.clone();
        let table = table.to_owned();
        py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            let mut total = 0u64;
            for batch in &batches {
                total += rt.block_on(conn.insert(&table, batch)).into_pyresult()?;
            }
            Ok(total)
        })
    }

    /// Insert CSV string data into a table.
    fn insert_csv(&self, py: Python<'_>, table: &str, data: &str) -> PyResult<u64> {
        self.check_closed()?;
        let batches = conversion::csv_str_to_batches(data, None)?;
        let conn = self.inner.clone();
        let table = table.to_owned();
        py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            let mut total = 0u64;
            for batch in &batches {
                total += rt.block_on(conn.insert(&table, batch)).into_pyresult()?;
            }
            Ok(total)
        })
    }

    /// Create a streaming writer for batched inserts.
    fn writer(&self, table: &str) -> PyResult<Writer> {
        self.check_closed()?;
        Ok(Writer::new(self.inner.clone(), table.to_owned()))
    }

    /// Execute a SQL query and return the full result.
    fn query(&self, py: Python<'_>, sql: &str) -> PyResult<QueryResult> {
        self.check_closed()?;
        let conn = self.inner.clone();
        let sql = sql.to_owned();
        py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            let result = rt.block_on(conn.query(&sql)).into_pyresult()?;
            Ok(QueryResult::from_core(result))
        })
    }

    /// Execute a SQL query and stream results in batches.
    fn stream(&self, py: Python<'_>, sql: &str) -> PyResult<QueryResultIter> {
        self.check_closed()?;
        let conn = self.inner.clone();
        let sql = sql.to_owned();
        let stream = py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            rt.block_on(conn.stream(&sql)).into_pyresult()
        })?;
        Ok(QueryResultIter { inner: stream })
    }

    /// Subscribe to a continuous query (sync iterator).
    fn subscribe(&self, py: Python<'_>, sql: &str) -> PyResult<Subscription> {
        self.check_closed()?;
        let conn = self.inner.clone();
        let sql = sql.to_owned();
        let sub = py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            rt.block_on(conn.subscribe(&sql)).into_pyresult()
        })?;
        Ok(Subscription::from_core(sub))
    }

    /// Subscribe to a continuous query (async iterator).
    fn subscribe_async<'py>(&self, py: Python<'py>, sql: &str) -> PyResult<Bound<'py, PyAny>> {
        self.check_closed()?;
        let conn = self.inner.clone();
        let sql = sql.to_owned();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let sub = conn.subscribe(&sql).await.into_pyresult()?;
            Ok(AsyncSubscription::from_core(sub))
        })
    }

    /// Get the schema of a table as a PyArrow Schema.
    fn schema(&self, py: Python<'_>, table: &str) -> PyResult<PyObject> {
        self.check_closed()?;
        let conn = self.inner.clone();
        let table = table.to_owned();
        let schema = py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            rt.block_on(conn.schema(&table)).into_pyresult()
        })?;
        let pa_schema = pyo3_arrow::PyArrowConvert::to_pyarrow(&Arc::new(schema), py)?;
        Ok(pa_schema.into_pyobject(py)?.into_any().unbind())
    }

    /// Create a new table with the given schema.
    fn create_table(&self, py: Python<'_>, name: &str, schema: &Bound<'_, PyAny>) -> PyResult<()> {
        self.check_closed()?;
        let arrow_schema = conversion::python_to_schema(py, schema)?;
        let conn = self.inner.clone();
        let name = name.to_owned();
        py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            rt.block_on(conn.create_table(&name, &arrow_schema)).into_pyresult()
        })
    }

    /// List all tables in the database.
    fn list_tables(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.check_closed()?;
        let conn = self.inner.clone();
        py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            rt.block_on(conn.list_tables()).into_pyresult()
        })
    }

    /// Close the connection.
    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        if !self.closed {
            let conn = self.inner.clone();
            py.allow_threads(|| {
                let rt = crate::async_support::runtime();
                rt.block_on(conn.close()).into_pyresult()
            })?;
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
        Ok(false) // don't suppress exceptions
    }

    fn __repr__(&self) -> String {
        if self.closed {
            "Connection(closed)".to_owned()
        } else {
            "Connection(open)".to_owned()
        }
    }
}

impl PyConnection {
    /// Create a new connection wrapper from a core connection.
    pub fn from_core(conn: laminardb_core::Connection) -> Self {
        Self {
            inner: Arc::new(conn),
            closed: false,
        }
    }

    fn check_closed(&self) -> PyResult<()> {
        if self.closed {
            Err(ConnectionError::new_err("Connection is closed"))
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Stream iterator (for `Connection.stream()`)
// ---------------------------------------------------------------------------

#[pyclass(name = "_QueryResultIter")]
pub struct QueryResultIter {
    inner: laminardb_core::QueryStream,
}

unsafe impl Send for QueryResultIter {}
unsafe impl Sync for QueryResultIter {}

#[pymethods]
impl QueryResultIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        let stream = &mut self.inner;
        let next = py.allow_threads(|| {
            let rt = crate::async_support::runtime();
            rt.block_on(stream.next())
        });
        match next {
            Some(result) => {
                let batch = result.into_pyresult()?;
                Ok(Some(QueryResult::from_core(batch)))
            }
            None => Ok(None),
        }
    }
}
