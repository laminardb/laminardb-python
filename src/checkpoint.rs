//! Checkpoint result class.

use pyo3::prelude::*;

/// The result of a checkpoint operation.
#[pyclass(name = "CheckpointResult", frozen)]
pub struct PyCheckpointResult {
    checkpoint_id: u64,
}

unsafe impl Send for PyCheckpointResult {}
unsafe impl Sync for PyCheckpointResult {}

#[pymethods]
impl PyCheckpointResult {
    /// The checkpoint ID assigned by the database.
    #[getter]
    fn checkpoint_id(&self) -> u64 {
        self.checkpoint_id
    }

    fn __bool__(&self) -> bool {
        true
    }

    fn __int__(&self) -> u64 {
        self.checkpoint_id
    }

    fn __repr__(&self) -> String {
        format!("CheckpointResult(checkpoint_id={})", self.checkpoint_id)
    }
}

impl PyCheckpointResult {
    pub fn from_id(checkpoint_id: u64) -> Self {
        Self { checkpoint_id }
    }
}
