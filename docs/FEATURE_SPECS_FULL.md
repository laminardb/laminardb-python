# LaminarDB Python Bindings -- Feature Specifications

> Generated: 2026-02-22
> Covers features F-PY-001 through F-PY-010 for the `laminardb` Python package.
> Based on the actual codebase implementation in Rust (PyO3 0.27) and Python.

---

# F-PY-001: Connection & Database Management

## Metadata
- **Phase**: 1 -- Core
- **Priority**: P0
- **Dependencies**: `laminar_db::api::Connection`, PyO3, `pyo3-async-runtimes`
- **Main Repo Module**: `src/lib.rs`, `src/connection.rs`, `src/config.rs`
- **Implementation Status**: Implemented

## Summary

Provides the primary entry points for opening and managing LaminarDB database connections. Supports file-path-based databases, in-memory mode, URI-based connections, optional configuration via `LaminarConfig`, and both synchronous and asynchronous context manager protocols. A module-level default connection enables DuckDB-style top-level `laminardb.sql()` and `laminardb.execute()` convenience functions without explicit connection management.

## Python API

```python
import laminardb

# Module-level entry points (Rust-backed)
conn = laminardb.open(path: str, *, config: LaminarConfig | None = None) -> Connection
conn = laminardb.connect(uri: str) -> Connection

# Configuration
config = laminardb.LaminarConfig(
    *,
    buffer_size: int = 65536,
    storage_dir: str | None = None,
    checkpoint_interval_ms: int | None = None,
    table_spill_threshold: int = 1_000_000,
)

# Connection properties
conn.is_closed -> bool
conn.is_checkpoint_enabled -> bool
conn.pipeline_state -> str
conn.pipeline_watermark -> int
conn.total_events_processed -> int
conn.source_count -> int
conn.sink_count -> int
conn.active_query_count -> int

# Connection methods
conn.start() -> None
conn.checkpoint() -> int | None
conn.close() -> None
conn.shutdown() -> None
conn.cancel_query(query_id: int) -> None
conn.explain(query: str) -> str
conn.stats(table: str) -> dict

# Context manager protocols
with laminardb.open(":memory:") as conn: ...
async with laminardb.open(":memory:") as conn: ...

# Module-level default connection (Python-side)
laminardb.sql(query: str, params=None) -> QueryResult
laminardb.execute(query: str, params=None) -> ExecuteResult

# Aliases
laminardb.Config = laminardb.LaminarConfig
```

## Usage Examples

```python
import laminardb

# In-memory database
db = laminardb.open(":memory:")

# With configuration
config = laminardb.LaminarConfig(
    buffer_size=1024,
    checkpoint_interval_ms=5000,
    storage_dir="/tmp/laminar_data",
)
db = laminardb.open("mydb", config=config)

# Context manager for automatic cleanup
with laminardb.open(":memory:") as db:
    db.execute("CREATE SOURCE events (ts TIMESTAMP, value DOUBLE)")
    db.insert("events", {"ts": 1, "value": 42.0})
    result = db.query("SELECT * FROM events")

# Async context manager
async with laminardb.open(":memory:") as db:
    db.insert("events", {"ts": 1, "value": 42.0})

# Module-level convenience (auto-creates in-memory connection)
laminardb.execute("CREATE SOURCE logs (msg VARCHAR)")
result = laminardb.sql("SELECT * FROM logs")

# URI-based connection
db = laminardb.connect("laminar://localhost:5432/mydb")

# Pipeline control
db.start()
state = db.pipeline_state
wm = db.pipeline_watermark
db.shutdown()
```

## Implementation Notes
- `open()` and `connect()` are `#[pyfunction]` in `lib.rs` that wrap `laminar_db::api::Connection::open()` and `::open_with_config()`.
- `PyConnection` wraps `Arc<Mutex<laminar_db::api::Connection>>` for thread safety.
- All blocking operations release the GIL via `py.allow_threads()`.
- A global Tokio multi-thread runtime is entered before every API call to ensure spawned background tasks execute properly.
- `__enter__` / `__exit__` implement sync context manager; `__aenter__` / `__aexit__` use `pyo3_async_runtimes::tokio::future_into_py` for async support.
- `close()` attempts `Arc::try_unwrap` for clean shutdown; if other references exist, the connection is simply marked closed.
- The module-level default connection is managed in Python (`__init__.py`) with a `threading.Lock` guard.
- `LaminarConfig` properties: `buffer_size` (default 65536), `storage_dir`, `checkpoint_interval_ms`, `table_spill_threshold` (default 1,000,000).
- `check_closed()` raises `ConnectionError` on every method call if the connection has been closed.

## Testing Requirements
- Open and close in-memory connection
- Open with custom `LaminarConfig` and verify properties
- Context manager auto-closes on exit
- Async context manager auto-closes on exit
- Operations on closed connection raise `ConnectionError`
- Module-level `sql()` and `execute()` auto-create default connection
- `pipeline_state`, `pipeline_watermark`, `total_events_processed` return valid values
- `checkpoint()` returns an ID when checkpointing is enabled
- `shutdown()` gracefully drains in-flight events
- `repr()` shows `Connection(open)` / `Connection(closed)`

---

# F-PY-002: Schema Management

## Metadata
- **Phase**: 1 -- Core
- **Priority**: P0
- **Dependencies**: `laminar_db::api::Connection`, `pyo3-arrow`, `arrow-schema`
- **Main Repo Module**: `src/connection.rs`, `src/conversion.rs`, `src/catalog.rs`
- **Implementation Status**: Implemented

## Summary

Provides methods for creating tables (sources), retrieving schemas, and listing database objects. Schemas can be specified as PyArrow Schema objects or as dictionaries mapping column names to type strings. The catalog info API exposes detailed metadata about sources, sinks, streams, and queries via dedicated info classes.

## Python API

```python
# Schema / table management
conn.create_table(name: str, schema: pyarrow.Schema | dict[str, str]) -> None
conn.schema(name: str) -> pyarrow.Schema
conn.list_tables() -> list[str]
conn.list_streams() -> list[str]
conn.list_sinks() -> list[str]

# DuckDB-style aliases
conn.tables() -> list[str]                    # alias for list_tables()
conn.materialized_views() -> list[str]        # alias for list_streams()

# Catalog info (rich metadata)
conn.sources() -> list[SourceInfo]
conn.sinks() -> list[SinkInfo]
conn.streams() -> list[StreamInfo]
conn.queries() -> list[QueryInfo]

# Info classes
class SourceInfo:
    name: str                         # Source name
    schema: pyarrow.Schema            # Arrow schema
    watermark_column: str | None      # Watermark column name

class SinkInfo:
    name: str                         # Sink name

class StreamInfo:
    name: str                         # Stream name
    sql: str | None                   # SQL definition

class QueryInfo:
    id: int                           # Query ID
    sql: str                          # SQL text
    active: bool                      # Whether actively running
```

## Usage Examples

```python
import pyarrow as pa
import laminardb

db = laminardb.open(":memory:")

# Create table with PyArrow schema
schema = pa.schema([
    ("ts", pa.timestamp("us")),
    ("symbol", pa.utf8()),
    ("price", pa.float64()),
])
db.create_table("trades", schema)

# Create table with dict schema
db.create_table("sensors", {"ts": "timestamp", "value": "float64", "id": "int32"})

# Retrieve schema
schema = db.schema("trades")
print(schema)  # ts: timestamp[us], symbol: string, price: double

# List objects
tables = db.list_tables()       # ["trades", "sensors"]
streams = db.list_streams()     # []
sinks = db.list_sinks()         # []

# Catalog info with rich metadata
for src in db.sources():
    print(f"{src.name}: {src.schema}, watermark={src.watermark_column}")

for stream in db.streams():
    print(f"{stream.name}: {stream.sql}")

for q in db.queries():
    print(f"Query {q.id}: active={q.active}, sql={q.sql}")
```

## Implementation Notes
- `create_table()` converts the Python schema to Arrow field definitions, generates `CREATE SOURCE name (col1 TYPE, ...)` DDL, and executes it.
- `schema()` first tries `conn.get_schema(name)` for sources; on failure, looks up the stream's SQL definition from catalog and executes it to extract the schema, then cancels the stream.
- Schema from dict uses `parse_type_string()` supporting: `int8/i8`, `int16/i16`, `int32/i32/int`, `int64/i64`, `uint*`, `float16/f16`, `float32/f32/float`, `float64/f64/double`, `bool/boolean`, `string/str/utf8`, `large_string/large_utf8`, `binary/bytes`, `date32/date`, `timestamp/datetime`.
- Arrow DataType to SQL mapping covers all common types with fallback to `VARCHAR`.
- `SourceInfo.schema` returns a PyArrow Schema via `pyo3-arrow`'s PyCapsule export.
- All info classes use `#[pyclass(frozen)]` for immutability and thread safety.

## Testing Requirements
- Create table with PyArrow Schema and retrieve it
- Create table with dict schema and verify round-trip
- `schema()` works for both sources and streams
- `list_tables()` / `list_streams()` / `list_sinks()` return correct lists
- Creating a duplicate table raises `SchemaError` (`TABLE_EXISTS`)
- Schema of nonexistent table raises `SchemaError` (`TABLE_NOT_FOUND`)
- `sources()` returns `SourceInfo` with correct properties
- `streams()` returns `StreamInfo` with SQL definitions
- `queries()` reflects active and completed queries
- DuckDB aliases (`tables()`, `materialized_views()`) return same results

---

# F-PY-003: Data Ingestion

## Metadata
- **Phase**: 1 -- Core
- **Priority**: P0
- **Dependencies**: `laminar_db::api::Connection`, `laminar_db::api::Writer`, `pyo3-arrow`, `arrow-json`, `arrow-csv`
- **Main Repo Module**: `src/connection.rs`, `src/conversion.rs`, `src/writer.rs`
- **Implementation Status**: Implemented

## Summary

Supports inserting data into sources in 10 different input formats, including single dicts, list of dicts, columnar dicts, Pandas DataFrames, Polars DataFrames, PyArrow RecordBatch/Table, JSON strings, CSV strings, and any object implementing the Arrow PyCapsule interface (`__arrow_c_stream__` or `__arrow_c_array__`). Also provides a streaming `Writer` for batched inserts with watermark emission.

## Python API

```python
# Direct insert (10 input formats)
conn.insert(table: str, data: Any) -> int  # returns rows inserted

# Specialized string inserts
conn.insert_json(table: str, json_str: str) -> int
conn.insert_csv(table: str, csv_str: str) -> int

# Streaming writer
writer = conn.writer(table: str) -> Writer

class Writer:
    name: str                          # Source name (property)
    schema: pyarrow.Schema             # Source schema (property)
    current_watermark: int             # Current watermark value (property)
    def insert(data: Any) -> None      # Write data (any supported format)
    def flush() -> int                 # Flush buffer, returns 0
    def watermark(timestamp: int) -> None  # Emit watermark
    def close() -> None                # Flush and close
    # Context manager support
    def __enter__() -> Writer
    def __exit__(...) -> bool          # Auto-closes on exit
```

## Usage Examples

```python
import laminardb
import pyarrow as pa

db = laminardb.open(":memory:")
db.create_table("events", {"ts": "int64", "value": "float64"})

# Single dict (one row)
db.insert("events", {"ts": 1, "value": 42.0})

# List of dicts (row-oriented)
db.insert("events", [
    {"ts": 2, "value": 43.0},
    {"ts": 3, "value": 44.0},
])

# Columnar dict
db.insert("events", {"ts": [4, 5, 6], "value": [45.0, 46.0, 47.0]})

# Pandas DataFrame
import pandas as pd
df = pd.DataFrame({"ts": [7, 8], "value": [48.0, 49.0]})
db.insert("events", df)

# Polars DataFrame
import polars as pl
df = pl.DataFrame({"ts": [9, 10], "value": [50.0, 51.0]})
db.insert("events", df)

# PyArrow RecordBatch or Table
batch = pa.record_batch({"ts": [11, 12], "value": [52.0, 53.0]})
db.insert("events", batch)

# JSON string
db.insert_json("events", '[{"ts": 13, "value": 54.0}]')
db.insert_json("events", '{"ts": 14, "value": 55.0}')  # NDJSON

# CSV string
db.insert_csv("events", "ts,value\n15,56.0\n16,57.0")

# Streaming writer with context manager
with db.writer("events") as w:
    w.insert({"ts": 20, "value": 60.0})
    w.insert({"ts": 21, "value": 61.0})
    w.watermark(21)
    print(w.current_watermark)  # 21
# auto-flushed and closed on exit
```

## Implementation Notes
- Conversion priority in `python_to_batches()`: (1) plain Python dicts/lists checked first to avoid Arrow extraction intercepting native types, (2) Arrow PyCapsule `__arrow_c_stream__`, (3) single RecordBatch `__arrow_c_array__`, (4) Pandas via `pyarrow.Table.from_pandas()`, (5) Polars via `.to_arrow()`, (6) fallback error.
- JSON strings support both NDJSON format and JSON arrays; arrays are re-serialized to NDJSON before `arrow_json::ReaderBuilder` parsing.
- CSV parsing uses `arrow_csv::ReaderBuilder` with automatic schema inference.
- Dict conversion: if values are lists it is treated as columnar; if values are scalars it is treated as a single row.
- List of dicts tries `pyarrow.Table.from_pylist()` first; falls back to NDJSON serialization via Python's `json` module.
- Writer wraps `laminar_db::api::Writer` in `Mutex<Option<...>>` for safe close semantics.
- Writer `insert()` accepts all the same input formats as `Connection.insert()`.
- Writer `close()` takes the inner `Option` to mark as closed; subsequent calls to `check_closed()` raise `IngestionError`.
- All insert operations release the GIL and enter the Tokio runtime.
- `Connection.insert()` returns total rows across all batches.

## Testing Requirements
- Insert single dict and verify row count
- Insert list of dicts and verify row count
- Insert columnar dict and verify data
- Insert Pandas DataFrame
- Insert Polars DataFrame
- Insert PyArrow RecordBatch and Table
- Insert JSON string (both array and NDJSON)
- Insert CSV string
- Insert via PyCapsule-compatible object
- Unsupported type raises `TypeError`
- Empty list raises `TypeError`
- Writer context manager auto-closes
- Writer insert, flush, watermark operations
- Writer on closed writer raises `IngestionError`
- Schema mismatch on insert raises `IngestionError` (`BATCH_SCHEMA_MISMATCH`)

---

# F-PY-004: Query Execution

## Metadata
- **Phase**: 1 -- Core
- **Priority**: P0
- **Dependencies**: `laminar_db::api::Connection`, `laminar_db::api::ExecuteResult`, `pyo3-arrow`
- **Main Repo Module**: `src/connection.rs`, `src/query.rs`, `src/execute.rs`, `src/conversion.rs`
- **Implementation Status**: Implemented

## Summary

Provides SQL query execution with materialized results, streaming iteration, and DDL/DML introspection. `QueryResult` supports zero-copy export to PyArrow, Pandas, Polars, and Python dicts, along with DBAPI-style fetch methods, Jupyter rendering, and DuckDB-style aliases. `ExecuteResult` wraps DDL, DML, and query results with type introspection and backward-compatible `int()` conversion.

## Python API

```python
# Query methods on Connection
conn.query(sql: str) -> QueryResult           # Materialized full result
conn.stream(sql: str) -> _QueryStreamIter     # Streaming batch iterator
conn.execute(sql: str) -> ExecuteResult        # DDL/DML/query introspection
conn.sql(query: str, params=None) -> QueryResult  # DuckDB alias for query()
conn.query_stream(name: str, filter: str | None = None) -> QueryResult

class QueryResult:
    # Properties
    num_rows: int
    num_columns: int
    num_batches: int
    schema: pyarrow.Schema
    columns: list[str]

    # Multi-format export
    def to_arrow() -> pyarrow.Table
    def to_pandas() -> pandas.DataFrame
    def to_polars() -> polars.DataFrame
    def to_dicts() -> dict[str, list]
    def to_df() -> Any                    # Auto-detect best library

    # DuckDB-style aliases
    def df() -> pandas.DataFrame
    def pl(*, lazy: bool = False) -> polars.DataFrame | polars.LazyFrame
    def arrow() -> pyarrow.Table

    # DBAPI-style fetch
    def fetchone() -> tuple | None
    def fetchall() -> list[tuple]
    def fetchmany(size: int = 1) -> list[tuple]

    # Display
    def show(max_rows: int = 20) -> None
    def _repr_html_() -> str              # Jupyter rendering

    # PyCapsule export
    def __arrow_c_stream__(requested_schema=None) -> PyCapsule

    # Protocol support
    def __len__() -> int
    def __iter__() -> Iterator[tuple]
    def __repr__() -> str

class _QueryStreamIter:
    is_active: bool
    def try_next() -> QueryResult | None  # Non-blocking poll
    def cancel() -> None
    def __iter__() -> _QueryStreamIter    # Blocking iteration
    def __next__() -> QueryResult         # Blocking next

class ExecuteResult:
    result_type: str          # "ddl" | "rows_affected" | "query" | "metadata"
    rows_affected: int
    ddl_type: str | None      # e.g. "CREATE SOURCE"
    ddl_object: str | None    # e.g. "sensors"
    def to_query_result() -> QueryResult | None
    def __int__() -> int      # rows affected
    def __bool__() -> bool
    def __repr__() -> str
```

## Usage Examples

```python
import laminardb

db = laminardb.open(":memory:")
db.execute("CREATE SOURCE trades (ts BIGINT, symbol VARCHAR, price DOUBLE)")
db.insert("trades", [
    {"ts": 1, "symbol": "AAPL", "price": 150.0},
    {"ts": 2, "symbol": "GOOG", "price": 2800.0},
])

# Materialized query
result = db.query("SELECT * FROM trades WHERE price > 100")
print(result.num_rows)      # 2
print(result.columns)       # ["ts", "symbol", "price"]

# Multi-format export
arrow_table = result.to_arrow()
pandas_df = result.to_pandas()
polars_df = result.to_polars()
dicts = result.to_dicts()   # {"ts": [1, 2], "symbol": ["AAPL", "GOOG"], ...}
best_df = result.to_df()    # Auto-detects polars > pandas > pyarrow

# DuckDB-style aliases
df = result.df()             # Pandas
pl_df = result.pl()          # Polars
pl_lazy = result.pl(lazy=True)  # Polars LazyFrame
arrow = result.arrow()       # PyArrow Table

# DBAPI-style fetch
first_row = result.fetchone()  # (1, "AAPL", 150.0)
all_rows = result.fetchall()   # [(1, "AAPL", 150.0), (2, "GOOG", 2800.0)]
some = result.fetchmany(1)     # [(1, "AAPL", 150.0)]

# Iteration
for row in result:
    print(row)  # tuples

# Streaming query
for batch in db.stream("SELECT * FROM trades"):
    print(batch.to_arrow())

# DDL/DML execution with introspection
result = db.execute("CREATE SOURCE logs (msg VARCHAR)")
print(result.result_type)  # "ddl"
print(result.ddl_type)     # "CREATE SOURCE"
print(result.ddl_object)   # "logs"

# Query stream by name
db.execute("CREATE STREAM avg_prices AS SELECT symbol, AVG(price) as avg FROM trades GROUP BY symbol")
result = db.query_stream("avg_prices")
result_filtered = db.query_stream("avg_prices", filter="avg > 500")

# EXPLAIN
plan = db.explain("SELECT * FROM trades")
print(plan)

# Statistics
stats = db.stats("trades")
print(stats)  # {"name": "trades", "total_events": 2, ...}
```

## Implementation Notes
- `query()` uses `conn.execute(sql)` internally and consumes the `QueryStream` with blocking `next()` to avoid the race condition in the API's `query()` method which uses non-blocking `try_next()`.
- `stream()` wraps `conn.query_stream(sql)` as a `_QueryStreamIter` Python iterator with both `__iter__/__next__` (blocking) and `try_next()` (non-blocking).
- `execute()` returns `ExecuteResult` which wraps the `laminar_db::api::ExecuteResult` enum variants: `Ddl`, `RowsAffected`, `Query`, `Metadata`.
- `QueryResult` stores `Vec<RecordBatch>` and `SchemaRef` internally; all conversions go through `pyo3-arrow`.
- `to_df()` priority: Polars > Pandas > PyArrow Table.
- `fetchall()` converts via PyArrow `to_pylist()` then transforms `list[dict]` to `list[tuple]`.
- `_repr_html_()` delegates to Pandas for rich Jupyter rendering; falls back to `<pre>` on error.
- `__arrow_c_stream__()` delegates to the PyArrow table's PyCapsule export for zero-copy interop.
- `query_stream(name, filter)` looks up the stream's SQL from catalog and optionally wraps it in a `WHERE` clause.
- `explain()` prepends `EXPLAIN` to the SQL and formats the result as text via PyArrow/Pandas.

## Testing Requirements
- `query()` returns correct `QueryResult` for SELECT statements
- All export formats produce correct data: `to_arrow()`, `to_pandas()`, `to_polars()`, `to_dicts()`, `to_df()`
- DuckDB aliases (`df()`, `pl()`, `pl(lazy=True)`, `arrow()`) work correctly
- `fetchone()` returns first row as tuple, `None` for empty result
- `fetchall()` returns all rows as list of tuples
- `fetchmany(n)` returns at most n rows
- `__len__()` matches `num_rows`
- `__iter__` yields tuples
- `__arrow_c_stream__` works for zero-copy export
- `stream()` produces iterable batches
- `execute()` returns correct `ExecuteResult` variants for DDL, DML, queries
- `ExecuteResult.__int__()` returns rows affected
- `query_stream()` works for named streams with and without filter
- `explain()` returns plan text
- Invalid SQL raises `QueryError`

---

# F-PY-005: Error Handling & Exception Hierarchy

## Metadata
- **Phase**: 1 -- Core
- **Priority**: P0
- **Dependencies**: `laminar_db::api::ApiError`, `laminar_db::api::codes`
- **Main Repo Module**: `src/error.rs`, `src/lib.rs` (codes submodule)
- **Implementation Status**: Implemented

## Summary

Defines a structured exception hierarchy rooted at `LaminarError`, with specialized subclasses for different failure categories. Each exception carries a numeric `.code` attribute corresponding to the error codes in the `laminardb.codes` module. The `IntoPyResult` trait provides ergonomic conversion from Rust `ApiError` to Python exceptions throughout the codebase.

## Python API

```python
# Exception hierarchy
class LaminarError(Exception):          # Base exception
    code: int                           # Numeric error code

class ConnectionError(LaminarError): ...
class QueryError(LaminarError): ...
class IngestionError(LaminarError): ...
class SchemaError(LaminarError): ...
class SubscriptionError(LaminarError): ...
class StreamError(LaminarError): ...
class CheckpointError(LaminarError): ...
class ConnectorError(LaminarError): ...

# Error codes module
laminardb.codes.CONNECTION_FAILED       # 1xx
laminardb.codes.CONNECTION_CLOSED
laminardb.codes.CONNECTION_IN_USE

laminardb.codes.TABLE_NOT_FOUND         # 2xx
laminardb.codes.TABLE_EXISTS
laminardb.codes.SCHEMA_MISMATCH
laminardb.codes.INVALID_SCHEMA

laminardb.codes.INGESTION_FAILED        # 3xx
laminardb.codes.WRITER_CLOSED
laminardb.codes.BATCH_SCHEMA_MISMATCH

laminardb.codes.QUERY_FAILED            # 4xx
laminardb.codes.SQL_PARSE_ERROR
laminardb.codes.QUERY_CANCELLED

laminardb.codes.SUBSCRIPTION_FAILED     # 5xx
laminardb.codes.SUBSCRIPTION_CLOSED
laminardb.codes.SUBSCRIPTION_TIMEOUT

laminardb.codes.INTERNAL_ERROR          # 9xx
laminardb.codes.SHUTDOWN
```

## Usage Examples

```python
import laminardb
from laminardb import codes

db = laminardb.open(":memory:")

# Catch specific error
try:
    db.query("INVALID SQL")
except laminardb.QueryError as e:
    print(f"Query failed with code {e.code}: {e}")

# Catch by base class
try:
    db.schema("nonexistent")
except laminardb.LaminarError as e:
    print(f"Error code: {e.code}")

# Check error codes
try:
    db.create_table("t", {"a": "int32"})
    db.create_table("t", {"a": "int32"})  # Duplicate
except laminardb.SchemaError as e:
    if e.code == codes.TABLE_EXISTS:
        print("Table already exists")

# Closed connection
db.close()
try:
    db.query("SELECT 1")
except laminardb.ConnectionError:
    print("Connection is closed")
```

## Implementation Notes
- Exception classes are created via PyO3's `create_exception!` macro with docstrings.
- `core_error_to_pyerr()` maps `ApiError` codes to the appropriate Python exception class using code ranges: 100-199 -> `ConnectionError`, 200-299 -> `SchemaError`, 300-399 -> `IngestionError`, 400-499 -> `QueryError`, 500-599 -> `SubscriptionError`, all others -> `LaminarError`.
- Each exception instance has a `.code` integer attribute set via `val.setattr("code", code)` in `Python::with_gil`.
- Error messages include both the human-readable code name and the numeric code: `"Table not found (201): ..."`.
- The `IntoPyResult<T>` trait blanket implementation converts `Result<T, ApiError>` to `PyResult<T>`, used throughout all Rust source files.
- The `codes` submodule is registered in `_laminardb` module init, exposing 18 integer constants.
- `ConnectionError` is raised directly (not via code mapping) for closed-connection checks.
- `IngestionError` is raised directly for writer-closed checks and conversion failures.

## Testing Requirements
- All exception types are subclasses of `LaminarError`
- `LaminarError` is a subclass of `Exception`
- Each error type is raised for the appropriate failure mode
- `.code` attribute is set on all mapped exceptions
- Error codes match `laminardb.codes` constants
- Error messages include code name and number
- Unknown error codes fall back to `LaminarError`
- `codes` module has all 18 constants with correct integer values
- Direct Python-side exceptions (`ConnectionError` for closed, `IngestionError` for closed writer) work correctly

---

# F-PY-006: Streaming Queries & Continuous Processing

## Metadata
- **Phase**: 2 -- Streaming
- **Priority**: P0
- **Dependencies**: `laminar_db::api::QueryStream`, `laminar_db::api::ArrowSubscription`, `pyo3-async-runtimes`
- **Main Repo Module**: `src/subscription.rs`, `src/stream_subscription.rs`, `src/async_support.rs`
- **Implementation Status**: Implemented

## Summary

Provides four subscription types for continuous query processing. `Subscription` and `AsyncSubscription` wrap arbitrary SQL streaming queries via `QueryStream`. `StreamSubscription` and `AsyncStreamSubscription` wrap named stream subscriptions via `ArrowSubscription`, with additional features like schema access, timeout-based polling, and true stream lifecycle management. All types support Python's iterator/async iterator protocols, non-blocking `try_next()`, and cancellation.

## Python API

```python
# SQL-based subscriptions (via query_stream)
sub = conn.subscribe(sql: str) -> Subscription
sub = await conn.subscribe_async(sql: str) -> AsyncSubscription

# Named stream subscriptions (via ArrowSubscription)
sub = conn.subscribe_stream(name: str) -> StreamSubscription
sub = await conn.subscribe_stream_async(name: str) -> AsyncStreamSubscription

class Subscription:
    is_active: bool
    def try_next() -> QueryResult | None    # Non-blocking
    def cancel() -> None
    def __iter__() -> Subscription
    def __next__() -> QueryResult           # Blocking, raises StopIteration

class AsyncSubscription:
    is_active: bool
    def cancel() -> None
    def __aiter__() -> AsyncSubscription
    async def __anext__() -> QueryResult    # Raises StopAsyncIteration

class StreamSubscription:
    is_active: bool
    schema: pyarrow.Schema
    def next() -> QueryResult | None                    # Blocking wait
    def next_timeout(timeout_ms: int) -> QueryResult | None  # With timeout
    def try_next() -> QueryResult | None                # Non-blocking
    def cancel() -> None
    def __iter__() -> StreamSubscription
    def __next__() -> QueryResult           # Blocking, raises StopIteration

class AsyncStreamSubscription:
    is_active: bool
    schema: pyarrow.Schema
    def cancel() -> None
    def __aiter__() -> AsyncStreamSubscription
    async def __anext__() -> QueryResult    # Raises StopAsyncIteration
```

## Usage Examples

```python
import laminardb

db = laminardb.open(":memory:")
db.create_table("trades", {"ts": "int64", "symbol": "utf8", "price": "float64"})

# SQL-based sync subscription
sub = db.subscribe("SELECT symbol, AVG(price) FROM trades GROUP BY symbol")
for batch in sub:
    print(batch.to_dicts())
    if some_condition:
        sub.cancel()
        break

# SQL-based async subscription
async for batch in await db.subscribe_async("SELECT * FROM trades"):
    df = batch.to_pandas()
    process(df)

# Named stream subscription
db.execute("CREATE STREAM avg_trades AS SELECT symbol, AVG(price) as avg FROM trades GROUP BY symbol")
db.start()

stream_sub = db.subscribe_stream("avg_trades")
print(stream_sub.schema)  # PyArrow Schema

# Blocking with timeout
batch = stream_sub.next_timeout(1000)  # Wait up to 1 second
if batch:
    print(batch.to_dicts())

# Non-blocking poll
batch = stream_sub.try_next()

# Iterate until done
for batch in stream_sub:
    print(batch.to_arrow())

stream_sub.cancel()

# Named stream async subscription
async for batch in await db.subscribe_stream_async("avg_trades"):
    print(batch.to_polars())
```

## Implementation Notes
- `Subscription` wraps `Mutex<Option<laminar_db::api::QueryStream>>`. The `Option` allows distinguishing cancelled (None) from finished (Some but not active).
- `AsyncSubscription` also wraps `QueryStream` but implements `__aiter__/__anext__` via `pyo3_async_runtimes::tokio::future_into_py`. The blocking `stream.next()` call happens under the Mutex lock before entering the async future.
- `StreamSubscription` wraps `Mutex<Option<laminar_db::api::ArrowSubscription>>` and additionally exposes `schema` property, `next()` (blocking), `next_timeout(timeout_ms)`, and `try_next()`.
- `AsyncStreamSubscription` follows the same pattern as `AsyncSubscription` but wraps `ArrowSubscription`.
- All types implement `unsafe impl Send` and `unsafe impl Sync` -- safe because the inner type is protected by `parking_lot::Mutex`.
- `cancel()` on stream subscriptions calls `sub.cancel()` and then `guard.take()` to mark as cancelled.
- `__next__` / `__anext__` check `is_active()` before attempting to fetch; raise `StopIteration` / `StopAsyncIteration` when done.
- All blocking operations release the GIL via `py.allow_threads()` and enter the Tokio runtime.
- `next_timeout()` uses `Duration::from_millis(timeout_ms)` for the underlying `sub.next_timeout(duration)` call.

## Testing Requirements
- `subscribe()` returns an iterable `Subscription`
- Sync subscription receives batches from continuous query
- `try_next()` returns `None` when no data is available (non-blocking)
- `cancel()` stops the subscription; `is_active` becomes `False`
- `subscribe_async()` works with `async for`
- `subscribe_stream()` returns `StreamSubscription` with `schema` property
- `next_timeout()` returns `None` on timeout
- `try_next()` on stream subscription is non-blocking
- `subscribe_stream_async()` works with `async for`
- Cancelled subscriptions stop yielding data
- `StopIteration` / `StopAsyncIteration` raised when stream ends
- Repr shows correct state: active, finished, cancelled

---

# F-PY-007: Window Functions

## Metadata
- **Phase**: 2 -- Streaming
- **Priority**: P1
- **Dependencies**: `laminar_db` SQL engine (DataFusion-based)
- **Main Repo Module**: Implemented in the core SQL engine, exposed via SQL pass-through
- **Implementation Status**: Implemented (via SQL)

## Summary

Supports streaming window functions via SQL syntax. Three window types are available: tumbling windows (`TUMBLE`), hopping/sliding windows (`HOP`), and session windows (`SESSION`). Windows are specified in the SQL `GROUP BY` clause alongside the time column and an `INTERVAL` expression. There is no programmatic Python API for configuring windows -- they are purely SQL-driven and accessed through the standard query/subscribe methods.

## Python API

```python
# No dedicated Python API -- window functions are used via SQL

# Tumbling window
conn.query("SELECT col, AGG(val) FROM source GROUP BY col, TUMBLE(ts_col, INTERVAL 'duration')")

# Hopping window
conn.query("SELECT col, AGG(val) FROM source GROUP BY col, HOP(ts_col, INTERVAL 'slide', INTERVAL 'size')")

# Session window
conn.query("SELECT col, AGG(val) FROM source GROUP BY col, SESSION(ts_col, INTERVAL 'gap')")

# Window functions work with all query/subscribe methods:
conn.query(sql)
conn.stream(sql)
conn.subscribe(sql)
conn.subscribe_async(sql)
conn.execute(sql)
```

## Usage Examples

```python
import laminardb

db = laminardb.open(":memory:")
db.create_table("trades", {"ts": "timestamp", "symbol": "utf8", "qty": "int64", "price": "float64"})

# Insert sample data
db.insert("trades", [
    {"ts": 1000000, "symbol": "AAPL", "qty": 100, "price": 150.0},
    {"ts": 1030000, "symbol": "AAPL", "qty": 200, "price": 151.0},
    {"ts": 1060000, "symbol": "GOOG", "qty": 50,  "price": 2800.0},
])

# Tumbling window: 1-minute aggregation
result = db.query("""
    SELECT symbol, SUM(qty) as total_qty, AVG(price) as avg_price
    FROM trades
    GROUP BY symbol, TUMBLE(ts, INTERVAL '1 minute')
""")
print(result.to_pandas())

# Hopping window: 30-second slide, 1-minute size
result = db.query("""
    SELECT symbol, COUNT(*) as trade_count
    FROM trades
    GROUP BY symbol, HOP(ts, INTERVAL '30 seconds', INTERVAL '1 minute')
""")

# Session window: 10-second gap
result = db.query("""
    SELECT symbol, SUM(qty) as session_qty
    FROM trades
    GROUP BY symbol, SESSION(ts, INTERVAL '10 seconds')
""")

# Continuous windowed subscription
sub = db.subscribe("""
    SELECT symbol, SUM(qty) as total_qty
    FROM trades
    GROUP BY symbol, TUMBLE(ts, INTERVAL '5 seconds')
""")
for batch in sub:
    print(batch.to_dicts())
    break

# Create a named stream with windowed query
db.execute("""
    CREATE STREAM trade_summaries AS
    SELECT symbol, SUM(qty) as total_qty, AVG(price) as avg_price
    FROM trades
    GROUP BY symbol, TUMBLE(ts, INTERVAL '1 minute')
""")
```

## Implementation Notes
- Window functions are implemented in the core LaminarDB SQL engine (DataFusion-based) and are exposed to Python purely through SQL pass-through.
- `TUMBLE(column, INTERVAL)` creates fixed non-overlapping time windows.
- `HOP(column, INTERVAL slide, INTERVAL size)` creates overlapping sliding windows.
- `SESSION(column, INTERVAL gap)` creates dynamic windows that close when no events arrive within the gap.
- Windows require a timestamp column and work with the watermark mechanism for determining when windows close.
- All window queries can be used with `query()` (batch), `stream()` (streaming batches), `subscribe()` (continuous sync), and `subscribe_async()` (continuous async).
- No Python-side configuration wrapper is provided; the SQL syntax is the interface.
- Window output includes `window_start` and `window_end` columns added by the engine.

## Testing Requirements
- Tumbling window query produces correct aggregations
- Hopping window query produces overlapping results
- Session window query groups events by gap
- Window queries work with `query()`, `stream()`, and `subscribe()`
- Window queries work with `CREATE STREAM` for named continuous views
- Invalid interval syntax raises `QueryError`
- Window queries work with multiple GROUP BY columns

---

# F-PY-008: Watermark Configuration

## Metadata
- **Phase**: 2 -- Streaming
- **Priority**: P1
- **Dependencies**: `laminar_db::api::Writer`, `laminar_db::api::Connection`
- **Main Repo Module**: `src/writer.rs`, `src/connection.rs`, `src/catalog.rs`
- **Implementation Status**: Partial

## Summary

Watermarks track event-time progress and trigger window computations. The Python bindings support watermark emission through the Writer class and watermark configuration through SQL DDL. Writer-level watermarks allow programmatic control over event-time advancement. Source-level watermark configuration (watermark column and strategy) is handled via SQL `CREATE SOURCE ... WITH (watermark_column = '...')` and exposed for inspection through the `SourceInfo.watermark_column` property. Pipeline-level watermark is available via `Connection.pipeline_watermark`.

## Python API

```python
# Writer-level watermark emission
writer = conn.writer("source_name")
writer.watermark(timestamp: int) -> None       # Emit watermark
writer.current_watermark -> int                # Read current watermark

# Source-level watermark configuration (SQL-only)
conn.execute("CREATE SOURCE events (ts TIMESTAMP, val DOUBLE) WITH (watermark_column = 'ts')")

# Watermark inspection
info = conn.sources()
info[0].watermark_column -> str | None         # Configured watermark column

# Pipeline-level watermark
conn.pipeline_watermark -> int                 # Global pipeline watermark

# Source-level metrics include watermark
metrics = conn.source_metrics("events")
metrics.watermark -> int                       # Current watermark for this source

# Stream-level metrics include watermark
stream_metrics = conn.stream_metrics("my_stream")
stream_metrics.watermark -> int
```

## Usage Examples

```python
import laminardb

db = laminardb.open(":memory:")

# Create source with watermark column
db.execute("CREATE SOURCE events (ts BIGINT, value DOUBLE) WITH (watermark_column = 'ts')")

# Verify watermark configuration
sources = db.sources()
for s in sources:
    print(f"{s.name}: watermark_column={s.watermark_column}")
    # events: watermark_column=ts

# Programmatic watermark via Writer
with db.writer("events") as w:
    w.insert({"ts": 1000, "value": 42.0})
    w.insert({"ts": 2000, "value": 43.0})

    # Advance watermark to indicate all events up to ts=2000 have been seen
    w.watermark(2000)
    print(w.current_watermark)  # 2000

    w.insert({"ts": 3000, "value": 44.0})
    w.watermark(3000)

# Check pipeline watermark
print(db.pipeline_watermark)

# Check source-level watermark via metrics
metrics = db.source_metrics("events")
if metrics:
    print(f"Source watermark: {metrics.watermark}")
```

## Implementation Notes
- `Writer.watermark(timestamp)` calls `guard.as_ref().unwrap().watermark(timestamp)` on the underlying `laminar_db::api::Writer`. This is a non-blocking call that does not require `py.allow_threads()`.
- `Writer.current_watermark` reads the current watermark value from the writer.
- Watermark column configuration is SQL-based via `WITH (watermark_column = '...')` in CREATE SOURCE DDL.
- `SourceInfo.watermark_column` exposes the configured watermark column name, or `None` if not configured.
- `Connection.pipeline_watermark` returns the global pipeline watermark as `i64`.
- `SourceMetrics.watermark` and `StreamMetrics.watermark` return per-source and per-stream watermark values.
- There is no programmatic Python API for setting watermark strategies (e.g., `set_watermark(stream, strategy)`); all configuration is through SQL.
- Watermarks drive window function evaluation: windows close when the watermark advances past the window end time.

## Testing Requirements
- Writer `watermark()` sets the watermark value
- Writer `current_watermark` reflects the last emitted watermark
- `SourceInfo.watermark_column` returns configured column
- `SourceInfo.watermark_column` returns `None` when not configured
- `pipeline_watermark` returns a valid integer
- `source_metrics().watermark` matches expected value after emission
- Watermark advances trigger window computation
- Watermark on closed writer raises `IngestionError`

---

# F-PY-009: Emit Strategies

## Metadata
- **Phase**: 2 -- Streaming
- **Priority**: P2
- **Dependencies**: `laminar_db` SQL engine
- **Main Repo Module**: Core SQL engine (no dedicated Python module)
- **Implementation Status**: Partial

## Summary

Emit strategies control when streaming query results are materialized and delivered to subscribers. Strategies are configured via the SQL `EMIT` clause in streaming queries. The Python bindings provide no dedicated programmatic API for configuring emit strategies -- they are purely SQL-driven. Currently, the available strategies include watermark-triggered emission (default for windowed queries) and change-triggered emission. The strategy affects the timing and frequency of result batches delivered through subscriptions and streaming queries.

## Python API

```python
# No dedicated Python API -- emit strategies are configured via SQL

# Watermark-triggered emission (default for windowed queries)
conn.subscribe("""
    SELECT symbol, SUM(qty)
    FROM trades
    GROUP BY symbol, TUMBLE(ts, INTERVAL '1 minute')
""")

# EMIT clause in SQL (when supported by the engine)
conn.subscribe("""
    SELECT symbol, SUM(qty)
    FROM trades
    GROUP BY symbol, TUMBLE(ts, INTERVAL '1 minute')
    EMIT CHANGES
""")

# Emit strategies are observable through subscription behavior:
# - Batches arrive at different frequencies depending on the strategy
# - Window-based queries emit when the watermark advances past window boundaries
# - Non-windowed queries may emit on every change
```

## Usage Examples

```python
import laminardb

db = laminardb.open(":memory:")
db.create_table("events", {"ts": "int64", "category": "utf8", "value": "float64"})

# Default emit: watermark-driven for windowed queries
sub = db.subscribe("""
    SELECT category, AVG(value) as avg_val
    FROM events
    GROUP BY category, TUMBLE(ts, INTERVAL '10 seconds')
""")

# Insert data and advance watermark via writer
with db.writer("events") as w:
    w.insert({"ts": 1, "category": "A", "value": 10.0})
    w.insert({"ts": 5, "category": "A", "value": 20.0})
    w.watermark(10)  # Triggers window emission

# Check for emitted results
batch = sub.try_next()
if batch:
    print(batch.to_dicts())

# Non-windowed continuous query emits on each change
sub2 = db.subscribe("SELECT * FROM events WHERE value > 5")
db.insert("events", {"ts": 11, "category": "B", "value": 30.0})

batch = sub2.try_next()
if batch:
    print(batch.to_pandas())

sub.cancel()
sub2.cancel()
```

## Implementation Notes
- Emit strategies are implemented in the core LaminarDB SQL engine and are not directly configurable through Python API methods.
- The default emit strategy for windowed queries is watermark-driven: results are emitted when the watermark advances past window boundaries.
- Non-windowed streaming queries emit on data changes (each new batch of inserts may produce output).
- The `EMIT` SQL clause syntax (e.g., `EMIT CHANGES`, `EMIT AFTER WATERMARK`) depends on the core engine's support level.
- Python bindings observe emit behavior through the timing and content of batches received from `subscribe()`, `subscribe_async()`, `subscribe_stream()`, and `subscribe_stream_async()`.
- No `EmitStrategy` Python class or `set_emit_strategy()` method exists.
- Future versions may add a programmatic Python API for configuring emit strategies.

## Testing Requirements
- Windowed query emits results when watermark advances past window end
- Non-windowed query emits results on data insertion
- Multiple windows produce multiple emission batches
- Empty windows do not produce output
- Subscription receives batches with correct timing based on emit strategy
- EMIT clause in SQL (if supported) changes emission behavior

---

# F-PY-010: Materialized Views

## Metadata
- **Phase**: 3 -- High-Level
- **Priority**: P1
- **Dependencies**: `Connection`, `StreamSubscription`, `QueryResult`, `Schema` (types.py)
- **Main Repo Module**: `python/laminardb/types.py`, `python/laminardb/__init__.py`
- **Implementation Status**: Partial

## Summary

Provides a high-level Python wrapper class `MaterializedView` for working with named streams as materialized views. Created via `laminardb.mv(conn, name, sql_def)` or directly. Supports querying the current state of the view (by re-executing its SQL definition against source data), retrieving its schema, and subscribing to changes with an optional callback handler that runs in a background daemon thread. Materialized views are created via SQL `CREATE STREAM ... AS SELECT ...` and the `MaterializedView` class is a convenience wrapper, not a Rust-level API.

## Python API

```python
# Factory function
view = laminardb.mv(conn: Connection, name: str, sql_def: str | None = None) -> MaterializedView

# Direct construction
view = MaterializedView(conn: Connection, name: str, sql: str | None = None)

class MaterializedView:
    name: str                           # Stream/view name (property)
    sql: str | None                     # SQL definition (property)

    def query(where: str = "") -> QueryResult
        # Re-executes stream SQL as snapshot query
        # Optional WHERE clause (without WHERE keyword)

    def schema() -> Schema
        # Returns Schema wrapper around PyArrow Schema

    def subscribe(handler: Callable[[ChangeEvent], None] | None = None) -> StreamSubscription | Thread
        # If handler is None: returns raw StreamSubscription
        # If handler provided: starts background daemon thread
```

## Usage Examples

```python
import laminardb

db = laminardb.open(":memory:")
db.create_table("trades", {"ts": "int64", "symbol": "utf8", "price": "float64"})

# Create the stream via SQL
db.execute("""
    CREATE STREAM trade_avg AS
    SELECT symbol, AVG(price) as avg_price
    FROM trades
    GROUP BY symbol
""")

# Create MaterializedView wrapper
view = laminardb.mv(db, "trade_avg")

# Or construct directly
view = laminardb.MaterializedView(db, "trade_avg")

# Insert data
db.insert("trades", [
    {"ts": 1, "symbol": "AAPL", "price": 150.0},
    {"ts": 2, "symbol": "GOOG", "price": 2800.0},
    {"ts": 3, "symbol": "AAPL", "price": 155.0},
])

# Query current state (re-executes the stream's SQL definition)
result = view.query()
print(result.to_pandas())

# Query with filter
result = view.query(where="avg_price > 200")
print(result.to_dicts())

# Get schema
schema = view.schema()
print(schema)          # Schema(symbol: string, avg_price: double)
print(schema.names)    # ["symbol", "avg_price"]
print(schema.columns)  # [Column(name='symbol', ...), Column(name='avg_price', ...)]
print(len(schema))     # 2

# Subscribe without handler (raw subscription)
sub = view.subscribe()
for batch in sub:
    print(batch.to_arrow())
    break
sub.cancel()

# Subscribe with handler (background thread)
def on_change(event: laminardb.ChangeEvent):
    print(f"Received {len(event)} rows")
    df = event.df()         # Pandas DataFrame
    arrow = event.arrow()   # PyArrow Table
    for row in event:
        print(f"  {row.op}: {row.to_dict()}")

thread = view.subscribe(handler=on_change)
# thread is a daemon Thread -- will stop when main thread exits

# ChangeEvent / ChangeRow usage
event = laminardb.ChangeEvent(query_result, op="Insert")
print(len(event))      # Number of rows
for row in event:
    print(row.op)       # "Insert"
    print(row["symbol"])  # Column access via __getitem__
    print(row.keys())   # Column names
    print(row.values()) # Column values
    print(row.to_dict())  # Row as dict
```

## Implementation Notes
- `MaterializedView` is a pure-Python class in `types.py` -- it does not wrap any Rust type.
- `query(where)` delegates to `conn.query_stream(name, filter)` which looks up the stream's SQL definition from the catalog and re-executes it, optionally wrapping in a `WHERE` clause.
- `schema()` calls `conn.schema(name)` and wraps the result in the `Schema` convenience class.
- `subscribe(handler)` calls `conn.subscribe_stream(name)` to get a `StreamSubscription`. If a handler is provided, it wraps iteration in a daemon `threading.Thread`.
- `ChangeEvent` lazily materializes `ChangeRow` objects from a `QueryResult` via `_materialize()`.
- `ChangeEvent` supports `df()`, `pl()`, and `arrow()` for direct conversion.
- `ChangeRow` provides dict-like access via `__getitem__`, `keys()`, `values()`, and `to_dict()`.
- `Schema` wraps a PyArrow Schema with `columns` (list of `Column` dataclasses), `names`, `__len__`, and `__getitem__`.
- The `laminardb.mv()` factory function in `__init__.py` is a thin wrapper around `MaterializedView(conn, name, sql_def)`.
- No Rust-level `create_materialized_view()` method exists -- creation is via `conn.execute("CREATE STREAM ... AS SELECT ...")`.
- The `MaterializedView` class does not manage the lifecycle of the underlying stream; dropping the view does not drop the stream.

## Testing Requirements
- `MaterializedView` can be constructed via `laminardb.mv()` and directly
- `view.name` and `view.sql` return correct values
- `view.query()` re-executes stream SQL and returns results
- `view.query(where="...")` applies filter correctly
- `view.schema()` returns correct `Schema` with `columns`, `names`, `__len__`, `__getitem__`
- `view.subscribe()` without handler returns `StreamSubscription`
- `view.subscribe(handler=fn)` starts a daemon thread
- `ChangeEvent` correctly wraps `QueryResult` with lazy materialization
- `ChangeEvent.df()`, `.pl()`, `.arrow()` produce correct output
- `ChangeRow` supports `__getitem__`, `keys()`, `values()`, `to_dict()`
- Query on nonexistent stream raises `QueryError`
- `Schema` convenience methods work: indexing by int and string
