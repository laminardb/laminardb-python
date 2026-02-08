"""LaminarDB Python bindings — streaming SQL database.

LaminarDB is a streaming SQL database that supports real-time data
ingestion, SQL queries, and continuous query subscriptions.

Key classes:
    Connection      Manage database connections and operations.
    Writer          Streaming writer for batched inserts.
    QueryResult     SQL query results with multi-format export.
    Subscription    Synchronous continuous query subscription.
    AsyncSubscription  Asynchronous continuous query subscription.

Functions:
    open(path)      Open a file-based LaminarDB database.
    connect(uri)    Connect to a LaminarDB database via URI.

Exceptions:
    LaminarError        Base exception for all LaminarDB errors.
    ConnectionError     Connection lifecycle errors.
    QueryError          SQL query failures.
    IngestionError      Data insertion failures.
    SchemaError         Schema operation failures.
    SubscriptionError   Subscription failures.
"""

from laminardb._laminardb import (
    AsyncSubscription,
    Connection,
    ConnectionError,
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
    StreamInfo,
    StreamMetrics,
    Subscription,
    SubscriptionError,
    Writer,
    __version__,
    codes,
    connect,
    open,
)

__all__ = [
    "__version__",
    "codes",
    "connect",
    "open",
    "Connection",
    "ExecuteResult",
    "LaminarConfig",
    "Writer",
    "QueryResult",
    "Subscription",
    "AsyncSubscription",
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
]
