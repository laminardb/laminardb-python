//! LaminarDB Python bindings — streaming SQL database.
//!
//! This crate provides Python bindings for `laminar-db` using PyO3 0.27.
//! It exposes a high-level API for connecting to databases, inserting data
//! in multiple formats, querying with SQL, and subscribing to continuous queries.

// PyO3 0.27 deprecates `allow_threads` and `downcast` in favor of 0.28 APIs
// (`detach` and `cast`), but pyo3-arrow 0.15 requires PyO3 0.27. Suppress
// until the ecosystem catches up with 0.28.
#![allow(deprecated)]

mod async_support;
mod catalog;
mod config;
mod connection;
mod conversion;
mod error;
mod execute;
mod metrics;
mod query;
mod stream_subscription;
mod subscription;
mod writer;

use pyo3::prelude::*;

use connection::PyConnection;
use error::IntoPyResult;

/// The native extension module for laminardb.
#[pymodule]
fn _laminardb(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Version
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Classes
    m.add_class::<PyConnection>()?;
    m.add_class::<writer::Writer>()?;
    m.add_class::<query::QueryResult>()?;
    m.add_class::<subscription::Subscription>()?;
    m.add_class::<async_support::AsyncSubscription>()?;
    m.add_class::<connection::QueryStreamIter>()?;
    m.add_class::<stream_subscription::StreamSubscription>()?;
    m.add_class::<stream_subscription::AsyncStreamSubscription>()?;
    m.add_class::<execute::ExecuteResult>()?;
    m.add_class::<config::PyLaminarConfig>()?;

    // Catalog info classes
    m.add_class::<catalog::PySourceInfo>()?;
    m.add_class::<catalog::PySinkInfo>()?;
    m.add_class::<catalog::PyStreamInfo>()?;
    m.add_class::<catalog::PyQueryInfo>()?;

    // Topology & metrics classes
    m.add_class::<metrics::PyPipelineNode>()?;
    m.add_class::<metrics::PyPipelineEdge>()?;
    m.add_class::<metrics::PyPipelineTopology>()?;
    m.add_class::<metrics::PyPipelineMetrics>()?;
    m.add_class::<metrics::PySourceMetrics>()?;
    m.add_class::<metrics::PyStreamMetrics>()?;

    // Exceptions
    error::register_exceptions(m)?;

    // Error code constants submodule
    register_codes(m)?;

    // Module-level functions
    m.add_function(wrap_pyfunction!(open, m)?)?;
    m.add_function(wrap_pyfunction!(connect, m)?)?;

    Ok(())
}

/// Register the `codes` submodule with all error code constants.
fn register_codes(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    use laminar_db::api::codes;
    let py = parent.py();
    let codes_module = PyModule::new(py, "codes")?;

    // Connection (100-199)
    codes_module.add("CONNECTION_FAILED", codes::CONNECTION_FAILED)?;
    codes_module.add("CONNECTION_CLOSED", codes::CONNECTION_CLOSED)?;
    codes_module.add("CONNECTION_IN_USE", codes::CONNECTION_IN_USE)?;

    // Schema (200-299)
    codes_module.add("TABLE_NOT_FOUND", codes::TABLE_NOT_FOUND)?;
    codes_module.add("TABLE_EXISTS", codes::TABLE_EXISTS)?;
    codes_module.add("SCHEMA_MISMATCH", codes::SCHEMA_MISMATCH)?;
    codes_module.add("INVALID_SCHEMA", codes::INVALID_SCHEMA)?;

    // Ingestion (300-399)
    codes_module.add("INGESTION_FAILED", codes::INGESTION_FAILED)?;
    codes_module.add("WRITER_CLOSED", codes::WRITER_CLOSED)?;
    codes_module.add("BATCH_SCHEMA_MISMATCH", codes::BATCH_SCHEMA_MISMATCH)?;

    // Query (400-499)
    codes_module.add("QUERY_FAILED", codes::QUERY_FAILED)?;
    codes_module.add("SQL_PARSE_ERROR", codes::SQL_PARSE_ERROR)?;
    codes_module.add("QUERY_CANCELLED", codes::QUERY_CANCELLED)?;

    // Subscription (500-599)
    codes_module.add("SUBSCRIPTION_FAILED", codes::SUBSCRIPTION_FAILED)?;
    codes_module.add("SUBSCRIPTION_CLOSED", codes::SUBSCRIPTION_CLOSED)?;
    codes_module.add("SUBSCRIPTION_TIMEOUT", codes::SUBSCRIPTION_TIMEOUT)?;

    // Internal (900-999)
    codes_module.add("INTERNAL_ERROR", codes::INTERNAL_ERROR)?;
    codes_module.add("SHUTDOWN", codes::SHUTDOWN)?;

    parent.add_submodule(&codes_module)?;
    Ok(())
}

/// Open a LaminarDB database.
///
/// The `path` parameter names the database instance.  LaminarDB currently
/// operates in-memory; the path is accepted for API compatibility but does
/// not enable file-based persistence.  Use `LaminarConfig(storage_dir=...)`
/// if you need WAL / checkpoint storage.
///
/// Example:
///     db = laminardb.open("my_database")
///     db = laminardb.open("mydb", config=laminardb.LaminarConfig(buffer_size=1024))
#[pyfunction]
#[pyo3(signature = (path, *, config=None))]
#[allow(unused_variables)]
fn open(
    py: Python<'_>,
    path: &str,
    config: Option<&config::PyLaminarConfig>,
) -> PyResult<PyConnection> {
    py.allow_threads(|| {
        let _rt = async_support::runtime().enter();
        let conn = match config {
            Some(cfg) => {
                laminar_db::api::Connection::open_with_config(cfg.to_core()).into_pyresult()?
            }
            None => laminar_db::api::Connection::open().into_pyresult()?,
        };
        Ok(PyConnection::from_core(conn))
    })
}

/// Connect to a LaminarDB database.
///
/// Currently equivalent to `open()` — the URI is accepted for forward
/// compatibility but remote connections are not yet supported.
///
/// Example:
///     db = laminardb.connect("laminar://localhost:5432/mydb")
#[pyfunction]
fn connect(py: Python<'_>, _uri: &str) -> PyResult<PyConnection> {
    py.allow_threads(|| {
        let _rt = async_support::runtime().enter();
        let conn = laminar_db::api::Connection::open().into_pyresult()?;
        Ok(PyConnection::from_core(conn))
    })
}
