//! `QueryResult` class providing zero-copy exports to multiple formats.

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use pyo3::prelude::*;

use crate::conversion;

/// The result of a SQL query.
#[pyclass(name = "QueryResult")]
pub struct QueryResult {
    batches: Vec<RecordBatch>,
    schema: SchemaRef,
}

unsafe impl Send for QueryResult {}
unsafe impl Sync for QueryResult {}

#[pymethods]
impl QueryResult {
    /// Number of rows in the result.
    #[getter]
    fn num_rows(&self) -> usize {
        self.batches.iter().map(|b| b.num_rows()).sum()
    }

    /// Number of columns in the result.
    #[getter]
    fn num_columns(&self) -> usize {
        self.schema.fields().len()
    }

    /// Column names.
    #[getter]
    fn columns(&self) -> Vec<String> {
        self.schema
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect()
    }

    /// Convert to a PyArrow Table.
    fn to_arrow<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        conversion::batches_to_pyarrow(py, &self.batches, &self.schema)
    }

    /// Convert to a Pandas DataFrame.
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        conversion::batches_to_pandas(py, &self.batches, &self.schema)
    }

    /// Convert to a Polars DataFrame.
    fn to_polars<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        conversion::batches_to_polars(py, &self.batches, &self.schema)
    }

    /// Convert to a list of Python dicts.
    fn to_dicts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        conversion::batches_to_dicts(py, &self.batches, &self.schema)
    }

    /// Auto-detect best available library and convert.
    fn to_df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        conversion::batches_to_best_df(py, &self.batches, &self.schema)
    }

    /// Arrow PyCapsule C Stream interface for zero-copy export.
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Export via PyArrow table's __arrow_c_stream__
        let table = self.to_arrow(py)?;
        match requested_schema {
            Some(schema) => table.call_method1("__arrow_c_stream__", (schema,)),
            None => table.call_method0("__arrow_c_stream__"),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "QueryResult(rows={}, columns={})",
            self.num_rows(),
            self.num_columns()
        )
    }
}

impl QueryResult {
    /// Create from a core query result.
    pub fn from_core(result: laminar_db::api::QueryResult) -> Self {
        let schema = result.schema();
        let batches = result.into_batches();
        Self { batches, schema }
    }

    /// Create from a single RecordBatch (used by stream iterator).
    pub fn from_batch(batch: RecordBatch) -> Self {
        let schema = batch.schema();
        Self {
            batches: vec![batch],
            schema,
        }
    }
}
