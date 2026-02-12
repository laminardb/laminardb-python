"""Tests for DuckDB-style API aliases on QueryResult and Connection."""

import pytest

import laminardb
from conftest import requires_pandas, requires_polars, requires_pyarrow

SAMPLE_SQL = """
    SELECT * FROM (VALUES
        (1, 'sensor_a', 42.0),
        (2, 'sensor_b', 43.5),
        (3, 'sensor_a', 44.1)
    ) AS t(ts, device, value)
"""


@pytest.fixture
def conn(tmp_path):
    c = laminardb.open(str(tmp_path / "duckdb_api_test.db"))
    yield c
    c.close()


# ---------------------------------------------------------------------------
# QueryResult DuckDB-style aliases
# ---------------------------------------------------------------------------


class TestQueryResultAliases:
    @requires_pandas
    def test_df(self, conn):
        import pandas as pd

        result = conn.query(SAMPLE_SQL)
        df = result.df()
        assert isinstance(df, pd.DataFrame)
        assert len(df) == 3

    @requires_polars
    def test_pl(self, conn):
        import polars as pl

        result = conn.query(SAMPLE_SQL)
        df = result.pl()
        assert isinstance(df, pl.DataFrame)
        assert len(df) == 3

    @requires_polars
    def test_pl_lazy(self, conn):
        import polars as pl

        result = conn.query(SAMPLE_SQL)
        lf = result.pl(lazy=True)
        assert isinstance(lf, pl.LazyFrame)
        df = lf.collect()
        assert len(df) == 3

    @requires_pyarrow
    def test_arrow(self, conn):
        import pyarrow as pa

        result = conn.query(SAMPLE_SQL)
        table = result.arrow()
        assert isinstance(table, pa.Table)
        assert table.num_rows == 3

    @requires_pyarrow
    def test_fetchall(self, conn):
        result = conn.query(SAMPLE_SQL)
        rows = result.fetchall()
        assert isinstance(rows, list)
        assert len(rows) == 3
        assert all(isinstance(r, tuple) for r in rows)

    @requires_pyarrow
    def test_fetchone(self, conn):
        result = conn.query(SAMPLE_SQL)
        row = result.fetchone()
        assert isinstance(row, tuple)

    @requires_pyarrow
    def test_fetchone_empty(self, conn):
        result = conn.query(
            "SELECT * FROM (VALUES (1)) AS t(x) WHERE x > 100"
        )
        row = result.fetchone()
        assert row is None

    @requires_pyarrow
    def test_fetchmany(self, conn):
        result = conn.query(SAMPLE_SQL)
        rows = result.fetchmany(2)
        assert isinstance(rows, list)
        assert len(rows) == 2

    @requires_pyarrow
    def test_fetchmany_default(self, conn):
        result = conn.query(SAMPLE_SQL)
        rows = result.fetchmany()
        assert len(rows) == 1

    @requires_pyarrow
    def test_show(self, conn, capsys):
        result = conn.query(SAMPLE_SQL)
        result.show()
        captured = capsys.readouterr()
        assert "sensor_a" in captured.out

    @requires_pandas
    def test_repr_html(self, conn):
        result = conn.query(SAMPLE_SQL)
        html = result._repr_html_()
        assert isinstance(html, str)
        assert "<" in html  # Should contain HTML tags

    def test_len(self, conn):
        result = conn.query(SAMPLE_SQL)
        assert len(result) == 3

    @requires_pyarrow
    def test_iter(self, conn):
        result = conn.query(SAMPLE_SQL)
        rows = list(result)
        assert len(rows) == 3
        assert all(isinstance(r, tuple) for r in rows)


# ---------------------------------------------------------------------------
# Connection DuckDB-style aliases
# ---------------------------------------------------------------------------


class TestConnectionAliases:
    def test_sql(self, conn):
        result = conn.sql(SAMPLE_SQL)
        assert isinstance(result, laminardb.QueryResult)
        assert result.num_rows == 3

    def test_sql_with_filter(self, conn):
        result = conn.sql(
            f"SELECT * FROM ({SAMPLE_SQL}) sub WHERE value > 43.0"
        )
        assert result.num_rows == 2

    def test_tables(self, conn):
        conn.create_table(
            "test_src",
            {"id": "int64", "name": "string"},
        )
        tables = conn.tables()
        assert "test_src" in tables

    def test_tables_matches_list_tables(self, conn):
        assert conn.tables() == conn.list_tables()

    def test_materialized_views(self, conn):
        views = conn.materialized_views()
        assert isinstance(views, list)

    def test_materialized_views_matches_list_streams(self, conn):
        assert conn.materialized_views() == conn.list_streams()

    def test_explain(self, conn):
        plan = conn.explain(SAMPLE_SQL)
        assert isinstance(plan, str)

    def test_stats(self, conn):
        conn.create_table(
            "stats_src",
            {"id": "int64", "val": "float64"},
        )
        s = conn.stats("stats_src")
        assert isinstance(s, dict)
        assert s["name"] == "stats_src"
