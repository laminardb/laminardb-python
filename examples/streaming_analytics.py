"""Streaming analytics: MaterializedView + ChangeEvent pattern.

Demonstrates the high-level streaming types for real-time analytics.

Run: python examples/streaming_analytics.py
"""

import time

import laminardb
from laminardb import ChangeEvent

# ── Setup ──
conn = laminardb.open(":memory:")
conn.execute("CREATE SOURCE readings (ts BIGINT, sensor VARCHAR, temp DOUBLE)")

# ── Create a stream (materialized view) ── must be before start()
conn.execute(
    "CREATE STREAM hot_readings AS "
    "SELECT * FROM readings WHERE temp > 40.0"
)

conn.start()

# ── Wrap as a MaterializedView for convenience ──
mv = laminardb.mv(conn, "hot_readings", "SELECT * FROM readings WHERE temp > 40.0")
print(f"Created: {mv}")
print(f"Schema: {mv.schema()}")

# ── Subscribe with a callback ──
events_received = []


def on_change(event: ChangeEvent) -> None:
    """Called for each batch of changes."""
    events_received.append(len(event))
    print(f"  Received {len(event)} hot readings")


# Start background subscription
thread = mv.subscribe(handler=on_change)
print(f"Subscription thread: {thread}")

# ── Insert data in batches ──
print("\nInserting sensor data...")
for batch_num in range(5):
    data = [
        {"ts": batch_num * 10 + i, "sensor": f"s{i % 3}", "temp": 35.0 + i * 2.5}
        for i in range(10)
    ]
    conn.insert("readings", data)
    time.sleep(0.1)

# Give the subscription time to process
time.sleep(0.5)

print(f"\nTotal change events received: {len(events_received)}")
print(f"Total rows in change events: {sum(events_received)}")

# ── Query the materialized view ──
print("\n=== Current hot readings ===")
try:
    result = mv.query()
    result.show()
except Exception as e:
    print(f"(Query failed: {e})")

# ── Clean up ──
conn.close()
print("\nDone!")
