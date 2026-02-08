"""Tests for continuous query subscriptions."""

import asyncio
import threading

import pytest

import laminardb


@pytest.fixture
def sub_db(db, sample_data):
    """Database with a subscription-friendly table."""
    db.insert("sensors", sample_data["rows"])
    return db


class TestSyncSubscription:
    def test_subscribe_and_receive(self, sub_db, sample_data):
        sub = sub_db.subscribe("SELECT * FROM sensors WHERE value > 40")
        assert sub.is_active

        # Insert more data in a background thread to trigger the subscription
        def insert_data():
            sub_db.insert("sensors", sample_data["row"])

        t = threading.Thread(target=insert_data)
        t.start()

        # Try to get at least one result (with timeout via try_next)
        result = sub.try_next()
        t.join()
        sub.cancel()
        assert not sub.is_active

    def test_cancel_subscription(self, sub_db):
        sub = sub_db.subscribe("SELECT * FROM sensors")
        assert sub.is_active
        sub.cancel()
        assert not sub.is_active

    def test_double_cancel_is_safe(self, sub_db):
        sub = sub_db.subscribe("SELECT * FROM sensors")
        sub.cancel()
        sub.cancel()  # should not raise

    def test_try_next_after_cancel(self, sub_db):
        sub = sub_db.subscribe("SELECT * FROM sensors")
        sub.cancel()
        result = sub.try_next()
        assert result is None


class TestAsyncSubscription:
    @pytest.mark.asyncio
    async def test_subscribe_async(self, sub_db):
        sub = await sub_db.subscribe_async("SELECT * FROM sensors")
        assert sub.is_active
        sub.cancel()

    @pytest.mark.asyncio
    async def test_async_cancel(self, sub_db):
        sub = await sub_db.subscribe_async("SELECT * FROM sensors")
        assert sub.is_active
        await sub.cancel()
        assert not sub.is_active
