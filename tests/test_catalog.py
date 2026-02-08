"""Tests for catalog info classes (SourceInfo, SinkInfo, StreamInfo, QueryInfo)."""

import pytest

import laminardb


class TestSourceInfo:
    def test_sources_returns_list(self, db):
        sources = db.sources()
        assert isinstance(sources, list)

    def test_sources_contains_created_source(self, db):
        sources = db.sources()
        names = [s.name for s in sources]
        assert "sensors" in names

    def test_source_info_properties(self, db):
        sources = db.sources()
        sensor = next(s for s in sources if s.name == "sensors")
        assert sensor.name == "sensors"
        # watermark_column may or may not be set
        assert sensor.watermark_column is None or isinstance(
            sensor.watermark_column, str
        )

    def test_source_info_schema(self, db):
        pytest.importorskip("pyarrow")
        sources = db.sources()
        sensor = next(s for s in sources if s.name == "sensors")
        schema = sensor.schema
        field_names = [schema.field(i).name for i in range(len(schema))]
        assert "ts" in field_names
        assert "device" in field_names
        assert "value" in field_names

    def test_source_info_repr(self, db):
        sources = db.sources()
        sensor = next(s for s in sources if s.name == "sensors")
        r = repr(sensor)
        assert "SourceInfo" in r
        assert "sensors" in r


class TestSinkInfo:
    def test_sinks_returns_list(self, db):
        sinks = db.sinks()
        assert isinstance(sinks, list)


class TestStreamInfo:
    def test_streams_returns_list(self, db):
        streams = db.streams()
        assert isinstance(streams, list)


class TestQueryInfo:
    def test_queries_returns_list(self, db):
        queries = db.queries()
        assert isinstance(queries, list)


class TestCatalogAfterClose:
    def test_sources_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.sources()

    def test_sinks_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.sinks()

    def test_streams_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.streams()

    def test_queries_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.queries()
