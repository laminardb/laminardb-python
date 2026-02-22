"""Tests for pipeline topology and metrics classes."""

import pytest

import laminardb


class TestPipelineTopology:
    def test_topology_returns_topology(self, db):
        topo = db.topology()
        assert isinstance(topo, laminardb.PipelineTopology)

    def test_topology_has_nodes_and_edges(self, db):
        topo = db.topology()
        assert isinstance(topo.nodes, list)
        assert isinstance(topo.edges, list)

    def test_topology_contains_source_node(self, db):
        topo = db.topology()
        names = [n.name for n in topo.nodes]
        assert "sensors" in names

    def test_pipeline_node_properties(self, db):
        topo = db.topology()
        sensor = next(n for n in topo.nodes if n.name == "sensors")
        assert sensor.node_type == "source"
        assert sensor.sql is None or isinstance(sensor.sql, str)

    def test_pipeline_node_repr(self, db):
        topo = db.topology()
        sensor = next(n for n in topo.nodes if n.name == "sensors")
        r = repr(sensor)
        assert "PipelineNode" in r
        assert "sensors" in r

    def test_pipeline_edge_properties(self, db):
        topo = db.topology()
        for edge in topo.edges:
            assert isinstance(edge.from_node, str)
            assert isinstance(edge.to_node, str)
            assert "PipelineEdge" in repr(edge)

    def test_topology_repr(self, db):
        topo = db.topology()
        r = repr(topo)
        assert "PipelineTopology" in r

    def test_topology_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.topology()


class TestPipelineMetrics:
    def test_metrics_returns_pipeline_metrics(self, db):
        m = db.metrics()
        assert isinstance(m, laminardb.PipelineMetrics)

    def test_pipeline_metrics_properties(self, db):
        m = db.metrics()
        assert isinstance(m.total_events_ingested, int)
        assert isinstance(m.total_events_emitted, int)
        assert isinstance(m.total_events_dropped, int)
        assert isinstance(m.total_cycles, int)
        assert isinstance(m.total_batches, int)
        assert isinstance(m.uptime_secs, float)
        assert isinstance(m.state, str)
        assert isinstance(m.source_count, int)
        assert isinstance(m.stream_count, int)
        assert isinstance(m.sink_count, int)
        assert isinstance(m.pipeline_watermark, int)

    def test_pipeline_metrics_repr(self, db):
        m = db.metrics()
        r = repr(m)
        assert "PipelineMetrics" in r

    def test_metrics_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.metrics()


class TestSourceMetrics:
    def test_source_metrics_returns_metrics_or_none(self, db):
        m = db.source_metrics("sensors")
        # May be None if metrics haven't been populated yet
        assert m is None or isinstance(m, laminardb.SourceMetrics)

    def test_source_metrics_nonexistent_returns_none(self, db):
        m = db.source_metrics("nonexistent")
        assert m is None

    def test_all_source_metrics_returns_list(self, db):
        ms = db.all_source_metrics()
        assert isinstance(ms, list)

    def test_source_metrics_properties(self, db):
        db.insert("sensors", {"ts": 1, "device": "a", "value": 1.0})
        m = db.source_metrics("sensors")
        assert m is not None, "source_metrics should return metrics after insert"
        assert isinstance(m.name, str)
        assert isinstance(m.total_events, int)
        assert isinstance(m.pending, int)
        assert isinstance(m.capacity, int)
        assert isinstance(m.is_backpressured, bool)
        assert isinstance(m.watermark, int)
        assert isinstance(m.utilization, float)
        assert "SourceMetrics" in repr(m)


class TestStreamMetrics:
    def test_stream_metrics_nonexistent_returns_none(self, db):
        m = db.stream_metrics("nonexistent")
        assert m is None

    def test_all_stream_metrics_returns_list(self, db):
        ms = db.all_stream_metrics()
        assert isinstance(ms, list)


class TestConnectionPipelineProperties:
    def test_pipeline_state(self, db):
        state = db.pipeline_state
        assert isinstance(state, str)

    def test_pipeline_watermark(self, db):
        wm = db.pipeline_watermark
        assert isinstance(wm, int)

    def test_total_events_processed(self, db):
        total = db.total_events_processed
        assert isinstance(total, int)
        assert total >= 0

    def test_source_count(self, db):
        count = db.source_count
        assert isinstance(count, int)
        assert count >= 1  # at least "sensors"

    def test_sink_count(self, db):
        count = db.sink_count
        assert isinstance(count, int)

    def test_active_query_count(self, db):
        count = db.active_query_count
        assert isinstance(count, int)

    def test_pipeline_properties_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            _ = conn.pipeline_state
