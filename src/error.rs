//! Error hierarchy mapping LaminarDB core errors to Python exceptions.
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

create_exception!(laminardb, LaminarError, PyException, "Base exception for all LaminarDB errors.");
create_exception!(laminardb, ConnectionError, LaminarError, "Raised when a connection cannot be established or is lost.");
create_exception!(laminardb, QueryError, LaminarError, "Raised when a SQL query fails.");
create_exception!(laminardb, IngestionError, LaminarError, "Raised when data ingestion fails.");
create_exception!(laminardb, SchemaError, LaminarError, "Raised when a schema operation fails.");
create_exception!(laminardb, SubscriptionError, LaminarError, "Raised when a subscription operation fails.");

// ---------------------------------------------------------------------------
// Core error → Python exception mapping
// ---------------------------------------------------------------------------

/// Map a `laminardb_core::ApiError` to the appropriate Python exception.
pub fn core_error_to_pyerr(err: laminardb_core::ApiError) -> PyErr {
    use laminardb_core::ApiError;
    match &err {
        ApiError::Connection(_) => ConnectionError::new_err(err.to_string()),
        ApiError::Query(_) => QueryError::new_err(err.to_string()),
        ApiError::Ingestion(_) => IngestionError::new_err(err.to_string()),
        ApiError::Schema(_) => SchemaError::new_err(err.to_string()),
        ApiError::Subscription(_) => SubscriptionError::new_err(err.to_string()),
        _ => LaminarError::new_err(err.to_string()),
    }
}

/// Convenience trait to convert `Result<T, laminardb_core::ApiError>` to `PyResult<T>`.
pub trait IntoPyResult<T> {
    fn into_pyresult(self) -> PyResult<T>;
}

impl<T> IntoPyResult<T> for Result<T, laminardb_core::ApiError> {
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
    parent.add("SubscriptionError", parent.py().get_type::<SubscriptionError>())?;
    Ok(())
}
