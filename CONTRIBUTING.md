# Contributing to laminardb-python

Thank you for your interest in contributing to the LaminarDB Python bindings!

## Prerequisites

- **Rust 1.85+** (stable) — [rustup.rs](https://rustup.rs)
- **Python 3.11+** — [python.org](https://www.python.org/downloads/)
- **maturin** — `pip install maturin`
- **System dependencies** — protobuf compiler (`protoc`), OpenSSL dev headers, pkg-config

## Getting Started

### 1. Clone both repos

The Python bindings depend on the `laminardb` monorepo as a sibling directory:

```bash
git clone https://github.com/laminardb/laminardb.git
git clone https://github.com/laminardb/laminardb-python.git
```

Directory structure:

```
parent/
  laminardb/           # Rust monorepo (laminar-db, laminar-core, etc.)
  laminardb-python/    # This repo (Python bindings)
```

The `Cargo.toml` references `laminardb/` as a relative path dependency. If your monorepo lives elsewhere, create a symlink:

```bash
cd laminardb-python
ln -s /path/to/laminardb laminardb
```

### 2. Set up a virtual environment

```bash
cd laminardb-python
python -m venv .venv
source .venv/bin/activate   # or .venv\Scripts\activate on Windows
pip install maturin
```

### 3. Build and install in dev mode

```bash
maturin develop --extras dev
```

This compiles the Rust extension and installs it into your virtualenv along with dev dependencies (pytest, mypy, pandas, polars, pyarrow).

## Running Checks

### Tests

```bash
pytest tests/ -v
```

Run a specific test file:

```bash
pytest tests/test_query.py -v
```

### Type checking

```bash
mypy python/laminardb --strict
```

### Rust checks

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo check
```

## How the Binding Layer Works

The Python package consists of two layers:

1. **Rust extension** (`src/*.rs`) — PyO3-based native module compiled to `_laminardb.so`/`.pyd`
2. **Python package** (`python/laminardb/`) — Pure Python wrappers, type stubs, and `__init__.py`

### Key patterns

- **`py.allow_threads()`** — Every blocking Rust call releases the GIL so other Python threads can run
- **`Arc<Mutex<Connection>>`** — The core connection is thread-safe and shareable
- **`conversion.rs`** — Handles all 10 input formats (dict, list, DataFrame, Arrow, JSON, CSV)
- **`pyo3-arrow`** — Zero-copy Arrow ↔ PyArrow conversion via PyCapsule interface
- **Error mapping** — `error.rs` maps `laminar_db::api::ApiError` variants to Python exception classes

### Adding a new Connection method

1. Add the method in `src/connection.rs` under `#[pymethods]`
2. Add the type stub in `python/laminardb/_laminardb.pyi`
3. Export from `python/laminardb/__init__.py` if needed
4. Write a test in `tests/`
5. Run `mypy python/laminardb --strict` to verify stubs

## Code Style

- **Rust**: Follow `rustfmt` defaults. Run `cargo fmt` before committing.
- **Python**: Follow PEP 8. Use type annotations everywhere.
- **Commits**: Use conventional commit messages (e.g., `feat:`, `fix:`, `docs:`, `chore:`)
- **Tests**: Every new feature or bug fix should include a test

## Pull Request Process

1. Fork the repo and create a feature branch from `main`
2. Make your changes with tests
3. Ensure all checks pass: `pytest`, `mypy --strict`, `cargo clippy`, `cargo fmt --check`
4. Open a PR against `main` with a clear description of what and why
5. Address review feedback

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
