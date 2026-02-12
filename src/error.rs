//! Error hierarchy mapping LaminarDB API errors to Python exceptions.
//!
//! Exception hierarchy:
//!   LaminarError (base)
//!   ├── ConnectionError
//!   ├── QueryError
//!   ├── IngestionError
//!   ├── SchemaError
//!   └── SubscriptionError

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// Python exception classes
// ---------------------------------------------------------------------------

create_exception!(
    laminardb,
    LaminarError,
    PyException,
    "Base exception for all LaminarDB errors."
);
create_exception!(
    laminardb,
    ConnectionError,
    LaminarError,
    "Raised when a connection cannot be established or is lost."
);
create_exception!(
    laminardb,
    QueryError,
    LaminarError,
    "Raised when a SQL query fails."
);
create_exception!(
    laminardb,
    IngestionError,
    LaminarError,
    "Raised when data ingestion fails."
);
create_exception!(
    laminardb,
    SchemaError,
    LaminarError,
    "Raised when a schema operation fails."
);
create_exception!(
    laminardb,
    SubscriptionError,
    LaminarError,
    "Raised when a subscription operation fails."
);
create_exception!(
    laminardb,
    StreamError,
    LaminarError,
    "Raised when a stream or materialized view operation fails."
);
create_exception!(
    laminardb,
    CheckpointError,
    LaminarError,
    "Raised when a checkpoint operation fails."
);
create_exception!(
    laminardb,
    ConnectorError,
    LaminarError,
    "Raised when a connector operation fails."
);

// ---------------------------------------------------------------------------
// Core error → Python exception mapping
// ---------------------------------------------------------------------------

/// Map a `laminar_db::api::ApiError` to the appropriate Python exception.
///
/// The resulting exception carries a `.code` integer attribute matching
/// the error codes in `laminardb.codes`.
pub fn core_error_to_pyerr(err: laminar_db::api::ApiError) -> PyErr {
    use laminar_db::api::codes;
    let code = err.code();
    let msg = format!("{} ({}): {}", error_code_name(code), code, err.message());
    let py_err = match code {
        codes::CONNECTION_FAILED | codes::CONNECTION_CLOSED | codes::CONNECTION_IN_USE => {
            ConnectionError::new_err(msg)
        }
        codes::TABLE_NOT_FOUND
        | codes::TABLE_EXISTS
        | codes::SCHEMA_MISMATCH
        | codes::INVALID_SCHEMA => SchemaError::new_err(msg),
        codes::INGESTION_FAILED | codes::WRITER_CLOSED | codes::BATCH_SCHEMA_MISMATCH => {
            IngestionError::new_err(msg)
        }
        codes::QUERY_FAILED | codes::SQL_PARSE_ERROR | codes::QUERY_CANCELLED => {
            QueryError::new_err(msg)
        }
        codes::SUBSCRIPTION_FAILED | codes::SUBSCRIPTION_CLOSED | codes::SUBSCRIPTION_TIMEOUT => {
            SubscriptionError::new_err(msg)
        }
        _ => LaminarError::new_err(msg),
    };
    // Attach the numeric error code to the exception instance
    Python::with_gil(|py| {
        let val = py_err.value(py);
        let _ = val.setattr("code", code);
    });
    py_err
}

/// Return a human-readable name for an error code.
fn error_code_name(code: i32) -> &'static str {
    use laminar_db::api::codes;
    match code {
        codes::CONNECTION_FAILED => "Connection failed",
        codes::CONNECTION_CLOSED => "Connection closed",
        codes::CONNECTION_IN_USE => "Connection in use",
        codes::TABLE_NOT_FOUND => "Table not found",
        codes::TABLE_EXISTS => "Table exists",
        codes::SCHEMA_MISMATCH => "Schema mismatch",
        codes::INVALID_SCHEMA => "Invalid schema",
        codes::INGESTION_FAILED => "Ingestion failed",
        codes::WRITER_CLOSED => "Writer closed",
        codes::BATCH_SCHEMA_MISMATCH => "Batch schema mismatch",
        codes::QUERY_FAILED => "Query failed",
        codes::SQL_PARSE_ERROR => "SQL parse error",
        codes::QUERY_CANCELLED => "Query cancelled",
        codes::SUBSCRIPTION_FAILED => "Subscription failed",
        codes::SUBSCRIPTION_CLOSED => "Subscription closed",
        codes::SUBSCRIPTION_TIMEOUT => "Subscription timeout",
        codes::INTERNAL_ERROR => "Internal error",
        codes::SHUTDOWN => "Shutdown",
        _ => "Unknown error",
    }
}

/// Convenience trait to convert `Result<T, laminar_db::api::ApiError>` to `PyResult<T>`.
pub trait IntoPyResult<T> {
    fn into_pyresult(self) -> PyResult<T>;
}

impl<T> IntoPyResult<T> for Result<T, laminar_db::api::ApiError> {
    fn into_pyresult(self) -> PyResult<T> {
        self.map_err(core_error_to_pyerr)
    }
}

/// Register exception classes into the Python module.
pub fn register_exceptions(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    parent.add("LaminarError", parent.py().get_type::<LaminarError>())?;
    parent.add("ConnectionError", parent.py().get_type::<ConnectionError>())?;
    parent.add("QueryError", parent.py().get_type::<QueryError>())?;
    parent.add("IngestionError", parent.py().get_type::<IngestionError>())?;
    parent.add("SchemaError", parent.py().get_type::<SchemaError>())?;
    parent.add(
        "SubscriptionError",
        parent.py().get_type::<SubscriptionError>(),
    )?;
    parent.add("StreamError", parent.py().get_type::<StreamError>())?;
    parent.add("CheckpointError", parent.py().get_type::<CheckpointError>())?;
    parent.add("ConnectorError", parent.py().get_type::<ConnectorError>())?;
    Ok(())
}
