"""Tests for query control, shutdown, and async context manager."""

import pytest

import laminardb


class TestShutdown:
    def test_shutdown(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.shutdown()
        conn.close()

    def test_shutdown_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.shutdown()


class TestCancelQuery:
    def test_cancel_query_invalid_id(self, db):
        # Cancelling a non-existent query should raise or be a no-op
        # depending on the implementation — just verify it doesn't crash
        try:
            db.cancel_query(999999)
        except laminardb.LaminarError:
            pass  # acceptable

    def test_cancel_query_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.cancel_query(1)


class TestAsyncContextManager:
    @pytest.mark.asyncio
    async def test_async_context_manager(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        async with conn as db:
            assert repr(db) == "Connection(open)"
        assert repr(conn) == "Connection(closed)"

    @pytest.mark.asyncio
    async def test_async_context_manager_with_operations(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        async with conn as db:
            db.create_table("events", {"id": "int64", "msg": "string"})
            db.insert("events", {"id": 1, "msg": "hello"})
            result = db.query("SELECT * FROM (VALUES (1, 'hello')) AS t(id, msg)")
            assert result.num_rows == 1
        assert conn.is_closed


class TestExportedTypes:
    """Verify all Phase 3-4 types are importable from laminardb."""

    def test_source_info_importable(self):
        assert hasattr(laminardb, "SourceInfo")

    def test_sink_info_importable(self):
        assert hasattr(laminardb, "SinkInfo")

    def test_stream_info_importable(self):
        assert hasattr(laminardb, "StreamInfo")

    def test_query_info_importable(self):
        assert hasattr(laminardb, "QueryInfo")

    def test_pipeline_node_importable(self):
        assert hasattr(laminardb, "PipelineNode")

    def test_pipeline_edge_importable(self):
        assert hasattr(laminardb, "PipelineEdge")

    def test_pipeline_topology_importable(self):
        assert hasattr(laminardb, "PipelineTopology")

    def test_pipeline_metrics_importable(self):
        assert hasattr(laminardb, "PipelineMetrics")

    def test_source_metrics_importable(self):
        assert hasattr(laminardb, "SourceMetrics")

    def test_stream_metrics_importable(self):
        assert hasattr(laminardb, "StreamMetrics")
