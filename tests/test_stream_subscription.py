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
    def test_checkpoint_barrier_frame(self, tmp_path):
        config = laminardb.LaminarConfig(
            storage_dir=str(tmp_path / "storage"),
            checkpoint_interval_ms=60_000,
        )
        db = laminardb.open("barrier_test", config=config)
        sub = None
        try:
            db.create_table("events", {"id": "int64", "msg": "string"})
            db.execute("CREATE STREAM filtered AS SELECT * FROM events")
            db.start()
            sub = db.subscribe_stream("filtered")
            db.insert("events", {"id": 1, "msg": "hello"})

            batch_frame = sub.next_frame_timeout(2000)
            assert batch_frame is not None and batch_frame.is_batch

            checkpoint = db.checkpoint()
            barrier = sub.next_frame_timeout(2000)
            assert barrier is not None
            assert barrier.kind == "barrier"
            assert barrier.is_barrier
            assert not barrier.is_batch
            assert barrier.batch is None
            assert barrier.epoch == checkpoint.checkpoint_id
            assert barrier.checkpoint_id == checkpoint.checkpoint_id
            assert isinstance(barrier.sequence, int)
            assert isinstance(barrier.through_sequence, int)
            assert "kind='barrier'" in repr(barrier)
        finally:
            if sub is not None:
                sub.cancel()
            db.close()

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

    def test_try_next_frame_no_data(self, conn):
        sub = conn.subscribe_stream("filtered")
        assert sub.try_next_frame() is None
        sub.cancel()

    def test_next_frame_timeout_no_data(self, conn):
        sub = conn.subscribe_stream("filtered")
        with pytest.raises(laminardb.SubscriptionError, match="timeout"):
            sub.next_frame_timeout(100)
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

    def test_context_manager(self, conn):
        with conn.subscribe_stream("filtered") as sub:
            assert sub.is_active
        assert not sub.is_active

    def test_context_manager_cancels_on_exception(self, conn):
        with pytest.raises(ValueError):
            with conn.subscribe_stream("filtered") as sub:
                raise ValueError("test")
        assert not sub.is_active

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

    def test_subscription_batch_frame(self, conn):
        sub = conn.subscribe_stream("filtered")
        conn.insert("events", {"id": 1, "msg": "hello"})
        frame = sub.next_frame_timeout(2000)
        assert isinstance(frame, laminardb.SubscriptionFrame)
        assert frame.kind == "batch"
        assert frame.is_batch
        assert not frame.is_barrier
        assert isinstance(frame.sequence, int)
        assert frame.batch is not None
        assert frame.batch.num_rows > 0
        assert frame.epoch is None
        assert frame.checkpoint_id is None
        assert frame.through_sequence is None
        assert "kind='batch'" in repr(frame)
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

    @pytest.mark.asyncio
    async def test_async_try_next_no_data(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        # No data inserted yet, try_next should return None
        result = sub.try_next()
        assert result is None
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_try_next_frame_no_data(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        assert sub.try_next_frame() is None
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_try_next_after_cancel(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        sub.cancel()
        result = sub.try_next()
        assert result is None

    @pytest.mark.asyncio
    async def test_async_next_timeout_no_data(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        # No data inserted, timeout raises SubscriptionError
        with pytest.raises(laminardb.SubscriptionError, match="timeout"):
            sub.next_timeout(100)
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_subscribe_stream_with_data(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        conn.insert("events", {"id": 1, "msg": "hello"})
        # Use next_timeout so we don't block forever
        result = sub.next_timeout(2000)
        if result is not None:
            assert result.num_rows > 0
        else:
            pytest.skip("data did not arrive within timeout")
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_subscription_batch_frame(self, conn):
        sub = await conn.subscribe_stream_async("filtered")
        conn.insert("events", {"id": 1, "msg": "hello"})
        frame = await sub.next_frame()
        assert isinstance(frame, laminardb.SubscriptionFrame)
        assert frame.is_batch
        assert frame.batch is not None
        assert frame.batch.num_rows > 0
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_context_manager(self, conn):
        async with await conn.subscribe_stream_async("filtered") as sub:
            assert sub.is_active
        assert not sub.is_active
