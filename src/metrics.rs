//! Pipeline metrics and topology classes.

use pyo3::prelude::*;
use pyo3_arrow::PySchema;

// ---------------------------------------------------------------------------
// Pipeline Topology
// ---------------------------------------------------------------------------

/// A node in the pipeline topology graph.
#[pyclass(name = "PipelineNode", frozen)]
pub struct PyPipelineNode {
    name: String,
    node_type: String,
    schema: Option<arrow_schema::SchemaRef>,
    sql: Option<String>,
}

unsafe impl Send for PyPipelineNode {}
unsafe impl Sync for PyPipelineNode {}

#[pymethods]
impl PyPipelineNode {
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// One of "source", "stream", or "sink".
    #[getter]
    fn node_type(&self) -> &str {
        &self.node_type
    }

    #[getter]
    fn schema(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        match &self.schema {
            Some(s) => {
                let py_schema = PySchema::from(s.clone());
                let obj = py_schema.into_pyarrow(py)?;
                Ok(Some(obj.into_pyobject(py)?.into_any().unbind()))
            }
            None => Ok(None),
        }
    }

    #[getter]
    fn sql(&self) -> Option<&str> {
        self.sql.as_deref()
    }

    fn __repr__(&self) -> String {
        format!("PipelineNode(name='{}', type='{}')", self.name, self.node_type)
    }
}

impl PyPipelineNode {
    pub fn from_core(node: laminar_db::PipelineNode) -> Self {
        let node_type = match node.node_type {
            laminar_db::PipelineNodeType::Source => "source",
            laminar_db::PipelineNodeType::Stream => "stream",
            laminar_db::PipelineNodeType::Sink => "sink",
        };
        Self {
            name: node.name,
            node_type: node_type.to_owned(),
            schema: node.schema,
            sql: node.sql,
        }
    }
}

/// An edge in the pipeline topology graph.
#[pyclass(name = "PipelineEdge", frozen)]
pub struct PyPipelineEdge {
    from_node: String,
    to_node: String,
}

#[pymethods]
impl PyPipelineEdge {
    /// Source node name.
    #[getter]
    #[allow(clippy::wrong_self_convention)]
    fn from_node(&self) -> &str {
        &self.from_node
    }

    /// Target node name.
    #[getter]
    fn to_node(&self) -> &str {
        &self.to_node
    }

    fn __repr__(&self) -> String {
        format!("PipelineEdge('{}' -> '{}')", self.from_node, self.to_node)
    }
}

impl PyPipelineEdge {
    pub fn from_core(edge: laminar_db::PipelineEdge) -> Self {
        Self {
            from_node: edge.from,
            to_node: edge.to,
        }
    }
}

/// The pipeline topology: all nodes and edges.
#[pyclass(name = "PipelineTopology", frozen)]
pub struct PyPipelineTopology {
    nodes: Vec<PyPipelineNode>,
    edges: Vec<PyPipelineEdge>,
}

unsafe impl Send for PyPipelineTopology {}
unsafe impl Sync for PyPipelineTopology {}

#[pymethods]
impl PyPipelineTopology {
    #[getter]
    fn nodes(&self) -> Vec<PyPipelineNode> {
        self.nodes
            .iter()
            .map(|n| PyPipelineNode {
                name: n.name.clone(),
                node_type: n.node_type.clone(),
                schema: n.schema.clone(),
                sql: n.sql.clone(),
            })
            .collect()
    }

    #[getter]
    fn edges(&self) -> Vec<PyPipelineEdge> {
        self.edges
            .iter()
            .map(|e| PyPipelineEdge {
                from_node: e.from_node.clone(),
                to_node: e.to_node.clone(),
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "PipelineTopology(nodes={}, edges={})",
            self.nodes.len(),
            self.edges.len()
        )
    }
}

impl PyPipelineTopology {
    pub fn from_core(topo: laminar_db::PipelineTopology) -> Self {
        Self {
            nodes: topo.nodes.into_iter().map(PyPipelineNode::from_core).collect(),
            edges: topo.edges.into_iter().map(PyPipelineEdge::from_core).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline Metrics
// ---------------------------------------------------------------------------

/// Pipeline-wide metrics snapshot.
#[pyclass(name = "PipelineMetrics", frozen)]
pub struct PyPipelineMetrics {
    inner: laminar_db::PipelineMetrics,
}

#[pymethods]
impl PyPipelineMetrics {
    #[getter]
    fn total_events_ingested(&self) -> u64 {
        self.inner.total_events_ingested
    }

    #[getter]
    fn total_events_emitted(&self) -> u64 {
        self.inner.total_events_emitted
    }

    #[getter]
    fn total_events_dropped(&self) -> u64 {
        self.inner.total_events_dropped
    }

    #[getter]
    fn total_cycles(&self) -> u64 {
        self.inner.total_cycles
    }

    #[getter]
    fn total_batches(&self) -> u64 {
        self.inner.total_batches
    }

    #[getter]
    fn uptime_secs(&self) -> f64 {
        self.inner.uptime.as_secs_f64()
    }

    #[getter]
    fn state(&self) -> String {
        format!("{:?}", self.inner.state)
    }

    #[getter]
    fn source_count(&self) -> usize {
        self.inner.source_count
    }

    #[getter]
    fn stream_count(&self) -> usize {
        self.inner.stream_count
    }

    #[getter]
    fn sink_count(&self) -> usize {
        self.inner.sink_count
    }

    #[getter]
    fn pipeline_watermark(&self) -> i64 {
        self.inner.pipeline_watermark
    }

    fn __repr__(&self) -> String {
        format!(
            "PipelineMetrics(state={:?}, ingested={}, emitted={})",
            self.inner.state,
            self.inner.total_events_ingested,
            self.inner.total_events_emitted
        )
    }
}

impl PyPipelineMetrics {
    pub fn from_core(m: laminar_db::PipelineMetrics) -> Self {
        Self { inner: m }
    }
}

/// Metrics for a specific source.
#[pyclass(name = "SourceMetrics", frozen)]
pub struct PySourceMetrics {
    inner: laminar_db::SourceMetrics,
}

#[pymethods]
impl PySourceMetrics {
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn total_events(&self) -> u64 {
        self.inner.total_events
    }

    #[getter]
    fn pending(&self) -> usize {
        self.inner.pending
    }

    #[getter]
    fn capacity(&self) -> usize {
        self.inner.capacity
    }

    #[getter]
    fn is_backpressured(&self) -> bool {
        self.inner.is_backpressured
    }

    #[getter]
    fn watermark(&self) -> i64 {
        self.inner.watermark
    }

    #[getter]
    fn utilization(&self) -> f64 {
        self.inner.utilization
    }

    fn __repr__(&self) -> String {
        format!(
            "SourceMetrics(name='{}', events={}, pending={})",
            self.inner.name, self.inner.total_events, self.inner.pending
        )
    }
}

impl PySourceMetrics {
    pub fn from_core(m: laminar_db::SourceMetrics) -> Self {
        Self { inner: m }
    }
}

/// Metrics for a specific stream.
#[pyclass(name = "StreamMetrics", frozen)]
pub struct PyStreamMetrics {
    inner: laminar_db::StreamMetrics,
}

#[pymethods]
impl PyStreamMetrics {
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn total_events(&self) -> u64 {
        self.inner.total_events
    }

    #[getter]
    fn pending(&self) -> usize {
        self.inner.pending
    }

    #[getter]
    fn capacity(&self) -> usize {
        self.inner.capacity
    }

    #[getter]
    fn is_backpressured(&self) -> bool {
        self.inner.is_backpressured
    }

    #[getter]
    fn watermark(&self) -> i64 {
        self.inner.watermark
    }

    #[getter]
    fn sql(&self) -> Option<&str> {
        self.inner.sql.as_deref()
    }

    fn __repr__(&self) -> String {
        format!(
            "StreamMetrics(name='{}', events={}, pending={})",
            self.inner.name, self.inner.total_events, self.inner.pending
        )
    }
}

impl PyStreamMetrics {
    pub fn from_core(m: laminar_db::StreamMetrics) -> Self {
        Self { inner: m }
    }
}
