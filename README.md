# laminardb

[![PyPI](https://img.shields.io/pypi/v/laminardb)](https://pypi.org/project/laminardb/)
[![CI](https://github.com/laminardb/laminardb-python/actions/workflows/ci.yml/badge.svg)](https://github.com/laminardb/laminardb-python/actions/workflows/ci.yml)
[![Python 3.11+](https://img.shields.io/pypi/pyversions/laminardb)](https://pypi.org/project/laminardb/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Python bindings for the [LaminarDB](https://github.com/laminardb/laminardb) streaming SQL database. Built with Rust and Apache Arrow for zero-copy performance.

```python
import laminardb

conn = laminardb.open(":memory:")
conn.execute("CREATE SOURCE sensors (ts BIGINT, device VARCHAR, value DOUBLE)")
conn.insert("sensors", [
    {"ts": 1, "device": "sensor_a", "value": 42.0},
    {"ts": 2, "device": "sensor_b", "value": 43.5},
])

result = conn.sql("SELECT * FROM sensors WHERE value > 42.0")
result.show()           # print table
df = result.df()        # pandas DataFrame
pl_df = result.pl()     # polars DataFrame
table = result.arrow()  # pyarrow Table
conn.close()
```

## Installation

```bash
pip install laminardb                # core
pip install laminardb[pandas]        # + pandas & pyarrow
pip install laminardb[polars]        # + polars
```

Requires **Python 3.11+**. Wheels: Linux x86_64, macOS ARM64, Windows x86_64.

## Core Concepts

Data flows through **sources** (ingest) → **streams** (transform via continuous SQL) → **sinks** (output). See the [LaminarDB docs](https://github.com/laminardb/laminardb) for the full SQL reference.

## Connection

```python
conn = laminardb.open(":memory:")                           # in-memory
conn = laminardb.open("mydb")                               # path-based
conn = laminardb.connect(":memory:")                         # alias for open(":memory:")

config = laminardb.LaminarConfig(buffer_size=131072, checkpoint_interval_ms=5000)
conn = laminardb.open(":memory:", config=config)

with laminardb.open(":memory:") as conn:   # context manager (auto-closes)
    ...

async with laminardb.open(":memory:") as conn:  # async context manager
    ...
```

**Properties:** `is_closed`, `pipeline_state`, `pipeline_watermark`, `total_events_processed`, `source_count`, `sink_count`, `active_query_count`, `is_checkpoint_enabled`

## Data Ingestion

All `insert()` calls return the number of rows inserted.

```python
conn.insert("t", {"ts": 1, "val": 1.0})                   # single dict
conn.insert("t", [{"ts": 1, "val": 1.0}, {"ts": 2, "val": 2.0}])  # list of dicts
conn.insert("t", {"ts": [1, 2], "val": [1.0, 2.0]})       # columnar dict
conn.insert("t", pandas_df)                                 # pandas DataFrame
conn.insert("t", polars_df)                                 # polars DataFrame
conn.insert("t", pa_table)                                  # pyarrow RecordBatch/Table
conn.insert("t", arrow_capsule)                             # __arrow_c_stream__ (zero-copy)
conn.insert_json("t", '[{"ts": 1, "val": 1.0}]')           # JSON string
conn.insert_csv("t", "ts,val\n1,1.0\n")                    # CSV string
```

### Streaming writer

```python
with conn.writer("sensors") as w:
    w.insert({"ts": 1, "device": "a", "value": 42.0})
    w.watermark(1)          # advance event-time watermark
    print(w.name)           # "sensors"
    print(w.schema)         # pyarrow.Schema
    print(w.current_watermark)
```

## Query Results

```python
result = conn.sql("SELECT * FROM sensors")  # or conn.query(...)

result.num_rows, result.num_columns, result.columns, result.schema, len(result)

# Export
result.to_pandas()    # pandas.DataFrame          result.df()          # alias
result.to_polars()    # polars.DataFrame          result.pl()          # alias
result.to_arrow()     # pyarrow.Table             result.arrow()       # alias
result.to_dicts()     # dict[str, list]           result.to_df()       # auto-detect
result.fetchall()     # list[tuple]               result.fetchone()    # tuple | None
result.fetchmany(10)  # list[tuple]               result.show()        # print (20 rows)
result.pl(lazy=True)  # polars.LazyFrame
result.__arrow_c_stream__()  # zero-copy PyCapsule

for row in result:    # iterate as tuples
    ts, device, value = row

# Stream large results in batches
for batch in conn.stream("SELECT * FROM sensors"):
    batch.to_pandas()
```

## DDL & Sources

```python
conn.execute("CREATE SOURCE metrics (ts BIGINT, value DOUBLE)")  # SQL
conn.create_table("metrics", {"ts": "int64", "value": "float64"})  # Python dict schema
conn.create_table("logs", pa.schema([("ts", pa.int64())]))  # PyArrow schema

exec_result = conn.execute("CREATE SOURCE s (id BIGINT)")
exec_result.result_type   # "ddl" | "query" | "rows_affected" | "metadata"
exec_result.ddl_type      # "CREATE SOURCE"
exec_result.ddl_object    # "s"
int(exec_result)          # rows affected
exec_result.to_query_result()  # QueryResult | None

conn.list_tables()        # source names (alias: conn.tables())
conn.schema("sensors")    # pyarrow.Schema
conn.explain("SELECT 1")  # query plan string
conn.stats("sensors")     # per-source statistics dict
```

## Subscriptions

### Sync — continuous query

```python
sub = conn.subscribe("SELECT * FROM sensors")
for result in sub:                    # blocks until data arrives
    df = result.to_pandas()
batch = sub.try_next()                # non-blocking: QueryResult | None
sub.cancel()
```

### Async — continuous query

```python
sub = await conn.subscribe_async("SELECT * FROM sensors")
async for result in sub:
    df = result.to_pandas()
sub.cancel()
```

### Named stream subscriptions

```python
sub = conn.subscribe_stream("alerts")
sub.schema                            # pyarrow.Schema
batch = sub.try_next()                # non-blocking
batch = sub.next()                    # blocking (indefinite)
try:
    batch = sub.next_timeout(1000)    # blocking with timeout (ms)
except laminardb.SubscriptionError:   # raises on timeout
    pass
sub.cancel()

# Async variant:
sub = await conn.subscribe_stream_async("alerts")
async for batch in sub:
    batch.df()
sub.cancel()
```

### Callback subscriptions

```python
handle = conn.subscribe_callback("SELECT * FROM sensors", lambda b: print(b.to_dicts()))
handle = conn.subscribe_stream_callback("alerts", on_data)
handle.is_active    # bool
handle.wait()       # block until callback thread finishes
handle.cancel()     # stop receiving
```

## Pipeline Control

```python
conn.start()                    # start the streaming pipeline
conn.shutdown()                 # graceful shutdown
conn.cancel_query(query_id)     # cancel a running query

if conn.is_checkpoint_enabled:  # requires checkpoint_interval_ms in config
    result = conn.checkpoint()  # CheckpointResult
    print(result.checkpoint_id)
```

## Catalog & Metrics

```python
# Catalog
conn.list_tables()    # alias: tables()
conn.list_streams()   # alias: materialized_views()
conn.list_sinks()

for s in conn.sources():    print(s.name, s.schema, s.watermark_column)  # SourceInfo
for s in conn.streams():    print(s.name, s.sql)                         # StreamInfo
for s in conn.sinks():      print(s.name)                                # SinkInfo
for q in conn.queries():    print(q.id, q.sql, q.active)                 # QueryInfo

# Pipeline metrics
m = conn.metrics()          # PipelineMetrics
m.total_events_ingested, m.total_events_emitted, m.total_events_dropped
m.total_cycles, m.uptime_secs, m.state, m.pipeline_watermark

for sm in conn.all_source_metrics():   # SourceMetrics
    print(sm.name, sm.total_events, sm.utilization, sm.is_backpressured, sm.watermark)
for stm in conn.all_stream_metrics():  # StreamMetrics
    print(stm.name, stm.total_events, stm.watermark, stm.sql)

# Topology (DAG)
topo = conn.topology()
for node in topo.nodes:   print(f"[{node.node_type}] {node.name}")  # PipelineNode
for edge in topo.edges:   print(f"{edge.from_node} --> {edge.to_node}")  # PipelineEdge
```

## Error Handling

All exceptions inherit from `LaminarError` and carry a numeric `.code` attribute.

```python
try:
    conn.query("INVALID SQL")
except laminardb.QueryError as e:
    print(e, e.code)
```

| Exception | When raised |
|-----------|------------|
| `LaminarError` | Base class |
| `ConnectionError` | Connection lifecycle failures |
| `QueryError` | SQL syntax errors, missing tables |
| `IngestionError` | Data insertion failures, type mismatches |
| `SchemaError` | Schema conflicts, table already exists |
| `SubscriptionError` | Subscription failures, timeouts |
| `StreamError` | Stream/materialized view failures |
| `CheckpointError` | Checkpoint operation failures |
| `ConnectorError` | External connector failures |

Error codes: `laminardb.codes.CONNECTION_CLOSED` (101), `TABLE_NOT_FOUND` (200), `QUERY_FAILED` (400), `SUBSCRIPTION_TIMEOUT` (502), and [more](python/laminardb/_laminardb.pyi).

## Python Wrapper Types

Convenience types in `laminardb.types`:

| Type | Description |
|------|-------------|
| `Column(name, type, nullable)` | Frozen dataclass for column metadata |
| `Schema` | Wraps `pyarrow.Schema` with `columns`, `names`, indexing |
| `Metrics` | Wraps `PipelineMetrics` with `events_per_second`, `uptime_secs`, `state` |
| `ChangeRow` | Dict-like row with `.op` and `to_dict()` |
| `ChangeEvent` | Iterable batch of `ChangeRow`s with `df()`, `pl()`, `arrow()` |
| `MaterializedView` | Stream wrapper with `query()`, `subscribe()`, `schema()` |

```python
mv = laminardb.mv(conn, "alerts")
mv.query().show()
handle = mv.subscribe(handler=lambda e: [print(r["device"]) for r in e])
```

## Building from Source

```bash
git clone https://github.com/laminardb/laminardb-python.git
git clone https://github.com/laminardb/laminardb.git  # sibling directory
cd laminardb-python
python -m venv .venv && source .venv/bin/activate  # .venv\Scripts\activate on Windows
pip install maturin && maturin develop --extras dev
```

**Prerequisites:** Rust 1.85+, Python 3.11+, cmake, pkg-config, libclang-dev (Linux)

```bash
pytest tests/ -v               # tests
cargo fmt --check              # formatting
cargo clippy -- -D warnings    # lints
mypy python/laminardb --strict # type checking
```

## Contributing

See the [LaminarDB](https://github.com/laminardb/laminardb) main repository for contribution guidelines and the full SQL reference.

## License

MIT — see [LICENSE](LICENSE) for details.
