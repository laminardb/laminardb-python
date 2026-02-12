# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **DuckDB-style API** on `QueryResult`:
  - `df()`, `pl(lazy=False)`, `arrow()` — DataFrame conversion aliases
  - `fetchall()`, `fetchone()`, `fetchmany(size)` — row-oriented fetch methods
  - `show(max_rows=20)` — terminal preview
  - `_repr_html_()` — Jupyter notebook HTML rendering
  - `__len__()`, `__iter__()` — `len(result)` and `for row in result`
- **DuckDB-style API** on `Connection`:
  - `sql(query)` — alias for `query()`
  - `tables()` — alias for `list_tables()`
  - `materialized_views()` — alias for `list_streams()`
  - `explain(query)` — query execution plan
  - `stats(table)` — table statistics as dict
- **New exceptions**: `StreamError`, `CheckpointError`, `ConnectorError` (all subclass `LaminarError`)
- **Python wrapper types** (`laminardb.types`):
  - `Schema`, `Column`, `TableStats`, `Watermark`, `CheckpointStatus`, `Metrics`
  - `ChangeRow`, `ChangeEvent` — structured change data capture
  - `MaterializedView` — high-level stream wrapper with `query()`, `subscribe()`, `schema()`
- **Module-level functions**: `laminardb.sql()`, `laminardb.execute()`, `laminardb.mv()` with thread-safe default connection
- **Aliases**: `Config = LaminarConfig`, `BatchWriter = Writer`
- **Benchmarks**: ingestion, query, and streaming throughput benchmarks
- **Examples**: `quickstart.py` (DuckDB-style API), `streaming_analytics.py` (MaterializedView pattern)

### Fixed

- CI workflows now clone the laminardb monorepo for local path dependencies
- CI test matrix updated to Python 3.11+ (matches abi3-py311 build target)
- `maturin develop` in CI now runs inside a virtualenv

### Changed

- Minimum Python version is now **3.11** (was 3.10) to match the `abi3-py311` stable ABI target

## [0.1.0] - 2026-02-08

### Added

- **Connection management**: `open(path)` and `connect(uri)` with context manager support
- **Configuration**: `LaminarConfig` with `buffer_size`, `storage_dir`, `checkpoint_interval_ms`, `table_spill_threshold`
- **Schema operations**: `create_table()`, `list_tables()`, `list_streams()`, `list_sinks()`, `schema()`, `execute()`
- **Data ingestion** from 10 input types:
  - Python dicts (single row, list of rows, columnar)
  - pandas DataFrame, polars DataFrame
  - pyarrow RecordBatch, pyarrow Table
  - JSON string (`insert_json()`), CSV string (`insert_csv()`)
  - Any object implementing `__arrow_c_stream__` (zero-copy)
- **Streaming writer** with context manager, `insert()`, `flush()`, `close()`, `watermark()`, `current_watermark`
- **SQL queries** with `query()` and streaming `stream()` iterator
- **ExecuteResult**: rich DDL/DML introspection with `result_type`, `ddl_type`, `ddl_object`, `rows_affected`, `int()`, `bool()` coercion
- **Result export** to 6 output formats:
  - `to_arrow()` → pyarrow.Table
  - `to_pandas()` → pandas.DataFrame
  - `to_polars()` → polars.DataFrame
  - `to_dicts()` → columnar dict
  - `to_df()` → auto-detect best library
  - `__arrow_c_stream__` → zero-copy Arrow PyCapsule
- **Subscriptions**: sync (`subscribe()`) and async (`subscribe_async()`) continuous queries
  - Iterator/async iterator protocols
  - `try_next()` non-blocking poll (sync)
  - `is_active` property, `cancel()` method
- **Named stream subscriptions**: `subscribe_stream()` / `subscribe_stream_async()` with schema access, `next_timeout(ms)`, `try_next()`
- **Catalog introspection**: `sources()`, `sinks()`, `streams()`, `queries()` returning typed info objects
- **Pipeline control**: `start()`, `shutdown()`, `checkpoint()`, `cancel_query()`
- **Pipeline metrics**: `metrics()`, `topology()`, `source_metrics()`, `stream_metrics()`, per-source/stream observability
- **Connection properties**: `is_closed`, `pipeline_state`, `pipeline_watermark`, `total_events_processed`, `source_count`, `sink_count`, `active_query_count`, `is_checkpoint_enabled`
- **Async context manager**: `async with conn as db: ...`
- **Exception hierarchy**: `LaminarError` base with `ConnectionError`, `QueryError`, `IngestionError`, `SchemaError`, `SubscriptionError`
- **Error codes**: numeric `.code` attribute on all exceptions, `laminardb.codes` module with 44 constants
- **PEP 561 type stubs** with `py.typed` marker (800+ lines, passes `mypy --strict`)
- **GIL release** on all blocking operations via `py.allow_threads()`
- **Free-threaded Python** support (all pyclasses are `Send + Sync`)
- **CI/CD pipeline**: lint, test (Python 3.11-3.13), type check, multi-platform wheel builds
- **PyPI trusted publishing** via OIDC

### Platform Support

- Linux x86_64, aarch64
- macOS x86_64, aarch64 (Apple Silicon)
- Windows x86_64

[Unreleased]: https://github.com/laminardb/laminardb-python/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/laminardb/laminardb-python/releases/tag/v0.1.0
