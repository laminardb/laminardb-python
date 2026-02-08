"""Tests for continuous query subscriptions."""

import pytest

import laminardb


@pytest.fixture
def conn(tmp_path):
    """A connection for subscription tests."""
    c = laminardb.open(str(tmp_path / "sub_test.db"))
    yield c
    c.close()


# Inline SQL that produces data (subscriptions need a query that returns rows).
SAMPLE_SQL = """
    SELECT * FROM (VALUES
        (1, 'sensor_a', 42.0),
        (2, 'sensor_b', 43.5)
    ) AS t(ts, device, value)
"""


class TestSyncSubscription:
    def test_subscribe_creates_active_sub(self, conn):
        sub = conn.subscribe(SAMPLE_SQL)
        assert sub.is_active
        sub.cancel()

    def test_cancel_subscription(self, conn):
        sub = conn.subscribe(SAMPLE_SQL)
        assert sub.is_active
        sub.cancel()
        assert not sub.is_active

    def test_double_cancel_is_safe(self, conn):
        sub = conn.subscribe(SAMPLE_SQL)
        sub.cancel()
        sub.cancel()  # should not raise

    def test_try_next_after_cancel(self, conn):
        sub = conn.subscribe(SAMPLE_SQL)
        sub.cancel()
        result = sub.try_next()
        assert result is None


class TestAsyncSubscription:
    @pytest.mark.asyncio
    async def test_subscribe_async(self, conn):
        sub = await conn.subscribe_async(SAMPLE_SQL)
        assert sub.is_active
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_cancel(self, conn):
        sub = await conn.subscribe_async(SAMPLE_SQL)
        assert sub.is_active
        sub.cancel()  # cancel is synchronous
        assert not sub.is_active
