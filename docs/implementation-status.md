# Implementation Status

Single source-of-truth mapping every Rust feature to its Python binding status.

**Current version**: 0.15.0

---

## Core API Surface

### Connection Management

| Rust API | Python Method | Status |
|----------|---------------|--------|
| `Connection::open()` | `laminardb.open(path)` | Implemented |
| `Connection::open_with_config()` | `laminardb.open(path, config=...)` | Implemented |
| `Connection::open()` (via URI) | `laminardb.connect(uri)` | Implemented |
| Context manager | `with laminardb.open(...) as conn:` | Implemented |
| Async context manager | `async with laminardb.open(...) as conn:` | Implemented |
| `Connection::close()` | `conn.close()` | Implemented |

### Data Ingestion

| Rust API | Python Method | Status |
|----------|---------------|--------|
| `Connection::insert()` | `conn.insert(table, data)` | Implemented |
| JSON string ingestion | `conn.insert_json(table, json_str)` | Implemented |
| CSV string ingestion | `conn.insert_csv(table, csv_str)` | Implemented |
| Streaming writer | `conn.writer(table)` | Implemented |
| Writer insert | `writer.insert(data)` | Implemented |
| Writer flush | `writer.flush()` | Implemented |
| Writer watermark | `writer.watermark(ts)` | Implemented |
| Writer close | `writer.close()` | Implemented |

### Input Format Support (10 formats)

| Format | Python Type | Status |
|--------|-------------|--------|
| Single row dict | `dict` (scalar values) | Implemented |
| Columnar dict | `dict` (list values) | Implemented |
| Row-oriented list | `list[dict]` | Implemented |
| pandas DataFrame | `pandas.DataFrame` | Implemented |
| polars DataFrame | `polars.DataFrame` | Implemented |
| PyArrow RecordBatch | `pyarrow.RecordBatch` | Implemented |
| PyArrow Table | `pyarrow.Table` | Implemented |
| JSON string | via `insert_json()` | Implemented |
| CSV string | via `insert_csv()` | Implemented |
| Arrow PyCapsule | `__arrow_c_stream__` (zero-copy) | Implemented |

### Schema Operations

| Rust API | Python Method | Status |
|----------|---------------|--------|
| Create table (dict schema) | `conn.create_table(name, dict)` | Implemented |
| Create table (PyArrow schema) | `conn.create_table(name, pa.schema)` | Implemented |
| Get schema | `conn.schema(table)` | Implemented |
| List tables | `conn.list_tables()` | Implemented |
| List streams | `conn.list_streams()` | Implemented |
| List sinks | `conn.list_sinks()` | Implemented |
| Execute DDL/DML | `conn.execute(sql)` | Implemented |

### Querying

| Rust API | Python Method | Status |
|----------|---------------|--------|
| SQL query | `conn.query(sql)` | Implemented |
| SQL query (alias) | `conn.sql(sql)` | Implemented |
| Stream results | `conn.stream(sql)` | Implemented |
| Explain plan | `conn.explain(sql)` | Implemented |
| Table statistics | `conn.stats(table)` | Implemented |
| Query stream/view | `conn.query_stream(name, where)` | Implemented |
| Cancel query | `conn.cancel_query(query_id)` | Implemented |

### Query Result Export (6 formats)

| Method | Output Type | Status |
|--------|-------------|--------|
| `to_arrow()` / `arrow()` | `pyarrow.Table` | Implemented |
| `to_pandas()` / `df()` | `pandas.DataFrame` | Implemented |
| `to_polars()` / `pl()` | `polars.DataFrame` | Implemented |
| `to_dicts()` | `dict[str, list]` | Implemented |
| `to_df()` | Auto-detect best library | Implemented |
| `__arrow_c_stream__` | Arrow PyCapsule (zero-copy) | Implemented |
| `fetchall()` | `list[tuple]` | Implemented |
| `fetchone()` | `tuple \| None` | Implemented |
| `fetchmany(n)` | `list[tuple]` | Implemented |
| `show(max_rows)` | Terminal preview | Implemented |
| `_repr_html_()` | Jupyter HTML rendering | Implemented |
| `__iter__` / `__len__` | Row iteration, length | Implemented |

### Subscriptions

| Rust API | Python Method | Status |
|----------|---------------|--------|
| Sync subscription | `conn.subscribe(sql)` | Implemented |
| Async subscription | `await conn.subscribe_async(sql)` | Implemented |
| Named stream (sync) | `conn.subscribe_stream(name)` | Implemented |
| Named stream (async) | `await conn.subscribe_stream_async(name)` | Implemented |
| Non-blocking poll | `sub.try_next()` | Implemented |
| Timeout poll | `sub.next_timeout(ms)` | Implemented |
| Blocking next | `sub.next()` | Implemented |
| Cancel | `sub.cancel()` | Implemented |
| Iterator protocol | `for batch in sub:` | Implemented |
| Async iterator protocol | `async for batch in sub:` | Implemented |

### Pipeline Control

| Rust API | Python Method | Status |
|----------|---------------|--------|
| Start pipeline | `conn.start()` | Implemented |
| Shutdown pipeline | `conn.shutdown()` | Implemented |
| Trigger checkpoint | `conn.checkpoint()` | Implemented |
| Pipeline state | `conn.pipeline_state` | Implemented |
| Pipeline watermark | `conn.pipeline_watermark` | Implemented |
| Total events | `conn.total_events_processed` | Implemented |
| Source count | `conn.source_count` | Implemented |
| Sink count | `conn.sink_count` | Implemented |
| Active queries | `conn.active_query_count` | Implemented |
| Checkpoint enabled | `conn.is_checkpoint_enabled` | Implemented |
| Connection closed | `conn.is_closed` | Implemented |

### Catalog Introspection

| Rust API | Python Method | Status |
|----------|---------------|--------|
| Source info | `conn.sources()` | Implemented |
| Sink info | `conn.sinks()` | Implemented |
| Stream info | `conn.streams()` | Implemented |
| Query info | `conn.queries()` | Implemented |

### Pipeline Metrics & Observability

| Rust API | Python Method | Status |
|----------|---------------|--------|
| Pipeline metrics | `conn.metrics()` | Implemented |
| Source metrics | `conn.source_metrics(name)` | Implemented |
| All source metrics | `conn.all_source_metrics()` | Implemented |
| Stream metrics | `conn.stream_metrics(name)` | Implemented |
| All stream metrics | `conn.all_stream_metrics()` | Implemented |
| Pipeline topology (DAG) | `conn.topology()` | Implemented |

### DuckDB-Style Aliases

| DuckDB Style | Equivalent | Status |
|--------------|-----------|--------|
| `conn.sql(query)` | `conn.query(query)` | Implemented |
| `conn.tables()` | `conn.list_tables()` | Implemented |
| `conn.materialized_views()` | `conn.list_streams()` | Implemented |
| `result.df()` | `result.to_pandas()` | Implemented |
| `result.pl()` | `result.to_polars()` | Implemented |
| `result.arrow()` | `result.to_arrow()` | Implemented |
| `Config` | `LaminarConfig` | Implemented |
| `BatchWriter` | `Writer` | Implemented |

---

## Connector Parity

Connectors are used via SQL DDL from Python. The Python bindings expose all connectors enabled at compile time in `Cargo.toml`.

| Connector | Source | Sink | Feature Flag | Python Status |
|-----------|--------|------|-------------|---------------|
| Kafka | yes | yes | `kafka` | Available via SQL DDL |
| WebSocket | yes | yes | `websocket` | Available via SQL DDL |
| PostgreSQL CDC | yes | -- | `postgres-cdc` | Available via SQL DDL |
| PostgreSQL Sink | -- | yes | `postgres-sink` | Available via SQL DDL |
| MySQL CDC | yes | -- | `mysql-cdc` | Available via SQL DDL |
| Delta Lake | yes | yes | `delta-lake` | Available via SQL DDL |
| Apache Iceberg | -- | yes | `iceberg` | Available via SQL DDL |
| Parquet Lookup | yes | -- | `parquet-lookup` | Available via SQL DDL |

---

## Python-Only Features

| Feature | Module | Status |
|---------|--------|--------|
| `MaterializedView` wrapper | `types.py` | Implemented |
| `ChangeEvent` / `ChangeRow` CDC types | `types.py` | Implemented |
| `Schema` / `Column` wrappers | `types.py` | Implemented |
| `Metrics` wrapper | `types.py` | Implemented |
| `TableStats` dataclass | `types.py` | Implemented |
| `Watermark` dataclass | `types.py` | Implemented |
| `CheckpointStatus` dataclass | `types.py` | Implemented |
| Module-level `sql()` / `execute()` | `__init__.py` | Implemented |
| Module-level `mv()` | `__init__.py` | Implemented |
| PEP 561 type stubs | `_laminardb.pyi` | Implemented |

---

## Rust-Only Features (Not Exposed to Python)

These features exist in the `laminar-db` or `laminar-core` Rust crates but are not surfaced in the Python bindings.

| Feature | Why Not Exposed |
|---------|----------------|
| `LaminarDB::builder()` API | Python uses `open()` + `LaminarConfig` instead |
| Typed subscriptions (`subscribe::<T>()`) | Python is dynamically typed; uses Arrow-based results |
| `#[derive(Record)]` / `#[derive(FromRow)]` macros | Rust compile-time; Python uses dict/DataFrame ingestion |
| UDF registration | Not yet implemented in Python bindings |
| Deployment profiles | Rust-only configuration; builder API pattern |
| Distributed delta mode | Feature-gated in Rust; not exposed to Python |
| JIT compilation controls | Enabled at compile time; no Python-level knobs |
| Connector SDK (custom connectors) | Rust trait-based; Python users use SQL DDL |
| Ring 0/1/2 architecture controls | Internal engine detail |
| WAL/checkpoint low-level config | Exposed partially via `LaminarConfig` |

---

## Error Code Coverage

All 17 error codes from `laminar_db::api::codes` are registered in the Python `laminardb.codes` module:

| Code | Constant | Python Exception |
|------|----------|-----------------|
| 100 | `CONNECTION_FAILED` | `ConnectionError` |
| 101 | `CONNECTION_CLOSED` | `ConnectionError` |
| 102 | `CONNECTION_IN_USE` | `ConnectionError` |
| 200 | `TABLE_NOT_FOUND` | `SchemaError` |
| 201 | `TABLE_EXISTS` | `SchemaError` |
| 202 | `SCHEMA_MISMATCH` | `SchemaError` |
| 203 | `INVALID_SCHEMA` | `SchemaError` |
| 300 | `INGESTION_FAILED` | `IngestionError` |
| 301 | `WRITER_CLOSED` | `IngestionError` |
| 302 | `BATCH_SCHEMA_MISMATCH` | `IngestionError` |
| 400 | `QUERY_FAILED` | `QueryError` |
| 401 | `SQL_PARSE_ERROR` | `QueryError` |
| 402 | `QUERY_CANCELLED` | `QueryError` |
| 500 | `SUBSCRIPTION_FAILED` | `SubscriptionError` |
| 501 | `SUBSCRIPTION_CLOSED` | `SubscriptionError` |
| 502 | `SUBSCRIPTION_TIMEOUT` | `SubscriptionError` |
| 900 | `INTERNAL_ERROR` | `LaminarError` |
| 901 | `SHUTDOWN` | `LaminarError` |

---

## Exception Hierarchy

```
LaminarError (base)
├── ConnectionError     (100-199)
├── QueryError          (400-499)
├── IngestionError      (300-399)
├── SchemaError         (200-299)
├── SubscriptionError   (500-599)
├── StreamError
├── CheckpointError
└── ConnectorError
```

---

## Rust Module Index

| Module | Responsibility |
|--------|---------------|
| `lib.rs` | Module entry point, `open()`/`connect()`, error codes submodule |
| `connection.rs` | `Connection` pyclass (40+ methods), DuckDB aliases |
| `query.rs` | `QueryResult` pyclass with multi-format export |
| `conversion.rs` | Python <-> Arrow type conversions (10 input formats) |
| `writer.rs` | `Writer` pyclass for streaming inserts with watermarks |
| `subscription.rs` | `Subscription` pyclass for continuous queries (sync) |
| `async_support.rs` | Tokio runtime, `AsyncSubscription` for asyncio |
| `stream_subscription.rs` | `StreamSubscription` + `AsyncStreamSubscription` |
| `execute.rs` | `ExecuteResult` for DDL/DML introspection |
| `error.rs` | Exception hierarchy, core error -> Python error mapping |
| `config.rs` | `LaminarConfig` pyclass |
| `catalog.rs` | `SourceInfo`, `SinkInfo`, `StreamInfo`, `QueryInfo` |
| `metrics.rs` | `PipelineMetrics`, `PipelineTopology`, source/stream metrics |

---

## Phase Completion Summary

| Area | Python Coverage |
|------|----------------|
| Core API (Connection methods) | 40+ methods, 100% of Rust `api` module |
| Input formats | 10/10 |
| Output formats | 6/6 + fetch methods |
| Connectors (via SQL DDL) | 8/8 compiled connectors |
| Error codes | 17/17 |
| Exception types | 9/9 |
| Subscription types | 4/4 (sync, async, stream sync, stream async) |
| Pipeline observability | 100% (metrics, topology, source/stream metrics) |
| Type stubs (PEP 561) | 800+ lines, passes `mypy --strict` |
| Free-threaded Python | All pyclasses `Send + Sync` |
