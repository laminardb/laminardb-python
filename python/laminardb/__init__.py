"""LaminarDB Python bindings — streaming SQL database.

LaminarDB is a streaming SQL database that supports real-time data
ingestion, SQL queries, and continuous query subscriptions.

Key classes:
    Connection      Manage database connections and operations.
    Writer          Streaming writer for batched inserts.
    QueryResult     SQL query results with multi-format export.
    Subscription    Synchronous continuous query subscription.
    AsyncSubscription  Asynchronous continuous query subscription.
    StreamSubscription  Synchronous named-stream subscription.
    AsyncStreamSubscription  Asynchronous named-stream subscription.
    CallbackSubscription  Push-based subscription with callbacks.
    MaterializedView  High-level wrapper for named streams.
    Schema          Convenience wrapper around PyArrow Schema.
    ChangeEvent     A batch of change rows from a subscription.
    ChangeRow       A single row from a change stream.

Functions:
    open(path)      Open a LaminarDB database (currently in-memory).
    connect(uri)    Open a LaminarDB database (URI reserved for future use).
    sql(query)      Execute SQL on the default connection.
    execute(query)  Execute SQL on the default connection.

Exceptions:
    LaminarError        Base exception for all LaminarDB errors.
    ConnectionError     Connection lifecycle errors.
    QueryError          SQL query failures.
    IngestionError      Data insertion failures.
    SchemaError         Schema operation failures.
    SubscriptionError   Subscription failures.
    StreamError         Stream/materialized view failures.
    CheckpointError     Checkpoint failures.
    ConnectorError      Connector failures.
"""

import threading

from laminardb._laminardb import (
    AsyncStreamSubscription,
    AsyncSubscription,
    CallbackSubscription,
    CheckpointError,
    CheckpointResult,
    Connection,
    ConnectionError,
    ConnectorError,
    ExecuteResult,
    IngestionError,
    LaminarConfig,
    LaminarError,
    PipelineEdge,
    PipelineMetrics,
    PipelineNode,
    PipelineTopology,
    QueryError,
    QueryInfo,
    QueryResult,
    SchemaError,
    SinkInfo,
    SourceInfo,
    SourceMetrics,
    StreamError,
    StreamInfo,
    StreamMetrics,
    StreamSubscription,
    Subscription,
    SubscriptionError,
    Writer,
    __version__,
    codes,
    connect,
    open,
)

from laminardb.types import (
    ChangeEvent,
    ChangeRow,
    CheckpointStatus,
    Column,
    MaterializedView,
    Metrics,
    Schema,
    TableStats,
    Watermark,
)

# ---------------------------------------------------------------------------
# Aliases
# ---------------------------------------------------------------------------

Config = LaminarConfig
BatchWriter = Writer

# ---------------------------------------------------------------------------
# Module-level default connection
# ---------------------------------------------------------------------------

_default_connection: Connection | None = None
_default_lock = threading.Lock()


def _get_default() -> Connection:
    """Get or create the default in-memory connection."""
    global _default_connection
    with _default_lock:
        if _default_connection is None or _default_connection.is_closed:
            _default_connection = open(":memory:")
        return _default_connection


def sql(query: str) -> QueryResult:
    """Execute a SQL query on the default connection.

    Args:
        query: The SQL query to execute.

    Returns:
        QueryResult with the query results.
    """
    return _get_default().sql(query)


def execute(query: str) -> ExecuteResult:
    """Execute a SQL statement on the default connection.

    Args:
        query: The SQL statement to execute.

    Returns:
        ExecuteResult with the statement result.
    """
    return _get_default().execute(query)


def mv(conn: Connection, name: str, sql_def: str | None = None) -> MaterializedView:
    """Create a MaterializedView helper for a named stream.

    Args:
        conn: The connection to use.
        name: The stream/view name.
        sql_def: Optional SQL definition.

    Returns:
        A MaterializedView wrapper.
    """
    return MaterializedView(conn, name, sql_def)


__all__ = [
    "__version__",
    "codes",
    "connect",
    "open",
    # Module-level functions
    "sql",
    "execute",
    "mv",
    # Core classes
    "CheckpointResult",
    "Connection",
    "ExecuteResult",
    "LaminarConfig",
    "Writer",
    "QueryResult",
    "Subscription",
    "AsyncSubscription",
    "StreamSubscription",
    "AsyncStreamSubscription",
    "CallbackSubscription",
    # Aliases
    "Config",
    "BatchWriter",
    # High-level types
    "MaterializedView",
    "Schema",
    "Column",
    "ChangeEvent",
    "ChangeRow",
    "CheckpointStatus",
    "Metrics",
    "TableStats",
    "Watermark",
    # Catalog info
    "SourceInfo",
    "SinkInfo",
    "StreamInfo",
    "QueryInfo",
    # Pipeline topology
    "PipelineNode",
    "PipelineEdge",
    "PipelineTopology",
    # Pipeline metrics
    "PipelineMetrics",
    "SourceMetrics",
    "StreamMetrics",
    # Exceptions
    "LaminarError",
    "ConnectionError",
    "QueryError",
    "IngestionError",
    "SchemaError",
    "SubscriptionError",
    "StreamError",
    "CheckpointError",
    "ConnectorError",
]
