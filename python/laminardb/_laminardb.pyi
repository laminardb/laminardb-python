"""Type stubs for the laminardb native extension module."""

from collections.abc import AsyncIterator, Callable, Iterator
from types import TracebackType
from typing import Any, Union

__version__: str

# Supported input types for data ingestion
DataInput = Union[
    dict[str, Any],          # single row or columnar dict
    list[dict[str, Any]],    # list of row dicts
    str,                     # JSON or CSV string
    Any,                     # PyArrow RecordBatch/Table, Pandas/Polars DataFrame
]

# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------

class LaminarError(Exception):
    """Base exception for all LaminarDB errors."""
    ...

class ConnectionError(LaminarError):
    """Raised when a connection cannot be established or is lost."""
    ...

class QueryError(LaminarError):
    """Raised when a SQL query fails."""
    ...

class IngestionError(LaminarError):
    """Raised when data ingestion fails."""
    ...

class SchemaError(LaminarError):
    """Raised when a schema operation fails."""
    ...

class SubscriptionError(LaminarError):
    """Raised when a subscription operation fails."""
    ...

# ---------------------------------------------------------------------------
# Connection
# ---------------------------------------------------------------------------

class Connection:
    """A connection to a LaminarDB database."""

    def __enter__(self) -> Connection: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None: ...
    def __repr__(self) -> str: ...

    def insert(self, table: str, data: DataInput) -> int:
        """Insert data into a table. Returns the number of rows inserted."""
        ...

    def insert_json(self, table: str, data: str) -> int:
        """Insert JSON string data into a table."""
        ...

    def insert_csv(self, table: str, data: str) -> int:
        """Insert CSV string data into a table."""
        ...

    def writer(self, table: str) -> Writer:
        """Create a streaming writer for batched inserts."""
        ...

    def query(self, sql: str) -> QueryResult:
        """Execute a SQL query and return the full result."""
        ...

    def stream(self, sql: str) -> Iterator[QueryResult]:
        """Execute a SQL query and stream results in batches."""
        ...

    def subscribe(self, sql: str) -> Subscription:
        """Subscribe to a continuous query (sync iterator)."""
        ...

    async def subscribe_async(self, sql: str) -> AsyncSubscription:
        """Subscribe to a continuous query (async iterator)."""
        ...

    def schema(self, table: str) -> Any:
        """Get the schema of a table as a PyArrow Schema."""
        ...

    def create_table(self, name: str, schema: Any) -> None:
        """Create a new table with the given schema."""
        ...

    def list_tables(self) -> list[str]:
        """List all tables in the database."""
        ...

    def close(self) -> None:
        """Close the connection."""
        ...

# ---------------------------------------------------------------------------
# Writer
# ---------------------------------------------------------------------------

class Writer:
    """A streaming writer for batched inserts into a table."""

    def __enter__(self) -> Writer: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None: ...

    def insert(self, data: DataInput) -> None:
        """Add data to the write buffer."""
        ...

    def flush(self) -> int:
        """Flush the buffer to the database. Returns rows written."""
        ...

    def close(self) -> None:
        """Flush remaining data and close the writer."""
        ...

# ---------------------------------------------------------------------------
# QueryResult
# ---------------------------------------------------------------------------

class QueryResult:
    """The result of a SQL query."""

    @property
    def num_rows(self) -> int: ...
    @property
    def num_columns(self) -> int: ...
    @property
    def columns(self) -> list[str]: ...

    def to_arrow(self) -> Any:
        """Convert to a PyArrow Table."""
        ...

    def to_pandas(self) -> Any:
        """Convert to a Pandas DataFrame."""
        ...

    def to_polars(self) -> Any:
        """Convert to a Polars DataFrame."""
        ...

    def to_dicts(self) -> list[dict[str, Any]]:
        """Convert to a list of Python dicts."""
        ...

    def to_df(self) -> Any:
        """Auto-detect best available library and convert."""
        ...

    def __arrow_c_stream__(self, requested_schema: Any = None) -> Any:
        """Export via Arrow PyCapsule interface."""
        ...

# ---------------------------------------------------------------------------
# Subscription
# ---------------------------------------------------------------------------

class Subscription:
    """A synchronous subscription to a continuous query."""

    @property
    def is_active(self) -> bool: ...

    def __iter__(self) -> Iterator[QueryResult]: ...
    def __next__(self) -> QueryResult: ...

    def try_next(self) -> QueryResult | None:
        """Non-blocking poll for the next result."""
        ...

    def cancel(self) -> None:
        """Cancel the subscription."""
        ...

class AsyncSubscription:
    """An asynchronous subscription to a continuous query."""

    @property
    def is_active(self) -> bool: ...

    def __aiter__(self) -> AsyncIterator[QueryResult]: ...
    async def __anext__(self) -> QueryResult: ...

    def cancel(self) -> None:
        """Cancel the subscription."""
        ...

# ---------------------------------------------------------------------------
# Module-level functions
# ---------------------------------------------------------------------------

def open(path: str) -> Connection:
    """Open a LaminarDB database at the given file path."""
    ...

def connect(uri: str) -> Connection:
    """Connect to a LaminarDB database via URI."""
    ...
