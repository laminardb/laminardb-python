"""Tests for error codes and exception attributes."""

import pytest

import laminardb


class TestErrorCodes:
    def test_codes_module_exists(self):
        assert hasattr(laminardb, "codes")

    def test_connection_codes(self):
        assert laminardb.codes.CONNECTION_FAILED == 100
        assert laminardb.codes.CONNECTION_CLOSED == 101
        assert laminardb.codes.CONNECTION_IN_USE == 102

    def test_schema_codes(self):
        assert laminardb.codes.TABLE_NOT_FOUND == 200
        assert laminardb.codes.TABLE_EXISTS == 201
        assert laminardb.codes.SCHEMA_MISMATCH == 202
        assert laminardb.codes.INVALID_SCHEMA == 203

    def test_ingestion_codes(self):
        assert laminardb.codes.INGESTION_FAILED == 300
        assert laminardb.codes.WRITER_CLOSED == 301
        assert laminardb.codes.BATCH_SCHEMA_MISMATCH == 302

    def test_query_codes(self):
        assert laminardb.codes.QUERY_FAILED == 400
        assert laminardb.codes.SQL_PARSE_ERROR == 401
        assert laminardb.codes.QUERY_CANCELLED == 402

    def test_subscription_codes(self):
        assert laminardb.codes.SUBSCRIPTION_FAILED == 500
        assert laminardb.codes.SUBSCRIPTION_CLOSED == 501
        assert laminardb.codes.SUBSCRIPTION_TIMEOUT == 502

    def test_internal_codes(self):
        assert laminardb.codes.INTERNAL_ERROR == 900
        assert laminardb.codes.SHUTDOWN == 901


class TestExceptionCodeAttribute:
    def test_query_error_has_code(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        try:
            with pytest.raises(laminardb.QueryError) as exc_info:
                conn.query("INVALID SQL STATEMENT")
            assert hasattr(exc_info.value, "code")
            assert isinstance(exc_info.value.code, int)
        finally:
            conn.close()

    def test_checkpoint_error_has_code(self, tmp_path):
        conn = laminardb.open(str(tmp_path / "test.db"))
        try:
            with pytest.raises(laminardb.LaminarError) as exc_info:
                conn.checkpoint()
            assert hasattr(exc_info.value, "code")
            assert isinstance(exc_info.value.code, int)
        finally:
            conn.close()
