# laminardb

[![PyPI](https://img.shields.io/pypi/v/laminardb)](https://pypi.org/project/laminardb/)
[![CI](https://github.com/laminardb/laminardb-python/actions/workflows/ci.yml/badge.svg)](https://github.com/laminardb/laminardb-python/actions/workflows/ci.yml)
[![Python 3.11+](https://img.shields.io/pypi/pyversions/laminardb)](https://pypi.org/project/laminardb/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Python bindings for the [LaminarDB](https://github.com/laminardb/laminardb) streaming SQL database. Built with [PyO3](https://pyo3.rs) and [Apache Arrow](https://arrow.apache.org/) for zero-copy performance.

```python
import laminardb

conn = laminardb.open(":memory:")
conn.execute("CREATE SOURCE sensors (ts BIGINT, device VARCHAR, value DOUBLE)")
conn.insert("sensors", [
    {"ts": 1, "device": "sensor_a", "value": 42.0},
    {"ts": 2, "device": "sensor_b", "value": 43.5},
    {"ts": 3, "device": "sensor_a", "value": 44.1},
])

conn.sql("SELECT * FROM sensors WHERE value > 43.0").df()
#    ts    device  value
# 0   2  sensor_b   43.5
# 1   3  sensor_a   44.1
```

## Why laminardb?

- **DuckDB-style API** &mdash; `.sql()`, `.df()`, `.arrow()`, `.fetchall()`, `.show()` — feels like DuckDB
- **Streaming-first** &mdash; continuous query subscriptions, materialized views, change events
- **10 input formats** &mdash; dicts, pandas, polars, pyarrow, JSON, CSV, Arrow PyCapsule
- **6 output formats** &mdash; pandas, polars, pyarrow, dicts, auto-detect, PyCapsule (zero-copy)
- **Full type safety** &mdash; PEP 561 stubs with `py.typed`, passes `mypy --strict`
- **Thread-safe** &mdash; GIL release on all blocking ops, `Send + Sync` for free-threaded Python 3.13t+
- **Pipeline observability** &mdash; topology, metrics, watermarks, catalog introspection

## Installation

```bash
pip install laminardb
```

With optional DataFrame libraries:

```bash
pip install laminardb[pandas]    # pandas + pyarrow
pip install laminardb[polars]    # polars
pip install laminardb[pyarrow]   # pyarrow only
pip install laminardb[dev]       # all dev dependencies
```

Requires **Python 3.11+** and a supported platform (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64).

---

## Quick Start

### Connect and query

```python
import laminardb

# Open a connection
conn = laminardb.open(":memory:")

# Create a source and insert data
conn.execute("CREATE SOURCE sensors (ts BIGINT, device VARCHAR, value DOUBLE)")
conn.insert("sensors", [
    {"ts": 1, "device": "sensor_a", "value": 42.0},
    {"ts": 2, "device": "sensor_b", "value": 43.5},
    {"ts": 3, "device": "sensor_a", "value": 44.1},
])

# Query with DuckDB-style .sql()
result = conn.sql("SELECT * FROM sensors WHERE value > 43.0")

result.show()          # print to terminal
df = result.df()       # -> pandas DataFrame
pl_df = result.pl()    # -> polars DataFrame
table = result.arrow() # -> pyarrow Table
rows = result.fetchall()  # -> list of tuples

conn.close()
```

### Module-level queries

```python
import laminardb

# Uses a thread-safe default in-memory connection
result = laminardb.sql("SELECT 1 + 1 AS answer")
result.show()
```

### Context manager

```python
with laminardb.open(":memory:") as conn:
    conn.execute("CREATE SOURCE events (id BIGINT, name VARCHAR)")
    conn.insert("events", [{"id": 1, "name": "click"}, {"id": 2, "name": "view"}])
    conn.sql("SELECT * FROM events").show()
# auto-closes on exit
```

---

## Connections

```python
# In-memory database
conn = laminardb.open(":memory:")

# Path-based (reserved for future persistence)
conn = laminardb.open("mydb")

# URI-based (reserved for future remote connections)
conn = laminardb.connect("laminar://localhost:5432/mydb")

# With configuration
config = laminardb.LaminarConfig(
    buffer_size=131072,
    checkpoint_interval_ms=5000,
    table_spill_threshold=2_000_000,
)
conn = laminardb.open(":memory:", config=config)
```

### Connection properties

```python
conn.is_closed              # bool
conn.pipeline_state         # "running", "stopped", etc.
conn.pipeline_watermark     # global watermark (int64)
conn.total_events_processed # total events across all sources
conn.source_count           # number of sources
conn.sink_count             # number of sinks
conn.active_query_count     # number of active queries
conn.is_checkpoint_enabled  # whether checkpointing is enabled
```

---

## Creating Sources

Sources define the schema for data ingestion.

```python
# Dict schema (column_name -> Arrow type string)
conn.create_table("events", {
    "ts": "int64",
    "kind": "string",
    "payload": "float64",
})

# PyArrow schema
import pyarrow as pa
conn.create_table("events", pa.schema([
    ("ts", pa.int64()),
    ("kind", pa.string()),
    ("payload", pa.float64()),
]))

# Via SQL
conn.execute("CREATE SOURCE events (ts BIGINT, kind VARCHAR, payload DOUBLE)")
```

---

## Data Ingestion

LaminarDB accepts **10 input formats**. All `insert()` calls return the number of rows inserted.

```python
# Single dict (one row)
conn.insert("sensors", {"ts": 1, "device": "a", "value": 42.0})

# List of dicts (row-oriented)
conn.insert("sensors", [
    {"ts": 1, "device": "a", "value": 42.0},
    {"ts": 2, "device": "b", "value": 43.5},
])

# Columnar dict
conn.insert("sensors", {
    "ts": [1, 2, 3],
    "device": ["a", "b", "c"],
    "value": [42.0, 43.5, 44.1],
})

# pandas DataFrame
conn.insert("sensors", pd.DataFrame({"ts": [1, 2], "device": ["a", "b"], "value": [1.0, 2.0]}))

# polars DataFrame
conn.insert("sensors", pl.DataFrame({"ts": [1, 2], "device": ["a", "b"], "value": [1.0, 2.0]}))

# pyarrow RecordBatch or Table
conn.insert("sensors", pa.record_batch({"ts": [1], "device": ["a"], "value": [1.0]}))
conn.insert("sensors", pa.table({"ts": [1], "device": ["a"], "value": [1.0]}))

# JSON string
conn.insert_json("sensors", '[{"ts": 1, "device": "a", "value": 1.0}]')

# CSV string
conn.insert_csv("sensors", "ts,device,value\n1,a,1.0\n2,b,2.0\n")

# Any object with __arrow_c_stream__ (zero-copy)
conn.insert("sensors", arrow_capsule_object)
```

### Streaming writer

For high-throughput ingestion with watermark support:

```python
with conn.writer("sensors") as w:
    w.insert({"ts": 1, "device": "a", "value": 1.0})
    w.insert({"ts": 2, "device": "b", "value": 2.0})
    w.watermark(2)  # emit watermark
    w.flush()       # explicit flush

    print(w.name)              # "sensors"
    print(w.schema)            # pyarrow.Schema
    print(w.current_watermark) # 2
# auto-flush on context exit
```

---

## Querying

### SQL queries

```python
# Standard query — collects all results
result = conn.query("SELECT * FROM sensors WHERE value > 43.0")

# DuckDB-style alias
result = conn.sql("SELECT * FROM sensors WHERE value > 43.0")

# DDL / DML execution
exec_result = conn.execute("CREATE SOURCE metrics (ts BIGINT, value DOUBLE)")
print(exec_result.result_type)  # "ddl"
print(exec_result.ddl_type)     # "CREATE SOURCE"
print(exec_result.ddl_object)   # "metrics"
print(int(exec_result))         # rows affected
```

### QueryResult

Every query returns a `QueryResult` with multiple export options:

```python
result = conn.sql("SELECT * FROM sensors")

# Properties
result.num_rows       # total row count
result.num_columns    # column count
result.num_batches    # Arrow batch count
result.columns        # list of column names
result.schema         # pyarrow.Schema
len(result)           # same as num_rows
```

#### Export to DataFrames

```python
result.to_pandas()    # -> pandas.DataFrame
result.to_polars()    # -> polars.DataFrame
result.to_arrow()     # -> pyarrow.Table
result.to_dicts()     # -> dict[str, list]  (columnar)
result.to_df()        # -> auto-detect best library (polars > pandas > pyarrow)
```

#### DuckDB-style methods

```python
result.df()                # -> pandas.DataFrame
result.pl()                # -> polars.DataFrame
result.pl(lazy=True)       # -> polars.LazyFrame
result.arrow()             # -> pyarrow.Table
result.fetchall()          # -> list[tuple]
result.fetchone()          # -> tuple | None
result.fetchmany(10)       # -> list[tuple] (up to 10 rows)
result.show()              # print preview (default 20 rows)
result.show(max_rows=50)   # print more rows
result._repr_html_()       # Jupyter HTML rendering
```

#### Iteration

```python
# Iterate over rows as tuples
for row in result:
    ts, device, value = row
    print(f"{device}: {value}")

# Zero-copy Arrow PyCapsule export
capsule = result.__arrow_c_stream__()
```

### Streaming large results

For result sets that don't fit in memory:

```python
for batch in conn.stream("SELECT * FROM sensors"):
    print(batch.num_rows)   # each batch is a QueryResult
    df = batch.to_pandas()
```

### Explain queries

```python
plan = conn.explain("SELECT * FROM sensors WHERE value > 43.0")
print(plan)
```

---

## Subscriptions

Subscribe to continuous queries for real-time streaming.

### Synchronous

```python
sub = conn.subscribe("SELECT * FROM sensors")

# Iterator protocol
for result in sub:
    df = result.to_pandas()
    process(df)

# Or non-blocking poll
batch = sub.try_next()  # returns QueryResult or None

sub.cancel()  # safe to call multiple times
print(sub.is_active)  # False
```

### Asynchronous

```python
sub = await conn.subscribe_async("SELECT * FROM sensors")

async for result in sub:
    df = result.to_pandas()
    await process(df)

sub.cancel()
```

### Named stream subscriptions

Subscribe directly to a named stream with schema access:

```python
sub = conn.subscribe_stream("hot_readings")
print(sub.schema)  # pyarrow.Schema

# Blocking with timeout
batch = sub.next_timeout(1000)  # wait up to 1000ms

# Non-blocking
batch = sub.try_next()

# Async variant
sub = await conn.subscribe_stream_async("hot_readings")
async for batch in sub:
    process(batch)
```

---

## Materialized Views

High-level wrapper for working with streaming queries:

```python
from laminardb import MaterializedView, ChangeEvent

# Set up a stream
conn.execute("CREATE STREAM hot_readings AS SELECT * FROM sensors WHERE value > 40.0")
conn.start()

# Wrap as MaterializedView
mv = laminardb.mv(conn, "hot_readings", "SELECT * FROM sensors WHERE value > 40.0")
print(mv.name)     # "hot_readings"
print(mv.sql)      # "SELECT * FROM sensors WHERE value > 40.0"
print(mv.schema()) # Schema wrapper around pyarrow.Schema

# Query current state
result = mv.query()
result.show()

# Subscribe with a callback
def on_change(event: ChangeEvent):
    print(f"Received {len(event)} changes")
    for row in event:
        print(row["device"], row["value"])

thread = mv.subscribe(handler=on_change)  # runs in background daemon thread
```

### ChangeEvent and ChangeRow

```python
def on_change(event: ChangeEvent):
    print(len(event))        # number of rows
    df = event.df()          # -> pandas DataFrame
    table = event.arrow()    # -> pyarrow Table

    for row in event:
        print(row.op)        # "Insert", "Delete", "UpdateInsert", "UpdateDelete"
        print(row["device"]) # dict-style access
        print(row.keys())    # column names
        print(row.to_dict()) # full row as dict
```

---

## Pipeline Control

```python
conn.start()        # start the streaming pipeline
conn.shutdown()     # graceful shutdown
conn.checkpoint()   # trigger a checkpoint (returns checkpoint ID)
conn.cancel_query(query_id)  # cancel a running query
```

---

## Catalog Introspection

```python
# List names
conn.list_tables()   # or conn.tables()
conn.list_streams()  # or conn.materialized_views()
conn.list_sinks()

# Get schema
schema = conn.schema("sensors")  # -> pyarrow.Schema

# Detailed catalog info
for source in conn.sources():
    print(source.name, source.schema, source.watermark_column)

for stream in conn.streams():
    print(stream.name, stream.sql)

for sink in conn.sinks():
    print(sink.name)

for query in conn.queries():
    print(query.id, query.sql, query.active)
```

### Table statistics

```python
stats = conn.stats("sensors")
print(stats["name"])            # "sensors"
print(stats["total_events"])    # total ingested events
print(stats["pending"])         # pending events
print(stats["capacity"])        # buffer capacity
print(stats["is_backpressured"])
print(stats["watermark"])
print(stats["utilization"])     # buffer utilization %
```

---

## Pipeline Metrics

Full observability into the streaming pipeline:

```python
# Pipeline-wide metrics
m = conn.metrics()
print(m.total_events_ingested)
print(m.total_events_emitted)
print(m.total_events_dropped)
print(m.total_cycles)
print(m.total_batches)
print(m.uptime_secs)
print(m.state)                # "running", "stopped", etc.
print(m.source_count, m.stream_count, m.sink_count)
print(m.pipeline_watermark)

# Per-source metrics
for sm in conn.all_source_metrics():
    print(sm.name, sm.total_events, sm.pending, sm.utilization)

sm = conn.source_metrics("sensors")  # specific source, or None

# Per-stream metrics
for stm in conn.all_stream_metrics():
    print(stm.name, stm.total_events, stm.sql)

# Pipeline topology (DAG)
topo = conn.topology()
for node in topo.nodes:
    print(node.name, node.node_type, node.sql)
for edge in topo.edges:
    print(f"{edge.from_node} -> {edge.to_node}")
```

---

## Error Handling

Structured exception hierarchy with numeric error codes:

```python
import laminardb

try:
    conn.query("INVALID SQL")
except laminardb.QueryError as e:
    print(f"Query failed: {e}")
    print(f"Error code: {e.code}")
except laminardb.LaminarError as e:
    print(f"LaminarDB error: {e}")
```

### Exception hierarchy

| Exception | Raised when |
|-----------|------------|
| `LaminarError` | Base class for all LaminarDB errors |
| `ConnectionError` | Connection cannot be established or is lost |
| `QueryError` | SQL query fails (syntax error, missing table, etc.) |
| `IngestionError` | Data insertion fails (type mismatch, invalid format) |
| `SchemaError` | Schema operation fails (table exists, invalid schema) |
| `SubscriptionError` | Subscription operation fails |
| `StreamError` | Stream / materialized view operation fails |
| `CheckpointError` | Checkpoint operation fails |
| `ConnectorError` | Connector operation fails |

### Error codes

Every exception carries a numeric `.code` attribute. Constants are available via `laminardb.codes`:

```python
from laminardb import codes

codes.CONNECTION_FAILED     # 100
codes.CONNECTION_CLOSED     # 101
codes.TABLE_NOT_FOUND       # 200
codes.TABLE_EXISTS          # 201
codes.SCHEMA_MISMATCH       # 202
codes.INGESTION_FAILED      # 300
codes.WRITER_CLOSED         # 301
codes.QUERY_FAILED          # 400
codes.SQL_PARSE_ERROR       # 401
codes.SUBSCRIPTION_FAILED   # 500
codes.INTERNAL_ERROR        # 900
```

---

## Configuration

```python
config = laminardb.LaminarConfig(
    buffer_size=65536,             # source buffer size (default: 65536)
    storage_dir="/tmp/laminar",    # storage directory (optional)
    checkpoint_interval_ms=10000,  # checkpoint interval in ms (optional)
    table_spill_threshold=1_000_000,  # spill threshold (default: 1M)
)

conn = laminardb.open(":memory:", config=config)
```

`laminardb.Config` is an alias for `laminardb.LaminarConfig`.

---

## Python Types

High-level Python wrappers in `laminardb.types`:

```python
from laminardb import Schema, Column, TableStats, Watermark, CheckpointStatus, Metrics
```

| Type | Description |
|------|-------------|
| `Column(name, type, nullable)` | Frozen dataclass for column metadata |
| `Schema` | Wraps `pyarrow.Schema` with `columns`, `names`, indexing by name/position |
| `TableStats(row_count, size_bytes)` | Frozen dataclass |
| `Watermark(current, lag_ms)` | Frozen dataclass |
| `CheckpointStatus(checkpoint_id, enabled)` | Frozen dataclass |
| `Metrics` | Wrapper around `PipelineMetrics` with `events_per_second`, `uptime_secs`, `state` |
| `ChangeRow` | Dict-like row with `.op`, `["col"]`, `.keys()`, `.to_dict()` |
| `ChangeEvent` | Iterable batch of `ChangeRow`s with `.df()`, `.arrow()`, `.pl()` |
| `MaterializedView` | High-level stream wrapper with `.query()`, `.subscribe()`, `.schema()` |

---

## Async Support

```python
import asyncio
import laminardb

async def main():
    async with laminardb.open(":memory:") as conn:
        conn.execute("CREATE SOURCE events (id BIGINT, name VARCHAR)")
        conn.insert("events", [{"id": 1, "name": "click"}])

        sub = await conn.subscribe_async("SELECT * FROM events")
        async for batch in sub:
            print(batch.to_dicts())
            break
        sub.cancel()

asyncio.run(main())
```

---

## Type Safety

LaminarDB ships with PEP 561 type stubs (`py.typed` marker). All public APIs are fully typed and pass `mypy --strict`:

```bash
mypy python/laminardb --strict
```

IDEs like VS Code and PyCharm provide full autocompletion out of the box.

---

## Performance

| Feature | Details |
|---------|---------|
| Zero-copy export | Arrow PyCapsule interface (`__arrow_c_stream__`) avoids serialization |
| GIL release | Every blocking Rust call uses `py.allow_threads()` |
| Free-threaded Python | All types are `Send + Sync`, ready for Python 3.13t / 3.14t |
| Batch buffering | Streaming writer for efficient bulk inserts |
| LTO release builds | Link-time optimization + symbol stripping |

### Benchmarks

Run the included benchmarks:

```bash
python benchmarks/bench_ingestion.py   # ingestion throughput by format
python benchmarks/bench_query.py       # query + conversion throughput
python benchmarks/bench_streaming.py   # subscription iteration throughput
```

---

## Examples

```bash
python examples/quickstart.py            # DuckDB-style API walkthrough
python examples/streaming_analytics.py   # MaterializedView + ChangeEvent pattern
```

---

## API Aliases

For users coming from DuckDB:

| DuckDB style | Equivalent |
|--------------|-----------|
| `conn.sql(query)` | `conn.query(query)` |
| `conn.tables()` | `conn.list_tables()` |
| `conn.materialized_views()` | `conn.list_streams()` |
| `result.df()` | `result.to_pandas()` |
| `result.pl()` | `result.to_polars()` |
| `result.arrow()` | `result.to_arrow()` |
| `result.fetchall()` | rows as `list[tuple]` |
| `result.fetchone()` | first row as `tuple` or `None` |
| `result.fetchmany(n)` | up to `n` rows as `list[tuple]` |
| `laminardb.Config` | `laminardb.LaminarConfig` |
| `laminardb.BatchWriter` | `laminardb.Writer` |

---

## Contributing

### Development setup

```bash
# Clone with the laminardb monorepo dependency
git clone https://github.com/laminardb/laminardb-python.git
git clone https://github.com/laminardb/laminardb.git  # sibling directory

# Build and install in development mode
cd laminardb-python
python -m venv .venv
source .venv/bin/activate   # or .venv\Scripts\activate on Windows
pip install maturin
maturin develop --extras dev
```

### Running checks

```bash
# Tests
pytest tests/ -v

# Rust checks
cargo fmt --check
cargo clippy -- -D warnings

# Type checking
mypy python/laminardb --strict
```

### Project structure

```
python/laminardb/          # Python package
  __init__.py              # Public API, module-level functions, aliases
  types.py                 # High-level Python types (Schema, MaterializedView, etc.)
  _laminardb.pyi           # PEP 561 type stubs
  py.typed                 # Type marker
src/                       # Rust source (PyO3 bindings)
  lib.rs                   # Module entry point, open()/connect()
  connection.rs            # Connection class (40+ methods)
  query.rs                 # QueryResult class
  conversion.rs            # Python <-> Arrow type conversion
  writer.rs                # Streaming writer
  subscription.rs          # Sync subscription iterator
  async_support.rs         # Tokio runtime + async subscription
  stream_subscription.rs   # Named stream subscriptions (sync + async)
  execute.rs               # ExecuteResult for DDL/DML introspection
  error.rs                 # Exception hierarchy
  config.rs                # LaminarConfig
  catalog.rs               # Catalog info classes
  metrics.rs               # Pipeline topology and metrics
tests/                     # pytest test suite (15 files, 233+ tests)
benchmarks/                # Performance benchmarks
examples/                  # Example scripts
docs/                      # Architecture and feature specs
```

---

## License

MIT &mdash; see [LICENSE](LICENSE) for details.
