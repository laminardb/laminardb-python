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

- **DuckDB-style API** &mdash; `.sql()`, `.df()`, `.arrow()`, `.fetchall()`, `.show()` &mdash; feels like DuckDB
- **Streaming-first** &mdash; sources, streams, sinks, watermarks, continuous subscriptions
- **10 input formats** &mdash; dicts, pandas, polars, pyarrow, JSON, CSV, Arrow PyCapsule
- **6 output formats** &mdash; pandas, polars, pyarrow, dicts, auto-detect, PyCapsule (zero-copy)
- **Full type safety** &mdash; PEP 561 stubs with `py.typed`, passes `mypy --strict`
- **Thread-safe** &mdash; GIL release on all blocking ops, `Send + Sync` for free-threaded Python 3.13t+
- **Pipeline observability** &mdash; topology DAG, per-component metrics, watermark tracking

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

## Table of Contents

- [Quick Start](#quick-start)
- [Core Concepts: Sources, Streams, Sinks](#core-concepts-sources-streams-sinks)
- [Connections](#connections)
- [Sources](#sources)
- [Data Ingestion](#data-ingestion)
- [Streams (Materialized Views)](#streams-materialized-views)
- [Sinks](#sinks)
- [Watermarks & Event Time](#watermarks--event-time)
- [Querying](#querying)
- [Subscriptions](#subscriptions)
- [Streaming Examples](#streaming-examples)
- [Pipeline Control](#pipeline-control)
- [Catalog Introspection](#catalog-introspection)
- [Pipeline Metrics & Observability](#pipeline-metrics--observability)
- [Error Handling](#error-handling)
- [Configuration](#configuration)
- [Python Types](#python-types)
- [Async Support](#async-support)
- [Performance](#performance)
- [API Aliases (DuckDB Compatibility)](#api-aliases-duckdb-compatibility)
- [Contributing](#contributing)

---

## Quick Start

### Connect and query

```python
import laminardb

conn = laminardb.open(":memory:")
conn.execute("CREATE SOURCE sensors (ts BIGINT, device VARCHAR, value DOUBLE)")
conn.insert("sensors", [
    {"ts": 1, "device": "sensor_a", "value": 42.0},
    {"ts": 2, "device": "sensor_b", "value": 43.5},
    {"ts": 3, "device": "sensor_a", "value": 44.1},
])

result = conn.sql("SELECT * FROM sensors WHERE value > 43.0")
result.show()          # print to terminal
df = result.df()       # -> pandas DataFrame
pl_df = result.pl()    # -> polars DataFrame
table = result.arrow() # -> pyarrow Table
rows = result.fetchall()  # -> list of tuples
conn.close()
```

### Real-time streaming in 10 lines

```python
import laminardb

conn = laminardb.open(":memory:")
conn.execute("CREATE SOURCE readings (ts BIGINT, sensor VARCHAR, temp DOUBLE)")
conn.execute("CREATE STREAM alerts AS SELECT * FROM readings WHERE temp > 100.0")
conn.start()

# Subscribe to the alerts stream
sub = conn.subscribe_stream("alerts")
conn.insert("readings", {"ts": 1, "sensor": "boiler_1", "temp": 105.3})

batch = sub.next_timeout(2000)  # wait up to 2s
if batch:
    batch.show()  # shows the alert row
sub.cancel()
conn.close()
```

### Module-level queries

```python
import laminardb

# Uses a thread-safe default in-memory connection
laminardb.sql("SELECT 1 + 1 AS answer").show()
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

## Core Concepts: Sources, Streams, Sinks

LaminarDB is a **streaming SQL database**. Data flows through a pipeline of three primitives:

```
Sources  -->  Streams  -->  Sinks
(ingest)     (transform)    (output)
```

| Concept | What it is | SQL | Python API |
|---------|-----------|-----|------------|
| **Source** | Entry point for data. Defines a schema and accepts inserts. | `CREATE SOURCE` | `conn.create_table()`, `conn.insert()` |
| **Stream** | A continuously-maintained SQL query over sources or other streams. Like a materialized view that updates in real time. | `CREATE STREAM ... AS SELECT ...` | `conn.execute()`, `conn.subscribe_stream()` |
| **Sink** | An output destination that receives data from streams. | `CREATE SINK` | `conn.execute()` |

### How it works

1. **Create sources** &mdash; define the schema for incoming data
2. **Create streams** &mdash; write SQL transformations that run continuously
3. **Start the pipeline** &mdash; `conn.start()` begins processing
4. **Insert data** &mdash; push rows into sources via `insert()` or a `Writer`
5. **Subscribe** &mdash; receive real-time results from streams as data flows through
6. **Query** &mdash; run ad-hoc SQL at any time with `conn.sql()`

```python
conn = laminardb.open(":memory:")

# 1. Define sources
conn.execute("CREATE SOURCE page_views (ts BIGINT, user_id VARCHAR, page VARCHAR)")
conn.execute("CREATE SOURCE clicks     (ts BIGINT, user_id VARCHAR, button VARCHAR)")

# 2. Define streams (continuous SQL transformations)
conn.execute("""
    CREATE STREAM active_users AS
    SELECT user_id, COUNT(*) AS view_count
    FROM page_views
    GROUP BY user_id
""")

conn.execute("""
    CREATE STREAM click_through AS
    SELECT p.user_id, p.page, c.button
    FROM page_views p
    JOIN clicks c ON p.user_id = c.user_id
""")

# 3. Start the pipeline
conn.start()

# 4. Insert data into sources
conn.insert("page_views", {"ts": 1, "user_id": "alice", "page": "/home"})
conn.insert("clicks", {"ts": 2, "user_id": "alice", "button": "signup"})

# 5. Subscribe to stream results
sub = conn.subscribe_stream("active_users")
batch = sub.next_timeout(2000)
if batch:
    batch.show()
sub.cancel()

# 6. Ad-hoc query
conn.sql("SELECT * FROM page_views").show()

conn.close()
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

## Sources

Sources are the **entry points** for data into the pipeline. Each source has a fixed schema and an internal buffer for incoming rows.

### Creating sources

```python
# Via SQL
conn.execute("CREATE SOURCE sensors (ts BIGINT, device VARCHAR, value DOUBLE)")

# Via Python dict schema (column_name -> Arrow type string)
conn.create_table("metrics", {
    "ts": "int64",
    "name": "string",
    "value": "float64",
})

# Via PyArrow schema
import pyarrow as pa
conn.create_table("logs", pa.schema([
    ("ts", pa.int64()),
    ("level", pa.string()),
    ("message", pa.string()),
]))
```

### Discovering sources

```python
conn.list_tables()    # -> ["sensors", "metrics", "logs"]
conn.tables()         # DuckDB-style alias

# Detailed source info
for source in conn.sources():
    print(source.name)              # "sensors"
    print(source.schema)            # pyarrow.Schema
    print(source.watermark_column)  # watermark column name, or None

# Get a specific source's schema
schema = conn.schema("sensors")  # -> pyarrow.Schema
```

### Source metrics

```python
sm = conn.source_metrics("sensors")
if sm:
    print(sm.name)              # "sensors"
    print(sm.total_events)      # total rows ingested
    print(sm.pending)           # rows buffered, not yet processed
    print(sm.capacity)          # buffer capacity
    print(sm.is_backpressured)  # True if buffer is full
    print(sm.watermark)         # current source watermark
    print(sm.utilization)       # 0.0 - 1.0 buffer usage

# All sources at once
for sm in conn.all_source_metrics():
    print(f"{sm.name}: {sm.total_events} events, {sm.utilization:.0%} buffer used")
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

For high-throughput ingestion with explicit watermark control:

```python
with conn.writer("sensors") as w:
    for i in range(1000):
        w.insert({"ts": i, "device": f"d{i % 10}", "value": float(i)})

        # Emit watermark every 100 rows to signal time progress
        if i % 100 == 99:
            w.watermark(i)

    print(w.name)              # "sensors"
    print(w.schema)            # pyarrow.Schema
    print(w.current_watermark) # 999
# auto-flush on context exit
```

---

## Streams (Materialized Views)

Streams are **continuously-updated SQL queries**. When new data arrives at a source, all dependent streams are automatically recomputed.

### Creating streams

```python
# Filter stream
conn.execute("""
    CREATE STREAM high_temp AS
    SELECT * FROM sensors WHERE value > 100.0
""")

# Aggregation stream
conn.execute("""
    CREATE STREAM sensor_stats AS
    SELECT device, COUNT(*) AS cnt, AVG(value) AS avg_value
    FROM sensors
    GROUP BY device
""")

# Join stream (across multiple sources)
conn.execute("""
    CREATE STREAM enriched_readings AS
    SELECT s.ts, s.value, d.location, d.owner
    FROM sensors s
    JOIN device_registry d ON s.device = d.device_id
""")

# Chained streams (stream of a stream)
conn.execute("""
    CREATE STREAM critical_alerts AS
    SELECT * FROM high_temp WHERE value > 200.0
""")
```

### Windowed aggregations

LaminarDB supports time-based window functions in streaming SQL. Use them in `GROUP BY` clauses to aggregate data over sliding or fixed time intervals.

```python
# Tumbling window — fixed, non-overlapping 5-second intervals
conn.execute("""
    CREATE STREAM avg_temp_5s AS
    SELECT device, AVG(value) AS avg_value, COUNT(*) AS cnt
    FROM sensors
    GROUP BY device, TUMBLE(ts, 5000)
""")

# Hopping window — 5-second windows that advance every 2.5 seconds (overlapping)
conn.execute("""
    CREATE STREAM avg_temp_hop AS
    SELECT device, AVG(value) AS avg_value, COUNT(*) AS cnt
    FROM sensors
    GROUP BY device, HOPPING(ts, 5000, 2500)
""")

# Session window — groups events within 10-second inactivity gaps
conn.execute("""
    CREATE STREAM session_stats AS
    SELECT device, AVG(value) AS avg_value, COUNT(*) AS cnt
    FROM sensors
    GROUP BY device, SESSION(ts, 10000)
""")
```

| Window Type | Syntax | Behavior |
|-------------|--------|----------|
| **TUMBLE** | `TUMBLE(ts_col, size_ms)` | Fixed, non-overlapping windows |
| **HOPPING** | `HOPPING(ts_col, size_ms, hop_ms)` | Fixed windows that advance by hop interval (windows overlap when hop < size) |
| **SESSION** | `SESSION(ts_col, gap_ms)` | Dynamic windows that close after an inactivity gap |

### Discovering streams

```python
conn.list_streams()            # -> ["high_temp", "sensor_stats", ...]
conn.materialized_views()      # DuckDB-style alias

for stream in conn.streams():
    print(stream.name)  # "high_temp"
    print(stream.sql)   # "SELECT * FROM sensors WHERE value > 100.0"
```

### Stream metrics

```python
stm = conn.stream_metrics("high_temp")
if stm:
    print(stm.name)              # "high_temp"
    print(stm.total_events)      # events emitted by this stream
    print(stm.pending)           # events waiting to be processed
    print(stm.capacity)          # output buffer capacity
    print(stm.is_backpressured)  # downstream can't keep up
    print(stm.watermark)         # stream watermark position
    print(stm.sql)               # SQL definition

for stm in conn.all_stream_metrics():
    print(f"{stm.name}: {stm.total_events} emitted, watermark={stm.watermark}")
```

### MaterializedView wrapper

For a higher-level API, wrap a stream in a `MaterializedView`:

```python
from laminardb import MaterializedView, ChangeEvent

mv = laminardb.mv(conn, "high_temp", "SELECT * FROM sensors WHERE value > 100.0")

# Query current state
result = mv.query()
result.show()

# Query with filter
result = mv.query(where="device = 'boiler_1'")

# Get schema
print(mv.schema())  # Schema wrapper

# Subscribe with callback
def on_alert(event: ChangeEvent):
    for row in event:
        print(f"[{row.op}] {row['device']}: {row['value']}")

thread = mv.subscribe(handler=on_alert)  # background daemon thread
```

---

## Sinks

Sinks are **output destinations** that receive data from streams. They define where processed data goes after transformation.

```python
# Create a sink via SQL
conn.execute("CREATE SINK output_sink FROM high_temp")

# Discover sinks
conn.list_sinks()  # -> ["output_sink"]

for sink in conn.sinks():
    print(sink.name)  # "output_sink"

print(conn.sink_count)  # 1
```

---

## Watermarks & Event Time

Watermarks are the foundation of **event-time processing** in LaminarDB. A watermark is a timestamp that declares: *"all events with timestamps up to this point have been seen."*

### Why watermarks matter

In streaming systems, data can arrive out of order. Watermarks let the engine know when it's safe to:
- Close time-based windows (e.g., "all data for the 10:00-10:05 window has arrived")
- Emit aggregation results
- Detect late-arriving data
- Advance the pipeline's logical clock

### Emitting watermarks

Use the streaming `Writer` to emit watermarks alongside data:

```python
import time

conn = laminardb.open(":memory:")
conn.execute("CREATE SOURCE readings (ts BIGINT, sensor VARCHAR, temp DOUBLE)")
conn.start()

with conn.writer("readings") as w:
    # Simulate a timeseries: readings every second
    for t in range(100):
        w.insert({
            "ts": t * 1000,  # millisecond timestamps
            "sensor": f"sensor_{t % 5}",
            "temp": 20.0 + (t % 30) * 0.5,
        })

        # Advance the watermark every 10 events
        # This tells the engine: "no more events with ts <= this value"
        if t % 10 == 9:
            w.watermark(t * 1000)
            print(f"Watermark advanced to {w.current_watermark}")
```

### Reading watermarks

Watermarks are visible at every level of the pipeline:

```python
# Writer-level: current watermark for a specific source
with conn.writer("readings") as w:
    w.watermark(5000)
    print(w.current_watermark)  # 5000

# Source-level: via source metrics
sm = conn.source_metrics("readings")
if sm:
    print(sm.watermark)  # source watermark position

# Stream-level: via stream metrics
stm = conn.stream_metrics("alerts")
if stm:
    print(stm.watermark)  # how far this stream has processed

# Pipeline-level: global watermark (min across all sources)
print(conn.pipeline_watermark)

# Via PipelineMetrics
m = conn.metrics()
print(m.pipeline_watermark)
```

### Watermark Python type

For application-level watermark tracking:

```python
from laminardb import Watermark

wm = Watermark(current=5000, lag_ms=200)
print(wm.current)  # 5000
print(wm.lag_ms)   # 200 (how far behind real-time)
```

---

## Querying

### SQL queries

```python
# Standard query &mdash; collects all results
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

Subscribe directly to a named stream with schema access and timeout support:

```python
sub = conn.subscribe_stream("alerts")
print(sub.schema)       # pyarrow.Schema of the stream
print(sub.is_active)    # True

# Blocking with timeout (recommended for most use cases)
batch = sub.next_timeout(1000)  # wait up to 1000ms, returns None on timeout

# Non-blocking poll
batch = sub.try_next()  # returns immediately, None if no data

# Blocking (waits indefinitely)
batch = sub.next()

# Iterator protocol
for batch in sub:
    batch.show()

sub.cancel()
```

### Async stream subscriptions

```python
sub = await conn.subscribe_stream_async("alerts")
print(sub.schema)  # pyarrow.Schema

async for batch in sub:
    df = batch.df()
    await send_to_dashboard(df)

sub.cancel()
```

---

## Streaming Examples

### IoT sensor monitoring

A complete pipeline that ingests sensor readings, detects anomalies, and tracks per-device statistics:

```python
import time
import threading
import laminardb
from laminardb import ChangeEvent

conn = laminardb.open(":memory:")

# ── Define sources ──
conn.execute("""
    CREATE SOURCE sensors (
        ts       BIGINT,
        device   VARCHAR,
        temp     DOUBLE,
        humidity DOUBLE
    )
""")

# ── Define streams ──
# Real-time anomaly detection
conn.execute("""
    CREATE STREAM temp_alerts AS
    SELECT ts, device, temp
    FROM sensors
    WHERE temp > 80.0 OR temp < -20.0
""")

# Per-device running statistics
conn.execute("""
    CREATE STREAM device_stats AS
    SELECT
        device,
        COUNT(*)   AS reading_count,
        AVG(temp)  AS avg_temp,
        MAX(temp)  AS max_temp,
        MIN(temp)  AS min_temp
    FROM sensors
    GROUP BY device
""")

# ── Start the pipeline ──
conn.start()

# ── Subscribe to alerts ──
alert_count = 0

def on_alert(event: ChangeEvent):
    global alert_count
    for row in event:
        alert_count += 1
        print(f"  ALERT [{row.op}] {row['device']}: temp={row['temp']}")

mv = laminardb.mv(conn, "temp_alerts")
alert_thread = mv.subscribe(handler=on_alert)

# ── Ingest sensor data with watermarks ──
with conn.writer("sensors") as w:
    for t in range(200):
        w.insert({
            "ts": t * 1000,
            "device": f"device_{t % 5}",
            "temp": 20.0 + (t % 50) * 1.5,   # some will exceed 80.0
            "humidity": 40.0 + (t % 20) * 1.0,
        })

        if t % 50 == 49:
            w.watermark(t * 1000)

time.sleep(0.5)  # let the pipeline process

# ── Query current state ──
print("\n=== Device Statistics ===")
conn.sql("SELECT * FROM device_stats").show()

print(f"\nTotal alerts: {alert_count}")

# ── Observe pipeline health ──
m = conn.metrics()
print(f"Pipeline: {m.state}, {m.total_events_ingested} events ingested")
print(f"Uptime: {m.uptime_secs:.1f}s, watermark: {m.pipeline_watermark}")

conn.close()
```

### Timeseries analytics

Compute rolling metrics over time-ordered data:

```python
import laminardb

conn = laminardb.open(":memory:")

# ── Source: stock trades ──
conn.execute("""
    CREATE SOURCE trades (
        ts      BIGINT,
        symbol  VARCHAR,
        price   DOUBLE,
        volume  BIGINT
    )
""")

# ── Stream: per-symbol price summary ──
conn.execute("""
    CREATE STREAM price_summary AS
    SELECT
        symbol,
        COUNT(*)    AS trade_count,
        AVG(price)  AS avg_price,
        MAX(price)  AS high,
        MIN(price)  AS low,
        SUM(volume) AS total_volume
    FROM trades
    GROUP BY symbol
""")

# ── Stream: high-volume trades ──
conn.execute("""
    CREATE STREAM whale_trades AS
    SELECT ts, symbol, price, volume
    FROM trades
    WHERE volume > 10000
""")

conn.start()

# ── Simulate a trading day ──
import random
symbols = ["AAPL", "GOOGL", "MSFT", "AMZN"]

with conn.writer("trades") as w:
    for t in range(1000):
        sym = random.choice(symbols)
        w.insert({
            "ts": 1700000000 + t,
            "symbol": sym,
            "price": round(100 + random.gauss(0, 5), 2),
            "volume": random.randint(100, 50000),
        })
        if t % 100 == 99:
            w.watermark(1700000000 + t)

# ── Query results ──
print("=== Price Summary ===")
conn.sql("SELECT * FROM price_summary").show()

print("\n=== Whale Trades ===")
conn.sql("SELECT * FROM whale_trades").show(max_rows=10)

# ── Export to pandas for further analysis ──
df = conn.sql("SELECT * FROM price_summary").df()
print(f"\nHighest avg price: {df.loc[df['avg_price'].idxmax(), 'symbol']}")

conn.close()
```

### Multi-source event correlation

Join events from different sources in real time:

```python
import laminardb

conn = laminardb.open(":memory:")

# ── Two event sources ──
conn.execute("CREATE SOURCE logins    (ts BIGINT, user_id VARCHAR, ip VARCHAR)")
conn.execute("CREATE SOURCE purchases (ts BIGINT, user_id VARCHAR, amount DOUBLE)")

# ── Stream: correlate logins with purchases ──
conn.execute("""
    CREATE STREAM user_activity AS
    SELECT
        l.user_id,
        l.ip,
        p.amount,
        p.ts AS purchase_ts
    FROM logins l
    JOIN purchases p ON l.user_id = p.user_id
""")

# ── Stream: suspicious activity (purchase from new IP) ──
conn.execute("""
    CREATE STREAM large_purchases AS
    SELECT user_id, amount, purchase_ts
    FROM user_activity
    WHERE amount > 500.0
""")

conn.start()

# ── Ingest correlated events ──
conn.insert("logins", [
    {"ts": 1, "user_id": "alice", "ip": "1.2.3.4"},
    {"ts": 2, "user_id": "bob",   "ip": "5.6.7.8"},
])
conn.insert("purchases", [
    {"ts": 3, "user_id": "alice", "amount": 29.99},
    {"ts": 4, "user_id": "bob",   "amount": 999.00},
    {"ts": 5, "user_id": "alice", "amount": 750.00},
])

import time; time.sleep(0.3)

# ── Check results ──
print("=== User Activity ===")
conn.sql("SELECT * FROM user_activity").show()

print("\n=== Large Purchases ===")
conn.sql("SELECT * FROM large_purchases").show()

conn.close()
```

### Real-time dashboard with async subscriptions

```python
import asyncio
import laminardb

async def dashboard():
    async with laminardb.open(":memory:") as conn:
        conn.execute("CREATE SOURCE metrics (ts BIGINT, service VARCHAR, latency_ms DOUBLE)")
        conn.execute("""
            CREATE STREAM slow_requests AS
            SELECT * FROM metrics WHERE latency_ms > 500.0
        """)
        conn.start()

        # Subscribe to slow requests asynchronously
        sub = await conn.subscribe_stream_async("slow_requests")

        # Simulate ingestion in a background task
        async def ingest():
            import random
            for t in range(100):
                conn.insert("metrics", {
                    "ts": t,
                    "service": random.choice(["api", "web", "worker"]),
                    "latency_ms": random.expovariate(1/300) ,  # some will be > 500ms
                })
                await asyncio.sleep(0.01)

        asyncio.create_task(ingest())

        # Process alerts as they arrive
        count = 0
        async for batch in sub:
            for row in batch.fetchall():
                ts, service, latency = row
                print(f"  Slow request: {service} @ {latency:.0f}ms")
                count += 1
            if count >= 5:
                break

        sub.cancel()
        print(f"\nDetected {count} slow requests")

asyncio.run(dashboard())
```

### Producer-consumer with background threads

```python
import threading
import time
import laminardb

conn = laminardb.open(":memory:")
conn.execute("CREATE SOURCE events (ts BIGINT, type VARCHAR, payload DOUBLE)")
conn.start()

# ── Producer thread ──
def produce():
    with conn.writer("events") as w:
        for t in range(500):
            w.insert({"ts": t, "type": f"evt_{t % 3}", "payload": float(t)})
            if t % 50 == 49:
                w.watermark(t)
        print(f"Producer done: {w.current_watermark} final watermark")

# ── Consumer thread ──
consumed = 0

def consume():
    global consumed
    sub = conn.subscribe("SELECT * FROM events")
    timeout = time.time() + 5.0
    while time.time() < timeout:
        batch = sub.try_next()
        if batch:
            consumed += batch.num_rows
    sub.cancel()

producer = threading.Thread(target=produce)
consumer = threading.Thread(target=consume)

consumer.start()
producer.start()
producer.join()
time.sleep(0.5)
consumer.join()

print(f"Consumed {consumed} events")

# Check pipeline metrics
m = conn.metrics()
print(f"Pipeline: ingested={m.total_events_ingested}, emitted={m.total_events_emitted}")
conn.close()
```

---

## Pipeline Control

```python
# Start the streaming pipeline (required before subscriptions receive data)
conn.start()

# Graceful shutdown (drains in-flight events)
conn.shutdown()

# Trigger a checkpoint (returns checkpoint ID, or None if disabled)
checkpoint_id = conn.checkpoint()

# Cancel a specific running query
conn.cancel_query(query_id)

# Check pipeline state
print(conn.pipeline_state)         # "running", "stopped", etc.
print(conn.is_checkpoint_enabled)  # True/False
```

---

## Catalog Introspection

```python
# List names
conn.list_tables()   # or conn.tables()     -> source names
conn.list_streams()  # or conn.materialized_views() -> stream names
conn.list_sinks()                            # -> sink names

# Get a source's schema
schema = conn.schema("sensors")  # -> pyarrow.Schema

# Detailed catalog info with typed objects
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
print(stats["name"])             # "sensors"
print(stats["total_events"])     # total ingested events
print(stats["pending"])          # events in buffer
print(stats["capacity"])         # buffer capacity
print(stats["is_backpressured"]) # True if buffer full
print(stats["watermark"])        # current watermark
print(stats["utilization"])      # 0.0 - 1.0
```

---

## Pipeline Metrics & Observability

Full visibility into every component of the streaming pipeline:

```python
# ── Pipeline-wide metrics ──
m = conn.metrics()
print(m.total_events_ingested)   # total rows ingested across all sources
print(m.total_events_emitted)    # total rows emitted by streams
print(m.total_events_dropped)    # dropped events (backpressure, errors)
print(m.total_cycles)            # pipeline processing cycles
print(m.total_batches)           # batches processed
print(m.uptime_secs)             # pipeline uptime in seconds
print(m.state)                   # "running", "stopped", etc.
print(m.source_count)            # registered sources
print(m.stream_count)            # registered streams
print(m.sink_count)              # registered sinks
print(m.pipeline_watermark)      # global watermark (min across sources)

# ── Per-source metrics ──
for sm in conn.all_source_metrics():
    print(f"  {sm.name}: {sm.total_events} events, "
          f"{sm.utilization:.0%} buffer, wm={sm.watermark}")

# ── Per-stream metrics ──
for stm in conn.all_stream_metrics():
    print(f"  {stm.name}: {stm.total_events} emitted, "
          f"wm={stm.watermark}, sql={stm.sql}")

# ── Pipeline topology (DAG visualization) ──
topo = conn.topology()
for node in topo.nodes:
    print(f"  [{node.node_type}] {node.name}")
    if node.sql:
        print(f"    SQL: {node.sql}")
for edge in topo.edges:
    print(f"  {edge.from_node} --> {edge.to_node}")
```

### Metrics Python wrapper

```python
from laminardb import Metrics

metrics = Metrics(conn.metrics())
print(metrics.events_per_second)   # calculated throughput
print(metrics.uptime_secs)         # pipeline uptime
print(metrics.state)               # pipeline state string
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
from laminardb import (
    Schema, Column, TableStats, Watermark,
    CheckpointStatus, Metrics, ChangeRow, ChangeEvent, MaterializedView,
)
```

| Type | Description |
|------|-------------|
| `Column(name, type, nullable)` | Frozen dataclass for column metadata |
| `Schema` | Wraps `pyarrow.Schema` with `columns`, `names`, indexing by name/position |
| `TableStats(row_count, size_bytes)` | Frozen dataclass |
| `Watermark(current, lag_ms)` | Frozen dataclass for watermark state |
| `CheckpointStatus(checkpoint_id, enabled)` | Frozen dataclass |
| `Metrics` | Wrapper with `events_per_second`, `uptime_secs`, `state` |
| `ChangeRow` | Dict-like row with `.op`, `["col"]`, `.keys()`, `.to_dict()` |
| `ChangeEvent` | Iterable batch of `ChangeRow`s with `.df()`, `.arrow()`, `.pl()` |
| `MaterializedView` | Stream wrapper with `.query()`, `.subscribe(handler=)`, `.schema()` |

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

```bash
python benchmarks/bench_ingestion.py   # ingestion throughput by format
python benchmarks/bench_query.py       # query + conversion throughput
python benchmarks/bench_streaming.py   # subscription iteration throughput
```

---

## API Aliases (DuckDB Compatibility)

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
  writer.rs                # Streaming writer with watermark support
  subscription.rs          # Sync subscription iterator
  async_support.rs         # Tokio runtime + async subscription
  stream_subscription.rs   # Named stream subscriptions (sync + async)
  execute.rs               # ExecuteResult for DDL/DML introspection
  error.rs                 # Exception hierarchy with error codes
  config.rs                # LaminarConfig
  catalog.rs               # Catalog info (SourceInfo, SinkInfo, StreamInfo, QueryInfo)
  metrics.rs               # Pipeline topology and metrics
tests/                     # pytest test suite (15 files, 233+ tests)
benchmarks/                # Performance benchmarks
examples/                  # Example scripts
docs/                      # Architecture and feature specs
```

---

## License

MIT &mdash; see [LICENSE](LICENSE) for details.
