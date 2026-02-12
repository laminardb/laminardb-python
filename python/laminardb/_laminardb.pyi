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
    code: int
    """Numeric error code (see laminardb.codes)."""
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

class StreamError(LaminarError):
    """Raised when a stream or materialized view operation fails."""
    ...

class CheckpointError(LaminarError):
    """Raised when a checkpoint operation fails."""
    ...

class ConnectorError(LaminarError):
    """Raised when a connector operation fails."""
    ...

# ---------------------------------------------------------------------------
# Error codes
# ---------------------------------------------------------------------------

class codes:
    """Error code constants for LaminarDB errors."""

    # Connection (100-199)
    CONNECTION_FAILED: int
    CONNECTION_CLOSED: int
    CONNECTION_IN_USE: int

    # Schema (200-299)
    TABLE_NOT_FOUND: int
    TABLE_EXISTS: int
    SCHEMA_MISMATCH: int
    INVALID_SCHEMA: int

    # Ingestion (300-399)
    INGESTION_FAILED: int
    WRITER_CLOSED: int
    BATCH_SCHEMA_MISMATCH: int

    # Query (400-499)
    QUERY_FAILED: int
    SQL_PARSE_ERROR: int
    QUERY_CANCELLED: int

    # Subscription (500-599)
    SUBSCRIPTION_FAILED: int
    SUBSCRIPTION_CLOSED: int
    SUBSCRIPTION_TIMEOUT: int

    # Internal (900-999)
    INTERNAL_ERROR: int
    SHUTDOWN: int

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

class LaminarConfig:
    """Configuration for a LaminarDB connection."""

    def __init__(
        self,
        *,
        buffer_size: int = 65536,
        storage_dir: str | None = None,
        checkpoint_interval_ms: int | None = None,
        table_spill_threshold: int = 1_000_000,
    ) -> None: ...

    @property
    def buffer_size(self) -> int: ...
    @property
    def storage_dir(self) -> str | None: ...
    @property
    def checkpoint_interval_ms(self) -> int | None: ...
    @property
    def table_spill_threshold(self) -> int: ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# ExecuteResult
# ---------------------------------------------------------------------------

class ExecuteResult:
    """The result of a SQL execute() call.

    Supports int(result) for backward-compatible row count access.
    """

    @property
    def result_type(self) -> str:
        """One of 'ddl', 'rows_affected', 'query', or 'metadata'."""
        ...
    @property
    def rows_affected(self) -> int: ...
    @property
    def ddl_type(self) -> str | None: ...
    @property
    def ddl_object(self) -> str | None: ...

    def to_query_result(self) -> QueryResult | None:
        """Convert to a QueryResult if this was a query or metadata result."""
        ...

    def __int__(self) -> int: ...
    def __bool__(self) -> bool: ...
    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Connection
# ---------------------------------------------------------------------------

class Connection:
    """A connection to a LaminarDB database."""

    @property
    def is_closed(self) -> bool:
        """Whether the connection is closed."""
        ...
    @property
    def is_checkpoint_enabled(self) -> bool:
        """Whether checkpointing is enabled for this connection."""
        ...
    @property
    def pipeline_state(self) -> str:
        """Get the pipeline state as a string."""
        ...
    @property
    def pipeline_watermark(self) -> int:
        """Get the global pipeline watermark."""
        ...
    @property
    def total_events_processed(self) -> int:
        """Get total events processed across all sources."""
        ...
    @property
    def source_count(self) -> int:
        """Get the number of registered sources."""
        ...
    @property
    def sink_count(self) -> int:
        """Get the number of registered sinks."""
        ...
    @property
    def active_query_count(self) -> int:
        """Get the number of active queries."""
        ...

    def __enter__(self) -> Connection: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None: ...
    async def __aenter__(self) -> Connection: ...
    async def __aexit__(
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

    def stream(self, sql: str) -> _QueryStreamIter:
        """Execute a SQL query and stream results in batches."""
        ...

    def subscribe(self, sql: str) -> Subscription:
        """Subscribe to a continuous query (sync iterator)."""
        ...

    async def subscribe_async(self, sql: str) -> AsyncSubscription:
        """Subscribe to a continuous query (async iterator)."""
        ...

    def subscribe_stream(self, name: str) -> StreamSubscription:
        """Subscribe to a named stream (sync iterator).

        Unlike subscribe(sql), this subscribes to a named stream
        created via CREATE STREAM ... AS SELECT ...
        """
        ...

    async def subscribe_stream_async(self, name: str) -> AsyncStreamSubscription:
        """Subscribe to a named stream (async iterator).

        Unlike subscribe_async(sql), this subscribes to a named stream
        created via CREATE STREAM ... AS SELECT ...
        """
        ...

    def schema(self, table: str) -> Any:
        """Get the schema of a table as a PyArrow Schema."""
        ...

    def create_table(self, name: str, schema: Any) -> None:
        """Create a new table with the given schema."""
        ...

    def list_tables(self) -> list[str]:
        """List all sources in the database."""
        ...

    def list_streams(self) -> list[str]:
        """List all streams in the database."""
        ...

    def list_sinks(self) -> list[str]:
        """List all sinks in the database."""
        ...

    def start(self) -> None:
        """Start the streaming pipeline."""
        ...

    def checkpoint(self) -> int | None:
        """Trigger a checkpoint. Returns the checkpoint ID or None."""
        ...

    def execute(self, sql: str) -> ExecuteResult:
        """Execute a SQL statement (DDL, DML, or query).

        Returns an ExecuteResult. Use int(result) for backward-compatible
        row count access.
        """
        ...

    # ── DuckDB-style aliases ──

    def sql(self, query: str, params: Any = None) -> QueryResult:
        """Execute a SQL query (DuckDB-style alias for query())."""
        ...

    def tables(self) -> list[str]:
        """List all tables/sources (alias for list_tables())."""
        ...

    def materialized_views(self) -> list[str]:
        """List all materialized views/streams (alias for list_streams())."""
        ...

    def explain(self, query: str) -> str:
        """Show the query execution plan."""
        ...

    def stats(self, table: str) -> dict[str, Any]:
        """Get statistics for a table/source."""
        ...

    # ── Catalog info ──

    def sources(self) -> list[SourceInfo]:
        """List source info with schemas and watermark columns."""
        ...

    def sinks(self) -> list[SinkInfo]:
        """List sink info."""
        ...

    def streams(self) -> list[StreamInfo]:
        """List stream info with SQL definitions."""
        ...

    def queries(self) -> list[QueryInfo]:
        """List active and completed query info."""
        ...

    # ── Pipeline topology & metrics ──

    def topology(self) -> PipelineTopology:
        """Get the pipeline topology graph."""
        ...

    def metrics(self) -> PipelineMetrics:
        """Get pipeline-wide metrics snapshot."""
        ...

    def source_metrics(self, name: str) -> SourceMetrics | None:
        """Get metrics for a specific source, or None if not found."""
        ...

    def all_source_metrics(self) -> list[SourceMetrics]:
        """Get metrics for all sources."""
        ...

    def stream_metrics(self, name: str) -> StreamMetrics | None:
        """Get metrics for a specific stream, or None if not found."""
        ...

    def all_stream_metrics(self) -> list[StreamMetrics]:
        """Get metrics for all streams."""
        ...

    # ── Query control & shutdown ──

    def cancel_query(self, query_id: int) -> None:
        """Cancel a running query by ID."""
        ...

    def shutdown(self) -> None:
        """Gracefully shut down the streaming pipeline."""
        ...

    def close(self) -> None:
        """Close the connection."""
        ...

# ---------------------------------------------------------------------------
# Writer
# ---------------------------------------------------------------------------

class Writer:
    """A streaming writer for batched inserts into a table."""

    @property
    def name(self) -> str:
        """The name of the source this writer is writing to."""
        ...
    @property
    def schema(self) -> Any:
        """The schema of the source as a PyArrow Schema."""
        ...
    @property
    def current_watermark(self) -> int:
        """The current watermark value."""
        ...

    def __enter__(self) -> Writer: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None: ...
    def __repr__(self) -> str: ...

    def insert(self, data: DataInput) -> None:
        """Add data to the write buffer."""
        ...

    def flush(self) -> int:
        """Flush the buffer to the database. Returns rows written."""
        ...

    def watermark(self, timestamp: int) -> None:
        """Emit a watermark timestamp."""
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
    def schema(self) -> Any:
        """The schema as a PyArrow Schema."""
        ...
    @property
    def num_rows(self) -> int: ...
    @property
    def num_columns(self) -> int: ...
    @property
    def num_batches(self) -> int: ...
    @property
    def columns(self) -> list[str]: ...

    def __repr__(self) -> str: ...
    def __len__(self) -> int:
        """Number of rows (enables len(result))."""
        ...
    def __iter__(self) -> Iterator[tuple[Any, ...]]:
        """Iterate over rows as tuples."""
        ...

    def to_arrow(self) -> Any:
        """Convert to a PyArrow Table."""
        ...

    def to_pandas(self) -> Any:
        """Convert to a Pandas DataFrame."""
        ...

    def to_polars(self) -> Any:
        """Convert to a Polars DataFrame."""
        ...

    def to_dicts(self) -> dict[str, list[Any]]:
        """Convert to a columnar dict mapping column names to value lists."""
        ...

    def to_df(self) -> Any:
        """Auto-detect best available library and convert."""
        ...

    def __arrow_c_stream__(self, requested_schema: Any = None) -> Any:
        """Export via Arrow PyCapsule interface."""
        ...

    # ── DuckDB-style methods ──

    def df(self) -> Any:
        """Convert to a Pandas DataFrame (DuckDB-style alias)."""
        ...

    def pl(self, *, lazy: bool = False) -> Any:
        """Convert to a Polars DataFrame. If lazy=True, returns LazyFrame."""
        ...

    def arrow(self) -> Any:
        """Convert to a PyArrow Table (DuckDB-style alias)."""
        ...

    def fetchall(self) -> list[tuple[Any, ...]]:
        """Fetch all rows as a list of tuples."""
        ...

    def fetchone(self) -> tuple[Any, ...] | None:
        """Fetch the first row as a tuple, or None if empty."""
        ...

    def fetchmany(self, size: int = 1) -> list[tuple[Any, ...]]:
        """Fetch up to size rows as a list of tuples."""
        ...

    def show(self, max_rows: int = 20) -> None:
        """Print a preview of the result."""
        ...

    def _repr_html_(self) -> str:
        """HTML representation for Jupyter notebooks."""
        ...

# ---------------------------------------------------------------------------
# QueryStream
# ---------------------------------------------------------------------------

class _QueryStreamIter:
    """A streaming query result iterator."""

    @property
    def is_active(self) -> bool:
        """Whether the stream is still active."""
        ...

    def try_next(self) -> QueryResult | None:
        """Non-blocking poll for the next result batch."""
        ...

    def cancel(self) -> None:
        """Cancel the stream."""
        ...

    def __repr__(self) -> str: ...
    def __iter__(self) -> Iterator[QueryResult]: ...
    def __next__(self) -> QueryResult: ...

# ---------------------------------------------------------------------------
# Subscription
# ---------------------------------------------------------------------------

class Subscription:
    """A synchronous subscription to a continuous query."""

    @property
    def is_active(self) -> bool: ...

    def __repr__(self) -> str: ...
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

    def __repr__(self) -> str: ...
    def __aiter__(self) -> AsyncIterator[QueryResult]: ...
    async def __anext__(self) -> QueryResult: ...

    def cancel(self) -> None:
        """Cancel the subscription."""
        ...

# ---------------------------------------------------------------------------
# StreamSubscription (true named-stream subscription)
# ---------------------------------------------------------------------------

class StreamSubscription:
    """A synchronous subscription to a named stream.

    Created via Connection.subscribe_stream(name). Unlike Subscription,
    this subscribes to a named stream (created via CREATE STREAM ...),
    not an arbitrary SQL query.
    """

    @property
    def is_active(self) -> bool: ...
    @property
    def schema(self) -> Any:
        """The subscription schema as a PyArrow Schema."""
        ...

    def next(self) -> QueryResult | None:
        """Blocking wait for the next batch."""
        ...

    def next_timeout(self, timeout_ms: int) -> QueryResult | None:
        """Blocking wait for the next batch with a timeout in milliseconds."""
        ...

    def try_next(self) -> QueryResult | None:
        """Non-blocking poll for the next batch."""
        ...

    def cancel(self) -> None:
        """Cancel the subscription."""
        ...

    def __repr__(self) -> str: ...
    def __iter__(self) -> Iterator[QueryResult]: ...
    def __next__(self) -> QueryResult: ...

class AsyncStreamSubscription:
    """An asynchronous subscription to a named stream.

    Created via Connection.subscribe_stream_async(name). Unlike
    AsyncSubscription, this subscribes to a named stream (created via
    CREATE STREAM ...), not an arbitrary SQL query.
    """

    @property
    def is_active(self) -> bool: ...
    @property
    def schema(self) -> Any:
        """The subscription schema as a PyArrow Schema."""
        ...

    def cancel(self) -> None:
        """Cancel the subscription."""
        ...

    def __repr__(self) -> str: ...
    def __aiter__(self) -> AsyncIterator[QueryResult]: ...
    async def __anext__(self) -> QueryResult: ...

# ---------------------------------------------------------------------------
# Catalog info
# ---------------------------------------------------------------------------

class SourceInfo:
    """Information about a registered source."""

    @property
    def name(self) -> str: ...
    @property
    def schema(self) -> Any:
        """The source schema as a PyArrow Schema."""
        ...
    @property
    def watermark_column(self) -> str | None: ...

    def __repr__(self) -> str: ...

class SinkInfo:
    """Information about a registered sink."""

    @property
    def name(self) -> str: ...

    def __repr__(self) -> str: ...

class StreamInfo:
    """Information about a registered stream."""

    @property
    def name(self) -> str: ...
    @property
    def sql(self) -> str | None: ...

    def __repr__(self) -> str: ...

class QueryInfo:
    """Information about a registered query."""

    @property
    def id(self) -> int: ...
    @property
    def sql(self) -> str: ...
    @property
    def active(self) -> bool: ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Pipeline topology
# ---------------------------------------------------------------------------

class PipelineNode:
    """A node in the pipeline topology graph."""

    @property
    def name(self) -> str: ...
    @property
    def node_type(self) -> str:
        """One of 'source', 'stream', or 'sink'."""
        ...
    @property
    def schema(self) -> Any | None:
        """The node schema as a PyArrow Schema, or None."""
        ...
    @property
    def sql(self) -> str | None: ...

    def __repr__(self) -> str: ...

class PipelineEdge:
    """An edge in the pipeline topology graph."""

    @property
    def from_node(self) -> str:
        """Source node name."""
        ...
    @property
    def to_node(self) -> str:
        """Target node name."""
        ...

    def __repr__(self) -> str: ...

class PipelineTopology:
    """The pipeline topology: all nodes and edges."""

    @property
    def nodes(self) -> list[PipelineNode]: ...
    @property
    def edges(self) -> list[PipelineEdge]: ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Pipeline metrics
# ---------------------------------------------------------------------------

class PipelineMetrics:
    """Pipeline-wide metrics snapshot."""

    @property
    def total_events_ingested(self) -> int: ...
    @property
    def total_events_emitted(self) -> int: ...
    @property
    def total_events_dropped(self) -> int: ...
    @property
    def total_cycles(self) -> int: ...
    @property
    def total_batches(self) -> int: ...
    @property
    def uptime_secs(self) -> float: ...
    @property
    def state(self) -> str: ...
    @property
    def source_count(self) -> int: ...
    @property
    def stream_count(self) -> int: ...
    @property
    def sink_count(self) -> int: ...
    @property
    def pipeline_watermark(self) -> int: ...

    def __repr__(self) -> str: ...

class SourceMetrics:
    """Metrics for a specific source."""

    @property
    def name(self) -> str: ...
    @property
    def total_events(self) -> int: ...
    @property
    def pending(self) -> int: ...
    @property
    def capacity(self) -> int: ...
    @property
    def is_backpressured(self) -> bool: ...
    @property
    def watermark(self) -> int: ...
    @property
    def utilization(self) -> float: ...

    def __repr__(self) -> str: ...

class StreamMetrics:
    """Metrics for a specific stream."""

    @property
    def name(self) -> str: ...
    @property
    def total_events(self) -> int: ...
    @property
    def pending(self) -> int: ...
    @property
    def capacity(self) -> int: ...
    @property
    def is_backpressured(self) -> bool: ...
    @property
    def watermark(self) -> int: ...
    @property
    def sql(self) -> str | None: ...

    def __repr__(self) -> str: ...

# ---------------------------------------------------------------------------
# Module-level functions
# ---------------------------------------------------------------------------

def open(path: str, *, config: LaminarConfig | None = None) -> Connection:
    """Open a LaminarDB database at the given file path."""
    ...

def connect(uri: str) -> Connection:
    """Connect to a LaminarDB database via URI."""
    ...
