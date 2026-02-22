"""Tests for Python wrapper types in laminardb.types."""

import pytest

import laminardb
from laminardb.types import (
    ChangeEvent,
    ChangeRow,
    Column,
    MaterializedView,
    Metrics,
    Schema,
)
from conftest import requires_pyarrow


# ---------------------------------------------------------------------------
# Column
# ---------------------------------------------------------------------------


class TestColumn:
    def test_construction(self):
        col = Column(name="id", type="int64", nullable=False)
        assert col.name == "id"
        assert col.type == "int64"
        assert col.nullable is False

    def test_defaults(self):
        col = Column(name="name", type="string")
        assert col.nullable is True

    def test_frozen(self):
        col = Column(name="id", type="int64")
        with pytest.raises(AttributeError):
            col.name = "other"


# ---------------------------------------------------------------------------
# Schema
# ---------------------------------------------------------------------------


@requires_pyarrow
class TestSchema:
    def test_from_pyarrow(self):
        import pyarrow as pa

        pa_schema = pa.schema([
            pa.field("id", pa.int64()),
            pa.field("name", pa.utf8(), nullable=True),
        ])
        schema = Schema(pa_schema)
        assert len(schema) == 2

    def test_columns(self):
        import pyarrow as pa

        pa_schema = pa.schema([
            pa.field("id", pa.int64()),
            pa.field("name", pa.utf8()),
        ])
        schema = Schema(pa_schema)
        cols = schema.columns
        assert len(cols) == 2
        assert cols[0].name == "id"
        assert cols[1].name == "name"

    def test_names(self):
        import pyarrow as pa

        pa_schema = pa.schema([
            pa.field("a", pa.int64()),
            pa.field("b", pa.float64()),
        ])
        schema = Schema(pa_schema)
        assert schema.names == ["a", "b"]

    def test_getitem_int(self):
        import pyarrow as pa

        pa_schema = pa.schema([
            pa.field("x", pa.int32()),
        ])
        schema = Schema(pa_schema)
        col = schema[0]
        assert col.name == "x"

    def test_getitem_str(self):
        import pyarrow as pa

        pa_schema = pa.schema([
            pa.field("x", pa.int32()),
            pa.field("y", pa.float64()),
        ])
        schema = Schema(pa_schema)
        col = schema["y"]
        assert col.name == "y"

    def test_arrow_schema(self):
        import pyarrow as pa

        pa_schema = pa.schema([pa.field("id", pa.int64())])
        schema = Schema(pa_schema)
        assert schema.arrow_schema is pa_schema

    def test_repr(self):
        import pyarrow as pa

        pa_schema = pa.schema([pa.field("id", pa.int64())])
        schema = Schema(pa_schema)
        r = repr(schema)
        assert "Schema" in r
        assert "id" in r


# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------


class TestMetrics:
    def test_construction(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "metrics_test.db"))
        pm = conn.metrics()
        m = Metrics(pm)
        assert m.uptime_secs >= 0
        assert isinstance(m.state, str)
        assert m.events_per_second >= 0
        r = repr(m)
        assert "Metrics" in r
        conn.close()


# ---------------------------------------------------------------------------
# ChangeRow
# ---------------------------------------------------------------------------


class TestChangeRow:
    def test_construction(self):
        row = ChangeRow("Insert", {"id": 1, "name": "test"})
        assert row.op == "Insert"
        assert row["id"] == 1
        assert row["name"] == "test"

    def test_keys(self):
        row = ChangeRow("Delete", {"a": 1, "b": 2})
        assert row.keys() == ["a", "b"]

    def test_values(self):
        row = ChangeRow("Insert", {"x": 10})
        assert row.values() == [10]

    def test_to_dict(self):
        row = ChangeRow("Insert", {"id": 1})
        d = row.to_dict()
        assert d == {"id": 1}
        # Ensure it's a copy
        d["id"] = 99
        assert row["id"] == 1

    def test_repr(self):
        row = ChangeRow("Insert", {"id": 1})
        r = repr(row)
        assert "ChangeRow" in r
        assert "Insert" in r


# ---------------------------------------------------------------------------
# ChangeEvent
# ---------------------------------------------------------------------------


@requires_pyarrow
class TestChangeEvent:
    def test_len(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "ce_test.db"))
        result = conn.query(
            "SELECT * FROM (VALUES (1, 'a'), (2, 'b')) AS t(id, name)"
        )
        event = ChangeEvent(result)
        assert len(event) == 2
        conn.close()

    def test_repr(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "ce_repr.db"))
        result = conn.query("SELECT 1 AS x")
        event = ChangeEvent(result)
        r = repr(event)
        assert "ChangeEvent" in r
        conn.close()


# ---------------------------------------------------------------------------
# MaterializedView
# ---------------------------------------------------------------------------


class TestMaterializedView:
    def test_construction(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "mv_test.db"))
        mv = MaterializedView(conn, "my_stream", "SELECT * FROM src")
        assert mv.name == "my_stream"
        assert mv.sql == "SELECT * FROM src"
        conn.close()

    def test_repr(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "mv_repr.db"))
        mv = MaterializedView(conn, "test_mv")
        r = repr(mv)
        assert "MaterializedView" in r
        assert "test_mv" in r
        conn.close()

    @requires_pyarrow
    def test_schema(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "mv_schema.db"))
        conn.create_table("src", {"id": "int64", "val": "float64"})
        mv = MaterializedView(conn, "src")
        schema = mv.schema()
        assert isinstance(schema, Schema)
        assert "id" in schema.names
        conn.close()

    @requires_pyarrow
    def test_schema_on_stream(self, tmp_path):
        """mv.schema() should work on streams, not just tables."""
        conn = laminardb.open(str(tmp_path / "mv_stream_schema.db"))
        conn.execute(
            "CREATE SOURCE sensors ("
            "  ts BIGINT NOT NULL,"
            "  device VARCHAR NOT NULL,"
            "  value DOUBLE NOT NULL"
            ")"
        )
        conn.execute(
            "CREATE STREAM high_val AS "
            "SELECT * FROM sensors WHERE value > 100.0"
        )
        conn.execute("CREATE SINK out FROM high_val")
        conn.start()
        mv = MaterializedView(conn, "high_val")
        schema = mv.schema()
        assert isinstance(schema, Schema)
        assert "device" in schema.names
        conn.close()
