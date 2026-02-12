//! `ExecuteResult` class wrapping `laminar_db::api::ExecuteResult`.
//!
//! Provides a richer return type for `Connection.execute()` that preserves
//! DDL info, metadata, and query results while supporting `int()` for
//! backward compatibility (rows affected).

use pyo3::prelude::*;

use crate::query::QueryResult;

/// The result of a SQL `execute()` call.
///
/// Supports `int(result)` for backward-compatible row count access.
#[pyclass(name = "ExecuteResult")]
pub struct ExecuteResult {
    result_type: &'static str,
    rows_affected: u64,
    ddl_type: Option<String>,
    ddl_object: Option<String>,
    query_result: Option<QueryResult>,
}

unsafe impl Send for ExecuteResult {}
unsafe impl Sync for ExecuteResult {}

#[pymethods]
impl ExecuteResult {
    /// The type of result: "ddl", "rows_affected", "query", or "metadata".
    #[getter]
    fn result_type(&self) -> &str {
        self.result_type
    }

    /// Number of rows affected (DML) or 0 (DDL/query/metadata).
    #[getter]
    fn rows_affected(&self) -> u64 {
        self.rows_affected
    }

    /// The DDL statement type (e.g. "CREATE SOURCE"), or None.
    #[getter]
    fn ddl_type(&self) -> Option<&str> {
        self.ddl_type.as_deref()
    }

    /// The DDL object name (e.g. "sensors"), or None.
    #[getter]
    fn ddl_object(&self) -> Option<&str> {
        self.ddl_object.as_deref()
    }

    /// Convert to a QueryResult if this was a query or metadata result.
    fn to_query_result(&self) -> Option<QueryResult> {
        self.query_result
            .as_ref()
            .map(|qr| QueryResult::new(qr.batches_ref().to_vec(), qr.schema_ref().clone()))
    }

    /// Support `int(result)` → rows affected count.
    fn __int__(&self) -> u64 {
        self.rows_affected
    }

    /// Support `bool(result)` → True if rows were affected or DDL succeeded.
    fn __bool__(&self) -> bool {
        self.rows_affected > 0 || self.result_type != "rows_affected"
    }

    fn __repr__(&self) -> String {
        match self.result_type {
            "ddl" => format!(
                "ExecuteResult(ddl, {} {})",
                self.ddl_type.as_deref().unwrap_or("?"),
                self.ddl_object.as_deref().unwrap_or("?"),
            ),
            "rows_affected" => format!("ExecuteResult(rows_affected={})", self.rows_affected),
            "query" => {
                if let Some(ref qr) = self.query_result {
                    format!("ExecuteResult(query, rows={})", qr.row_count())
                } else {
                    "ExecuteResult(query)".to_owned()
                }
            }
            "metadata" => {
                if let Some(ref qr) = self.query_result {
                    format!("ExecuteResult(metadata, rows={})", qr.row_count())
                } else {
                    "ExecuteResult(metadata)".to_owned()
                }
            }
            _ => "ExecuteResult(unknown)".to_owned(),
        }
    }
}

impl ExecuteResult {
    /// Create from the core API's ExecuteResult enum.
    pub fn from_core(result: laminar_db::api::ExecuteResult) -> Self {
        match result {
            laminar_db::api::ExecuteResult::Ddl(info) => Self {
                result_type: "ddl",
                rows_affected: 0,
                ddl_type: Some(info.statement_type),
                ddl_object: Some(info.object_name),
                query_result: None,
            },
            laminar_db::api::ExecuteResult::RowsAffected(n) => Self {
                result_type: "rows_affected",
                rows_affected: n,
                ddl_type: None,
                ddl_object: None,
                query_result: None,
            },
            laminar_db::api::ExecuteResult::Query(mut stream) => {
                let schema = stream.schema();
                let mut batches = Vec::new();
                while let Ok(Some(batch)) = stream.next() {
                    batches.push(batch);
                }
                let qr = QueryResult::new(batches, schema);
                let row_count = qr.row_count();
                Self {
                    result_type: "query",
                    rows_affected: row_count as u64,
                    ddl_type: None,
                    ddl_object: None,
                    query_result: Some(qr),
                }
            }
            laminar_db::api::ExecuteResult::Metadata(batch) => {
                let qr = QueryResult::from_batch(batch);
                Self {
                    result_type: "metadata",
                    rows_affected: 0,
                    ddl_type: None,
                    ddl_object: None,
                    query_result: Some(qr),
                }
            }
        }
    }
}
