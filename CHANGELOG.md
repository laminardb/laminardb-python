# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-08

### Added

- **Connection management**: `open(path)` and `connect(uri)` with context manager support
- **Schema operations**: `create_table()`, `list_tables()`, `schema()`, `execute()`
- **Data ingestion** from 10 input types:
  - Python dicts (single row, list of rows, columnar)
  - pandas DataFrame, polars DataFrame
  - pyarrow RecordBatch, pyarrow Table
  - JSON string (`insert_json()`), CSV string (`insert_csv()`)
  - Any object implementing `__arrow_c_stream__` (zero-copy)
- **Streaming writer** with context manager, `insert()`, `flush()`, `close()`
- **SQL queries** with `query()` and streaming `stream()` iterator
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
- **Exception hierarchy**: `LaminarError` base with `ConnectionError`, `QueryError`, `IngestionError`, `SchemaError`, `SubscriptionError`
- **PEP 561 type stubs** with `py.typed` marker
- **GIL release** on all blocking operations via `py.allow_threads()`
- **Free-threaded Python** support (all pyclasses are `Send + Sync`)
- **CI/CD pipeline**: lint, test (Python 3.11–3.13), type check, multi-platform wheel builds
- **PyPI trusted publishing** via OIDC

### Platform Support

- Linux x86_64, aarch64
- macOS x86_64, aarch64 (Apple Silicon)
- Windows x86_64

[0.1.0]: https://github.com/laminardb/laminardb-python/releases/tag/v0.1.0
