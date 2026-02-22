# Feature Specifications: Subscriptions, Admin & Advanced (F-PY-024 -- F-PY-032)

---

# F-PY-024: Reactive Subscriptions

## Metadata
- **Phase**: Core Streaming
- **Priority**: P0
- **Dependencies**: `Connection`, `QueryResult`, `arrow-rs`, `pyo3-async-runtimes`
- **Main Repo Module**: `laminar_db::api::QueryStream`, `laminar_db::api::ArrowSubscription`
- **Implementation Status**: Implemented

## Summary

Provides four subscription classes for consuming continuous query results and named-stream updates in both synchronous and asynchronous modes. SQL-based subscriptions (`subscribe`, `subscribe_async`) wrap `laminar_db::api::QueryStream` and deliver arbitrary streaming SQL query results. Named-stream subscriptions (`subscribe_stream`, `subscribe_stream_async`) wrap `laminar_db::api::ArrowSubscription` and deliver updates for streams created via `CREATE STREAM ... AS SELECT ...`. All subscription classes support non-blocking polling (`try_next`), cancellation (`cancel`), and an `is_active` property. Stream subscriptions additionally expose a `schema` property and timeout-based waiting (`next_timeout`). A Python-side callback mode is available through `MaterializedView.subscribe(handler=fn)`, which spawns a background daemon thread to deliver `ChangeEvent` objects.

## Python API

```python
# SQL-based continuous query subscriptions
sub: Subscription = db.subscribe(sql)            # sync iterator
asub: AsyncSubscription = await db.subscribe_async(sql)  # async iterator

# Named-stream subscriptions
ssub: StreamSubscription = db.subscribe_stream(name)           # sync iterator
assub: AsyncStreamSubscription = await db.subscribe_stream_async(name)  # async iterator

# Common methods on all subscription types
sub.is_active       # bool property
sub.cancel()        # stop subscription
sub.try_next()      # non-blocking poll -> QueryResult | None

# StreamSubscription-specific
ssub.schema         # pyarrow.Schema property
ssub.next()         # blocking wait -> QueryResult | None
ssub.next_timeout(timeout_ms)  # blocking wait with timeout -> QueryResult | None

# High-level callback mode (Python-side)
from laminardb import MaterializedView, ChangeEvent
mv = MaterializedView(conn, "my_stream")
thread = mv.subscribe(handler=lambda event: print(event))  # background thread
# or raw subscription:
raw_sub = mv.subscribe()  # returns StreamSubscription
```

## Usage Examples

```python
import laminardb

with laminardb.open(":memory:") as db:
    db.execute("CREATE SOURCE ticks (symbol VARCHAR, price DOUBLE, ts BIGINT)")
    db.execute("CREATE STREAM vwap AS SELECT symbol, SUM(price)/COUNT(*) AS avg_price FROM ticks GROUP BY symbol")
    db.start()

    # --- Sync iteration over SQL query stream ---
    sub = db.subscribe("SELECT * FROM ticks")
    db.insert("ticks", {"symbol": "AAPL", "price": 150.0, "ts": 1})
    batch = sub.try_next()  # non-blocking poll
    if batch is not None:
        print(batch.to_pandas())
    sub.cancel()

    # --- Sync iteration over named stream ---
    ssub = db.subscribe_stream("vwap")
    print(ssub.schema)  # pyarrow.Schema
    db.insert("ticks", {"symbol": "GOOG", "price": 2800.0, "ts": 2})
    result = ssub.next_timeout(1000)  # wait up to 1 second
    if result:
        print(result.to_dicts())
    ssub.cancel()

    # --- Callback mode ---
    from laminardb import MaterializedView, ChangeEvent
    mv = laminardb.mv(db, "vwap")
    def on_change(event: ChangeEvent):
        print(f"Got {len(event)} rows")
    thread = mv.subscribe(handler=on_change)
    db.insert("ticks", {"symbol": "AAPL", "price": 155.0, "ts": 3})
    # callback fires in background daemon thread
```

```python
import asyncio
import laminardb

async def main():
    async with laminardb.open(":memory:") as db:
        db.execute("CREATE SOURCE events (id BIGINT, data VARCHAR)")
        db.start()

        # --- Async iteration over SQL query stream ---
        asub = await db.subscribe_async("SELECT * FROM events")
        db.insert("events", {"id": 1, "data": "hello"})
        async for batch in asub:
            print(batch.to_pandas())
            break  # process one batch

        # --- Async iteration over named stream ---
        db.execute("CREATE STREAM derived AS SELECT id, UPPER(data) AS upper_data FROM events")
        assub = await db.subscribe_stream_async("derived")
        db.insert("events", {"id": 2, "data": "world"})
        async for batch in assub:
            print(batch.to_arrow())
            break

asyncio.run(main())
```

## Implementation Notes

- `Subscription` and `AsyncSubscription` wrap `laminar_db::api::QueryStream` (from `Connection::query_stream(sql)`). The Rust `QueryStream` is not `Send`, so `unsafe impl Send` and `unsafe impl Sync` are applied with `parking_lot::Mutex` protecting all access.
- `StreamSubscription` and `AsyncStreamSubscription` wrap `laminar_db::api::ArrowSubscription` (from `Connection::subscribe(stream_name)`). Same safety pattern with `Mutex<Option<ArrowSubscription>>`.
- Sync iterators implement `__iter__` + `__next__` with `PyStopIteration` on exhaustion. Async iterators implement `__aiter__` + `__anext__` with `PyStopAsyncIteration`.
- `__anext__` uses `pyo3_async_runtimes::tokio::future_into_py` to bridge the blocking `next()` call into a Python awaitable.
- `cancel()` on `StreamSubscription` both calls `sub.cancel()` and takes the `Option` to distinguish cancelled from finished state.
- The global Tokio runtime (from `async_support::runtime()`) must be entered for `StreamSubscription` operations because `ArrowSubscription::next()` relies on a Tokio context.
- `MaterializedView.subscribe(handler=fn)` spawns a `threading.Thread(daemon=True)` that iterates the `StreamSubscription` and wraps each `QueryResult` batch into a `ChangeEvent` before calling the handler.
- All blocking operations are wrapped in `py.allow_threads()` to release the GIL.

## Testing Requirements

- Verify `Subscription.__iter__` / `__next__` yields `QueryResult` batches and raises `StopIteration` on stream end.
- Verify `AsyncSubscription.__aiter__` / `__anext__` yields batches and raises `StopAsyncIteration`.
- Verify `StreamSubscription.schema` returns a valid `pyarrow.Schema`.
- Verify `StreamSubscription.next_timeout()` returns `None` when no data arrives within the deadline.
- Verify `try_next()` returns `None` immediately when no data is available (non-blocking).
- Verify `cancel()` stops the subscription and `is_active` returns `False`.
- Verify callback mode via `MaterializedView.subscribe(handler=fn)` delivers `ChangeEvent` objects in a background thread.
- Verify subscription behavior after `cancel()` (no crash, graceful `None` / `StopIteration`).
- Verify `__repr__` returns correct state strings: `active`, `finished`, `cancelled`.
- Stress test: concurrent subscriptions on separate threads do not deadlock.

---

# F-PY-025: Python Async Integration

## Metadata
- **Phase**: Core Streaming
- **Priority**: P0
- **Dependencies**: `pyo3-async-runtimes` (0.27, `tokio-runtime`), `tokio`, `OnceLock`
- **Main Repo Module**: `async_support.rs`, `connection.rs`
- **Implementation Status**: Implemented

## Summary

Full `asyncio` integration for LaminarDB Python bindings. A global persistent Tokio multi-thread runtime is lazily initialized via `OnceLock` and entered before every Rust API call. This ensures that background tasks spawned by `laminar-db` (query stream bridges, pipeline workers) run on a shared worker pool rather than being dropped when temporary per-call runtimes exit. The `Connection` class implements both sync (`__enter__`/`__exit__`) and async (`__aenter__`/`__aexit__`) context manager protocols. Async subscription creation (`subscribe_async`, `subscribe_stream_async`) returns Python awaitables via `pyo3_async_runtimes::tokio::future_into_py`. All `#[pyclass]` types are `Send + Sync` for compatibility with free-threaded Python (PEP 703).

## Python API

```python
import laminardb

# Async context manager
async with laminardb.open(":memory:") as db:
    ...

# Async subscription creation (returns awaitable)
asub = await db.subscribe_async("SELECT * FROM source")
assub = await db.subscribe_stream_async("stream_name")

# Async iteration
async for batch in asub:
    df = batch.to_pandas()

async for batch in assub:
    table = batch.to_arrow()
```

## Usage Examples

```python
import asyncio
import laminardb

async def streaming_pipeline():
    async with laminardb.open(":memory:") as db:
        db.execute("CREATE SOURCE market_data (symbol VARCHAR, price DOUBLE, volume BIGINT, ts BIGINT)")
        db.execute("""
            CREATE STREAM ohlc AS
            SELECT symbol,
                   FIRST_VALUE(price) AS open,
                   MAX(price) AS high,
                   MIN(price) AS low,
                   LAST_VALUE(price) AS close
            FROM market_data
            GROUP BY symbol
        """)
        db.start()

        sub = await db.subscribe_stream_async("ohlc")

        # Insert data from another coroutine
        async def produce():
            for i in range(100):
                db.insert("market_data", {
                    "symbol": "AAPL",
                    "price": 150.0 + i * 0.1,
                    "volume": 1000 + i,
                    "ts": i,
                })
                await asyncio.sleep(0.01)

        async def consume():
            async for batch in sub:
                print(batch.to_pandas())

        await asyncio.gather(produce(), consume())

asyncio.run(streaming_pipeline())
```

## Implementation Notes

- The global Tokio runtime is stored in `static RUNTIME: OnceLock<tokio::runtime::Runtime>` and built with `new_multi_thread().enable_all().thread_name("laminardb-worker")`.
- Every `Connection` method enters the runtime via `let _rt = runtime().enter()` before calling any `laminar-db` API.
- `subscribe_async()` and `subscribe_stream_async()` use `pyo3_async_runtimes::tokio::future_into_py` to create the subscription inside a Tokio context and return an `AsyncSubscription` or `AsyncStreamSubscription`.
- `__aenter__` wraps `Py<Self>` in a future that immediately resolves, returning the connection itself. `__aexit__` calls `self.close(py)` then resolves `false` (do not suppress exceptions).
- The `__anext__` implementation on async subscriptions acquires the mutex lock synchronously (since `MutexGuard` is not `Send`), performs the blocking `next()` call, then wraps the result in an async future for `future_into_py`.
- Thread naming (`laminardb-worker`) aids debugging when inspecting Tokio worker threads.
- All `#[pyclass]` types use `unsafe impl Send` and `unsafe impl Sync` with `parking_lot::Mutex` or `Arc<Mutex<..>>` interior mutability to satisfy free-threaded Python requirements.

## Testing Requirements

- Verify `async with laminardb.open(...)` properly opens and closes the connection.
- Verify `await db.subscribe_async(sql)` returns an `AsyncSubscription`.
- Verify `await db.subscribe_stream_async(name)` returns an `AsyncStreamSubscription`.
- Verify `async for batch in asub` yields `QueryResult` objects and terminates with `StopAsyncIteration`.
- Verify concurrent async subscriptions on the same event loop do not block each other.
- Verify the global Tokio runtime is reused across multiple `open()` calls (singleton via `OnceLock`).
- Verify GIL is released during all blocking Rust calls (no Python thread starvation).
- Verify `__aexit__` calls `close()` and does not suppress exceptions (returns `False`).

---

# F-PY-026: Checkpoint Control

## Metadata
- **Phase**: Persistence
- **Priority**: P1
- **Dependencies**: `LaminarConfig`, `laminar_core::streaming::StreamCheckpointConfig`
- **Main Repo Module**: `laminar_db::api::Connection::checkpoint`, `laminar_db::LaminarConfig`
- **Implementation Status**: Partial

## Summary

Provides basic checkpoint triggering and configuration for WAL-based persistence. Users can enable checkpointing by providing a `LaminarConfig` with `checkpoint_interval_ms` and optionally `storage_dir`. The `Connection.checkpoint()` method triggers a manual checkpoint and returns a checkpoint ID. The `is_checkpoint_enabled` property checks whether the connection was configured with checkpointing. The current implementation does not support restoring from a specific checkpoint ID, querying detailed checkpoint status (e.g., last checkpoint time, WAL size), or automatic checkpoint scheduling beyond the interval-based trigger in the core crate.

## Python API

```python
# Configuration
config = laminardb.LaminarConfig(
    checkpoint_interval_ms=5000,     # auto-checkpoint every 5 seconds
    storage_dir="/tmp/laminar_wal",  # WAL + checkpoint directory
)
db = laminardb.open("mydb", config=config)

# Manual checkpoint trigger
checkpoint_id: int | None = db.checkpoint()

# Check if enabled
enabled: bool = db.is_checkpoint_enabled

# Python-side wrapper (types.py)
from laminardb import CheckpointStatus
status = CheckpointStatus(checkpoint_id=checkpoint_id, enabled=enabled)
```

## Usage Examples

```python
import laminardb

# Persistent database with checkpointing
config = laminardb.LaminarConfig(
    buffer_size=65536,
    storage_dir="/data/laminar",
    checkpoint_interval_ms=10000,
    table_spill_threshold=1_000_000,
)

with laminardb.open("trading_db", config=config) as db:
    db.execute("CREATE SOURCE orders (id BIGINT, symbol VARCHAR, qty INT, price DOUBLE)")
    db.start()

    # Insert data
    for i in range(10000):
        db.insert("orders", {"id": i, "symbol": "AAPL", "qty": 100, "price": 150.0 + i * 0.01})

    # Trigger a manual checkpoint
    cp_id = db.checkpoint()
    print(f"Checkpoint ID: {cp_id}")

    # Verify checkpointing is enabled
    assert db.is_checkpoint_enabled is True

    # Wrap in the Python CheckpointStatus type
    from laminardb import CheckpointStatus
    status = CheckpointStatus(checkpoint_id=cp_id, enabled=True)
    print(status)  # CheckpointStatus(checkpoint_id=42, enabled=True)
```

## Implementation Notes

- `Connection.checkpoint()` delegates to `conn.checkpoint()` on the core `laminar_db::api::Connection`, which returns `Result<u64, ApiError>`. The Python binding wraps this in `Option<u64>` via `.map(Some)`.
- `Connection.is_checkpoint_enabled` delegates to `conn.is_checkpoint_enabled()` which checks whether a `StreamCheckpointConfig` was set during builder construction.
- `PyLaminarConfig.to_core()` constructs a `StreamCheckpointConfig` from `checkpoint_interval_ms` and `storage_dir`, embedding it in the `LaminarConfig`.
- The core crate handles automatic interval-based checkpointing internally; the Python API only exposes manual trigger and configuration.
- **Not implemented**: `restore(checkpoint_id)` for point-in-time recovery, `checkpoint_status()` for WAL size / last checkpoint time, listing available checkpoints.
- The `CheckpointStatus` dataclass in `types.py` is a pure Python convenience wrapper and is not connected to any Rust-backed checkpoint introspection API.

## Testing Requirements

- Verify `checkpoint()` returns a non-None integer when checkpointing is enabled.
- Verify `is_checkpoint_enabled` returns `True` when `checkpoint_interval_ms` is configured and `False` otherwise.
- Verify `checkpoint()` raises `CheckpointError` when checkpointing is not enabled.
- Verify `LaminarConfig` correctly passes `storage_dir` and `checkpoint_interval_ms` to the core config.
- Verify data survives across checkpoint + connection reopen (when persistence is fully wired).
- Verify `CheckpointStatus` dataclass correctly wraps the checkpoint ID and enabled flag.

---

# F-PY-027: Metrics & Monitoring

## Metadata
- **Phase**: Observability
- **Priority**: P1
- **Dependencies**: `Connection`, `laminar_db::PipelineMetrics`, `laminar_db::SourceMetrics`, `laminar_db::StreamMetrics`
- **Main Repo Module**: `metrics.rs`, `laminar_db::api::Connection::{metrics, source_metrics, stream_metrics}`
- **Implementation Status**: Implemented

## Summary

Exposes pipeline-wide, per-source, and per-stream metrics for monitoring LaminarDB workloads. `Connection.metrics()` returns a `PipelineMetrics` snapshot with aggregate counters (events ingested/emitted/dropped, cycles, batches), uptime, pipeline state, component counts, and the global watermark. Per-source metrics (`source_metrics`, `all_source_metrics`) include event counts, pending queue depth, capacity, backpressure status, watermark, and utilization ratio. Per-stream metrics (`stream_metrics`, `all_stream_metrics`) include event counts, pending depth, capacity, backpressure, watermark, and the stream's SQL definition. A Python-side `Metrics` wrapper in `types.py` adds a computed `events_per_second` property. No Prometheus exposition format or sink-specific metrics are currently implemented.

## Python API

```python
# Pipeline-wide metrics
pm: PipelineMetrics = db.metrics()
pm.total_events_ingested   # int
pm.total_events_emitted    # int
pm.total_events_dropped    # int
pm.total_cycles            # int
pm.total_batches           # int
pm.uptime_secs             # float
pm.state                   # str (e.g. "Running")
pm.source_count            # int
pm.stream_count            # int
pm.sink_count              # int
pm.pipeline_watermark      # int

# Per-source metrics
sm: SourceMetrics | None = db.source_metrics("ticks")
sm.name                    # str
sm.total_events            # int
sm.pending                 # int
sm.capacity                # int
sm.is_backpressured        # bool
sm.watermark               # int
sm.utilization             # float (0.0 to 1.0)
all_sm: list[SourceMetrics] = db.all_source_metrics()

# Per-stream metrics
stm: StreamMetrics | None = db.stream_metrics("vwap")
stm.name                   # str
stm.total_events           # int
stm.pending                # int
stm.capacity               # int
stm.is_backpressured       # bool
stm.watermark              # int
stm.sql                    # str | None
all_stm: list[StreamMetrics] = db.all_stream_metrics()

# Python-side convenience wrapper
from laminardb import Metrics
m = Metrics(db.metrics())
m.events_per_second        # float (computed: total_events_ingested / uptime_secs)
m.uptime_secs              # float
m.state                    # str
```

## Usage Examples

```python
import laminardb
import time

with laminardb.open(":memory:") as db:
    db.execute("CREATE SOURCE sensors (id BIGINT, temp DOUBLE, ts BIGINT)")
    db.execute("CREATE STREAM avg_temp AS SELECT AVG(temp) AS avg FROM sensors")
    db.start()

    # Insert some data
    for i in range(1000):
        db.insert("sensors", {"id": i, "temp": 20.0 + (i % 10) * 0.5, "ts": i})

    time.sleep(0.5)  # let pipeline process

    # Pipeline-wide metrics
    pm = db.metrics()
    print(f"State: {pm.state}")
    print(f"Ingested: {pm.total_events_ingested}, Emitted: {pm.total_events_emitted}")
    print(f"Uptime: {pm.uptime_secs:.2f}s")
    print(f"Sources: {pm.source_count}, Streams: {pm.stream_count}, Sinks: {pm.sink_count}")

    # Per-source metrics
    sm = db.source_metrics("sensors")
    if sm:
        print(f"Source '{sm.name}': {sm.total_events} events, {sm.utilization:.1%} utilized")
        if sm.is_backpressured:
            print("  WARNING: source is backpressured!")

    # All source metrics at once
    for s in db.all_source_metrics():
        print(f"  {s.name}: pending={s.pending}/{s.capacity}")

    # Per-stream metrics
    stm = db.stream_metrics("avg_temp")
    if stm:
        print(f"Stream '{stm.name}': {stm.total_events} events, SQL: {stm.sql}")

    # Python convenience wrapper
    from laminardb import Metrics
    m = Metrics(pm)
    print(f"Throughput: {m.events_per_second:.0f} events/sec")
```

## Implementation Notes

- All metrics classes (`PyPipelineMetrics`, `PySourceMetrics`, `PyStreamMetrics`) are `#[pyclass(frozen)]` -- immutable snapshots of the state at call time.
- `PyPipelineMetrics` wraps `laminar_db::PipelineMetrics` directly and exposes `uptime` as `uptime_secs` (converting `Duration` to `f64`).
- `PyPipelineMetrics.state` is formatted via `format!("{:?}", ...)` on the core `PipelineState` enum, producing strings like `"Running"`, `"Created"`, `"Stopped"`.
- `source_metrics(name)` returns `Option<PySourceMetrics>` -- `None` if the source is not found (no exception).
- The `Metrics` Python wrapper in `types.py` computes `events_per_second` as `total_events_ingested / uptime_secs` with a zero-guard for division.
- Connection-level convenience properties (`pipeline_state`, `pipeline_watermark`, `total_events_processed`, `source_count`, `sink_count`, `active_query_count`) provide quick access without constructing a full metrics snapshot.
- **Not implemented**: Prometheus text exposition format, histogram/percentile latency metrics, sink-specific metrics (only `SinkInfo.name` is available), or time-series metrics history.

## Testing Requirements

- Verify `metrics()` returns a `PipelineMetrics` with non-negative counters after data ingestion.
- Verify `source_metrics(name)` returns correct metrics for an existing source and `None` for a non-existent one.
- Verify `stream_metrics(name)` returns correct metrics including the SQL definition.
- Verify `all_source_metrics()` and `all_stream_metrics()` return lists with one entry per registered source/stream.
- Verify `Metrics.events_per_second` computes correctly (non-zero after ingestion, zero when uptime is zero).
- Verify `pipeline_state` returns the expected state string at each lifecycle phase.
- Verify `total_events_processed` increases monotonically with inserts.
- Verify metrics are consistent (e.g., `source_count` matches `len(all_source_metrics())`).

---

# F-PY-028: Pipeline Management

## Metadata
- **Phase**: Core Streaming
- **Priority**: P0
- **Dependencies**: `Connection`, `PipelineTopology`, `PipelineNode`, `PipelineEdge`
- **Main Repo Module**: `connection.rs`, `metrics.rs`, `laminar_db::api::Connection`
- **Implementation Status**: Partial

## Summary

Controls the lifecycle of the streaming pipeline embedded in each `Connection`. `start()` begins processing, `shutdown()` performs graceful shutdown with in-flight event drainage, and `close()` terminates the connection. Pipeline introspection is available through `pipeline_state` (lifecycle state string), `pipeline_watermark` (global watermark), `total_events_processed` (aggregate counter), and `topology()` (DAG of nodes and edges). Query management includes `queries()` for listing active/completed queries and `cancel_query(query_id)` for cancellation. The implementation is limited to a single unnamed pipeline per connection with no support for named pipelines, DAG visualization (Mermaid/DOT), or pipeline pause/resume.

## Python API

```python
# Lifecycle
db.start()                              # start streaming pipeline
db.shutdown()                           # graceful shutdown with event drainage
db.close()                              # terminate connection

# State inspection
db.pipeline_state       # str property: "Created" | "Running" | "Stopped"
db.pipeline_watermark   # int property: global watermark
db.total_events_processed  # int property
db.source_count         # int property
db.sink_count           # int property
db.active_query_count   # int property

# Topology
topo: PipelineTopology = db.topology()
topo.nodes              # list[PipelineNode]
topo.edges              # list[PipelineEdge]

node: PipelineNode
node.name               # str
node.node_type          # str: "source" | "stream" | "sink"
node.schema             # pyarrow.Schema | None
node.sql                # str | None

edge: PipelineEdge
edge.from_node          # str
edge.to_node            # str

# Query management
infos: list[QueryInfo] = db.queries()
info.id                 # int
info.sql                # str
info.active             # bool
db.cancel_query(query_id)
```

## Usage Examples

```python
import laminardb

with laminardb.open(":memory:") as db:
    # Define the pipeline
    db.execute("CREATE SOURCE trades (symbol VARCHAR, price DOUBLE, volume BIGINT, ts BIGINT)")
    db.execute("CREATE STREAM vwap AS SELECT symbol, SUM(price * volume) / SUM(volume) AS vwap_price FROM trades GROUP BY symbol")

    # Inspect before start
    assert db.pipeline_state == "Created"
    topo = db.topology()
    print(f"Pipeline has {len(topo.nodes)} nodes and {len(topo.edges)} edges")
    for node in topo.nodes:
        print(f"  {node.node_type}: {node.name}")
        if node.schema:
            print(f"    Schema: {node.schema}")
        if node.sql:
            print(f"    SQL: {node.sql}")
    for edge in topo.edges:
        print(f"  {edge.from_node} -> {edge.to_node}")

    # Start the pipeline
    db.start()
    assert db.pipeline_state == "Running"

    # Insert data
    db.insert("trades", [
        {"symbol": "AAPL", "price": 150.0, "volume": 1000, "ts": 1},
        {"symbol": "GOOG", "price": 2800.0, "volume": 500, "ts": 2},
    ])

    # Check counters
    print(f"Events processed: {db.total_events_processed}")
    print(f"Watermark: {db.pipeline_watermark}")
    print(f"Sources: {db.source_count}, Sinks: {db.sink_count}")
    print(f"Active queries: {db.active_query_count}")

    # Query management
    for q in db.queries():
        print(f"Query {q.id}: active={q.active}, sql={q.sql}")
        if q.active:
            db.cancel_query(q.id)

    # Graceful shutdown
    db.shutdown()
    assert db.pipeline_state == "Stopped"
```

## Implementation Notes

- `start()` delegates to `conn.start()` which begins the pipeline's processing loop on the Tokio runtime.
- `shutdown()` delegates to `conn.shutdown()` which signals the pipeline to stop and waits for in-flight events to drain before returning. This differs from `close()` which drops the connection immediately.
- `pipeline_state` calls `conn.pipeline_state()` which returns a `String` representation of the internal `PipelineState` enum.
- `topology()` returns a `PyPipelineTopology` containing `Vec<PyPipelineNode>` and `Vec<PyPipelineEdge>`. Node types are mapped from `PipelineNodeType::{Source, Stream, Sink}` to lowercase strings. Nodes may carry a `SchemaRef` (exposed as `pyarrow.Schema`) and an optional SQL definition.
- `cancel_query(query_id)` delegates to `conn.cancel_query(query_id)` which returns an `ApiError` if the query ID is not found.
- All topology and metrics classes use `#[pyclass(frozen)]` for thread safety and immutability.
- **Not implemented**: Named pipelines (multiple pipelines per connection), pipeline pause/resume, DAG visualization export (Mermaid, DOT, Graphviz), pipeline versioning.

## Testing Requirements

- Verify `pipeline_state` transitions: `"Created"` -> `"Running"` after `start()` -> `"Stopped"` after `shutdown()`.
- Verify `topology()` returns correct nodes and edges matching the defined sources and streams.
- Verify `PipelineNode.node_type` is one of `"source"`, `"stream"`, `"sink"`.
- Verify `PipelineNode.schema` returns a valid `pyarrow.Schema` for sources.
- Verify `PipelineNode.sql` returns the SQL definition for stream nodes and `None` for sources.
- Verify `cancel_query()` successfully cancels an active query.
- Verify `cancel_query()` raises an error for a non-existent query ID.
- Verify `shutdown()` waits for in-flight events before returning.
- Verify calling `start()` on a closed connection raises `ConnectionError`.
- Verify `total_events_processed` and `pipeline_watermark` update after inserts.

---

# F-PY-029: Configuration & Tuning

## Metadata
- **Phase**: Configuration
- **Priority**: P1
- **Dependencies**: `laminar_db::LaminarConfig`, `laminar_core::streaming::StreamCheckpointConfig`
- **Main Repo Module**: `config.rs`
- **Implementation Status**: Partial

## Summary

Provides the `LaminarConfig` class for configuring LaminarDB connections at creation time. Four parameters are supported: `buffer_size` (default ring buffer capacity per source), `storage_dir` (WAL and checkpoint directory path), `checkpoint_interval_ms` (automatic checkpoint frequency), and `table_spill_threshold` (row count before spilling to disk). All properties are read-only after construction. Configuration is passed to `laminardb.open()` via the `config` keyword argument. No runtime configuration changes, per-ring tuning, thread affinity, or memory limit settings are currently supported.

## Python API

```python
import laminardb

# Construction with keyword-only arguments and defaults
config = laminardb.LaminarConfig(
    buffer_size=65536,               # default: 65536
    storage_dir="/data/laminar",     # default: None
    checkpoint_interval_ms=5000,     # default: None (disabled)
    table_spill_threshold=1_000_000, # default: 1_000_000
)

# Read-only property access
config.buffer_size            # int
config.storage_dir            # str | None
config.checkpoint_interval_ms # int | None
config.table_spill_threshold  # int

# Pass to open()
db = laminardb.open("mydb", config=config)

# Alias
Config = laminardb.Config  # alias for LaminarConfig
```

## Usage Examples

```python
import laminardb

# Default configuration (all defaults)
db_default = laminardb.open(":memory:")

# High-throughput configuration
config = laminardb.LaminarConfig(
    buffer_size=1_048_576,           # 1M slots per ring buffer
    table_spill_threshold=10_000_000,# spill after 10M rows
)
db_fast = laminardb.open("fast_db", config=config)

# Persistent configuration with checkpointing
config = laminardb.LaminarConfig(
    storage_dir="/var/lib/laminar/prod",
    checkpoint_interval_ms=30000,    # checkpoint every 30 seconds
)
db_persistent = laminardb.open("prod_db", config=config)

# Inspect configuration
print(config)
# LaminarConfig(buffer_size=65536, storage_dir="/var/lib/laminar/prod",
#               checkpoint_interval_ms=30000, table_spill_threshold=1000000)

# Use the alias
cfg = laminardb.Config(buffer_size=2048)
```

## Implementation Notes

- `PyLaminarConfig` is a `#[pyclass]` with `#[new]` accepting keyword-only arguments via `#[pyo3(signature = (*, buffer_size=65536, storage_dir=None, checkpoint_interval_ms=None, table_spill_threshold=1_000_000))]`.
- `to_core()` converts to `laminar_db::LaminarConfig`, constructing a `StreamCheckpointConfig` when `checkpoint_interval_ms` is set. The `storage_dir` is mapped to both `LaminarConfig.storage_dir` and `StreamCheckpointConfig.data_dir`.
- The `Config` alias is defined in `__init__.py` as `Config = LaminarConfig`.
- `PyLaminarConfig` derives `Clone` so it can be reused across multiple `open()` calls.
- All properties are read-only `#[getter]` methods with no corresponding setters.
- **Not implemented**: Runtime config changes (`config_set(key, value)`), per-source ring buffer sizing, thread affinity / CPU pinning, memory limits, batch size tuning, WAL compaction settings, max concurrent queries.
- The core `LaminarConfig` struct uses `..LaminarConfig::default()` for any fields not set by the Python config, which means the Python API surface is a subset of what the core supports.

## Testing Requirements

- Verify `LaminarConfig()` with no arguments uses correct defaults (`buffer_size=65536`, `storage_dir=None`, `checkpoint_interval_ms=None`, `table_spill_threshold=1_000_000`).
- Verify all four properties are readable after construction.
- Verify `laminardb.open("db", config=config)` succeeds with a custom config.
- Verify `__repr__` displays all four parameters correctly.
- Verify `Config` alias resolves to `LaminarConfig`.
- Verify `to_core()` correctly maps `checkpoint_interval_ms` to `StreamCheckpointConfig.interval_ms`.
- Verify `to_core()` correctly maps `storage_dir` to both `LaminarConfig.storage_dir` and `StreamCheckpointConfig.data_dir`.
- Verify `LaminarConfig` is cloneable and can be reused across multiple connections.

---

# F-PY-030: Ad-Hoc Queries (kdb+ Style)

## Metadata
- **Phase**: Query
- **Priority**: P0
- **Dependencies**: `Connection`, `QueryResult`, `ExecuteResult`, `laminar_db::api::Connection::execute`
- **Main Repo Module**: `connection.rs`, `execute.rs`, `query.rs`
- **Implementation Status**: Partial

## Summary

Supports point-in-time SQL queries against the current state of sources and streams. `Connection.query(sql)` executes a SQL statement via `Connection::execute()` internally, consumes the full `QueryStream` using blocking `next()` calls, and returns a complete `QueryResult`. `Connection.execute(sql)` returns a richer `ExecuteResult` that distinguishes DDL, DML, query, and metadata results. Stream queries are supported via `query_stream(name, filter=None)` which looks up the stream's SQL definition from the catalog and re-executes it, optionally applying a WHERE clause. DuckDB-style aliases (`sql()`, `explain()`, `fetchall()`, `fetchone()`, `fetchmany()`) provide familiar ergonomics. No time-travel queries (`query_at(sql, timestamp)`) or historical replay are implemented.

## Python API

```python
# Point-in-time query
result: QueryResult = db.query(sql)
result: QueryResult = db.sql(query, params=None)  # DuckDB alias

# Raw execute (DDL + DML + query)
er: ExecuteResult = db.execute(sql)
er.result_type      # str: "ddl" | "rows_affected" | "query" | "metadata"
er.rows_affected    # int
er.ddl_type         # str | None (e.g. "CREATE SOURCE")
er.ddl_object       # str | None (e.g. "sensors")
er.to_query_result() # QueryResult | None
int(er)             # rows affected (backward compat)
bool(er)            # True if rows affected or DDL/query succeeded

# Stream-specific query
result: QueryResult = db.query_stream(name, filter=None)
result: QueryResult = db.query_stream("vwap", filter="symbol = 'AAPL'")

# Execution plan
plan: str = db.explain(query)

# Cursor-style fetching
rows: list[tuple] = result.fetchall()
row: tuple | None = result.fetchone()
rows: list[tuple] = result.fetchmany(size=10)

# Preview
result.show(max_rows=20)
```

## Usage Examples

```python
import laminardb

with laminardb.open(":memory:") as db:
    # Create and populate
    db.execute("CREATE SOURCE orders (id BIGINT, symbol VARCHAR, qty INT, price DOUBLE)")
    db.execute("CREATE STREAM portfolio AS SELECT symbol, SUM(qty) AS total_qty, AVG(price) AS avg_price FROM orders GROUP BY symbol")
    db.start()

    db.insert("orders", [
        {"id": 1, "symbol": "AAPL", "qty": 100, "price": 150.0},
        {"id": 2, "symbol": "GOOG", "qty": 50,  "price": 2800.0},
        {"id": 3, "symbol": "AAPL", "qty": 200, "price": 152.0},
    ])

    # Ad-hoc query on source data
    result = db.query("SELECT symbol, SUM(qty) AS total FROM orders GROUP BY symbol")
    print(result.to_pandas())
    #   symbol  total
    # 0   AAPL    300
    # 1   GOOG     50

    # Query a named stream (re-executes its SQL definition)
    portfolio = db.query_stream("portfolio")
    print(portfolio.to_dicts())

    # Filtered stream query
    aapl_only = db.query_stream("portfolio", filter="symbol = 'AAPL'")
    print(aapl_only.to_pandas())

    # ExecuteResult for DDL
    er = db.execute("CREATE SOURCE new_source (x INT)")
    assert er.result_type == "ddl"
    assert er.ddl_type == "CREATE SOURCE"
    assert er.ddl_object == "new_source"

    # Cursor-style
    result = db.query("SELECT * FROM orders")
    first = result.fetchone()   # single tuple
    top5 = result.fetchmany(5)  # list of tuples
    all_rows = result.fetchall() # all as tuples

    # Explain plan
    plan = db.explain("SELECT * FROM orders WHERE symbol = 'AAPL'")
    print(plan)

    # DuckDB-style alias
    result = db.sql("SELECT COUNT(*) FROM orders")
```

## Implementation Notes

- `query()` calls `conn.execute(sql)` rather than `conn.query()` because the core API's `query()` uses non-blocking `try_next()` internally, which can return before data arrives from the background DataFusion bridge. By using `execute()` and consuming the stream with blocking `next()`, all data is reliably captured.
- `query_stream(name, filter)` looks up the stream's SQL from `conn.stream_info()`, then wraps it in `SELECT * FROM ({sql}) AS {name} WHERE {filter}` if a filter is provided.
- `ExecuteResult.from_core()` handles four variants: `Ddl`, `RowsAffected`, `Query` (fully consumes stream), and `Metadata` (wraps single batch).
- `QueryResult` supports both Arrow-native exports (`to_arrow`, `to_polars`, `to_pandas`, `to_dicts`, `to_df`, `__arrow_c_stream__`) and cursor-style access (`fetchall`, `fetchone`, `fetchmany`). Cursor-style methods convert to tuples via `to_pylist()` for compatibility with DB-API patterns.
- `explain()` prefixes the query with `EXPLAIN` and returns the text representation.
- `show(max_rows)` slices the PyArrow table and prints via `pandas.DataFrame.to_string()`.
- **Not implemented**: Time-travel queries (`query_at(sql, timestamp)`), historical replay (`replay(source, start, end)`), parameterized queries (`params` argument is reserved but unused), window-based snapshot queries.

## Testing Requirements

- Verify `query()` returns a `QueryResult` with correct row count and data for `SELECT` statements.
- Verify `execute()` returns correct `result_type` for DDL, DML, and query statements.
- Verify `query_stream()` returns data for a named stream.
- Verify `query_stream(name, filter="...")` applies the WHERE clause correctly.
- Verify `query_stream()` raises `QueryError` for a non-existent stream name.
- Verify `fetchall()` returns a list of tuples.
- Verify `fetchone()` returns a single tuple or `None` for empty results.
- Verify `fetchmany(n)` returns at most `n` rows.
- Verify `explain()` returns a non-empty string plan.
- Verify `ExecuteResult.__int__()` and `__bool__()` work correctly for backward compatibility.
- Verify `sql()` alias delegates to `query()`.

---

# F-PY-031: User-Defined Functions (UDFs)

## Metadata
- **Phase**: Extensibility
- **Priority**: P2
- **Dependencies**: `laminar_db::LaminarDbBuilder`, `datafusion::logical_expr::{ScalarUDF, AggregateUDF}`, `arrow-rs`
- **Main Repo Module**: `laminar_db::LaminarDbBuilder::register_custom_udf`, `laminar_db::LaminarDbBuilder::register_custom_udaf`
- **Implementation Status**: Not Started

## Summary

Enables users to register custom scalar and aggregate functions written in Python for use within LaminarDB's SQL engine. The main `laminar-db` crate has `register_custom_udf(ScalarUDF)` and `register_custom_udaf(AggregateUDF)` methods on `LaminarDbBuilder`, but these are `pub(crate)` and only callable during database construction. A Python UDF bridge would need to: (1) accept a Python callable with type annotations, (2) wrap it as a DataFusion `ScalarUDF` that marshals Arrow arrays to/from Python on each batch evaluation, (3) register it before the connection's `LaminarDB` instance is built. This feature requires significant design work around GIL management (the UDF would need to acquire the GIL during evaluation from a Tokio worker thread), error propagation, and performance (Python function call overhead per batch).

## Python API

```python
import pyarrow as pa
import laminardb

# Decorator-based registration
@laminardb.udf(return_type=pa.float64())
def vwap(prices: pa.Array, volumes: pa.Array) -> pa.Array:
    """Volume-weighted average price."""
    return pa.compute.divide(
        pa.compute.sum(pa.compute.multiply(prices, volumes)),
        pa.compute.sum(volumes),
    )

# Manual registration on a connection
db.register_udf("vwap", vwap)

# Use in SQL
result = db.query("SELECT symbol, vwap(price, volume) FROM trades GROUP BY symbol")

# Aggregate UDF
@laminardb.udaf(
    return_type=pa.float64(),
    accumulator_type=pa.struct([("sum", pa.float64()), ("count", pa.int64())]),
)
class ExponentialMovingAverage:
    def __init__(self, alpha: float = 0.1):
        self.alpha = alpha

    def accumulate(self, values: pa.Array) -> None: ...
    def merge(self, other: "ExponentialMovingAverage") -> None: ...
    def evaluate(self) -> pa.Scalar: ...

db.register_udaf("ema", ExponentialMovingAverage)
result = db.query("SELECT symbol, ema(price) FROM trades GROUP BY symbol")
```

## Usage Examples

```python
import pyarrow as pa
import pyarrow.compute as pc
import laminardb

# Scalar UDF: z-score normalization
@laminardb.udf(return_type=pa.float64())
def zscore(values: pa.Array, mean: pa.Scalar, stddev: pa.Scalar) -> pa.Array:
    return pc.divide(pc.subtract(values, mean), stddev)

with laminardb.open(":memory:") as db:
    db.register_udf("zscore", zscore)
    db.execute("CREATE SOURCE readings (sensor_id INT, value DOUBLE)")
    db.start()

    db.insert("readings", [
        {"sensor_id": 1, "value": 10.0},
        {"sensor_id": 2, "value": 20.0},
        {"sensor_id": 3, "value": 15.0},
    ])

    result = db.query("""
        SELECT sensor_id,
               zscore(value, 15.0, 5.0) AS z
        FROM readings
    """)
    print(result.to_pandas())
```

## Implementation Notes

- The main crate's `register_custom_udf` and `register_custom_udaf` are `pub(crate)`, meaning the Python binding cannot call them directly on an existing `Connection`. The registration must happen during builder construction, which means the Python API would need to either: (a) accept UDFs before `open()` and pass them to a builder pattern, or (b) the core crate would need to expose `pub` registration methods on `Connection`.
- The Python-to-Rust UDF bridge would wrap the Python callable in a `ScalarUDF` that: acquires the GIL on each batch evaluation, converts Arrow arrays to `pyarrow` objects, calls the Python function, converts the result back to Rust Arrow arrays.
- GIL acquisition from Tokio worker threads is the primary design challenge. Since pipeline evaluation happens on Tokio threads (not the Python main thread), the UDF must call `Python::with_gil()` which can deadlock if the GIL is held by the main thread waiting on the same Tokio runtime.
- Performance concern: Python function call overhead (GIL acquisition + PyO3 type conversion) per batch could be significant for high-throughput pipelines. Vectorized (batch-at-a-time) evaluation mitigates this compared to row-at-a-time.
- Error propagation: Python exceptions raised in UDFs need to be caught and converted to `DataFusionError` for proper query error handling.
- Type safety: Input/output Arrow types must be declared upfront so DataFusion can plan and validate the query.

## Testing Requirements

- Verify scalar UDF registration and invocation in a `SELECT` query.
- Verify aggregate UDF registration with accumulate/merge/evaluate lifecycle.
- Verify type mismatch between declared return type and actual return raises an error.
- Verify Python exceptions in UDFs are caught and surfaced as `QueryError`.
- Verify UDFs work correctly within streaming queries (not just ad-hoc).
- Verify GIL handling does not deadlock when UDF is called from a Tokio worker thread.
- Verify performance: measure overhead of Python UDF vs. equivalent built-in function.
- Verify UDF with multiple arguments of different types.

---

# F-PY-032: Jupyter/Notebook Integration

## Metadata
- **Phase**: Developer Experience
- **Priority**: P2
- **Dependencies**: `QueryResult`, `pandas`, `pyarrow`
- **Main Repo Module**: `query.rs` (`_repr_html_`, `show`)
- **Implementation Status**: Partial

## Summary

Provides rich display integration for Jupyter notebooks and terminal-based previews. `QueryResult._repr_html_()` generates an HTML table representation by delegating to `pandas.DataFrame._repr_html_()`, enabling automatic rich rendering in Jupyter cells. `QueryResult.show(max_rows=20)` prints a terminal-friendly preview by slicing the result to `max_rows` and printing via `pandas.DataFrame.to_string()`. The current implementation covers basic HTML table rendering but does not include interactive widgets, live-updating displays for streaming queries, progress bars, cell magic commands (`%laminardb`), or syntax-highlighted SQL output.

## Python API

```python
# Jupyter rich display (called automatically by Jupyter)
result._repr_html_()   # returns HTML string

# Terminal preview
result.show()           # prints first 20 rows
result.show(max_rows=50) # prints first 50 rows

# Standard __repr__ (always available)
repr(result)            # "QueryResult(rows=100, columns=5)"
len(result)             # 100 (enables len() in notebooks)

# Iteration (enables display in notebook loops)
for row in result:
    print(row)  # tuple per row
```

## Usage Examples

```python
import laminardb

db = laminardb.open(":memory:")
db.execute("CREATE SOURCE trades (symbol VARCHAR, price DOUBLE, volume BIGINT, ts BIGINT)")
db.start()

db.insert("trades", [
    {"symbol": "AAPL", "price": 150.0, "volume": 1000, "ts": 1},
    {"symbol": "GOOG", "price": 2800.0, "volume": 500, "ts": 2},
    {"symbol": "MSFT", "price": 300.0, "volume": 750, "ts": 3},
])

# In a Jupyter cell, just evaluate the result:
result = db.query("SELECT * FROM trades")
result  # Jupyter automatically calls _repr_html_() and renders a table

# Terminal preview
result.show()
#   symbol   price  volume  ts
# 0   AAPL   150.0    1000   1
# 1   GOOG  2800.0     500   2
# 2   MSFT   300.0     750   3

# Large result preview
large_result = db.query("SELECT * FROM trades")  # imagine 10000 rows
large_result.show(max_rows=5)  # only first 5 rows printed

# Metric display in notebooks
from laminardb import Metrics
m = Metrics(db.metrics())
print(m)  # Metrics(state='Running', eps=5000.0, uptime=12.3s)

# Pipeline topology display
topo = db.topology()
print(topo)  # PipelineTopology(nodes=3, edges=2)
for node in topo.nodes:
    print(node)  # PipelineNode(name='trades', type='source')
```

```python
# Fallback when pandas is not installed
result = db.query("SELECT 1 AS x")
html = result._repr_html_()
# Returns "<pre>QueryResult(rows=1, columns=1)</pre>" if pandas import fails
```

## Implementation Notes

- `_repr_html_()` first attempts `self.to_pandas()` then calls `df._repr_html_()`. If pandas is not installed or conversion fails, it falls back to `<pre>{self.__repr__()}</pre>`.
- `show(max_rows)` converts to PyArrow table, slices with `table.slice(0, max_rows)`, converts to pandas, and prints via `df.to_string()`. The `print()` is done via `builtins.print()` from Python.
- `__len__` returns `num_rows` enabling `len(result)` in notebook cells.
- `__iter__` returns an iterator over rows as tuples (via `fetchall().__iter__()`), enabling `for row in result:` loops.
- `__repr__` returns `QueryResult(rows=N, columns=M)` for all contexts.
- All `__repr__` methods on metrics, topology, and catalog classes provide informative string representations suitable for notebook display.
- **Not implemented**: `_repr_mimebundle_()` for richer MIME type negotiation, interactive HTML tables (sortable/filterable), live-updating widgets for streaming subscriptions, `IPython.display` integration for progress bars, `%laminardb` / `%%laminardb` cell magic commands, syntax-highlighted SQL rendering, `_repr_latex_()` for LaTeX table export.

## Testing Requirements

- Verify `_repr_html_()` returns a non-empty HTML string containing `<table` when pandas is available.
- Verify `_repr_html_()` falls back to `<pre>...</pre>` when pandas conversion fails.
- Verify `show()` prints output to stdout (capture with `capsys` or `io.StringIO`).
- Verify `show(max_rows=5)` limits output to at most 5 rows.
- Verify `__repr__` returns the expected format string.
- Verify `__len__` returns correct row count.
- Verify `__iter__` yields tuples matching the row data.
- Verify `PipelineMetrics.__repr__`, `SourceMetrics.__repr__`, `StreamMetrics.__repr__`, `PipelineNode.__repr__`, `PipelineEdge.__repr__`, and `PipelineTopology.__repr__` all produce informative strings.
- Verify that notebook rendering does not crash when the result set is empty (0 rows).
