"""Tests for Connection lifecycle and management."""

import pytest

import laminardb


class TestConnectionLifecycle:
    def test_open_and_close(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        assert repr(conn) == "Connection(open)"
        conn.close()
        assert repr(conn) == "Connection(closed)"

    def test_context_manager(self, tmp_path):
        with laminardb.open(str(tmp_path / "test.db")) as conn:
            assert repr(conn) == "Connection(open)"
        assert repr(conn) == "Connection(closed)"

    def test_double_close_is_safe(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        conn.close()  # should not raise

    def test_operations_after_close_raise(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.list_tables()


class TestTableOperations:
    def test_create_and_list_tables(self, db):
        tables = db.list_tables()
        assert "sensors" in tables

    def test_create_table_with_dict_schema(self, db):
        db.create_table("logs", {"ts": "int64", "message": "string"})
        assert "logs" in db.list_tables()

    def test_get_schema(self, db):
        pytest.importorskip("pyarrow")
        schema = db.schema("sensors")
        field_names = [schema.field(i).name for i in range(len(schema))]
        assert "ts" in field_names
        assert "device" in field_names
        assert "value" in field_names


class TestConnectionRepr:
    def test_repr_open(self, db):
        assert "open" in repr(db)

    def test_repr_closed(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        assert "closed" in repr(conn)
