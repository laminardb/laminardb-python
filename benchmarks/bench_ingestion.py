"""Benchmark data ingestion throughput across formats.

Run: python benchmarks/bench_ingestion.py
"""

import json
import time

import laminardb


def bench(name: str, fn, n_rows: int) -> None:
    """Run a benchmark and print rows/second."""
    start = time.perf_counter()
    fn()
    elapsed = time.perf_counter() - start
    rps = n_rows / elapsed if elapsed > 0 else float("inf")
    print(f"  {name:25s}  {n_rows:>8d} rows  {elapsed:.4f}s  {rps:>12,.0f} rows/s")


def main() -> None:
    N = 10_000
    conn = laminardb.open(":memory:")
    conn.create_table("bench", {"id": "int64", "val": "float64", "tag": "string"})

    # --- dict (list of row dicts) ---
    rows = [{"id": i, "val": float(i), "tag": f"t{i % 10}"} for i in range(N)]
    bench("list[dict]", lambda: conn.insert("bench", rows), N)

    # --- columnar dict ---
    columnar = {
        "id": list(range(N)),
        "val": [float(i) for i in range(N)],
        "tag": [f"t{i % 10}" for i in range(N)],
    }
    bench("columnar dict", lambda: conn.insert("bench", columnar), N)

    # --- JSON string ---
    json_data = json.dumps(rows)
    bench("JSON string", lambda: conn.insert_json("bench", json_data), N)

    # --- CSV string ---
    csv_lines = ["id,val,tag"] + [f"{i},{float(i)},t{i % 10}" for i in range(N)]
    csv_data = "\n".join(csv_lines)
    bench("CSV string", lambda: conn.insert_csv("bench", csv_data), N)

    # --- pandas DataFrame ---
    try:
        import pandas as pd

        df = pd.DataFrame(columnar)
        bench("pandas DataFrame", lambda: conn.insert("bench", df), N)
    except ImportError:
        print("  pandas DataFrame           (skipped - pandas not installed)")

    # --- polars DataFrame ---
    try:
        import polars as pl

        pl_df = pl.DataFrame(columnar)
        bench("polars DataFrame", lambda: conn.insert("bench", pl_df), N)
    except ImportError:
        print("  polars DataFrame           (skipped - polars not installed)")

    # --- pyarrow RecordBatch ---
    try:
        import pyarrow as pa

        batch = pa.record_batch(columnar)
        bench("pyarrow RecordBatch", lambda: conn.insert("bench", batch), N)

        table = pa.table(columnar)
        bench("pyarrow Table", lambda: conn.insert("bench", table), N)
    except ImportError:
        print("  pyarrow RecordBatch        (skipped - pyarrow not installed)")

    conn.close()


if __name__ == "__main__":
    print("=== Ingestion Benchmarks ===\n")
    main()
