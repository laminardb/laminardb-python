"""High-level Python wrapper types for LaminarDB.

These pure-Python classes provide DuckDB-style convenience wrappers
around the lower-level Rust-backed types.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Callable, Iterator

if TYPE_CHECKING:
    import pyarrow  # type: ignore[import-untyped]


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

    def __init__(self, arrow_schema: pyarrow.Schema) -> None:
        self._schema = arrow_schema

    @property
    def arrow_schema(self) -> pyarrow.Schema:
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
        f = self._schema.field(key)
        return Column(name=f.name, type=str(f.type), nullable=f.nullable)

    def __repr__(self) -> str:
        cols = ", ".join(f"{c.name}: {c.type}" for c in self.columns)
        return f"Schema({cols})"


# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------


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
        return float(self._inner.total_events_ingested / uptime)

    @property
    def total_events(self) -> int:
        """Total events ingested across all sources."""
        return int(self._inner.total_events_ingested)

    @property
    def uptime_secs(self) -> float:
        """Pipeline uptime in seconds."""
        return float(self._inner.uptime_secs)

    @property
    def state(self) -> str:
        """Pipeline state."""
        return str(self._inner.state)

    def __repr__(self) -> str:
        return (
            f"Metrics(state={self.state!r}, "
            f"eps={self.events_per_second:.1f}, "
            f"uptime={self.uptime_secs:.1f}s)"
        )


# ---------------------------------------------------------------------------
# Change tracking types
# ---------------------------------------------------------------------------


class ChangeRow(Mapping[str, Any]):
    """A single row from a change stream with its operation type.

    Implements ``collections.abc.Mapping`` so it can be used directly
    as a dict-like object.
    """

    __slots__ = ("op", "_data")

    def __init__(self, op: str, data: dict[str, Any]) -> None:
        self.op = op
        self._data = data

    # Mapping ABC
    def __getitem__(self, key: str) -> Any:
        return self._data[key]

    def __iter__(self) -> Iterator[str]:
        return iter(self._data)

    def __len__(self) -> int:
        return len(self._data)

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
            dicts = self._result.to_dicts()
            columns = list(dicts.keys())
            n_rows = len(next(iter(dicts.values()))) if columns else 0
            rows: list[ChangeRow] = []
            for i in range(n_rows):
                row_dict = {col: dicts[col][i] for col in columns}
                rows.append(ChangeRow(op=self._op, data=row_dict))
            self._rows = rows
        return self._rows

    def __iter__(self) -> Iterator[ChangeRow]:
        return iter(self._materialize())

    def __len__(self) -> int:
        return int(self._result.num_rows)

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

        Re-executes the stream's SQL definition as a snapshot query
        against the current source data.

        Args:
            where: Optional WHERE clause (without the WHERE keyword).

        Returns:
            QueryResult from the query.
        """
        return self._conn.query_stream(self._name, where or None)

    def schema(self) -> Schema:
        """Get the schema of this materialized view."""
        arrow_schema = self._conn.schema(self._name)
        return Schema(arrow_schema)

    def subscribe(
        self, handler: Callable[[ChangeEvent], None] | None = None
    ) -> Any:
        """Subscribe to changes on this materialized view.

        Args:
            handler: Optional callback.  If provided, uses a
                ``CallbackSubscription`` with proper cancellation.
                If None, returns the raw ``StreamSubscription``.

        Returns:
            StreamSubscription if no handler, otherwise a
            CallbackSubscription that can be cancelled via ``.cancel()``.
        """
        if handler is None:
            return self._conn.subscribe_stream(self._name)

        def _on_data(query_result: Any) -> None:
            event = ChangeEvent(query_result)
            handler(event)

        return self._conn.subscribe_stream_callback(self._name, _on_data)

    def __repr__(self) -> str:
        return f"MaterializedView(name={self._name!r})"
