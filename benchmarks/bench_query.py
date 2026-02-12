"""Benchmark query + conversion throughput across output formats.

Run: python benchmarks/bench_query.py
"""

import time

import laminardb


def bench(name: str, fn, iterations: int = 100) -> None:
    """Run a benchmark and print average time."""
    start = time.perf_counter()
    for _ in range(iterations):
        fn()
    elapsed = time.perf_counter() - start
    avg_ms = (elapsed / iterations) * 1000
    print(f"  {name:25s}  {avg_ms:.3f} ms/query  ({iterations} iterations)")


QUERY = """
    SELECT * FROM (VALUES
        (1, 'sensor_a', 42.0, 'ok'),
        (2, 'sensor_b', 43.5, 'ok'),
        (3, 'sensor_a', 44.1, 'warn'),
        (4, 'sensor_c', 45.2, 'ok'),
        (5, 'sensor_b', 46.0, 'error')
    ) AS t(id, device, value, status)
"""


def main() -> None:
    conn = laminardb.open(":memory:")

    # Warm up
    conn.query(QUERY)

    # Raw query
    bench("query()", lambda: conn.query(QUERY))

    # arrow()
    bench("arrow()", lambda: conn.query(QUERY).arrow())

    # df()
    try:
        import pandas
        bench("df() [pandas]", lambda: conn.query(QUERY).df())
    except ImportError:
        print("  df() [pandas]              (skipped)")

    # pl()
    try:
        import polars
        bench("pl() [polars]", lambda: conn.query(QUERY).pl())
    except ImportError:
        print("  pl() [polars]              (skipped)")

    # fetchall()
    bench("fetchall()", lambda: conn.query(QUERY).fetchall())

    # to_dicts()
    bench("to_dicts()", lambda: conn.query(QUERY).to_dicts())

    # len()
    bench("len()", lambda: len(conn.query(QUERY)))

    conn.close()


if __name__ == "__main__":
    print("=== Query Conversion Benchmarks ===\n")
    main()
