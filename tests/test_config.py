"""Tests for LaminarConfig and open_with_config."""

import pytest

import laminardb


class TestLaminarConfig:
    def test_default_config(self):
        config = laminardb.LaminarConfig()
        assert config.buffer_size == 65536
        assert config.storage_dir is None
        assert config.checkpoint_interval_ms is None

    def test_custom_config(self):
        config = laminardb.LaminarConfig(
            buffer_size=1024,
        )
        assert config.buffer_size == 1024

    def test_config_repr(self):
        config = laminardb.LaminarConfig(buffer_size=2048)
        r = repr(config)
        assert "LaminarConfig" in r
        assert "2048" in r

    def test_config_exported(self):
        assert hasattr(laminardb, "LaminarConfig")


class TestOpenWithConfig:
    def test_open_with_config(self, tmp_path):
        config = laminardb.LaminarConfig(buffer_size=1024)
        conn = laminardb.open(str(tmp_path / "cfg_test.db"), config=config)
        assert not conn.is_closed
        conn.close()

    def test_open_with_config_creates_working_connection(self, tmp_path):
        config = laminardb.LaminarConfig(buffer_size=2048)
        conn = laminardb.open(str(tmp_path / "cfg_test2.db"), config=config)
        conn.create_table("test", {"id": "int64", "name": "string"})
        tables = conn.list_tables()
        assert "test" in tables
        conn.close()

    def test_open_with_checkpoint_config(self, tmp_path):
        config = laminardb.LaminarConfig(
            checkpoint_interval_ms=5000,
            storage_dir=str(tmp_path / "storage"),
        )
        conn = laminardb.open(str(tmp_path / "ckpt_test.db"), config=config)
        assert conn.is_checkpoint_enabled
        conn.close()
