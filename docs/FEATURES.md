# Feature Index

Comprehensive tracker of every binding capability. **76 Done, 4 Planned**.

## Connection & Lifecycle

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `open(path)` | Done | `lib.rs` | `open(path)` | `test_connection.py` |
| `connect(uri)` | Done | `lib.rs` | `connect(uri)` | `test_connection.py` |
| `open(path, config=)` | Done | `lib.rs` | `open(path, config=LaminarConfig())` | `test_config.py` |
| Context manager | Done | `connection.rs` | `__enter__`/`__exit__` | `test_connection.py` |
| Async context manager | Done | `connection.rs` | `__aenter__`/`__aexit__` | `test_shutdown.py` |
| `close()` | Done | `connection.rs` | `close()` | `test_connection.py` |
| `is_closed` property | Done | `connection.rs` | `conn.is_closed` | `test_connection.py` |
| `__repr__` | Done | `connection.rs` | `repr(conn)` | `test_connection.py` |
| `execute(sql)` | Done | `connection.rs` | `execute(sql)` → `ExecuteResult` | `test_execute.py` |
| `sql(query)` | Done | `connection.rs` | DuckDB-style alias for `query()` | `test_duckdb_api.py` |
| `tables()` | Done | `connection.rs` | DuckDB-style alias for `list_tables()` | `test_duckdb_api.py` |
| `materialized_views()` | Done | `connection.rs` | DuckDB-style alias for `list_streams()` | `test_duckdb_api.py` |
| `explain(query)` | Done | `connection.rs` | Show query plan | `test_duckdb_api.py` |
| `stats(table)` | Done | `connection.rs` | Table statistics dict | `test_duckdb_api.py` |
| File-based persistence | Planned | — | — | — |
| Remote connections | Planned | — | — | — |

> **Note**: `open(path)` and `connect(uri)` currently open in-memory databases (path/URI parameters are accepted but unused in v0.1.0).

## Configuration

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `LaminarConfig` | Done | `config.rs` | `LaminarConfig(buffer_size=, ...)` | `test_config.py` |
| `buffer_size` | Done | `config.rs` | default 65536 | `test_config.py` |
| `storage_dir` | Done | `config.rs` | optional | `test_config.py` |
| `checkpoint_interval_ms` | Done | `config.rs` | optional | `test_config.py` |
| `table_spill_threshold` | Done | `config.rs` | default 1M | `test_config.py` |

## Schema & Source Management

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `create_table(name, schema)` | Done | `connection.rs` | `create_table(name, schema)` | `test_connection.py` |
| `list_tables()` | Done | `connection.rs` | `list_tables()` | `test_connection.py` |
| `list_streams()` | Done | `connection.rs` | `list_streams()` | `test_connection.py` |
| `list_sinks()` | Done | `connection.rs` | `list_sinks()` | `test_connection.py` |
| `schema(table)` | Done | `connection.rs` | `schema(table)` | `test_connection.py` |

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
| `watermark(timestamp)` | Done | `writer.rs` | `writer.watermark(ts)` | `test_ingestion.py` |
| `current_watermark` | Done | `writer.rs` | `writer.current_watermark` | `test_ingestion.py` |

## Query & Results

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| SQL query | Done | `connection.rs` | `query(sql)` | `test_query.py` |
| Stream query | Done | `connection.rs` | `stream(sql)` | `test_query.py` |
| `ExecuteResult` | Done | `execute.rs` | `execute(sql)` with DDL/DML introspection | `test_execute.py` |
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
| Named stream subscription | Done | `stream_subscription.rs` | `subscribe_stream(name)` | `test_stream_subscription.py` |
| Async stream subscription | Done | `stream_subscription.rs` | `subscribe_stream_async(name)` | `test_stream_subscription.py` |
| `is_active` property | Done | `subscription.rs`, `stream_subscription.rs` | property | `test_subscription.py` |
| `cancel()` | Done | `subscription.rs`, `stream_subscription.rs` | `cancel()` | `test_subscription.py` |
| `try_next()` | Done | `subscription.rs`, `stream_subscription.rs` | `try_next()` | `test_subscription.py` |
| `next_timeout(ms)` | Done | `stream_subscription.rs` | `next_timeout(ms)` | `test_stream_subscription.py` |
| `schema` property | Done | `stream_subscription.rs` | `sub.schema` | `test_stream_subscription.py` |
| Iterator protocols | Done | `subscription.rs`, `stream_subscription.rs` | `__iter__`/`__aiter__` | `test_subscription.py` |
| Callback subscription | Done | `types.py` | `MaterializedView.subscribe(handler=)` | `test_types.py` |

## Pipeline Control

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `start()` | Done | `connection.rs` | `conn.start()` | `test_connection.py` |
| `shutdown()` | Done | `connection.rs` | `conn.shutdown()` | `test_shutdown.py` |
| `checkpoint()` | Done | `connection.rs` | `conn.checkpoint()` | `test_connection.py` |
| `cancel_query(id)` | Done | `connection.rs` | `conn.cancel_query(id)` | `test_shutdown.py` |
| `is_checkpoint_enabled` | Done | `connection.rs` | property | `test_connection.py` |
| `pipeline_state` | Done | `connection.rs` | property | `test_metrics.py` |
| `pipeline_watermark` | Done | `connection.rs` | property | `test_metrics.py` |
| `total_events_processed` | Done | `connection.rs` | property | `test_metrics.py` |

## Catalog Introspection

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `sources()` | Done | `connection.rs` | `conn.sources()` → `list[SourceInfo]` | `test_catalog.py` |
| `sinks()` | Done | `connection.rs` | `conn.sinks()` → `list[SinkInfo]` | `test_catalog.py` |
| `streams()` | Done | `connection.rs` | `conn.streams()` → `list[StreamInfo]` | `test_catalog.py` |
| `queries()` | Done | `connection.rs` | `conn.queries()` → `list[QueryInfo]` | `test_catalog.py` |
| `SourceInfo` | Done | `catalog.rs` | `name`, `schema`, `watermark_column` | `test_catalog.py` |
| `SinkInfo` | Done | `catalog.rs` | `name` | `test_catalog.py` |
| `StreamInfo` | Done | `catalog.rs` | `name`, `sql` | `test_catalog.py` |
| `QueryInfo` | Done | `catalog.rs` | `id`, `sql`, `active` | `test_catalog.py` |

## Pipeline Metrics

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `metrics()` | Done | `connection.rs` | `conn.metrics()` → `PipelineMetrics` | `test_metrics.py` |
| `topology()` | Done | `connection.rs` | `conn.topology()` → `PipelineTopology` | `test_metrics.py` |
| `source_metrics(name)` | Done | `connection.rs` | `conn.source_metrics(name)` | `test_metrics.py` |
| `all_source_metrics()` | Done | `connection.rs` | `conn.all_source_metrics()` | `test_metrics.py` |
| `stream_metrics(name)` | Done | `connection.rs` | `conn.stream_metrics(name)` | `test_metrics.py` |
| `all_stream_metrics()` | Done | `connection.rs` | `conn.all_stream_metrics()` | `test_metrics.py` |
| `PipelineMetrics` | Done | `metrics.rs` | Events, cycles, uptime, state | `test_metrics.py` |
| `SourceMetrics` | Done | `metrics.rs` | Events, pending, capacity, watermark | `test_metrics.py` |
| `StreamMetrics` | Done | `metrics.rs` | Events, pending, capacity, SQL | `test_metrics.py` |
| `PipelineTopology` | Done | `metrics.rs` | Nodes and edges | `test_metrics.py` |
| `PipelineNode` | Done | `metrics.rs` | Name, type, schema, SQL | `test_metrics.py` |
| `PipelineEdge` | Done | `metrics.rs` | from_node, to_node | `test_metrics.py` |

## Error Handling

| Feature | Status | Rust Source | Python API | Test |
|---------|--------|-------------|------------|------|
| `LaminarError` (base) | Done | `error.rs` | `LaminarError` | `test_errors.py` |
| `ConnectionError` | Done | `error.rs` | `ConnectionError` | `test_errors.py` |
| `QueryError` | Done | `error.rs` | `QueryError` | `test_errors.py` |
| `IngestionError` | Done | `error.rs` | `IngestionError` | `test_errors.py` |
| `SchemaError` | Done | `error.rs` | `SchemaError` | `test_errors.py` |
| `SubscriptionError` | Done | `error.rs` | `SubscriptionError` | `test_errors.py` |
| `StreamError` | Done | `error.rs` | `StreamError` | `test_module_functions.py` |
| `CheckpointError` | Done | `error.rs` | `CheckpointError` | `test_module_functions.py` |
| `ConnectorError` | Done | `error.rs` | `ConnectorError` | `test_module_functions.py` |
| Error code mapping | Done | `error.rs` | `IntoPyResult` trait | — |
| Numeric error codes | Done | `error.rs` | `.code` attribute + `laminardb.codes` | `test_errors.py` |

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

## Planned

| Feature | Priority | Notes |
|---------|----------|-------|
| File-based persistence | P3 | `open(path)` writes to disk |
| Remote connections | P3 | `connect(uri)` to remote server |
| Query cancellation timeout | P5 | `subscribe(sql, timeout=)` |
| Graceful pipeline drain | P6 | Drain in-flight events before shutdown |
