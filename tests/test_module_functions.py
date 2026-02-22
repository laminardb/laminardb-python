"""Tests for module-level functions, aliases, and new exceptions."""

import pytest

import laminardb


SAMPLE_SQL = """
    SELECT * FROM (VALUES
        (1, 'a', 10.0),
        (2, 'b', 20.0)
    ) AS t(id, name, value)
"""


# ---------------------------------------------------------------------------
# Module-level sql() / execute()
# ---------------------------------------------------------------------------


class TestModuleLevelFunctions:
    def test_sql(self):
        result = laminardb.sql(SAMPLE_SQL)
        assert isinstance(result, laminardb.QueryResult)
        assert result.num_rows == 2

    def test_sql_with_filter(self):
        result = laminardb.sql(
            f"SELECT * FROM ({SAMPLE_SQL}) sub WHERE value > 15.0"
        )
        assert result.num_rows == 1

    def test_execute(self):
        result = laminardb.execute(
            "SELECT 1 AS x"
        )
        assert isinstance(result, laminardb.ExecuteResult)


# ---------------------------------------------------------------------------
# Aliases
# ---------------------------------------------------------------------------


class TestAliases:
    def test_config_alias(self):
        assert laminardb.Config is laminardb.LaminarConfig

    def test_batch_writer_alias(self):
        assert laminardb.BatchWriter is laminardb.Writer

    def test_config_alias_works(self):
        cfg = laminardb.Config(buffer_size=1024)
        assert cfg.buffer_size == 1024


# ---------------------------------------------------------------------------
# New exceptions
# ---------------------------------------------------------------------------


class TestNewExceptions:
    def test_stream_error_is_laminar_error(self):
        assert issubclass(laminardb.StreamError, laminardb.LaminarError)

    def test_checkpoint_error_is_laminar_error(self):
        assert issubclass(laminardb.CheckpointError, laminardb.LaminarError)

    def test_connector_error_is_laminar_error(self):
        assert issubclass(laminardb.ConnectorError, laminardb.LaminarError)

    def test_stream_error_raise(self):
        with pytest.raises(laminardb.StreamError):
            raise laminardb.StreamError("test stream error")

    def test_checkpoint_error_raise(self):
        with pytest.raises(laminardb.CheckpointError):
            raise laminardb.CheckpointError("test checkpoint error")

    def test_connector_error_raise(self):
        with pytest.raises(laminardb.ConnectorError):
            raise laminardb.ConnectorError("test connector error")

    def test_stream_error_caught_as_base(self):
        with pytest.raises(laminardb.LaminarError):
            raise laminardb.StreamError("caught as base")

    def test_all_exceptions_in_module(self):
        assert hasattr(laminardb, "StreamError")
        assert hasattr(laminardb, "CheckpointError")
        assert hasattr(laminardb, "ConnectorError")


# ---------------------------------------------------------------------------
# __all__ exports
# ---------------------------------------------------------------------------


class TestExports:
    def test_all_contains_new_types(self):
        expected = [
            "MaterializedView",
            "Schema",
            "Column",
            "ChangeEvent",
            "ChangeRow",
            "Metrics",
        ]
        for name in expected:
            assert name in laminardb.__all__, f"{name} not in __all__"

    def test_all_contains_new_exceptions(self):
        for name in ["StreamError", "CheckpointError", "ConnectorError"]:
            assert name in laminardb.__all__, f"{name} not in __all__"

    def test_all_contains_aliases(self):
        for name in ["Config", "BatchWriter"]:
            assert name in laminardb.__all__, f"{name} not in __all__"

    def test_all_contains_module_functions(self):
        for name in ["sql", "execute", "mv"]:
            assert name in laminardb.__all__, f"{name} not in __all__"
