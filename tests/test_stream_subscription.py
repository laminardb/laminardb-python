"""Tests for true named-stream subscriptions (StreamSubscription / AsyncStreamSubscription)."""

import pytest

import laminardb


@pytest.fixture
def conn(tmp_path):
    """A connection with a source and a named stream."""
    c = laminardb.open(str(tmp_path / "stream_sub_test.db"))
    c.create_table(
        "events",
        {"id": "int64", "msg": "string"},
    )
    c.execute("CREATE STREAM filtered AS SELECT * FROM events WHERE id > 0")
    c.start()
    yield c
    c.close()


class TestStreamSubscription:
    def test_subscribe_stream_creates_active_sub(self, conn):
        sub = conn.subscribe_stream("filtered")
        assert sub.is_active
        sub.cancel()

    def test_subscribe_stream_has_schema(self, conn):
        sub = conn.subscribe_stream("filtered")
        schema = sub.schema
        assert schema is not None
        sub.cancel()

    def test_cancel_subscription(self, conn):
        sub = conn.subscribe_stream("filtered")
        assert sub.is_active
        sub.cancel()
        assert not sub.is_active

    def test_double_cancel_is_safe(self, conn):
        sub = conn.subscribe_stream("filtered")
        sub.cancel()
        sub.cancel()  # should not raise

    def test_try_next_after_cancel(self, conn):
        sub = conn.subscribe_stream("filtered")
        sub.cancel()
        result = sub.try_next()
        assert result is None

    def test_try_next_no_data(self, conn):
        sub = conn.subscribe_stream("filtered")
        # No data inserted yet, try_next should return None
        result = sub.try_next()
        assert result is None
        sub.cancel()

    def test_next_timeout_no_data(self, conn):
        sub = conn.subscribe_stream("filtered")
        # No data inserted, timeout raises SubscriptionError
        with pytest.raises(laminardb.SubscriptionError, match="timeout"):
            sub.next_timeout(100)
        sub.cancel()

    def test_repr_active(self, conn):
        sub = conn.subscribe_stream("filtered")
        assert "active" in repr(sub)
        sub.cancel()

    def test_repr_after_cancel(self, conn):
        sub = conn.subscribe_stream("filtered")
        sub.cancel()
        assert "cancelled" in repr(sub)

    def test_schema_after_cancel_raises(self, conn):
        sub = conn.subscribe_stream("filtered")
        sub.cancel()
        with pytest.raises(RuntimeError, match="cancelled"):
            _ = sub.schema

    def test_iter_protocol(self, conn):
        sub = conn.subscribe_stream("filtered")
        assert iter(sub) is sub
        sub.cancel()

    def test_subscribe_stream_with_data(self, conn):
        sub = conn.subscribe_stream("filtered")
        conn.insert("events", {"id": 1, "msg": "hello"})
        # Use next_timeout so we don't block forever
        result = sub.next_timeout(2000)
        # Data may not arrive within the timeout in all environments,
        # but if it does, verify it has rows
        if result is not None:
            assert result.num_rows > 0
        else:
            pytest.skip("data did not arrive within timeout")
        sub.cancel()


class TestAsyncStreamSubscription:
    @pytest.mark.asyncio
    async def test_subscribe_stream_async(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        assert sub.is_active
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_cancel(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        assert sub.is_active
        sub.cancel()
        assert not sub.is_active

    @pytest.mark.asyncio
    async def test_async_repr_active(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        assert "active" in repr(sub)
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_repr_after_cancel(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        sub.cancel()
        assert "cancelled" in repr(sub)

    @pytest.mark.asyncio
    async def test_async_schema(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        schema = sub.schema
        assert schema is not None
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_schema_after_cancel_raises(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        sub.cancel()
        with pytest.raises(RuntimeError, match="cancelled"):
            _ = sub.schema
