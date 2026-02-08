# Feature Index

Comprehensive tracker of every binding capability. **50 Done, 11 Planned**.

## Connection & Lifecycle

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `open(path)` | Done | `lib.rs` | `open(path)` | `test_connection.py` |
| `connect(uri)` | Done | `lib.rs` | `connect(uri)` | `test_connection.py` |
| Context manager | Done | `connection.rs` | `__enter__`/`__exit__` | `test_connection.py` |
| `close()` | Done | `connection.rs` | `close()` | `test_connection.py` |
| `__repr__` | Done | `connection.rs` | `repr(conn)` | `test_connection.py` |
| `execute(sql)` | Done | `connection.rs` | `execute(sql)` | `test_connection.py` |
| `is_closed` property | Planned | — | — | — |
| `open_with_config()` | Planned | — | — | — |
| File-based persistence | Planned | — | — | — |
| Remote connections | Planned | — | — | — |

> **Note**: `open(path)` and `connect(uri)` currently open in-memory databases (path/URI parameters are accepted but unused in v0.1.0).

## Schema & Source Management

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `create_table(name, schema)` | Done | `connection.rs` | `create_table(name, schema)` | `test_connection.py` |
| `list_tables()` | Done | `connection.rs` | `list_tables()` | `test_connection.py` |
| `schema(table)` | Done | `connection.rs` | `schema(table)` | `test_connection.py` |
| `execute(sql)` | Done | `connection.rs` | `execute(sql)` | `test_connection.py` |
| `list_streams()` | Planned | — | — | — |
| `list_sinks()` | Planned | — | — | — |

## Data Ingestion

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| Insert single dict | Done | `conversion.rs` | `insert(table, dict)` | `test_ingestion.py` |
| Insert list of dicts | Done | `conversion.rs` | `insert(table, [dicts])` | `test_ingestion.py` |
| Insert columnar dict | Done | `conversion.rs` | `insert(table, {col: [vals]})` | `test_ingestion.py` |
| Insert pandas DataFrame | Done | `conversion.rs` | `insert(table, df)` | `test_ingestion.py` |
| Insert polars DataFrame | Done | `conversion.rs` | `insert(table, pl_df)` | `test_ingestion.py` |
| Insert pyarrow RecordBatch | Done | `conversion.rs` | `insert(table, batch)` | `test_ingestion.py` |
| Insert pyarrow Table | Done | `conversion.rs` | `insert(table, table)` | `test_ingestion.py` |
| Insert JSON string | Done | `conversion.rs` | `insert_json(table, json)` | `test_ingestion.py` |
| Insert CSV string | Done | `conversion.rs` | `insert_csv(table, csv)` | `test_ingestion.py` |
| Arrow PyCapsule input | Done | `conversion.rs` | `__arrow_c_stream__` | `test_ingestion.py` |
| Streaming writer | Done | `writer.rs` | `writer(table)` | `test_writer.py` |
| Writer batch buffering | Done | `writer.rs` | auto-flush on context exit | `test_writer.py` |

## Query & Results

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| SQL query | Done | `connection.rs` | `query(sql)` | `test_query.py` |
| Stream query | Done | `connection.rs` | `stream(sql)` | `test_query.py` |
| `to_arrow()` | Done | `query.rs` | `result.to_arrow()` | `test_query.py` |
| `to_pandas()` | Done | `query.rs` | `result.to_pandas()` | `test_query.py` |
| `to_polars()` | Done | `query.rs` | `result.to_polars()` | `test_query.py` |
| `to_dicts()` | Done | `query.rs` | `result.to_dicts()` | `test_query.py` |
| `to_df()` | Done | `query.rs` | `result.to_df()` | `test_query.py` |
| `__arrow_c_stream__` | Done | `query.rs` | PyCapsule export | `test_query.py` |
| `num_rows` / `num_columns` | Done | `query.rs` | properties | `test_query.py` |
| `columns` | Done | `query.rs` | property | `test_query.py` |

## Subscriptions

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| Sync subscription | Done | `subscription.rs` | `subscribe(sql)` | `test_subscription.py` |
| Async subscription | Done | `async_support.rs` | `subscribe_async(sql)` | `test_subscription.py` |
| `is_active` property | Done | `subscription.rs`, `async_support.rs` | property | `test_subscription.py` |
| `cancel()` | Done | `subscription.rs`, `async_support.rs` | `cancel()` | `test_subscription.py` |
| `try_next()` | Done | `subscription.rs` | `try_next()` (sync only) | `test_subscription.py` |
| Iterator protocols | Done | `subscription.rs`, `async_support.rs` | `__iter__`/`__aiter__` | `test_subscription.py` |
| Subscription timeout | Planned | — | — | — |
| Callback subscription | Planned | — | — | — |

## Writer Advanced

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `watermark(timestamp)` | Planned | — | — | — |
| `current_watermark()` | Planned | — | — | — |

## Pipeline Control

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `start()` | Planned | — | — | — |
| `checkpoint()` | Planned | — | — | — |
| `is_checkpoint_enabled()` | Planned | — | — | — |

## Error Handling

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `LaminarError` (base) | Done | `error.rs` | `LaminarError` | `test_error.py` |
| `ConnectionError` | Done | `error.rs` | `ConnectionError` | `test_error.py` |
| `QueryError` | Done | `error.rs` | `QueryError` | `test_error.py` |
| `IngestionError` | Done | `error.rs` | `IngestionError` | `test_error.py` |
| `SchemaError` | Done | `error.rs` | `SchemaError` | `test_error.py` |
| `SubscriptionError` | Done | `error.rs` | `SubscriptionError` | `test_error.py` |
| Error code mapping | Done | `error.rs` | `IntoPyResult` trait | — |
| Numeric error codes | Planned | — | — | — |

## Developer Experience

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| PEP 561 type stubs | Done | `_laminardb.pyi` | `py.typed` marker | mypy |
| GIL release | Done | all modules | `py.allow_threads()` | — |
| Free-threaded Python | Done | all modules | `Send + Sync` | — |
| CI pipeline | Done | `.github/workflows/` | lint, test, typecheck | — |
| PyPI publishing | Done | `release.yml` | multi-platform wheels | — |
