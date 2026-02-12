//! `QueryResult` class providing zero-copy exports to multiple formats.

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use pyo3::prelude::*;
use pyo3_arrow::PySchema;

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

    /// The schema as a PyArrow Schema.
    #[getter]
    fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let py_schema = PySchema::from(self.schema.clone());
        let obj = py_schema.into_pyarrow(py)?;
        Ok(obj.into_pyobject(py)?.into_any().unbind())
    }

    /// Number of batches in the result.
    #[getter]
    fn num_batches(&self) -> usize {
        self.batches.len()
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

    // ── DuckDB-style aliases ──

    /// Convert to a Pandas DataFrame (DuckDB-style alias for `to_pandas()`).
    fn df<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_pandas(py)
    }

    /// Convert to a Polars DataFrame (DuckDB-style alias).
    ///
    /// If `lazy=True`, wraps the result in a Polars `LazyFrame`.
    #[pyo3(signature = (*, lazy = false))]
    fn pl<'py>(&self, py: Python<'py>, lazy: bool) -> PyResult<Bound<'py, PyAny>> {
        let df = self.to_polars(py)?;
        if lazy {
            df.call_method0("lazy")
        } else {
            Ok(df)
        }
    }

    /// Convert to a PyArrow Table (DuckDB-style alias for `to_arrow()`).
    fn arrow<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_arrow(py)
    }

    /// Fetch all rows as a list of tuples.
    fn fetchall<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table = self.to_arrow(py)?;
        let pylist = table.call_method0("to_pylist")?;
        // Convert list[dict] → list[tuple] by extracting values
        let builtins = py.import("builtins")?;
        let tuple_fn = builtins.getattr("tuple")?;
        let rows = pyo3::types::PyList::empty(py);
        for row in pylist.try_iter()? {
            let row: Bound<'py, PyAny> = row?;
            let values = row.call_method0("values")?;
            let tup = tuple_fn.call1((values,))?;
            rows.append(tup)?;
        }
        Ok(rows.into_any())
    }

    /// Fetch the first row as a tuple, or None if empty.
    fn fetchone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if self.num_rows() == 0 {
            return Ok(py.None().into_bound(py));
        }
        let rows = self.fetchall(py)?;
        rows.get_item(0)
    }

    /// Fetch up to `size` rows as a list of tuples.
    #[pyo3(signature = (size = 1))]
    fn fetchmany<'py>(&self, py: Python<'py>, size: usize) -> PyResult<Bound<'py, PyAny>> {
        let rows = self.fetchall(py)?;
        let len = rows.len()?;
        let end = std::cmp::min(size, len);
        let slice = rows.call_method1("__getitem__", (pyo3::types::PySlice::new(py, 0, end as isize, 1),))?;
        Ok(slice)
    }

    /// Print a preview of the result (up to `max_rows` rows).
    #[pyo3(signature = (max_rows = 20))]
    fn show<'py>(&self, py: Python<'py>, max_rows: usize) -> PyResult<()> {
        let table = self.to_arrow(py)?;
        let sliced = table.call_method1("slice", (0, max_rows))?;
        let df = sliced.call_method0("to_pandas")?;
        let text = df.call_method0("to_string")?;
        let builtins = py.import("builtins")?;
        builtins.call_method1("print", (text,))?;
        Ok(())
    }

    /// HTML representation for Jupyter notebooks.
    fn _repr_html_<'py>(&self, py: Python<'py>) -> PyResult<String> {
        match self.to_pandas(py) {
            Ok(df) => {
                let html = df.call_method0("_repr_html_")?;
                html.extract::<String>()
            }
            Err(_) => Ok(format!("<pre>{}</pre>", self.__repr__())),
        }
    }

    /// Number of rows (enables `len(result)`).
    fn __len__(&self) -> usize {
        self.num_rows()
    }

    /// Iterate over rows as tuples (enables `for row in result`).
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self.fetchall(py)?;
        rows.call_method0("__iter__")
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
    /// Create from pre-collected batches and schema.
    pub fn new(batches: Vec<RecordBatch>, schema: SchemaRef) -> Self {
        Self { batches, schema }
    }

    /// Total row count across all batches.
    pub fn row_count(&self) -> usize {
        self.batches.iter().map(|b| b.num_rows()).sum()
    }

    /// Read-only access to the batches (for cloning in ExecuteResult).
    pub fn batches_ref(&self) -> &[RecordBatch] {
        &self.batches
    }

    /// Read-only access to the schema.
    pub fn schema_ref(&self) -> &SchemaRef {
        &self.schema
    }

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
