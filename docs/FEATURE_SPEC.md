# Feature Specification

## Feature Matrix

| Feature | Status | Module |
|---|---|---|
| File-based connection (`open`) | Implemented | `lib.rs` |
| URI-based connection (`connect`) | Implemented | `lib.rs` |
| Context manager protocol | Implemented | `connection.rs` |
| Table creation (dict schema) | Implemented | `connection.rs` |
| Table creation (pyarrow schema) | Implemented | `connection.rs` |
| List tables | Implemented | `connection.rs` |
| Get table schema | Implemented | `connection.rs` |
| Insert: single dict | Implemented | `conversion.rs` |
| Insert: list of dicts | Implemented | `conversion.rs` |
| Insert: columnar dict | Implemented | `conversion.rs` |
| Insert: Pandas DataFrame | Implemented | `conversion.rs` |
| Insert: Polars DataFrame | Implemented | `conversion.rs` |
| Insert: PyArrow RecordBatch | Implemented | `conversion.rs` |
| Insert: PyArrow Table | Implemented | `conversion.rs` |
| Insert: JSON string | Implemented | `conversion.rs` |
| Insert: CSV string | Implemented | `conversion.rs` |
| Streaming writer | Implemented | `writer.rs` |
| SQL query | Implemented | `connection.rs` |
| Query result → PyArrow | Implemented | `query.rs` |
| Query result → Pandas | Implemented | `query.rs` |
| Query result → Polars | Implemented | `query.rs` |
| Query result → dicts | Implemented | `query.rs` |
| Query result → auto-detect | Implemented | `query.rs` |
| Arrow PyCapsule export | Implemented | `query.rs` |
| Stream query results | Implemented | `connection.rs` |
| Sync subscription | Implemented | `subscription.rs` |
| Async subscription | Implemented | `async_support.rs` |
| Non-blocking poll (`try_next`) | Implemented | `subscription.rs` |
| Custom exception hierarchy | Implemented | `error.rs` |
| PEP 561 type stubs | Implemented | `_laminardb.pyi` |
| Raw SQL execution (`execute`) | Implemented | `connection.rs` |
| Free-threaded Python support | Implemented | All modules |
| GIL release on blocking ops | Implemented | All modules |
| CI/CD pipeline | Implemented | `.github/workflows/` |
| Trusted PyPI publishing | Implemented | `release.yml` |

## Supported Input Types

| Type | Conversion Path |
|---|---|
| `dict` (scalar values) | Single row → JSON → Arrow |
| `dict` (list values) | Columnar → PyArrow → Arrow |
| `list[dict]` | Row-oriented → JSON → Arrow |
| `str` (JSON) | `insert_json()` → arrow-json reader |
| `str` (CSV) | `insert_csv()` → arrow-csv reader |
| `pyarrow.RecordBatch` | Direct via pyo3-arrow |
| `pyarrow.Table` | `.to_batches()` → pyo3-arrow |
| `pandas.DataFrame` | → PyArrow Table → pyo3-arrow |
| `polars.DataFrame` | `.to_arrow()` → PyArrow → pyo3-arrow |
| Arrow PyCapsule | `__arrow_c_stream__` (zero-copy) |

## Supported Output Types

| Method | Output Type |
|---|---|
| `to_arrow()` | `pyarrow.Table` |
| `to_pandas()` | `pandas.DataFrame` |
| `to_polars()` | `polars.DataFrame` |
| `to_dicts()` | `dict[str, list]` (columnar) |
| `to_df()` | Best available (polars > pandas > pyarrow) |
| `__arrow_c_stream__` | Arrow PyCapsule (zero-copy) |

## Async Patterns

### Sync usage
```python
with laminardb.open("mydb") as db:
    # Create a source and insert data
    db.create_table("sensors", {"ts": "int64", "value": "float64"})
    db.insert("sensors", {"ts": 1, "value": 42.0})

    # Query using inline SQL (sources and query tables are separate catalogs)
    result = db.query("SELECT * FROM (VALUES (1, 42.0)) AS t(ts, value)")
    df = result.to_pandas()
```

> **Note**: `open(path)` and `connect(uri)` currently open in-memory databases
> (the path/URI parameters are accepted but unused in v0.1.0).

### Async subscription
```python
async with laminardb.open("mydb") as db:
    async for result in await db.subscribe_async("SELECT * FROM sensors"):
        df = result.to_pandas()
        process(df)
```

### Streaming writer
```python
with db.writer("sensors") as w:
    for batch in incoming_data:
        w.insert(batch)
    # auto-flush on context exit
```

## Success Criteria

1. All input types convert correctly to Arrow and insert without data loss
2. Query results export to all output formats with correct types
3. Subscriptions deliver real-time updates with < 1ms overhead
4. GIL is released during all blocking operations
5. All `#[pyclass]` types are `Send + Sync` for free-threaded Python
6. Custom exceptions provide actionable error messages
7. Type stubs pass `mypy --strict`
8. CI runs on Python 3.11–3.13 with all tests passing
