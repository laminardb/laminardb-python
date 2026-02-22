"""Tests for SQL queries and result conversion."""

import pytest

import laminardb
from conftest import requires_pandas, requires_polars, requires_pyarrow


@pytest.fixture
def conn(tmp_path):
    """A bare connection for SQL tests (no source needed)."""
    c = laminardb.open(str(tmp_path / "query_test.db"))
    yield c
    c.close()


# Inline SQL that produces 3 rows, matching the "sensors" shape.
SAMPLE_SQL = """
    SELECT * FROM (VALUES
        (1, 'sensor_a', 42.0),
        (2, 'sensor_b', 43.5),
        (3, 'sensor_a', 44.1)
    ) AS t(ts, device, value)
"""


class TestBasicQuery:
    def test_select_all(self, conn):
        result = conn.query(SAMPLE_SQL)
        assert result.num_rows == 3

    def test_select_with_filter(self, conn):
        result = conn.query(f"SELECT * FROM ({SAMPLE_SQL}) sub WHERE value > 43.0")
        assert result.num_rows == 2

    def test_column_names(self, conn):
        result = conn.query(SAMPLE_SQL)
        assert set(result.columns) == {"ts", "device", "value"}


class TestQueryResultConversion:
    def test_to_dicts(self, conn):
        result = conn.query(SAMPLE_SQL)
        data = result.to_dicts()
        assert isinstance(data, dict)
        assert len(data["ts"]) == 3

    @requires_pyarrow
    def test_to_arrow(self, conn):
        import pyarrow as pa

        result = conn.query(SAMPLE_SQL)
        table = result.to_arrow()
        assert isinstance(table, pa.Table)
        assert table.num_rows == 3

    @requires_pandas
    def test_to_pandas(self, conn):
        import pandas as pd

        result = conn.query(SAMPLE_SQL)
        df = result.to_pandas()
        assert isinstance(df, pd.DataFrame)
        assert len(df) == 3

    @requires_polars
    def test_to_polars(self, conn):
        import polars as pl

        result = conn.query(SAMPLE_SQL)
        df = result.to_polars()
        assert isinstance(df, pl.DataFrame)
        assert len(df) == 3

    def test_to_df_auto_detect(self, conn):
        result = conn.query(SAMPLE_SQL)
        df = result.to_df()
        assert len(df) == 3


class TestQueryResultProperties:
    def test_num_rows(self, conn):
        result = conn.query(SAMPLE_SQL)
        assert result.num_rows == 3

    def test_num_columns(self, conn):
        result = conn.query(SAMPLE_SQL)
        assert result.num_columns == 3

    def test_num_batches(self, conn):
        result = conn.query(SAMPLE_SQL)
        assert result.num_batches >= 1

    @requires_pyarrow
    def test_schema_property(self, conn):
        import pyarrow as pa

        result = conn.query(SAMPLE_SQL)
        schema = result.schema
        assert isinstance(schema, pa.Schema)
        assert len(schema) == 3

    def test_repr(self, conn):
        result = conn.query(SAMPLE_SQL)
        assert "rows=3" in repr(result)
        assert "columns=3" in repr(result)


@requires_pyarrow
class TestArrowPyCapsule:
    def test_arrow_c_stream(self, conn):
        result = conn.query(SAMPLE_SQL)
        # The __arrow_c_stream__ method should be callable
        assert hasattr(result, "__arrow_c_stream__")


class TestStreamQuery:
    def test_stream_results(self, conn):
        total_rows = 0
        for batch in conn.stream(SAMPLE_SQL):
            total_rows += batch.num_rows
        assert total_rows == 3

    def test_stream_is_active(self, conn):
        stream = conn.stream(SAMPLE_SQL)
        assert stream.is_active
        # cancel to stop
        stream.cancel()
        assert not stream.is_active

    def test_stream_cancel(self, conn):
        stream = conn.stream(SAMPLE_SQL)
        stream.cancel()
        assert not stream.is_active

    def test_stream_try_next(self, conn):
        stream = conn.stream(SAMPLE_SQL)
        # try_next may return a result or None (non-blocking)
        result = stream.try_next()
        assert result is None or isinstance(result, laminardb.QueryResult)
        stream.cancel()

    def test_stream_repr(self, conn):
        stream = conn.stream(SAMPLE_SQL)
        r = repr(stream)
        assert "QueryStream" in r
        stream.cancel()


class TestQueryErrors:
    def test_invalid_sql(self, conn):
        with pytest.raises(laminardb.QueryError):
            conn.query("INVALID SQL STATEMENT")

    def test_nonexistent_table(self, conn):
        with pytest.raises(laminardb.LaminarError):
            conn.query("SELECT * FROM nonexistent")
