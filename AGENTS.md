# Repository guidance

## Project purpose

This repository provides the Python bindings for the LaminarDB streaming SQL
database. The native extension is written in Rust with PyO3 and built with
Maturin; the Python package supplies exports, convenience wrappers, and type
stubs. The bindings intentionally live outside the main LaminarDB Rust
workspace, so the core monorepo does not need a PyO3 dependency; its lack of
PyO3 metadata is not evidence that Python support is absent.

Treat the checked-in manifests and implementation as authoritative when a
version or API detail in prose documentation has drifted. In particular, the
current binding stack is PyO3 0.28, `pyo3-async-runtimes` 0.28,
`pyo3-arrow` 0.17, Arrow 58.4, Rust edition 2024/Rust 1.95+, and Python 3.11+
with `abi3-py311`.

## Repository layout

- `src/lib.rs`: extension entry point, module registration, and top-level
  `open()`/`connect()` functions.
- `src/connection.rs`: the `Connection` Python class and database operations.
- `src/conversion.rs`: Python-to-Arrow input conversion.
- `src/query.rs`: `QueryResult` and Arrow/Pandas/Polars/row exports.
- `src/writer.rs`: streaming `Writer`.
- `src/subscription.rs`: synchronous continuous-query subscription.
- `src/async_support.rs`: process-wide Tokio runtime and async query
  subscription.
- `src/stream_subscription.rs`: synchronous and asynchronous named-stream
  subscriptions.
- `src/callback.rs`: callback-based subscriptions and their worker thread.
- `src/error.rs`: Python exception hierarchy and core-error mapping.
- `src/config.rs`, `src/catalog.rs`, `src/metrics.rs`, `src/checkpoint.rs`, and
  `src/execute.rs`: their corresponding public Python value types.
- `python/laminardb/__init__.py`: public Python exports and convenience helpers.
- `python/laminardb/_laminardb.pyi`: public native-extension type contract.
- `python/laminardb/types.py`: pure-Python wrapper types.
- `tests/`: pytest suite; use the closest existing test as the pattern for new
  behavior.
- `docs/ARCHITECTURE.md`: binding architecture. Feature-spec documents can
  describe planned behavior, so confirm claims in source, stubs, and tests.

## Core dependency setup

`Cargo.toml` resolves `laminar-db` and `laminar-core` from
`laminardb/crates/...`. A checkout of the LaminarDB monorepo must therefore be
available at `<this-repository>/laminardb`, directly or through a symlink or
junction. CI clones it there. Do not assume Cargo commands can run until that
path exists, and do not modify the external checkout unless the task explicitly
requires coordinated core changes.

## Binding invariants

- Use Arrow's C Data/PyCapsule interface (`__arrow_c_stream__` and
  `__arrow_c_array__`) for Arrow-compatible objects; `pyo3-arrow` provides the
  zero-copy bridge. Detect native Python dict/list inputs first so failed Arrow
  extraction does not consume their conversion errors.
- Detach from the interpreter around every blocking core operation with
  `py.detach()`.
- Enter the persistent Tokio runtime inside blocking calls that invoke core
  APIs which spawn background work. Named-stream `Connection::subscribe()` and
  `ArrowSubscription::next_frame()` are explicit exceptions in core 0.30: they
  reject calls made inside an async runtime, so their sync binding paths must
  remain outside `runtime().enter()`.
- Async iteration must move blocking `next()` work to
  `tokio::task::spawn_blocking`; never block a Python asyncio or Tokio worker
  thread directly.
- Named-stream subscriptions use frames. Preserve checkpoint barriers in the
  `next_frame*` API, keep batch-only methods as barrier-skipping conveniences,
  and retain each core frame lease for the lifetime of its Python batch.
- Preserve free-threaded-Python safety. Python classes are `Send + Sync`, and
  subscription values are serialized through a mutex. `SyncCell<T>` contains
  explicit unsafe `Send`/`Sync` assertions, so changes to its access discipline
  require a fresh safety review.
- Subscription and writer lifecycle state uses `Mutex<Option<T>>`. Perform
  state checks and the associated operation under one lock guard; dropping and
  reacquiring the guard creates cancellation/close races.
- Convert `laminar_db::api::ApiError` through `IntoPyResult` so callers receive
  the documented exception subclass and numeric `.code`. Do not flatten core
  failures into generic Python exceptions.
- `ExecuteResult::from_core` is fallible and returns `PyResult<Self>` because a
  core query result can itself contain an error.
- Cleanup is part of the API: connections close and subscriptions cancel from
  context-manager exits and finalizers. Keep those paths idempotent.

## API facts and conversion behavior

Input conversion detects native dict/list inputs first, then tries Arrow
PyCapsule/PyArrow, Pandas, and Polars. JSON and CSV use the dedicated
`insert_json()` and `insert_csv()` paths. Arrow exports should remain zero-copy
where the underlying Arrow representation permits it.

The Rust core API and the public Python API are not identical. The core opens a
connection with `Connection::open()` and exposes primitives such as
`list_sources()`; Python adds aliases and conveniences including `connect()`,
`create_table()`, and `list_tables()`. Implement Python conveniences in the
binding layer instead of assuming a same-named core method exists. A named
stream timeout raises `SubscriptionError`; a normal exhausted or closed stream
may return no batch.

Do not enable Arrow's `pyarrow` Cargo feature alongside `pyo3-arrow`; both link
against Python and conflict. Keep Arrow's `ffi` feature for the capsule bridge.
The stable ABI target is Python 3.11, not 3.10, because the current Arrow bridge
requires Python 3.11+ buffer behavior.

## Making changes

For a public API change, keep all affected layers synchronized:

1. Implement the Rust binding, normally in the focused module under `src/`.
2. Update `python/laminardb/_laminardb.pyi`.
3. Update `python/laminardb/__init__.py` or `types.py` when an export or wrapper
   is involved.
4. Add or update focused tests under `tests/`.
5. Update `README.md`, `docs/implementation-status.md`, and architecture or
   feature documentation when user-visible behavior changes.

Follow `rustfmt`, use Python type annotations, and preserve the existing custom
exception behavior. Avoid unrelated changes in generated or local directories
such as `target/`, `.venv/`, caches, and tool-created worktrees.

## Build and verification

With the core checkout present and the virtual environment active:

```text
python -m maturin develop --extras dev
python -m pytest tests/ -v
python -m mypy python/laminardb --strict
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo check
```

On Windows, use `.venv/Scripts/python.exe -m ...` if the environment is not
activated. Run the narrowest relevant pytest target while iterating, followed
by the full affected checks before handoff. If the local core checkout is
absent, formatting and repository-only static inspection can still run, but
compilation and extension tests are not valid substitutes for a complete build.
The checked-in Cargo target configuration uses the Rust toolchain's bundled
`rust-lld` for `x86_64-pc-windows-msvc`; do not replace it with a machine-local
linker path.

Useful Arrow API details for the pinned versions:

- `PySchema::from(schema_ref).into_pyarrow(py)` converts schemas.
- `PyTable::try_new(batches, schema).into_pyarrow(py)` converts result batches.
- `data.extract::<PyTable>()` accepts Arrow-compatible Python input, including
  objects implementing `__arrow_c_stream__`.
- `arrow_json::reader::infer_json_schema` returns `(Schema, usize)`.
- `arrow_csv::ReaderBuilder::new(schema)` requires a `SchemaRef`.
