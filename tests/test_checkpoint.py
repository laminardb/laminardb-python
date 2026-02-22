"""Tests for checkpoint APIs."""

import pytest

import laminardb


class TestCheckpointDisabled:
    """Tests for checkpoint operations when checkpointing is disabled (default)."""

    def test_checkpoint_raises_when_disabled(self, db):
        with pytest.raises(laminardb.LaminarError):
            db.checkpoint()

    def test_is_checkpoint_enabled_false(self, db):
        assert db.is_checkpoint_enabled is False


class TestCheckpointEnabled:
    """Tests for checkpoint operations when checkpointing is enabled."""

    @pytest.fixture
    def ckpt_db(self, tmp_path):
        storage = tmp_path / "storage"
        storage.mkdir()
        config = laminardb.LaminarConfig(
            storage_dir=str(storage),
            checkpoint_interval_ms=60_000,
        )
        conn = laminardb.open("ckpt_test", config=config)
        conn.create_table(
            "events",
            {"ts": "int64", "value": "float64"},
        )
        conn.start()
        yield conn
        conn.close()

    def test_is_checkpoint_enabled_true(self, ckpt_db):
        assert ckpt_db.is_checkpoint_enabled is True

    def test_checkpoint_returns_result(self, ckpt_db):
        result = ckpt_db.checkpoint()
        assert isinstance(result, laminardb.CheckpointResult)
        assert result.checkpoint_id >= 0

    def test_checkpoint_result_bool(self, ckpt_db):
        result = ckpt_db.checkpoint()
        assert bool(result) is True

    def test_checkpoint_result_int(self, ckpt_db):
        result = ckpt_db.checkpoint()
        assert int(result) == result.checkpoint_id

    def test_checkpoint_result_repr(self, ckpt_db):
        result = ckpt_db.checkpoint()
        assert "CheckpointResult" in repr(result)
        assert str(result.checkpoint_id) in repr(result)

    def test_multiple_checkpoints_increasing_ids(self, ckpt_db):
        r1 = ckpt_db.checkpoint()
        r2 = ckpt_db.checkpoint()
        assert r2.checkpoint_id > r1.checkpoint_id


class TestCheckpointAfterClose:
    """Tests that checkpoint methods raise after the connection is closed."""

    def test_checkpoint_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            conn.checkpoint()

    def test_is_checkpoint_enabled_after_close_raises(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        conn.close()
        with pytest.raises(laminardb.ConnectionError, match="closed"):
            _ = conn.is_checkpoint_enabled
