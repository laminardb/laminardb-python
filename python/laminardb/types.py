"""High-level Python wrapper types for LaminarDB.

These pure-Python classes provide DuckDB-style convenience wrappers
around the lower-level Rust-backed types.
"""

from __future__ import annotations

import threading
from dataclasses import dataclass, field
from typing import Any, Callable, Iterator

# ---------------------------------------------------------------------------
# Column & Schema
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Column:
    """A column descriptor."""

    name: str
    type: str
    nullable: bool = True


class Schema:
    """Wraps a PyArrow Schema with convenience accessors."""

    def __init__(self, arrow_schema: Any) -> None:
        self._schema = arrow_schema

    @property
    def arrow_schema(self) -> Any:
        """The underlying PyArrow Schema."""
        return self._schema

    @property
    def columns(self) -> list[Column]:
        """List of Column descriptors."""
        return [
            Column(
                name=f.name,
                type=str(f.type),
                nullable=f.nullable,
            )
            for f in self._schema
        ]

    @property
    def names(self) -> list[str]:
        """Column names."""
        return [f.name for f in self._schema]

    def __len__(self) -> int:
        return len(self._schema)

    def __getitem__(self, key: int | str) -> Column:
        if isinstance(key, int):
            f = self._schema.field(key)
        else:
            f = self._schema.field(key)
        return Column(name=f.name, type=str(f.type), nullable=f.nullable)

    def __repr__(self) -> str:
        cols = ", ".join(f"{c.name}: {c.type}" for c in self.columns)
        return f"Schema({cols})"


# ---------------------------------------------------------------------------
# Stat / status types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class TableStats:
    """Statistics for a table/source."""

    row_count: int
    size_bytes: int


@dataclass(frozen=True)
class Watermark:
    """Watermark position for a source or stream."""

    current: int
    lag_ms: int


@dataclass(frozen=True)
class CheckpointStatus:
    """Checkpoint status for a connection."""

    checkpoint_id: int | None
    enabled: bool


class Metrics:
    """Wraps PipelineMetrics with convenience properties."""

    def __init__(self, pipeline_metrics: Any) -> None:
        self._inner = pipeline_metrics

    @property
    def events_per_second(self) -> float:
        """Estimated events per second."""
        uptime = self._inner.uptime_secs
        if uptime <= 0:
            return 0.0
        return self._inner.total_events_ingested / uptime

    @property
    def memory_used_bytes(self) -> int:
        """Total events ingested (proxy for memory usage)."""
        return self._inner.total_events_ingested

    @property
    def uptime_secs(self) -> float:
        """Pipeline uptime in seconds."""
        return self._inner.uptime_secs

    @property
    def state(self) -> str:
        """Pipeline state."""
        return self._inner.state

    def __repr__(self) -> str:
        return (
            f"Metrics(state={self.state!r}, "
            f"eps={self.events_per_second:.1f}, "
            f"uptime={self.uptime_secs:.1f}s)"
        )


# ---------------------------------------------------------------------------
# Change tracking types
# ---------------------------------------------------------------------------


class ChangeRow:
    """A single row from a change stream with its operation type."""

    def __init__(self, op: str, data: dict[str, Any]) -> None:
        self.op = op
        self._data = data

    def __getitem__(self, key: str) -> Any:
        return self._data[key]

    def keys(self) -> list[str]:
        """Column names."""
        return list(self._data.keys())

    def values(self) -> list[Any]:
        """Column values."""
        return list(self._data.values())

    def to_dict(self) -> dict[str, Any]:
        """Return the row data as a dict (without the op field)."""
        return dict(self._data)

    def __repr__(self) -> str:
        return f"ChangeRow(op={self.op!r}, {self._data})"


class ChangeEvent:
    """A batch of change rows from a subscription.

    Wraps a QueryResult and lazily materializes rows as ChangeRow objects.
    """

    def __init__(self, query_result: Any, op: str = "Insert") -> None:
        self._result = query_result
        self._op = op
        self._rows: list[ChangeRow] | None = None

    def _materialize(self) -> list[ChangeRow]:
        if self._rows is None:
            rows = []
            for tup in self._result:
                row_dict = {}
                for i, col in enumerate(self._result.columns):
                    row_dict[col] = tup[i] if isinstance(tup, tuple) else tup
                rows.append(ChangeRow(op=self._op, data=row_dict))
            self._rows = rows
        return self._rows

    def __iter__(self) -> Iterator[ChangeRow]:
        return iter(self._materialize())

    def __len__(self) -> int:
        return self._result.num_rows

    def df(self) -> Any:
        """Convert to a Pandas DataFrame."""
        return self._result.df()

    def pl(self, *, lazy: bool = False) -> Any:
        """Convert to a Polars DataFrame."""
        return self._result.pl(lazy=lazy)

    def arrow(self) -> Any:
        """Convert to a PyArrow Table."""
        return self._result.arrow()

    def __repr__(self) -> str:
        return f"ChangeEvent(rows={len(self)}, op={self._op!r})"


# ---------------------------------------------------------------------------
# MaterializedView
# ---------------------------------------------------------------------------


class MaterializedView:
    """A named materialized view / stream.

    Wraps a Connection and stream name, providing query and subscribe
    convenience methods.
    """

    def __init__(self, conn: Any, name: str, sql: str | None = None) -> None:
        self._conn = conn
        self._name = name
        self._sql = sql

    @property
    def name(self) -> str:
        """The stream/view name."""
        return self._name

    @property
    def sql(self) -> str | None:
        """The SQL definition, if known."""
        return self._sql

    def query(self, where: str = "") -> Any:
        """Query the materialized view.

        Args:
            where: Optional WHERE clause (without the WHERE keyword).

        Returns:
            QueryResult from the query.
        """
        sql = f"SELECT * FROM {self._name}"
        if where:
            sql += f" WHERE {where}"
        return self._conn.sql(sql)

    def schema(self) -> Schema:
        """Get the schema of this materialized view."""
        arrow_schema = self._conn.schema(self._name)
        return Schema(arrow_schema)

    def subscribe(
        self, handler: Callable[[ChangeEvent], None] | None = None
    ) -> Any:
        """Subscribe to changes on this materialized view.

        Args:
            handler: Optional callback. If provided, starts a background
                daemon thread that calls handler(ChangeEvent) for each
                batch. If None, returns the raw StreamSubscription.

        Returns:
            StreamSubscription if no handler, otherwise the background
            thread object.
        """
        sub = self._conn.subscribe_stream(self._name)
        if handler is None:
            return sub

        def _run() -> None:
            for batch in sub:
                event = ChangeEvent(batch)
                handler(event)

        t = threading.Thread(target=_run, daemon=True)
        t.start()
        return t

    def __repr__(self) -> str:
        return f"MaterializedView(name={self._name!r})"
