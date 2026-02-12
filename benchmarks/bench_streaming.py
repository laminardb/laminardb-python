"""Benchmark subscription iteration throughput.

Run: python benchmarks/bench_streaming.py
"""

import threading
import time

import laminardb


def main() -> None:
    conn = laminardb.open(":memory:")
    conn.create_table("events", {"ts": "int64", "val": "float64"})

    N = 5_000
    BATCH_SIZE = 100

    # Start pipeline
    conn.start()

    # Producer thread
    def produce() -> None:
        for i in range(0, N, BATCH_SIZE):
            batch = [{"ts": j, "val": float(j)} for j in range(i, min(i + BATCH_SIZE, N))]
            conn.insert("events", batch)

    producer = threading.Thread(target=produce)

    # Subscribe
    sub = conn.subscribe("SELECT * FROM events")

    producer.start()

    # Consume
    total = 0
    start = time.perf_counter()
    timeout = start + 10.0  # 10 second timeout

    while total < N and time.perf_counter() < timeout:
        batch = sub.try_next()
        if batch is not None:
            total += batch.num_rows

    elapsed = time.perf_counter() - start
    rps = total / elapsed if elapsed > 0 else 0

    sub.cancel()
    producer.join()

    print(f"  Subscription throughput:  {total} rows in {elapsed:.3f}s  ({rps:,.0f} rows/s)")

    conn.close()


if __name__ == "__main__":
    print("=== Streaming Benchmarks ===\n")
    main()
