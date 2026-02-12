# Feature Index

Comprehensive tracker of every binding capability. **65 Done, 11 Planned**.

## Connection & Lifecycle

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `open(path)` | Done | `lib.rs` | `open(path)` | `test_connection.py` |
| `connect(uri)` | Done | `lib.rs` | `connect(uri)` | `test_connection.py` |
| Context manager | Done | `connection.rs` | `__enter__`/`__exit__` | `test_connection.py` |
| `close()` | Done | `connection.rs` | `close()` | `test_connection.py` |
| `__repr__` | Done | `connection.rs` | `repr(conn)` | `test_connection.py` |
| `execute(sql)` | Done | `connection.rs` | `execute(sql)` | `test_connection.py` |
| `sql(query)` | Done | `connection.rs` | DuckDB-style alias for `query()` | `test_duckdb_api.py` |
| `tables()` | Done | `connection.rs` | DuckDB-style alias for `list_tables()` | `test_duckdb_api.py` |
| `materialized_views()` | Done | `connection.rs` | DuckDB-style alias for `list_streams()` | `test_duckdb_api.py` |
| `explain(query)` | Done | `connection.rs` | Show query plan | `test_duckdb_api.py` |
| `stats(table)` | Done | `connection.rs` | Table statistics dict | `test_duckdb_api.py` |
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
| `df()` | Done | `query.rs` | DuckDB-style alias for `to_pandas()` | `test_duckdb_api.py` |
| `pl(lazy=False)` | Done | `query.rs` | DuckDB-style Polars conversion | `test_duckdb_api.py` |
| `arrow()` | Done | `query.rs` | DuckDB-style alias for `to_arrow()` | `test_duckdb_api.py` |
| `fetchall()` | Done | `query.rs` | Fetch all rows as list of tuples | `test_duckdb_api.py` |
| `fetchone()` | Done | `query.rs` | Fetch first row or None | `test_duckdb_api.py` |
| `fetchmany(size)` | Done | `query.rs` | Fetch up to N rows | `test_duckdb_api.py` |
| `show(max_rows)` | Done | `query.rs` | Print result preview | `test_duckdb_api.py` |
| `_repr_html_()` | Done | `query.rs` | Jupyter HTML rendering | `test_duckdb_api.py` |
| `__len__()` | Done | `query.rs` | `len(result)` | `test_duckdb_api.py` |
| `__iter__()` | Done | `query.rs` | `for row in result` | `test_duckdb_api.py` |

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
| `StreamError` | Done | `error.rs` | `StreamError` | `test_module_functions.py` |
| `CheckpointError` | Done | `error.rs` | `CheckpointError` | `test_module_functions.py` |
| `ConnectorError` | Done | `error.rs` | `ConnectorError` | `test_module_functions.py` |
| Error code mapping | Done | `error.rs` | `IntoPyResult` trait | — |
| Numeric error codes | Planned | — | — | — |

## High-Level Python Types

| Feature | Status | Source | Python API | Test |
|---------|--------|--------|------------|------|
| `Schema` | Done | `types.py` | Wraps PyArrow Schema | `test_types.py` |
| `Column` | Done | `types.py` | Frozen dataclass | `test_types.py` |
| `MaterializedView` | Done | `types.py` | Stream wrapper with query/subscribe | `test_types.py` |
| `ChangeEvent` | Done | `types.py` | Batch of change rows | `test_types.py` |
| `ChangeRow` | Done | `types.py` | Single change row with dict access | `test_types.py` |
| `TableStats` | Done | `types.py` | Frozen dataclass | `test_types.py` |
| `Watermark` | Done | `types.py` | Frozen dataclass | `test_types.py` |
| `CheckpointStatus` | Done | `types.py` | Frozen dataclass | `test_types.py` |
| `Metrics` | Done | `types.py` | Wraps PipelineMetrics | `test_types.py` |

## Module-Level Functions & Aliases

| Feature | Status | Source | Python API | Test |
|---------|--------|--------|------------|------|
| `sql(query)` | Done | `__init__.py` | Module-level SQL on default conn | `test_module_functions.py` |
| `execute(query)` | Done | `__init__.py` | Module-level execute on default conn | `test_module_functions.py` |
| `mv(conn, name)` | Done | `__init__.py` | Create MaterializedView helper | `test_module_functions.py` |
| `Config` alias | Done | `__init__.py` | `Config = LaminarConfig` | `test_module_functions.py` |
| `BatchWriter` alias | Done | `__init__.py` | `BatchWriter = Writer` | `test_module_functions.py` |

## Developer Experience

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| PEP 561 type stubs | Done | `_laminardb.pyi` | `py.typed` marker | mypy |
| GIL release | Done | all modules | `py.allow_threads()` | — |
| Free-threaded Python | Done | all modules | `Send + Sync` | — |
| CI pipeline | Done | `.github/workflows/` | lint, test, typecheck | — |
| PyPI publishing | Done | `release.yml` | multi-platform wheels | — |
| Benchmarks | Done | `benchmarks/` | ingestion, query, streaming | — |
| Examples | Done | `examples/` | quickstart, streaming analytics | — |
