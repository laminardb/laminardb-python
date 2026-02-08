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


class TestConnectionProperties:
    def test_is_closed_false_when_open(self, db):
        assert db.is_closed is False

    def test_is_closed_true_after_close(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        assert conn.is_closed is True

    def test_is_checkpoint_enabled(self, db):
        # Just verify the property is accessible and returns a bool
        result = db.is_checkpoint_enabled
        assert isinstance(result, bool)

    def test_checkpoint_disabled_raises(self, db):
        # Checkpointing is disabled by default; checkpoint() raises
        with pytest.raises(laminardb.LaminarError):
            db.checkpoint()


class TestPipelineOperations:
    def test_list_streams(self, db):
        streams = db.list_streams()
        assert isinstance(streams, list)

    def test_list_sinks(self, db):
        sinks = db.list_sinks()
        assert isinstance(sinks, list)

    def test_start(self, db):
        # start() should not raise on a fresh connection
        db.start()

    def test_list_streams_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.list_streams()

    def test_list_sinks_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.list_sinks()

    def test_start_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.start()


class TestConnectionRepr:
    def test_repr_open(self, db):
        assert "open" in repr(db)

    def test_repr_closed(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        assert "closed" in repr(conn)
