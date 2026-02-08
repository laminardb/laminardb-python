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
use crate::catalog::{PyQueryInfo, PySinkInfo, PySourceInfo, PyStreamInfo};
use crate::conversion;
use crate::error::{ConnectionError, IntoPyResult, QueryError};
use crate::execute::ExecuteResult;
use crate::metrics::{
    PyPipelineMetrics, PyPipelineTopology, PySourceMetrics, PyStreamMetrics,
};
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

    /// List all streams in the database.
    fn list_streams(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.list_streams())
        })
    }

    /// List all sinks in the database.
    fn list_sinks(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.list_sinks())
        })
    }

    /// Start the streaming pipeline.
    fn start(&self, py: Python<'_>) -> PyResult<()> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            conn.start().into_pyresult()
        })
    }

    /// Whether the connection is closed.
    #[getter]
    fn is_closed(&self) -> bool {
        self.closed
    }

    /// Trigger a checkpoint. Returns the checkpoint ID on success, or None.
    fn checkpoint(&self, py: Python<'_>) -> PyResult<Option<u64>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            conn.checkpoint().into_pyresult()
        })
    }

    /// Whether checkpointing is enabled for this connection.
    #[getter]
    fn is_checkpoint_enabled(&self, py: Python<'_>) -> PyResult<bool> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.is_checkpoint_enabled())
        })
    }

    // ── Catalog info ──

    /// List source info with schemas and watermark columns.
    fn sources(&self, py: Python<'_>) -> PyResult<Vec<PySourceInfo>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.source_info().into_iter().map(PySourceInfo::from_core).collect())
        })
    }

    /// List sink info.
    fn sinks(&self, py: Python<'_>) -> PyResult<Vec<PySinkInfo>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.sink_info().into_iter().map(PySinkInfo::from_core).collect())
        })
    }

    /// List stream info with SQL definitions.
    fn streams(&self, py: Python<'_>) -> PyResult<Vec<PyStreamInfo>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.stream_info().into_iter().map(PyStreamInfo::from_core).collect())
        })
    }

    /// List active and completed query info.
    fn queries(&self, py: Python<'_>) -> PyResult<Vec<PyQueryInfo>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.query_info().into_iter().map(PyQueryInfo::from_core).collect())
        })
    }

    // ── Pipeline topology & state ──

    /// Get the pipeline topology graph.
    fn topology(&self, py: Python<'_>) -> PyResult<PyPipelineTopology> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(PyPipelineTopology::from_core(conn.pipeline_topology()))
        })
    }

    /// Get the pipeline state as a string.
    #[getter]
    fn pipeline_state(&self, py: Python<'_>) -> PyResult<String> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.pipeline_state())
        })
    }

    /// Get the global pipeline watermark.
    #[getter]
    fn pipeline_watermark(&self, py: Python<'_>) -> PyResult<i64> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.pipeline_watermark())
        })
    }

    /// Get total events processed across all sources.
    #[getter]
    fn total_events_processed(&self, py: Python<'_>) -> PyResult<u64> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.total_events_processed())
        })
    }

    /// Get the number of registered sources.
    #[getter]
    fn source_count(&self, py: Python<'_>) -> PyResult<usize> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.source_count())
        })
    }

    /// Get the number of registered sinks.
    #[getter]
    fn sink_count(&self, py: Python<'_>) -> PyResult<usize> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.sink_count())
        })
    }

    /// Get the number of active queries.
    #[getter]
    fn active_query_count(&self, py: Python<'_>) -> PyResult<usize> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.active_query_count())
        })
    }

    // ── Metrics ──

    /// Get pipeline-wide metrics snapshot.
    fn metrics(&self, py: Python<'_>) -> PyResult<PyPipelineMetrics> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(PyPipelineMetrics::from_core(conn.metrics()))
        })
    }

    /// Get metrics for a specific source, or None if not found.
    fn source_metrics(&self, py: Python<'_>, name: &str) -> PyResult<Option<PySourceMetrics>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let name = name.to_owned();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.source_metrics(&name).map(PySourceMetrics::from_core))
        })
    }

    /// Get metrics for all sources.
    fn all_source_metrics(&self, py: Python<'_>) -> PyResult<Vec<PySourceMetrics>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.all_source_metrics().into_iter().map(PySourceMetrics::from_core).collect())
        })
    }

    /// Get metrics for a specific stream, or None if not found.
    fn stream_metrics(&self, py: Python<'_>, name: &str) -> PyResult<Option<PyStreamMetrics>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let name = name.to_owned();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.stream_metrics(&name).map(PyStreamMetrics::from_core))
        })
    }

    /// Get metrics for all streams.
    fn all_stream_metrics(&self, py: Python<'_>) -> PyResult<Vec<PyStreamMetrics>> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            Ok(conn.all_stream_metrics().into_iter().map(PyStreamMetrics::from_core).collect())
        })
    }

    // ── Query control & shutdown ──

    /// Cancel a running query by ID.
    fn cancel_query(&self, py: Python<'_>, query_id: u64) -> PyResult<()> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            conn.cancel_query(query_id).into_pyresult()
        })
    }

    /// Gracefully shut down the streaming pipeline.
    ///
    /// Unlike `close()`, this waits for in-flight events to drain.
    fn shutdown(&self, py: Python<'_>) -> PyResult<()> {
        self.check_closed()?;
        let inner = self.inner.clone();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            conn.shutdown().into_pyresult()
        })
    }

    /// Execute a SQL statement (DDL, DML, or query).
    ///
    /// Returns an `ExecuteResult` that supports `int()` for backward-compatible
    /// row count access, plus `.result_type`, `.ddl_type`, `.ddl_object`, and
    /// `.to_query_result()` for richer introspection.
    fn execute(&self, py: Python<'_>, sql: &str) -> PyResult<ExecuteResult> {
        self.check_closed()?;
        let inner = self.inner.clone();
        let sql = sql.to_owned();
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let conn = inner.lock();
            let result = conn.execute(&sql).into_pyresult()?;
            Ok(ExecuteResult::from_core(result))
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

    fn __aenter__(slf: Py<Self>, py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        let obj: Py<PyAny> = slf.into_any();
        pyo3_async_runtimes::tokio::future_into_py(py, async move { Ok(obj) })
    }

    fn __aexit__<'py>(
        &mut self,
        py: Python<'py>,
        _exc_type: Option<&Bound<'py, PyAny>>,
        _exc_val: Option<&Bound<'py, PyAny>>,
        _exc_tb: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.close(py)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async { Ok(false) })
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
    /// Non-blocking poll for the next result batch.
    fn try_next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
        py.allow_threads(|| {
            let _rt = runtime().enter();
            let mut stream = self.inner.lock();
            match stream.try_next().into_pyresult()? {
                Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
                None => Ok(None),
            }
        })
    }

    /// Whether the stream is still active.
    #[getter]
    fn is_active(&self) -> bool {
        let stream = self.inner.lock();
        stream.is_active()
    }

    /// Cancel the stream.
    fn cancel(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            let mut stream = self.inner.lock();
            stream.cancel();
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        let stream = self.inner.lock();
        if stream.is_active() {
            "QueryStream(active)".to_owned()
        } else {
            "QueryStream(finished)".to_owned()
        }
    }

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
