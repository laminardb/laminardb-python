//! LaminarDB Python bindings — streaming SQL database.
//!
//! This crate provides Python bindings for `laminardb-core` using PyO3 0.28.
//! It exposes a high-level API for connecting to databases, inserting data
//! in multiple formats, querying with SQL, and subscribing to continuous queries.

mod async_support;
mod connection;
mod conversion;
mod error;
mod query;
mod subscription;
mod writer;

use pyo3::prelude::*;

use connection::PyConnection;
use error::IntoPyResult;

/// The native extension module for laminardb.
///
/// This module is re-exported by `laminardb.__init__` and should not be
/// imported directly.
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
    m.add_class::<connection::QueryResultIter>()?;

    // Exceptions
    error::register_exceptions(m)?;

    // Module-level functions
    m.add_function(wrap_pyfunction!(open, m)?)?;
    m.add_function(wrap_pyfunction!(connect, m)?)?;

    Ok(())
}

/// Open a LaminarDB database at the given file path.
///
/// Example:
///     db = laminardb.open("my_database")
#[pyfunction]
fn open(py: Python<'_>, path: &str) -> PyResult<PyConnection> {
    let path = path.to_owned();
    py.allow_threads(|| {
        let rt = async_support::runtime();
        let conn = rt.block_on(laminardb_core::Connection::open(&path)).into_pyresult()?;
        Ok(PyConnection::from_core(conn))
    })
}

/// Connect to a LaminarDB database via URI.
///
/// Example:
///     db = laminardb.connect("laminar://localhost:5432/mydb")
#[pyfunction]
fn connect(py: Python<'_>, uri: &str) -> PyResult<PyConnection> {
    let uri = uri.to_owned();
    py.allow_threads(|| {
        let rt = async_support::runtime();
        let conn = rt.block_on(laminardb_core::Connection::connect(&uri)).into_pyresult()?;
        Ok(PyConnection::from_core(conn))
    })
}
