"""Tests for SQL queries and result conversion."""

import pytest

import laminardb
from tests.conftest import requires_pandas, requires_polars, requires_pyarrow


@pytest.fixture
def populated_db(db, sample_data):
    """A database with sample data already inserted."""
    db.insert("sensors", sample_data["rows"])
    return db


class TestBasicQuery:
    def test_select_all(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors")
        assert result.num_rows == 3
        assert result.num_columns == 3

    def test_select_with_filter(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors WHERE value > 43.0")
        assert result.num_rows == 2

    def test_column_names(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors")
        assert set(result.columns) == {"ts", "device", "value"}


class TestQueryResultConversion:
    def test_to_dicts(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors")
        data = result.to_dicts()
        assert isinstance(data, (dict, list))

    @requires_pyarrow
    def test_to_arrow(self, populated_db):
        import pyarrow as pa

        result = populated_db.query("SELECT * FROM sensors")
        table = result.to_arrow()
        assert isinstance(table, pa.Table)
        assert table.num_rows == 3

    @requires_pandas
    def test_to_pandas(self, populated_db):
        import pandas as pd

        result = populated_db.query("SELECT * FROM sensors")
        df = result.to_pandas()
        assert isinstance(df, pd.DataFrame)
        assert len(df) == 3

    @requires_polars
    def test_to_polars(self, populated_db):
        import polars as pl

        result = populated_db.query("SELECT * FROM sensors")
        df = result.to_polars()
        assert isinstance(df, pl.DataFrame)
        assert len(df) == 3

    def test_to_df_auto_detect(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors")
        df = result.to_df()
        assert df is not None


class TestQueryResultProperties:
    def test_num_rows(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors")
        assert result.num_rows == 3

    def test_num_columns(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors")
        assert result.num_columns == 3

    def test_repr(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors")
        assert "rows=3" in repr(result)
        assert "columns=3" in repr(result)


@requires_pyarrow
class TestArrowPyCapsule:
    def test_arrow_c_stream(self, populated_db):
        result = populated_db.query("SELECT * FROM sensors")
        # The __arrow_c_stream__ method should be callable
        assert hasattr(result, "__arrow_c_stream__")


class TestStreamQuery:
    def test_stream_results(self, populated_db):
        total_rows = 0
        for batch in populated_db.stream("SELECT * FROM sensors"):
            total_rows += batch.num_rows
        assert total_rows == 3


class TestQueryErrors:
    def test_invalid_sql(self, db):
        with pytest.raises(laminardb.QueryError):
            db.query("INVALID SQL STATEMENT")

    def test_nonexistent_table(self, db):
        with pytest.raises(laminardb.QueryError):
            db.query("SELECT * FROM nonexistent")
