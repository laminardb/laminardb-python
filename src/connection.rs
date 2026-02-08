//! Python `Connection` class wrapping `laminar_db::api::Connection`.
//!
//! All blocking operations release the GIL via `py.allow_threads()`.
//! All API calls enter the global Tokio runtime context so that background
//! tasks spawned by laminar-db (e.g. query stream bridges) actually execute.

use std::sync::Arc;

use arrow_schema::SchemaRef;
use parking_lot::Mutex;
use pyo3::prelude::*;
use pyo3_arrow::PySchema;

use crate::async_support::{AsyncSubscription, runtime};
use crate::conversion;
use crate::error::{ConnectionError, IntoPyResult, QueryError};
use crate::query::QueryResult;
use crate::subscription::Subscription;
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
            let _rt = runtime().enter();
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
            let _rt = runtime().enter();
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
            let _rt = runtime().enter();
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
            let _rt = runtime().enter();
            let conn = inner.lock();
            conn.writer(&table).into_pyresult()
        })?;
        Ok(Writer::from_core(writer))
    }

    /// Execute a SQL query and return the full result.
    ///
    /// Uses `execute()` internally and consumes the query stream with blocking
    /// `next()` calls to avoid the race condition in the API's `query()` method
    /// (which uses non-blocking `try_next()` that returns before data arrives).
    fn query(&self, py: Python<'_>, sql: &str) -> PyResult<QueryResult> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            let result = conn.execute(&sql).into_pyresult()?;
            match result {
                laminar_db::api::ExecuteResult::Query(mut stream) => {
                    let schema = stream.schema();
                    let mut batches = Vec::new();
                    // Use blocking next() — waits for data from the background
                    // tokio task that bridges the DataFusion stream.
                    while let Some(batch) = stream.next().into_pyresult()? {
                        batches.push(batch);
                    }
                    Ok(QueryResult::new(batches, schema))
                }
                laminar_db::api::ExecuteResult::Metadata(batch) => {
                    Ok(QueryResult::from_batch(batch))
                }
                _ => Err(QueryError::new_err(format!(
                    "Expected query result, got DDL/DML: {}",
                    sql
                ))),
            }
        })
    }

    /// Execute a SQL query and stream results in batches.
    fn stream(&self, py: Python<'_>, sql: &str) -> PyResult<QueryStreamIter> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        let stream = py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            conn.query_stream(&sql).into_pyresult()
        })?;
        Ok(QueryStreamIter {
            inner: Mutex::new(stream),
        })
    }

    /// Subscribe to a continuous query (sync iterator).
    fn subscribe(&self, py: Python<'_>, sql: &str) -> PyResult<Subscription> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        let stream = py.allow_threads(|| {
            let _rt = runtime().enter();
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
            let _rt = runtime().enter();
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
            let _rt = runtime().enter();
            let conn = inner.lock();
            conn.get_schema(&table).into_pyresult()
        })?;
        let py_schema = PySchema::from(schema);
        let obj = py_schema.into_pyarrow(py)?;
        Ok(obj.into_pyobject(py)?.into_any().unbind())
    }

    /// Create a new source via SQL DDL.
    ///
    /// Creates a SOURCE for insert/writer operations. The source is also
    /// registered for streaming queries.
    fn create_table(&self, py: Python<'_>, name: &str, schema: &Bound<'_, PyAny>) -> PyResult<()> {
        self.check_closed()?;
        let arrow_schema = conversion::python_to_schema(py, schema)?;
        let columns: Vec<String> = arrow_schema
            .fields()
            .iter()
            .map(|f| format!("{} {}", f.name(), arrow_type_to_sql(f.data_type())))
            .collect();
        let col_defs = columns.join(", ");
        let inner = self.inner.clone();
        let name = name.to_owned();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            let ddl = format!("CREATE SOURCE {} ({})", name, col_defs);
            conn.execute(&ddl).into_pyresult()?;
            Ok(())
        })
    }

    /// List all sources in the database.
    fn list_tables(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.list_sources())
        })
    }

    /// Execute a SQL statement (DDL or DML). Returns rows affected for DML, 0 for DDL.
    fn execute(&self, py: Python<'_>, sql: &str) -> PyResult<u64> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            let result = conn.execute(&sql).into_pyresult()?;
            match result {
                laminar_db::api::ExecuteResult::RowsAffected(n) => Ok(n),
                _ => Ok(0),
            }
        })
    }

    /// Close the connection.
    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        if !self.closed {
            self.closed = true;
            let inner = self.inner.clone();
            py.allow_threads(|| -> PyResult<()> {
                let _rt = runtime().enter();
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
            let _rt = runtime().enter();
            let mut stream = self.inner.lock();
            stream.next().into_pyresult()
        })?;
        match result {
            Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
            None => Ok(None),
        }
    }
}
