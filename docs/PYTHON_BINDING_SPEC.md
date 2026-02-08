# Python Binding Feature Spec

Complete specification for bringing the laminardb Python library to full feature parity with the `laminar_db::api` Rust crate.

**Current state**: 45 tests passing, ~40% API coverage.
**Target state**: Full `api::Connection` surface + catalog, metrics, pipeline observability.

---

## Phase 1: Wire Through Existing API

**Blocked by**: Nothing — all Rust API methods already exist.
**Files**: `connection.rs`, `writer.rs`, `query.rs`, `subscription.rs`, `async_support.rs`, `_laminardb.pyi`, `__init__.py`

### P1-01: `Connection.list_streams() -> list[str]`

Expose `api::Connection::list_streams()`.

**Rust** (`connection.rs`):
```rust
fn list_streams(&self, py: Python<'_>) -> PyResult<Vec<String>> {
    self.check_closed()?;
    let inner = self.inner.clone();
    py.allow_threads(|| {
        let _rt = runtime().enter();
        let conn = inner.lock();
        Ok(conn.list_streams())
    })
}
```

**Stub** (`_laminardb.pyi`):
```python
def list_streams(self) -> list[str]:
    """List all streams in the database."""
    ...
```

**Test**: `test_connection.py` — create a source, run a CREATE STREAM DDL, verify it appears.

---

### P1-02: `Connection.list_sinks() -> list[str]`

Expose `api::Connection::list_sinks()`.

Identical pattern to P1-01 but calls `conn.list_sinks()`.

**Test**: Verify returns empty list on fresh connection, populated after CREATE SINK.

---

### P1-03: `Connection.start()`

Expose `api::Connection::start()` to start the streaming pipeline.

**Rust** (`connection.rs`):
```rust
fn start(&self, py: Python<'_>) -> PyResult<()> {
    self.check_closed()?;
    let inner = self.inner.clone();
    py.allow_threads(|| {
        let _rt = runtime().enter();
        let conn = inner.lock();
        conn.start().into_pyresult()
    })
}
```

**Stub**: `def start(self) -> None`

**Test**: Call `start()` on a fresh connection, verify no error.

---

### P1-04: `Connection.is_closed` property

Expose `api::Connection::is_closed()` as a Python property.

**Rust** (`connection.rs`):
```rust
#[getter]
fn is_closed(&self) -> bool {
    self.closed
}
```

Note: Use the Python-side `self.closed` flag (already tracked) rather than calling through to Rust, since the Python layer sets `closed = true` on `close()`.

**Stub**: `@property` `def is_closed(self) -> bool`

**Test**: `assert not conn.is_closed; conn.close(); assert conn.is_closed`

---

### P1-05: `Connection.checkpoint() -> int | None`

Expose `api::Connection::checkpoint()`.

**Rust** (`connection.rs`):
```rust
fn checkpoint(&self, py: Python<'_>) -> PyResult<Option<u64>> {
    self.check_closed()?;
    let inner = self.inner.clone();
    py.allow_threads(|| {
        let _rt = runtime().enter();
        let conn = inner.lock();
        conn.checkpoint().into_pyresult()
    })
}
```

**Stub**: `def checkpoint(self) -> int | None`

**Test**: Call on fresh connection (checkpointing disabled), verify returns `None`.

---

### P1-06: `Connection.is_checkpoint_enabled` property

**Rust** (`connection.rs`):
```rust
#[getter]
fn is_checkpoint_enabled(&self, py: Python<'_>) -> PyResult<bool> {
    self.check_closed()?;
    let inner = self.inner.clone();
    py.allow_threads(|| {
        let _rt = runtime().enter();
        let conn = inner.lock();
        Ok(conn.is_checkpoint_enabled())
    })
}
```

**Stub**: `@property` `def is_checkpoint_enabled(self) -> bool`

**Test**: Default config → `False`.

---

### P1-07: Writer — `schema`, `name`, `watermark()`, `current_watermark`

Expose the four missing `api::Writer` methods.

**Rust** (`writer.rs`):
```rust
#[getter]
fn name(&self) -> PyResult<String> {
    self.check_closed()?;
    let guard = self.inner.lock();
    Ok(guard.as_ref().unwrap().name().to_owned())
}

#[getter]
fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
    self.check_closed()?;
    let schema_ref = {
        let guard = self.inner.lock();
        guard.as_ref().unwrap().schema()
    };
    let py_schema = pyo3_arrow::PySchema::from(schema_ref);
    let obj = py_schema.into_pyarrow(py)?;
    Ok(obj.into_pyobject(py)?.into_any().unbind())
}

fn watermark(&self, timestamp: i64) -> PyResult<()> {
    self.check_closed()?;
    let guard = self.inner.lock();
    guard.as_ref().unwrap().watermark(timestamp);
    Ok(())
}

#[getter]
fn current_watermark(&self) -> PyResult<i64> {
    self.check_closed()?;
    let guard = self.inner.lock();
    Ok(guard.as_ref().unwrap().current_watermark())
}
```

**Stubs**:
```python
class Writer:
    @property
    def name(self) -> str: ...
    @property
    def schema(self) -> Any: ...  # pyarrow.Schema
    def watermark(self, timestamp: int) -> None: ...
    @property
    def current_watermark(self) -> int: ...
```

**Tests**:
- `writer.name == "sensors"`
- `writer.schema` has expected fields
- `writer.watermark(100); assert writer.current_watermark == 100`

---

### P1-08: `_QueryStreamIter` — `try_next()`, `is_active`, `cancel()`

Expose the three missing `QueryStream` methods on the Python iterator.

**Rust** (`connection.rs`, in `QueryStreamIter` impl):
```rust
fn try_next(&self, py: Python<'_>) -> PyResult<Option<QueryResult>> {
    py.allow_threads(|| {
        let _rt = runtime().enter();
        let mut stream = self.inner.lock();
        match stream.try_next().into_pyresult()? {
            Some(batch) => Ok(Some(QueryResult::from_batch(batch))),
            None => Ok(None),
        }
    })
}

#[getter]
fn is_active(&self) -> bool {
    let stream = self.inner.lock();
    stream.is_active()
}

fn cancel(&self, py: Python<'_>) -> PyResult<()> {
    py.allow_threads(|| {
        let mut stream = self.inner.lock();
        stream.cancel();
        Ok(())
    })
}
```

**Test**: `stream = conn.stream(SQL); assert stream.is_active; stream.cancel()`

---

### P1-09: `QueryResult.schema` property + `num_batches`

**Rust** (`query.rs`):
```rust
#[getter]
fn schema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let py_schema = pyo3_arrow::PySchema::from(self.schema.clone());
    let obj = py_schema.into_pyarrow(py)?;
    Ok(obj.into_pyobject(py)?.into_any().unbind())
}

#[getter]
fn num_batches(&self) -> usize {
    self.batches.len()
}
```

**Stubs**:
```python
@property
def schema(self) -> Any: ...  # pyarrow.Schema
@property
def num_batches(self) -> int: ...
```

**Test**: `result.schema` has same fields as source; `result.num_batches >= 1`.

---

### P1-10: `__repr__` for Writer, Subscription, AsyncSubscription, QueryStreamIter

**Rust additions**:
```rust
// Writer
fn __repr__(&self) -> String {
    let guard = self.inner.lock();
    match guard.as_ref() {
        Some(w) => format!("Writer(source='{}', open)", w.name()),
        None => "Writer(closed)".to_owned(),
    }
}

// Subscription
fn __repr__(&self) -> String {
    let guard = self.inner.lock();
    match guard.as_ref() {
        Some(s) if s.is_active() => "Subscription(active)".to_owned(),
        Some(_) => "Subscription(finished)".to_owned(),
        None => "Subscription(cancelled)".to_owned(),
    }
}

// AsyncSubscription — same pattern
// QueryStreamIter — same pattern
```

**Test**: Verify repr strings in each state.

---

## Phase 2: Error Codes & Richer Execute

**Blocked by**: Nothing.
**Files**: `error.rs`, `lib.rs`, `connection.rs`, `_laminardb.pyi`

### P2-01: Expose error codes on exceptions

Every `LaminarError` (and subclasses) should carry a `.code` integer attribute.

**Approach**: Change `core_error_to_pyerr()` to attach the code as an attribute on the exception instance. PyO3's `create_exception!` creates basic exception types — we need custom exception classes with a `code` field.

**Implementation**: Replace `create_exception!` with `#[pyclass(extends=PyException)]` for each error type:

```rust
#[pyclass(extends=pyo3::exceptions::PyException, name="LaminarError")]
pub struct PyLaminarError {
    #[pyo3(get)]
    code: i32,
}
```

Or simpler: keep `create_exception!` and set the code as a dynamic attribute:

```rust
pub fn core_error_to_pyerr(err: laminar_db::api::ApiError) -> PyErr {
    let code = err.code();
    let msg = err.to_string();
    let py_err = match code { /* existing mapping */ };
    // Attach code: py_err.value(py).setattr("code", code)
    py_err
}
```

**Stub**:
```python
class LaminarError(Exception):
    code: int
```

**Test**: `try: conn.query("BAD"); except laminardb.QueryError as e: assert e.code == 400`

---

### P2-02: Export error code constants

Expose `laminar_db::api::codes::*` as `laminardb.codes.*` or `laminardb.error_codes.*`.

**Rust** (`lib.rs`):
```rust
// In _laminardb module init
let codes = PyModule::new(m.py(), "codes")?;
codes.add("CONNECTION_FAILED", 100)?;
codes.add("CONNECTION_CLOSED", 101)?;
// ... all 15 constants
m.add_submodule(&codes)?;
```

**Stub**: New `codes` submodule in `_laminardb.pyi`.

**Test**: `assert laminardb.codes.QUERY_FAILED == 400`

---

### P2-03: Richer `execute()` return type

Currently `execute()` returns `int` (rows affected) and silently discards DDL info, metadata, and query results.

**New return type**: `ExecuteResult` class with discriminated variants:

```python
class ExecuteResult:
    @property
    def result_type(self) -> str:
        """One of 'ddl', 'rows_affected', 'query', 'metadata'."""
        ...
    @property
    def rows_affected(self) -> int | None: ...
    @property
    def ddl_type(self) -> str | None: ...      # e.g. "CREATE SOURCE"
    @property
    def ddl_object(self) -> str | None: ...    # e.g. "sensors"
    def to_query_result(self) -> QueryResult | None: ...
```

**Backward compatibility**: Keep `execute()` returning `int` for row count. Add `execute_ext()` or make `execute()` return `ExecuteResult` (which supports `__int__()` for backward compat via `RowsAffected(n) → int(n)`).

**Recommended approach**: `execute()` returns `ExecuteResult` which has `__int__()` returning the row count (0 for DDL, n for DML). This preserves `rows = conn.execute(sql)` usage while giving access to richer info.

**Test**: `result = conn.execute("CREATE SOURCE ..."); assert result.result_type == "ddl"; assert int(result) == 0`

---

## Phase 3: Configuration

**Blocked by**: Nothing.
**Files**: New `src/config.rs`, `lib.rs`, `_laminardb.pyi`

### P3-01: `LaminarConfig` pyclass

```python
class LaminarConfig:
    def __init__(
        self,
        *,
        buffer_size: int = 65536,
        storage_dir: str | None = None,
        checkpoint_interval_secs: float | None = None,
        table_spill_threshold: int = 1_000_000,
    ) -> None: ...
```

**Rust** (new `src/config.rs`):
```rust
#[pyclass(name = "LaminarConfig")]
#[derive(Clone)]
pub struct PyLaminarConfig {
    pub(crate) buffer_size: usize,
    pub(crate) storage_dir: Option<String>,
    pub(crate) checkpoint_interval_secs: Option<f64>,
    pub(crate) table_spill_threshold: usize,
}

#[pymethods]
impl PyLaminarConfig {
    #[new]
    #[pyo3(signature = (*, buffer_size=65536, storage_dir=None, checkpoint_interval_secs=None, table_spill_threshold=1_000_000))]
    fn new(...) -> Self { ... }
}

impl PyLaminarConfig {
    pub fn to_core(&self) -> laminar_db::LaminarConfig { ... }
}
```

### P3-02: `open()` with config kwarg

```python
def open(path: str, *, config: LaminarConfig | None = None) -> Connection: ...
def open_with_config(config: LaminarConfig) -> Connection: ...
```

**Rust** (`lib.rs`):
```rust
#[pyfunction]
#[pyo3(signature = (path, *, config=None))]
fn open(py: Python<'_>, _path: &str, config: Option<&PyLaminarConfig>) -> PyResult<PyConnection> {
    py.allow_threads(|| {
        let _rt = runtime().enter();
        let conn = match config {
            Some(cfg) => laminar_db::api::Connection::open_with_config(cfg.to_core()),
            None => laminar_db::api::Connection::open(),
        }.into_pyresult()?;
        Ok(PyConnection::from_core(conn))
    })
}
```

**Test**: Open with custom `buffer_size=1024`, verify connection works.

---

## Phase 4: Catalog & Observability

**Blocked by**: Main crate changes (see `MAIN_CRATE_API_PROMPT.md`).
**Files**: New `src/catalog_types.rs`, new `src/metrics.rs`, `connection.rs`, `lib.rs`, `_laminardb.pyi`

### P4-01: Catalog info classes

Python classes wrapping the Rust info types:

```python
class SourceInfo:
    @property
    def name(self) -> str: ...
    @property
    def schema(self) -> Any: ...  # pyarrow.Schema
    @property
    def watermark_column(self) -> str | None: ...

class SinkInfo:
    @property
    def name(self) -> str: ...

class StreamInfo:
    @property
    def name(self) -> str: ...
    @property
    def sql(self) -> str | None: ...

class QueryInfo:
    @property
    def id(self) -> int: ...
    @property
    def sql(self) -> str: ...
    @property
    def active(self) -> bool: ...
```

### P4-02: Connection catalog methods

```python
class Connection:
    def sources(self) -> list[SourceInfo]: ...
    def sinks(self) -> list[SinkInfo]: ...
    def streams(self) -> list[StreamInfo]: ...
    def queries(self) -> list[QueryInfo]: ...
```

These call the new `api::Connection` methods (added by main crate changes) and convert to Python objects.

### P4-03: Pipeline topology

```python
class PipelineNode:
    @property
    def name(self) -> str: ...
    @property
    def node_type(self) -> str: ...  # "source", "stream", "sink"
    @property
    def schema(self) -> Any | None: ...
    @property
    def sql(self) -> str | None: ...

class PipelineEdge:
    @property
    def from_node(self) -> str: ...  # "from" is a keyword
    @property
    def to_node(self) -> str: ...

class PipelineTopology:
    @property
    def nodes(self) -> list[PipelineNode]: ...
    @property
    def edges(self) -> list[PipelineEdge]: ...

class Connection:
    def topology(self) -> PipelineTopology: ...
```

### P4-04: Metrics classes

```python
class PipelineMetrics:
    @property
    def total_events_ingested(self) -> int: ...
    @property
    def total_events_emitted(self) -> int: ...
    @property
    def total_events_dropped(self) -> int: ...
    @property
    def total_cycles(self) -> int: ...
    @property
    def total_batches(self) -> int: ...
    @property
    def uptime_secs(self) -> float: ...
    @property
    def state(self) -> str: ...  # "Created", "Running", "Stopped", etc.
    @property
    def source_count(self) -> int: ...
    @property
    def stream_count(self) -> int: ...
    @property
    def sink_count(self) -> int: ...
    @property
    def pipeline_watermark(self) -> int: ...

class SourceMetrics:
    @property
    def name(self) -> str: ...
    @property
    def total_events(self) -> int: ...
    @property
    def pending(self) -> int: ...
    @property
    def capacity(self) -> int: ...
    @property
    def is_backpressured(self) -> bool: ...
    @property
    def watermark(self) -> int: ...
    @property
    def utilization(self) -> float: ...

class StreamMetrics:
    @property
    def name(self) -> str: ...
    @property
    def total_events(self) -> int: ...
    @property
    def pending(self) -> int: ...
    @property
    def capacity(self) -> int: ...
    @property
    def is_backpressured(self) -> bool: ...
    @property
    def watermark(self) -> int: ...
    @property
    def sql(self) -> str | None: ...
```

### P4-05: Connection metrics/pipeline methods

```python
class Connection:
    def metrics(self) -> PipelineMetrics: ...
    def source_metrics(self, name: str) -> SourceMetrics | None: ...
    def all_source_metrics(self) -> list[SourceMetrics]: ...
    def stream_metrics(self, name: str) -> StreamMetrics | None: ...
    def all_stream_metrics(self) -> list[StreamMetrics]: ...
    @property
    def pipeline_state(self) -> str: ...
    @property
    def pipeline_watermark(self) -> int: ...
    @property
    def total_events_processed(self) -> int: ...
    @property
    def source_count(self) -> int: ...
    @property
    def sink_count(self) -> int: ...
    @property
    def active_query_count(self) -> int: ...
```

---

## Phase 5: True Subscriptions & Watermarks

**Blocked by**: Main crate `api::Connection::subscribe()` method.
**Files**: `subscription.rs`, `async_support.rs`, `connection.rs`, `_laminardb.pyi`

### P5-01: Rewrite subscriptions to use `ArrowSubscription`

Currently `Subscription` and `AsyncSubscription` wrap `QueryStream` (a finite query result stream). The main crate has `ArrowSubscription` which is the proper type for continuous subscriptions with:
- `next()` — blocking wait
- `next_timeout(Duration)` — timeout-aware wait
- `try_next()` — non-blocking poll
- Proper channel-based delivery from the streaming pipeline

Once `api::Connection::subscribe()` exists, update `conn.subscribe(sql)` to call it instead of `conn.query_stream(sql)`.

### P5-02: `Subscription.next_timeout(seconds: float)`

```python
class Subscription:
    def next_timeout(self, timeout: float) -> QueryResult | None:
        """Wait up to `timeout` seconds for the next result.

        Returns None if the subscription is closed.
        Raises SubscriptionError on timeout.
        """
        ...
```

**Rust**: Convert `f64` seconds → `Duration`, call `ArrowSubscription::next_timeout()`.

### P5-03: `AsyncSubscription.next_timeout(seconds: float)`

Same as P5-02 but returns a coroutine.

### P5-04: Subscription `schema` property

```python
class Subscription:
    @property
    def schema(self) -> Any: ...  # pyarrow.Schema

class AsyncSubscription:
    @property
    def schema(self) -> Any: ...
```

### P5-05: Callback subscription (optional, advanced)

```python
def subscribe_callback(
    self,
    sql: str,
    on_data: Callable[[QueryResult], None],
    on_error: Callable[[LaminarError], None] | None = None,
) -> SubscriptionHandle:
    """Register callbacks for continuous query results.

    Callbacks are invoked from a background thread.
    """
    ...

class SubscriptionHandle:
    @property
    def is_active(self) -> bool: ...
    def cancel(self) -> None: ...
```

This mirrors the FFI `laminar_subscribe_callback` pattern. Implementation uses a background thread that calls `ArrowSubscription::next()` in a loop and invokes the Python callbacks (reacquiring GIL for each call).

---

## Phase 6: Query Control & Shutdown

**Blocked by**: Main crate changes.
**Files**: `connection.rs`, `_laminardb.pyi`

### P6-01: `Connection.cancel_query(query_id: int)`

```python
def cancel_query(self, query_id: int) -> None:
    """Cancel a running query by ID."""
    ...
```

### P6-02: `Connection.shutdown()`

```python
def shutdown(self) -> None:
    """Gracefully shut down the streaming pipeline.

    Unlike close(), this waits for in-flight events to drain.
    """
    ...
```

### P6-03: Async context manager

```python
class Connection:
    async def __aenter__(self) -> Connection: ...
    async def __aexit__(self, *args) -> None: ...
```

This allows `async with laminardb.open("db") as conn:` usage. The `__aexit__` calls `close()`.

---

## Test Plan Summary

| Phase | New Tests | Test File |
|-------|----------|-----------|
| 1 | ~20 tests | `test_connection.py`, `test_writer.py`, `test_query.py`, `test_subscription.py` |
| 2 | ~8 tests | `test_errors.py` (new), `test_execute.py` (new) |
| 3 | ~5 tests | `test_config.py` (new) |
| 4 | ~15 tests | `test_catalog.py` (new), `test_metrics.py` (new) |
| 5 | ~10 tests | `test_subscription.py` (extended) |
| 6 | ~5 tests | `test_connection.py` (extended) |

---

## Dependency on Main Crate

| Phase | Depends on main crate? | What's needed |
|-------|----------------------|---------------|
| 1 | No | — |
| 2 | No | — |
| 3 | No | — |
| 4 | **Yes** | `api::Connection` passthrough methods for catalog, metrics, pipeline |
| 5 | **Yes** | `api::Connection::subscribe()` returning `ArrowSubscription` |
| 6 | **Yes** | `api::Connection::cancel_query()`, `shutdown()` |

See `MAIN_CRATE_API_PROMPT.md` for the exact changes needed in the main crate.
