//! Python `Connection` class wrapping `laminar_db::api::Connection`.
//!
//! All blocking operations release the GIL via `py.allow_threads()`.

use std::sync::Arc;

use arrow_schema::SchemaRef;
use parking_lot::Mutex;
use pyo3::prelude::*;
use pyo3_arrow::PySchema;

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
///     with laminardb.open(":memory:") as conn:
///         conn.insert("sensors", {"ts": 1, "value": 42.0})
#[pyclass(name = "Connection")]
pub struct PyConnection {
    inner: Arc<Mutex<laminar_db::api::Connection>>,
    closed: bool,
}

// Safety: inner is Arc<Mutex<..>>, closed is only mutated with &mut self
unsafe impl Send for PyConnection {}
unsafe impl Sync for PyConnection {}

#[pymethods]
impl PyConnection {
    /// Insert data into a source. Returns the number of rows inserted.
    fn insert(&self, py: Python<'_>, table: &str, data: &Bound<'_, PyAny>) -> PyResult<u64> {
        self.check_closed()?;
        let batches = conversion::python_to_batches(py, data, None)?;
        let inner = self.inner.clone();
        let table = table.to_owned();
        py.allow_threads(|| {
            let conn = inner.lock();
            let mut total = 0u64;
            for batch in &batches {
                total += conn.insert(&table, batch.clone()).into_pyresult()?;
            }
            Ok(total)
        })
    }

    /// Insert JSON string data into a source.
    fn insert_json(&self, py: Python<'_>, table: &str, data: &str) -> PyResult<u64> {
        self.check_closed()?;
        let batches = conversion::json_str_to_batches(data)?;
        let inner = self.inner.clone();
        let table = table.to_owned();
        py.allow_threads(|| {
            let conn = inner.lock();
            let mut total = 0u64;
            for batch in &batches {
                total += conn.insert(&table, batch.clone()).into_pyresult()?;
            }
            Ok(total)
        })
    }

    /// Insert CSV string data into a source.
    fn insert_csv(&self, py: Python<'_>, table: &str, data: &str) -> PyResult<u64> {
        self.check_closed()?;
        let batches = conversion::csv_str_to_batches(data)?;
        let inner = self.inner.clone();
        let table = table.to_owned();
        py.allow_threads(|| {
            let conn = inner.lock();
            let mut total = 0u64;
            for batch in &batches {
                total += conn.insert(&table, batch.clone()).into_pyresult()?;
            }
            Ok(total)
        })
    }

    /// Create a streaming writer for batched inserts.
    fn writer(&self, py: Python<'_>, table: &str) -> PyResult<Writer> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let table = table.to_owned();
        let writer = py.allow_threads(|| {
            let conn = inner.lock();
            conn.writer(&table).into_pyresult()
        })?;
        Ok(Writer::from_core(writer))
    }

    /// Execute a SQL query and return the full result.
    fn query(&self, py: Python<'_>, sql: &str) -> PyResult<QueryResult> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        py.allow_threads(|| {
            let conn = inner.lock();
            let result = conn.query(&sql).into_pyresult()?;
            Ok(QueryResult::from_core(result))
        })
    }

    /// Execute a SQL query and stream results in batches.
    fn stream(&self, py: Python<'_>, sql: &str) -> PyResult<QueryStreamIter> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        let stream = py.allow_threads(|| {
            let conn = inner.lock();
            conn.query_stream(&sql).into_pyresult()
        })?;
        Ok(QueryStreamIter { inner: Mutex::new(stream) })
    }

    /// Subscribe to a continuous query (sync iterator).
    fn subscribe(&self, py: Python<'_>, sql: &str) -> PyResult<Subscription> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        let _exec = py.allow_threads(|| {
            let conn = inner.lock();
            conn.execute(&sql).into_pyresult()
        })?;
        // For subscriptions, we query_stream and wrap it
        let stream = py.allow_threads(|| {
            let conn = inner.lock();
            conn.query_stream(&sql).into_pyresult()
        })?;
        Ok(Subscription::from_core(stream))
    }

    /// Subscribe to a continuous query (async iterator).
    fn subscribe_async<'py>(&self, py: Python<'py>, sql: &str) -> PyResult<Bound<'py, PyAny>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let stream = {
                let conn = inner.lock();
                conn.query_stream(&sql).into_pyresult()?
            };
            Ok(AsyncSubscription::from_core(stream))
        })
    }

    /// Get the schema of a source as a PyArrow Schema.
    fn schema(&self, py: Python<'_>, table: &str) -> PyResult<Py<PyAny>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let table = table.to_owned();
        let schema: SchemaRef = py.allow_threads(|| {
            let conn = inner.lock();
            conn.get_schema(&table).into_pyresult()
        })?;
        let py_schema = PySchema::from(schema);
        let obj = py_schema.into_pyarrow(py)?;
        Ok(obj.into_pyobject(py)?.into_any().unbind())
    }

    /// Create a new source via SQL DDL.
    fn create_table(&self, py: Python<'_>, name: &str, schema: &Bound<'_, PyAny>) -> PyResult<()> {
        self.check_closed()?;
        let arrow_schema = conversion::python_to_schema(py, schema)?;
        // Build a CREATE SOURCE DDL statement
        let columns: Vec<String> = arrow_schema
            .fields()
            .iter()
            .map(|f| format!("{} {}", f.name(), arrow_type_to_sql(f.data_type())))
            .collect();
        let ddl = format!("CREATE SOURCE {} ({})", name, columns.join(", "));
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let conn = inner.lock();
            conn.execute(&ddl).into_pyresult()?;
            Ok(())
        })
    }

    /// List all sources in the database.
    fn list_tables(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let conn = inner.lock();
            Ok(conn.list_sources())
        })
    }

    /// Close the connection.
    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        if !self.closed {
            self.closed = true;
            // Connection::close consumes self, so we need to take it out
            // Since we use Arc<Mutex<>>, we attempt close if we're the last reference
            let inner = self.inner.clone();
            py.allow_threads(|| -> PyResult<()> {
                // Try to take ownership for close; if other refs exist, drop will handle it
                if let Ok(mutex) = Arc::try_unwrap(inner) {
                    let conn = mutex.into_inner();
                    let _ = conn.close();
                }
                Ok(())
            })?;
        }
        Ok(())
    }

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

    fn __repr__(&self) -> String {
        if self.closed {
            "Connection(closed)".to_owned()
        } else {
            "Connection(open)".to_owned()
        }
    }
}

impl PyConnection {
    pub fn from_core(conn: laminar_db::api::Connection) -> Self {
        Self {
            inner: Arc::new(Mutex::new(conn)),
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

/// Map Arrow DataType to a SQL type string for DDL generation.
fn arrow_type_to_sql(dt: &arrow_schema::DataType) -> &'static str {
    use arrow_schema::DataType;
    match dt {
        DataType::Int8 => "TINYINT",
        DataType::Int16 => "SMALLINT",
        DataType::Int32 => "INT",
        DataType::Int64 => "BIGINT",
        DataType::UInt8 => "TINYINT UNSIGNED",
        DataType::UInt16 => "SMALLINT UNSIGNED",
        DataType::UInt32 => "INT UNSIGNED",
        DataType::UInt64 => "BIGINT UNSIGNED",
        DataType::Float32 => "FLOAT",
        DataType::Float64 => "DOUBLE",
        DataType::Boolean => "BOOLEAN",
        DataType::Utf8 | DataType::LargeUtf8 => "VARCHAR",
        DataType::Binary | DataType::LargeBinary => "BLOB",
        DataType::Date32 | DataType::Date64 => "DATE",
        DataType::Timestamp(_, _) => "TIMESTAMP",
        _ => "VARCHAR",
    }
}

// ---------------------------------------------------------------------------
// Stream iterator (for `Connection.stream()`)
// ---------------------------------------------------------------------------

#[pyclass(name = "_QueryStreamIter")]
pub struct QueryStreamIter {
    inner: Mutex<laminar_db::api::QueryStream>,
}

unsafe impl Send for QueryStreamIter {}
unsafe impl Sync for QueryStreamIter {}

#[pymethods]
impl QueryStreamIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        let result = py.allow_threads(|| {
            let mut stream = self.inner.lock();
            stream.next().into_pyresult()
        })?;
        match result {
            Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
            None => Ok(None),
        }
    }
}
