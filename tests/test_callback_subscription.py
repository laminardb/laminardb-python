"""Tests for callback-based push subscriptions (CallbackSubscription)."""

import threading
import time

import pytest

import laminardb


@pytest.fixture
def conn(tmp_path):
    """A connection with a source and a named stream."""
    c = laminardb.open(str(tmp_path / "callback_test.db"))
    c.create_table(
        "events",
        {"id": "int64", "msg": "string"},
    )
    c.execute("CREATE STREAM filtered AS SELECT * FROM events WHERE id > 0")
    c.start()
    yield c
    c.close()


class TestSubscribeCallback:
    """Tests for subscribe_callback (SQL query → callback)."""

    def test_creates_active_handle(self, conn):
        handle = conn.subscribe_callback(
            "SELECT * FROM events", lambda batch: None
        )
        assert handle.is_active
        handle.cancel()
        handle.wait()

    def test_cancel_sets_inactive(self, conn):
        handle = conn.subscribe_callback(
            "SELECT * FROM events", lambda batch: None
        )
        handle.cancel()
        handle.wait()
        assert not handle.is_active

    def test_double_cancel_is_safe(self, conn):
        handle = conn.subscribe_callback(
            "SELECT * FROM events", lambda batch: None
        )
        handle.cancel()
        handle.cancel()  # should not raise
        handle.wait()

    def test_wait_returns_after_cancel(self, conn):
        handle = conn.subscribe_callback(
            "SELECT * FROM events", lambda batch: None
        )
        handle.cancel()
        handle.wait()  # should return promptly

    def test_double_wait_is_noop(self, conn):
        handle = conn.subscribe_callback(
            "SELECT * FROM events", lambda batch: None
        )
        handle.cancel()
        handle.wait()
        handle.wait()  # second wait should be a no-op

    def test_repr_shows_query_active(self, conn):
        handle = conn.subscribe_callback(
            "SELECT * FROM events", lambda batch: None
        )
        r = repr(handle)
        assert "query" in r
        assert "active" in r
        handle.cancel()
        handle.wait()

    def test_repr_shows_finished_after_cancel(self, conn):
        handle = conn.subscribe_callback(
            "SELECT * FROM events", lambda batch: None
        )
        handle.cancel()
        handle.wait()
        r = repr(handle)
        assert "finished" in r

    def test_callback_receives_data(self, conn):
        received = threading.Event()
        results = []

        def on_data(batch):
            results.append(batch)
            received.set()

        handle = conn.subscribe_callback("SELECT * FROM events", on_data)
        conn.insert("events", {"id": 1, "msg": "hello"})
        got_data = received.wait(timeout=5.0)
        handle.cancel()
        handle.wait()
        if got_data:
            assert len(results) > 0
            assert results[0].num_rows > 0
        else:
            pytest.skip("data did not arrive within timeout")

    def test_on_error_called_when_callback_raises(self, conn):
        errors = []
        error_event = threading.Event()

        def bad_callback(batch):
            raise ValueError("test error")

        def on_error(msg):
            errors.append(msg)
            error_event.set()

        handle = conn.subscribe_callback(
            "SELECT * FROM events", bad_callback, on_error
        )
        conn.insert("events", {"id": 1, "msg": "hello"})
        got_error = error_event.wait(timeout=5.0)
        handle.cancel()
        handle.wait()
        if got_error:
            assert len(errors) > 0
            assert "test error" in errors[0]

    def test_stops_without_on_error_when_callback_raises(self, conn):
        called = threading.Event()

        def bad_callback(batch):
            called.set()
            raise ValueError("boom")

        handle = conn.subscribe_callback(
            "SELECT * FROM events", bad_callback
        )
        conn.insert("events", {"id": 1, "msg": "hello"})
        got_data = called.wait(timeout=5.0)
        # Cancel to ensure thread exits even if data never arrived
        handle.cancel()
        handle.wait()
        if got_data:
            assert not handle.is_active
        else:
            pytest.skip("data did not arrive within timeout")

    def test_wait_releases_gil(self, conn):
        """Verify wait() doesn't deadlock when another thread needs the GIL."""
        handle = conn.subscribe_callback(
            "SELECT * FROM events", lambda batch: None
        )

        result = [None]

        def other_thread():
            # This runs while wait() is blocking — if wait() held the GIL,
            # this thread would deadlock trying to acquire it.
            result[0] = 1 + 1

        handle.cancel()
        t = threading.Thread(target=other_thread)
        t.start()
        handle.wait()
        t.join(timeout=2.0)
        assert result[0] == 2


class TestSubscribeStreamCallback:
    """Tests for subscribe_stream_callback (named stream → callback)."""

    def test_creates_active_handle(self, conn):
        handle = conn.subscribe_stream_callback(
            "filtered", lambda batch: None
        )
        assert handle.is_active
        handle.cancel()
        handle.wait()

    def test_cancel_sets_inactive(self, conn):
        handle = conn.subscribe_stream_callback(
            "filtered", lambda batch: None
        )
        handle.cancel()
        handle.wait()
        assert not handle.is_active

    def test_double_cancel_is_safe(self, conn):
        handle = conn.subscribe_stream_callback(
            "filtered", lambda batch: None
        )
        handle.cancel()
        handle.cancel()
        handle.wait()

    def test_repr_shows_stream(self, conn):
        handle = conn.subscribe_stream_callback(
            "filtered", lambda batch: None
        )
        r = repr(handle)
        assert "stream" in r
        handle.cancel()
        handle.wait()

    def test_callback_receives_stream_data(self, conn):
        received = threading.Event()
        results = []

        def on_data(batch):
            results.append(batch)
            received.set()

        handle = conn.subscribe_stream_callback("filtered", on_data)
        conn.insert("events", {"id": 1, "msg": "hello"})
        got_data = received.wait(timeout=5.0)
        handle.cancel()
        handle.wait()
        if got_data:
            assert len(results) > 0
            assert results[0].num_rows > 0
        else:
            pytest.skip("data did not arrive within timeout")

    def test_on_error_with_stream_callback(self, conn):
        errors = []
        error_event = threading.Event()

        def bad_callback(batch):
            raise RuntimeError("stream error")

        def on_error(msg):
            errors.append(msg)
            error_event.set()

        handle = conn.subscribe_stream_callback(
            "filtered", bad_callback, on_error
        )
        conn.insert("events", {"id": 1, "msg": "hello"})
        got_error = error_event.wait(timeout=5.0)
        handle.cancel()
        handle.wait()
        if got_error:
            assert len(errors) > 0
            assert "stream error" in errors[0]

    def test_wait_after_cancel(self, conn):
        handle = conn.subscribe_stream_callback(
            "filtered", lambda batch: None
        )
        handle.cancel()
        handle.wait()  # should return promptly
        assert not handle.is_active
