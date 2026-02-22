# Architecture

## System Overview

```
┌─────────────────────────────────────────────────────┐
│                   Python Application                 │
│                                                      │
│  import laminardb                                    │
│  db = laminardb.open("mydb")                         │
│  db.insert("sensors", pandas_df)                     │
│  result = db.query("SELECT 1 + 1 AS answer")        │
│  df = result.to_pandas()                             │
└──────────────────────┬──────────────────────────────┘
                       │  Python C API (PyO3 0.27)
┌──────────────────────▼──────────────────────────────┐
│              laminardb (Rust cdylib)                  │
│                                                      │
│  ┌─────────┐ ┌────────────┐ ┌──────────────────┐    │
│  │ lib.rs  │ │ error.rs   │ │ conversion.rs    │    │
│  │ entry   │ │ exceptions │ │ Python ↔ Arrow   │    │
│  └─────────┘ └────────────┘ └──────────────────┘    │
│                                                      │
│  ┌──────────────┐ ┌──────────┐ ┌──────────────┐    │
│  │connection.rs │ │ query.rs │ │ writer.rs    │    │
│  │ Connection   │ │ Result   │ │ Streaming    │    │
│  └──────────────┘ └──────────┘ └──────────────┘    │
│                                                      │
│  ┌────────────────┐ ┌───────────────────────┐       │
│  │subscription.rs │ │ async_support.rs      │       │
│  │ Continuous Qs  │ │ Tokio + asyncio       │       │
│  └────────────────┘ └───────────────────────┘       │
│                                                      │
│  ┌──────────────────────┐ ┌──────────────┐          │
│  │stream_subscription.rs│ │ execute.rs   │          │
│  │ Named stream subs    │ │ DDL/DML      │          │
│  └──────────────────────┘ └──────────────┘          │
│                                                      │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐        │
│  │config.rs │ │catalog.rs│ │ metrics.rs   │        │
│  │ Config   │ │ Catalog  │ │ Topology     │        │
│  └──────────┘ └──────────┘ └──────────────┘        │
└──────────────────────┬──────────────────────────────┘
                       │  Rust crate API
┌──────────────────────▼──────────────────────────────┐
│              laminar-db (Rust)                        │
│          Streaming SQL database engine               │
└─────────────────────────────────────────────────────┘
```

## Module Responsibilities

| Module | Responsibility |
|---|---|
| `lib.rs` | Module entry point, top-level `open()`/`connect()` functions, class registration |
| `error.rs` | Exception hierarchy, core error → Python error mapping |
| `conversion.rs` | All Python ↔ Arrow type conversions (input + output) |
| `connection.rs` | `Connection` pyclass with all database operations |
| `writer.rs` | `Writer` pyclass for batched streaming inserts |
| `query.rs` | `QueryResult` pyclass with multi-format export |
| `subscription.rs` | `Subscription` pyclass for continuous queries (sync) |
| `async_support.rs` | Tokio runtime, `AsyncSubscription` for asyncio |
| `stream_subscription.rs` | `StreamSubscription` + `AsyncStreamSubscription` for named streams |
| `execute.rs` | `ExecuteResult` pyclass for DDL/DML introspection |
| `config.rs` | `LaminarConfig` pyclass for database configuration |
| `catalog.rs` | `SourceInfo`, `SinkInfo`, `StreamInfo`, `QueryInfo` catalog types |
| `metrics.rs` | `PipelineMetrics`, `PipelineTopology`, per-source/stream metrics |

## Data Flow

### Input (Python → LaminarDB)

```
Python Object (dict, DataFrame, RecordBatch, JSON, CSV)
    │
    ▼ conversion.rs: python_to_batches()
Arrow RecordBatch[]
    │
    ▼ connection.rs: py.allow_threads()
laminar_db::api::Connection::insert()
```

### Output (LaminarDB → Python)

```
laminar_db::api::QueryResult
    │
    ▼ query.rs: QueryResult::from_core()
Arrow RecordBatch[] + Schema
    │
    ▼ conversion.rs: batches_to_*()
Python Object (PyArrow Table, Pandas DF, Polars DF, dicts)
```

## Thread Safety Model

- All `#[pyclass]` types are `Send + Sync` for free-threaded Python (3.13t/3.14t)
- `Connection` wraps `laminar_db::api::Connection` in `Arc` for safe sharing
- `Writer` uses `parking_lot::Mutex` for its internal buffer
- `Subscription` uses `Arc<Mutex<...>>` + `AtomicBool` for state
- ALL blocking Rust calls release the GIL via `py.allow_threads()`
- Tokio runtime is lazily initialized as a global `OnceLock`

## Error Handling Strategy

```
LaminarError (base)
├── ConnectionError    ← ApiError::Connection
├── QueryError         ← ApiError::Query
├── IngestionError     ← ApiError::Ingestion
├── SchemaError        ← ApiError::Schema
├── SubscriptionError  ← ApiError::Subscription
├── StreamError        ← stream/materialized view failures
├── CheckpointError    ← checkpoint operation failures
└── ConnectorError     ← connector operation failures
```

Core errors are mapped 1:1 to Python exceptions via `IntoPyResult` trait.
Internal errors use `anyhow` for context chains.

## Performance Considerations

1. **Zero-copy exports**: Arrow PyCapsule interface (`__arrow_c_stream__`) is tried first
2. **GIL release**: Every blocking operation calls `py.allow_threads()`
3. **Batch buffering**: `Writer` accumulates batches before flushing
4. **Lazy runtime**: Tokio runtime only created on first use
5. **LTO + strip**: Release builds use link-time optimization and symbol stripping
