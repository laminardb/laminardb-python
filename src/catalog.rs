//! Catalog info classes: SourceInfo, SinkInfo, StreamInfo, QueryInfo.

use pyo3::prelude::*;
use pyo3_arrow::PySchema;

/// Information about a registered source.
#[pyclass(name = "SourceInfo", frozen)]
pub struct PySourceInfo {
    name: String,
    schema: arrow_schema::SchemaRef,
    watermark_column: Option<String>,
}

unsafe impl Send for PySourceInfo {}
unsafe impl Sync for PySourceInfo {}

#[pymethods]
impl PySourceInfo {
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    #[getter]
    fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let py_schema = PySchema::from(self.schema.clone());
        let obj = py_schema.into_pyarrow(py)?;
        Ok(obj.into_pyobject(py)?.into_any().unbind())
    }

    #[getter]
    fn watermark_column(&self) -> Option<&str> {
        self.watermark_column.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "SourceInfo(name='{}', columns={})",
            self.name,
            self.schema.fields().len()
        )
    }
}

impl PySourceInfo {
    pub fn from_core(info: laminar_db::SourceInfo) -> Self {
        Self {
            name: info.name,
            schema: info.schema,
            watermark_column: info.watermark_column,
        }
    }
}

/// Information about a registered sink.
#[pyclass(name = "SinkInfo", frozen)]
pub struct PySinkInfo {
    name: String,
}

#[pymethods]
impl PySinkInfo {
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    fn __repr__(&self) -> String {
        format!("SinkInfo(name='{}')", self.name)
    }
}

impl PySinkInfo {
    pub fn from_core(info: laminar_db::SinkInfo) -> Self {
        Self { name: info.name }
    }
}

/// Information about a registered stream.
#[pyclass(name = "StreamInfo", frozen)]
pub struct PyStreamInfo {
    name: String,
    sql: Option<String>,
}

#[pymethods]
impl PyStreamInfo {
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    #[getter]
    fn sql(&self) -> Option<&str> {
        self.sql.as_deref()
    }

    fn __repr__(&self) -> String {
        format!("StreamInfo(name='{}')", self.name)
    }
}

impl PyStreamInfo {
    pub fn from_core(info: laminar_db::StreamInfo) -> Self {
        Self {
            name: info.name,
            sql: info.sql,
        }
    }
}

/// Information about a registered query.
#[pyclass(name = "QueryInfo", frozen)]
pub struct PyQueryInfo {
    id: u64,
    sql: String,
    active: bool,
}

#[pymethods]
impl PyQueryInfo {
    #[getter]
    fn id(&self) -> u64 {
        self.id
    }

    #[getter]
    fn sql(&self) -> &str {
        &self.sql
    }

    #[getter]
    fn active(&self) -> bool {
        self.active
    }

    fn __repr__(&self) -> String {
        format!(
            "QueryInfo(id={}, active={})",
            self.id, self.active
        )
    }
}

impl PyQueryInfo {
    pub fn from_core(info: laminar_db::QueryInfo) -> Self {
        Self {
            id: info.id,
            sql: info.sql,
            active: info.active,
        }
    }
}
