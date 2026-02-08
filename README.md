# laminardb

[![PyPI](https://img.shields.io/pypi/v/laminardb)](https://pypi.org/project/laminardb/)
[![CI](https://github.com/laminardb/laminardb-python/actions/workflows/ci.yml/badge.svg)](https://github.com/laminardb/laminardb-python/actions/workflows/ci.yml)
[![Python](https://img.shields.io/pypi/pyversions/laminardb)](https://pypi.org/project/laminardb/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Python bindings for the [LaminarDB](https://github.com/laminardb/laminardb) streaming SQL database, built with [PyO3](https://pyo3.rs) and [Apache Arrow](https://arrow.apache.org/).

**Highlights:**

- Ingest data from dicts, pandas, polars, pyarrow, JSON, or CSV
- Query with SQL and export to pandas, polars, pyarrow, or dicts
- Stream results with continuous query subscriptions (sync and async)
- Zero-copy data exchange via Arrow PyCapsule interface
- Full PEP 561 type stubs for IDE autocompletion
- GIL release on all blocking operations for thread-safe concurrency

## Installation

```bash
pip install laminardb
```

With optional dependencies:

```bash
pip install laminardb[pandas]    # pandas + pyarrow
pip install laminardb[polars]    # polars
pip install laminardb[pyarrow]   # pyarrow only
pip install laminardb[dev]       # all dev dependencies
```

## Quick Start

```python
import laminardb

# Open a connection (currently in-memory; path reserved for future use)
with laminardb.open("mydb") as db:
    # Create a source for ingestion
    db.create_table("sensors", {
        "ts": "int64",
        "device": "string",
        "value": "float64",
    })

    # Insert data (writes to the source)
    db.insert("sensors", [
        {"ts": 1, "device": "sensor_a", "value": 42.0},
        {"ts": 2, "device": "sensor_b", "value": 43.5},
        {"ts": 3, "device": "sensor_a", "value": 44.1},
    ])

    # Query using inline SQL
    result = db.query("""
        SELECT * FROM (VALUES
            (1, 'sensor_a', 42.0),
            (2, 'sensor_b', 43.5),
            (3, 'sensor_a', 44.1)
        ) AS t(ts, device, value)
        WHERE value > 43.0
    """)
    print(result.to_dicts())
    # {'ts': [2, 3], 'device': ['sensor_b', 'sensor_a'], 'value': [43.5, 44.1]}
```

> **Note on sources vs queries**: LaminarDB uses separate catalogs for ingestion sources and query tables. Data inserted via `insert()` goes into sources. SQL queries use DataFusion's query engine — use inline `VALUES` clauses or `SELECT` expressions for ad-hoc queries. Continuous subscriptions (below) bridge the two worlds by streaming from sources.

## Core Concepts

### Opening Connections

```python
# File-based (path reserved for future persistence support)
db = laminardb.open("mydb")

# URI-based (URI reserved for future remote connections)
db = laminardb.connect("laminar://localhost:5432/mydb")

# Always use a context manager or call close()
with laminardb.open("mydb") as db:
    ...  # auto-closes on exit
```

### Creating Sources

Sources define the schema for data ingestion. Pass a dict of column names to Arrow type strings, or a `pyarrow.Schema`:

```python
# Dict schema
db.create_table("events", {
    "ts": "int64",
    "kind": "string",
    "payload": "float64",
})

# PyArrow schema
import pyarrow as pa
db.create_table("events", pa.schema([
    ("ts", pa.int64()),
    ("kind", pa.string()),
    ("payload", pa.float64()),
]))

# List all sources
print(db.list_tables())  # ['events']

# Get a source's schema
schema = db.schema("events")  # returns pyarrow.Schema
```

### Inserting Data

LaminarDB accepts 10 input formats. All go through `insert(table, data)` except JSON and CSV which have dedicated methods.

**Single dict** (one row):
```python
db.insert("sensors", {"ts": 1, "device": "sensor_a", "value": 42.0})
```

**List of dicts** (row-oriented):
```python
db.insert("sensors", [
    {"ts": 1, "device": "sensor_a", "value": 42.0},
    {"ts": 2, "device": "sensor_b", "value": 43.5},
])
```

**Columnar dict** (column-oriented):
```python
db.insert("sensors", {
    "ts": [1, 2, 3],
    "device": ["sensor_a", "sensor_b", "sensor_a"],
    "value": [42.0, 43.5, 44.1],
})
```

**pandas DataFrame**:
```python
import pandas as pd
df = pd.DataFrame({"ts": [1, 2], "device": ["a", "b"], "value": [1.0, 2.0]})
db.insert("sensors", df)
```

**polars DataFrame**:
```python
import polars as pl
df = pl.DataFrame({"ts": [1, 2], "device": ["a", "b"], "value": [1.0, 2.0]})
db.insert("sensors", df)
```

**pyarrow RecordBatch**:
```python
import pyarrow as pa
batch = pa.record_batch({"ts": [1, 2], "device": ["a", "b"], "value": [1.0, 2.0]})
db.insert("sensors", batch)
```

**pyarrow Table**:
```python
table = pa.table({"ts": [1, 2], "device": ["a", "b"], "value": [1.0, 2.0]})
db.insert("sensors", table)
```

**JSON string**:
```python
db.insert_json("sensors", '[{"ts": 1, "device": "a", "value": 1.0}]')
```

**CSV string**:
```python
db.insert_csv("sensors", "ts,device,value\n1,a,1.0\n2,b,2.0\n")
```

**Arrow PyCapsule** (any object implementing `__arrow_c_stream__`):
```python
db.insert("sensors", arrow_capsule_object)  # zero-copy
```

All `insert()` calls return the number of rows inserted.

### Querying with SQL

```python
result = db.query("""
    SELECT * FROM (VALUES
        (1, 'a', 42.0),
        (2, 'b', 43.5)
    ) AS t(ts, device, value)
    WHERE value > 42.0
""")

print(result.num_rows)     # 1
print(result.num_columns)  # 3
print(result.columns)      # ['ts', 'device', 'value']
```

### Exporting Results

Every `QueryResult` supports 6 output formats:

```python
result = db.query("SELECT 1 AS a, 2 AS b, 3 AS c")

# PyArrow Table
table = result.to_arrow()

# pandas DataFrame
df = result.to_pandas()

# polars DataFrame
df = result.to_polars()

# Columnar dict {column_name: [values]}
data = result.to_dicts()

# Auto-detect best library (polars > pandas > pyarrow)
df = result.to_df()

# Arrow PyCapsule (zero-copy, for interop with other Arrow-aware libraries)
capsule = result.__arrow_c_stream__()
```

### Streaming Writer

For high-throughput ingestion, use a streaming writer that buffers batches:

```python
with db.writer("sensors") as w:
    w.insert({"ts": 1, "device": "a", "value": 1.0})
    w.insert({"ts": 2, "device": "b", "value": 2.0})
    w.flush()  # explicit flush
    w.insert({"ts": 3, "device": "c", "value": 3.0})
    # auto-flush on context exit
```

### Subscriptions

Subscribe to continuous queries for real-time streaming results.

**Synchronous** (iterator protocol):
```python
sub = db.subscribe("SELECT * FROM sensors")
assert sub.is_active

for result in sub:
    df = result.to_pandas()
    process(df)

# Or non-blocking poll
result = sub.try_next()  # returns None if no data yet

sub.cancel()  # safe to call multiple times
```

**Asynchronous** (async iterator protocol):
```python
sub = await db.subscribe_async("SELECT * FROM sensors")

async for result in sub:
    df = result.to_pandas()
    await process(df)

sub.cancel()
```

### Raw SQL Execution

For DDL and DML statements that don't return result sets:

```python
rows_affected = db.execute("CREATE SOURCE metrics (ts INT, value DOUBLE)")
rows_affected = db.execute("DROP SOURCE IF EXISTS old_table")
```

### Stream Query Results

For large result sets, stream results in batches:

```python
for batch in db.stream("SELECT * FROM (VALUES (1), (2), (3)) AS t(x)"):
    print(batch.num_rows)
```

## Error Handling

LaminarDB provides a structured exception hierarchy:

```python
import laminardb

try:
    db.query("INVALID SQL")
except laminardb.QueryError as e:
    print(f"Query failed: {e}")
except laminardb.ConnectionError as e:
    print(f"Connection issue: {e}")
except laminardb.LaminarError as e:
    print(f"LaminarDB error: {e}")
```

| Exception | Raised When |
|-----------|------------|
| `LaminarError` | Base class for all LaminarDB errors |
| `ConnectionError` | Connection cannot be established or is lost |
| `QueryError` | SQL query fails (syntax error, missing table, etc.) |
| `IngestionError` | Data insertion fails (type mismatch, invalid format) |
| `SchemaError` | Schema operation fails (table exists, invalid schema) |
| `SubscriptionError` | Subscription operation fails |

## Type Safety

LaminarDB ships with PEP 561 type stubs (`py.typed` marker), providing full autocompletion and type checking in IDEs. Run strict type checking with:

```bash
mypy python/laminardb --strict
```

## Performance

- **Zero-copy exports**: Arrow PyCapsule interface (`__arrow_c_stream__`) avoids serialization
- **GIL release**: Every blocking Rust call releases the GIL via `py.allow_threads()`
- **Free-threaded Python**: All internal types are `Send + Sync`, ready for Python 3.13t/3.14t
- **Batch buffering**: Streaming writer accumulates batches for efficient bulk inserts
- **LTO builds**: Release builds use link-time optimization and symbol stripping

## API Gaps (Planned)

The following laminar-db Rust API features are not yet exposed in Python:

| Feature | Rust API | Status |
|---------|----------|--------|
| Config customization | `Connection::open_with_config()` | Planned |
| Stream/sink listing | `list_streams()`, `list_sinks()` | Planned |
| Pipeline control | `start()`, `checkpoint()` | Planned |
| Writer watermarks | `watermark()`, `current_watermark()` | Planned |
| Subscription timeout | `next_timeout(Duration)` | Planned |
| Connection status | `is_closed()` | Planned |
| Numeric error codes | `ApiError::code()` | Planned |

See [docs/FEATURES.md](docs/FEATURES.md) for the full feature tracking index.

## Contributing

```bash
# Build and install in development mode
maturin develop --extras dev

# Run tests
pytest tests/ -v

# Rust checks
cargo fmt --check
cargo clippy -- -D warnings

# Type checking
mypy python/laminardb --strict
```

## License

MIT — see [LICENSE](LICENSE) for details.
