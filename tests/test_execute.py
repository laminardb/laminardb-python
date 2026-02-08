"""Tests for ExecuteResult and richer execute() return type."""

import pytest

import laminardb


@pytest.fixture
def conn(tmp_path):
    """A connection for execute tests."""
    c = laminardb.open(str(tmp_path / "exec_test.db"))
    yield c
    c.close()


class TestExecuteResult:
    def test_ddl_result(self, conn):
        result = conn.execute(
            "CREATE SOURCE sensors (ts BIGINT, device VARCHAR, value DOUBLE)"
        )
        assert result.result_type == "ddl"
        assert result.ddl_type is not None
        assert "CREATE" in result.ddl_type
        assert result.ddl_object == "sensors"
        assert result.rows_affected == 0

    def test_ddl_int_returns_zero(self, conn):
        result = conn.execute(
            "CREATE SOURCE logs (ts BIGINT, message VARCHAR)"
        )
        assert int(result) == 0

    def test_ddl_bool_returns_true(self, conn):
        result = conn.execute(
            "CREATE SOURCE events (ts BIGINT, kind VARCHAR)"
        )
        assert bool(result) is True

    def test_repr_ddl(self, conn):
        result = conn.execute(
            "CREATE SOURCE metrics (ts BIGINT, val DOUBLE)"
        )
        r = repr(result)
        assert "ddl" in r
        assert "metrics" in r

    def test_execute_result_class_exported(self):
        assert hasattr(laminardb, "ExecuteResult")


class TestExecuteBackwardCompat:
    def test_int_coercion(self, conn):
        # DDL returns 0 when coerced to int
        result = conn.execute(
            "CREATE SOURCE compat_test (ts BIGINT, value DOUBLE)"
        )
        n = int(result)
        assert isinstance(n, int)
        assert n == 0


class TestExecuteQuery:
    def test_execute_query_result(self, conn):
        result = conn.execute(
            "SELECT * FROM (VALUES (1, 'a'), (2, 'b')) AS t(id, name)"
        )
        # Queries via execute() return a query or metadata result
        assert result.result_type in ("query", "metadata")
        qr = result.to_query_result()
        assert qr is not None
        assert qr.num_rows == 2

    def test_execute_query_repr(self, conn):
        result = conn.execute(
            "SELECT * FROM (VALUES (1, 'x')) AS t(id, name)"
        )
        r = repr(result)
        assert "ExecuteResult" in r
