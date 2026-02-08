"""Tests for data ingestion in all supported formats."""

import pytest

import laminardb
from tests.conftest import requires_pandas, requires_polars, requires_pyarrow


class TestDictIngestion:
    def test_insert_single_dict(self, db, sample_data):
        count = db.insert("sensors", sample_data["row"])
        assert count == 1

    def test_insert_list_of_dicts(self, db, sample_data):
        count = db.insert("sensors", sample_data["rows"])
        assert count == 3

    def test_insert_columnar_dict(self, db, sample_data):
        count = db.insert("sensors", sample_data["columnar"])
        assert count == 3


class TestStringIngestion:
    def test_insert_json(self, db, sample_data):
        count = db.insert_json("sensors", sample_data["json"])
        assert count == 1

    def test_insert_csv(self, db, sample_data):
        count = db.insert_csv("sensors", sample_data["csv"])
        assert count == 2


@requires_pandas
class TestPandasIngestion:
    def test_insert_pandas_dataframe(self, db, sample_data):
        import pandas as pd

        df = pd.DataFrame(sample_data["rows"])
        count = db.insert("sensors", df)
        assert count == 3


@requires_polars
class TestPolarsIngestion:
    def test_insert_polars_dataframe(self, db, sample_data):
        import polars as pl

        df = pl.DataFrame(sample_data["rows"])
        count = db.insert("sensors", df)
        assert count == 3


@requires_pyarrow
class TestPyArrowIngestion:
    def test_insert_record_batch(self, db):
        import pyarrow as pa

        batch = pa.record_batch(
            {
                "ts": [1, 2],
                "device": ["a", "b"],
                "value": [1.0, 2.0],
            }
        )
        count = db.insert("sensors", batch)
        assert count == 2

    def test_insert_pyarrow_table(self, db):
        import pyarrow as pa

        table = pa.table(
            {
                "ts": [1, 2, 3],
                "device": ["a", "b", "c"],
                "value": [1.0, 2.0, 3.0],
            }
        )
        count = db.insert("sensors", table)
        assert count == 3


class TestIngestionErrors:
    def test_insert_unsupported_type(self, db):
        with pytest.raises(TypeError):
            db.insert("sensors", 42)

    def test_insert_empty_list(self, db):
        with pytest.raises(TypeError):
            db.insert("sensors", [])

    def test_insert_invalid_json(self, db):
        with pytest.raises(laminardb.IngestionError):
            db.insert_json("sensors", "not valid json")


class TestWriter:
    def test_writer_context_manager(self, db, sample_data):
        with db.writer("sensors") as w:
            w.insert(sample_data["row"])
            w.insert(sample_data["row"])
        # data is flushed on exit

    def test_writer_explicit_flush(self, db, sample_data):
        writer = db.writer("sensors")
        writer.insert(sample_data["row"])
        writer.flush()  # flush returns 0 (no row count from flush)
        writer.close()

    def test_writer_batch_insert(self, db, sample_data):
        with db.writer("sensors") as w:
            w.insert(sample_data["rows"])
            w.flush()
