//! `LaminarConfig` class for configuring database connections.

use std::path::PathBuf;

use pyo3::prelude::*;

/// Configuration for a LaminarDB connection.
///
/// Example:
///     config = laminardb.LaminarConfig(buffer_size=1024)
///     conn = laminardb.open("mydb", config=config)
#[pyclass(name = "LaminarConfig")]
#[derive(Clone)]
pub struct PyLaminarConfig {
    buffer_size: usize,
    storage_dir: Option<String>,
    checkpoint_interval_ms: Option<u64>,
    table_spill_threshold: usize,
}

#[pymethods]
impl PyLaminarConfig {
    #[new]
    #[pyo3(signature = (*, buffer_size=65536, storage_dir=None, checkpoint_interval_ms=None, table_spill_threshold=1_000_000))]
    fn new(
        buffer_size: usize,
        storage_dir: Option<String>,
        checkpoint_interval_ms: Option<u64>,
        table_spill_threshold: usize,
    ) -> Self {
        Self {
            buffer_size,
            storage_dir,
            checkpoint_interval_ms,
            table_spill_threshold,
        }
    }

    #[getter]
    fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    #[getter]
    fn storage_dir(&self) -> Option<&str> {
        self.storage_dir.as_deref()
    }

    #[getter]
    fn checkpoint_interval_ms(&self) -> Option<u64> {
        self.checkpoint_interval_ms
    }

    #[getter]
    fn table_spill_threshold(&self) -> usize {
        self.table_spill_threshold
    }

    fn __repr__(&self) -> String {
        format!(
            "LaminarConfig(buffer_size={}, storage_dir={:?}, checkpoint_interval_ms={:?}, table_spill_threshold={})",
            self.buffer_size,
            self.storage_dir,
            self.checkpoint_interval_ms,
            self.table_spill_threshold
        )
    }
}

impl PyLaminarConfig {
    /// Convert to the core `LaminarConfig` type.
    pub fn to_core(&self) -> laminar_db::LaminarConfig {
        use laminar_core::streaming::StreamCheckpointConfig;

        let checkpoint = self
            .checkpoint_interval_ms
            .map(|ms| StreamCheckpointConfig {
                interval_ms: Some(ms),
                data_dir: self.storage_dir.as_ref().map(PathBuf::from),
                ..StreamCheckpointConfig::default()
            });

        laminar_db::LaminarConfig {
            default_buffer_size: self.buffer_size,
            storage_dir: self.storage_dir.as_ref().map(PathBuf::from),
            checkpoint,
            table_spill_threshold: self.table_spill_threshold,
            ..laminar_db::LaminarConfig::default()
        }
    }
}
