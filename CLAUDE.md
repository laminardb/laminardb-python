# CLAUDE.md — LaminarDB Python Bindings

## Project Overview

Python bindings for the LaminarDB streaming SQL database, built with PyO3 and maturin.

## Dependencies (2026)

| Dependency | Version | Notes |
|---|---|---|
| PyO3 | 0.28 | `extension-module`, `abi3-py310`, `anyhow` features |
| pyo3-async-runtimes | 0.28 | `tokio-runtime` feature |
| pyo3-arrow | 0.12 | Arrow ↔ PyArrow conversion |
| arrow-rs | 57 | `pyarrow`, `ffi` features |
| maturin | >=1.7,<2.0 | Build system |
| Rust edition | 2024 | Minimum Rust 1.83 |
| Python | >=3.10 | abi3 stable ABI |
| pytest-asyncio | >=1.0 | `asyncio_mode = "auto"` |
| License | MIT | PEP 639 SPDX string |

## Architecture

```
python/laminardb/          # Python package (stubs + __init__)
src/                       # Rust source
  lib.rs                   # Module entry, open()/connect()
  error.rs                 # Exception hierarchy
  conversion.rs            # Python ↔ Arrow conversion
  connection.rs            # Connection pyclass
  writer.rs                # Streaming writer pyclass
  query.rs                 # QueryResult pyclass
  subscription.rs          # Sync subscription
  async_support.rs         # Tokio runtime + async subscription
tests/                     # pytest test suite
docs/                      # Architecture & feature spec
```

## Development Workflow

```bash
# Build and install in development mode
maturin develop --extras dev

# Run tests
pytest tests/ -v

# Run specific test
pytest tests/test_query.py -v

# Type checking
mypy python/laminardb --strict

# Rust checks
cargo fmt --check
cargo clippy -- -D warnings
cargo check
```

## PyO3 0.28 Key Patterns

- `Py<PyAny>` instead of `PyObject`
- Multi-phase module init is the default
- Free-threaded Python support is default
- `#[pyclass]` types must be `Send + Sync`
- Use `anyhow` feature for ergonomic error handling
- `py.allow_threads()` for ALL blocking operations

## Design Principles

1. **Zero-copy default** — Arrow PyCapsule interface checked first
2. **GIL release** — `py.allow_threads()` wraps every blocking Rust call
3. **Thread safety** — All pyclasses are `Send + Sync` for free-threaded Python
4. **Dev friendly** — Accept pandas, polars, pyarrow, dicts, JSON, CSV as input
5. **Fail fast** — Custom exception hierarchy with structured messages
6. **Single responsibility** — Each .rs file owns one concern

## Input Types Supported

- `dict` (single row or columnar)
- `list[dict]` (row-oriented)
- `pandas.DataFrame`
- `polars.DataFrame`
- `pyarrow.RecordBatch` / `pyarrow.Table`
- JSON string (`insert_json`)
- CSV string (`insert_csv`)
- Any object implementing `__arrow_c_stream__`

## Output Types Supported

- `to_arrow()` → `pyarrow.Table`
- `to_pandas()` → `pandas.DataFrame`
- `to_polars()` → `polars.DataFrame`
- `to_dicts()` → `dict[str, list]`
- `to_df()` → auto-detect best library
- `__arrow_c_stream__` → zero-copy PyCapsule
