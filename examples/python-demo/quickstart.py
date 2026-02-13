#!/usr/bin/env python3
"""
LaminarDB Quickstart: Real-Time Stock Analytics in 60 Seconds

Shows:
  - Embedded database (no server, no config)
  - SQL DDL on streaming sources
  - Continuous query streams with real-time subscriptions
  - ChangeEvent / ChangeRow API
  - Multi-format output: .df(), .pl(), .arrow(), .fetchall()
  - Pipeline metrics and catalog introspection

Run:  python quickstart.py
Deps: pip install laminardb
"""

import laminardb
from laminardb import ChangeEvent
import time
import random
import threading
from collections import deque

def main():
    # -- 1. Connect (embedded -- no server, no config) ---------------------
    db = laminardb.open(":memory:")
    print("[+] Connected to LaminarDB (in-memory)")

    # -- 2. Create a streaming source --------------------------------------
    db.execute("""
        CREATE SOURCE trades (
            symbol  VARCHAR,
            price   DOUBLE,
            volume  BIGINT,
            ts      BIGINT
        )
    """)
    print("[+] Created 'trades' source")

    # -- 3. Create streaming filters (continuous queries) ------------------
    # Stream 1: High-volume trades only
    db.execute("""
        CREATE STREAM big_trades AS
        SELECT symbol, price, volume, ts
        FROM trades
        WHERE volume > 25000
    """)

    # Stream 2: All trades pass through (for collecting output formats)
    db.execute("""
        CREATE STREAM all_trades AS
        SELECT symbol, price, volume, ts
        FROM trades
    """)

    print("[+] Created streams: big_trades, all_trades")

    # Start the streaming pipeline
    db.start()
    print("[+] Pipeline started\n")

    # -- 4. Subscribe to real-time alerts ----------------------------------
    mv_big = laminardb.mv(db, "big_trades")
    mv_all = laminardb.mv(db, "all_trades")
    alert_count = 0
    recent_trades: deque = deque(maxlen=20)

    def on_big_trade(event: ChangeEvent):
        """Fires for every high-volume trade -- this is the streaming magic."""
        nonlocal alert_count
        for row in event:
            alert_count += 1
            print(
                f"  >> BIG TRADE | {row['symbol']:>5s} | "
                f"${row['price']:>8.2f} | "
                f"vol={row['volume']:>7,} | "
                f"#{alert_count}"
            )

    def on_all_trades(event: ChangeEvent):
        """Collect recent trades for the final summary."""
        for row in event:
            recent_trades.append(row.to_dict())

    sub_big = mv_big.subscribe(handler=on_big_trade)
    sub_all = mv_all.subscribe(handler=on_all_trades)
    print("[+] Subscribed to big_trades alerts")
    print("[+] Subscribed to all_trades collector\n")

    # -- 5. Generate market data (simulates a real feed) -------------------
    symbols = {"AAPL": 185.0, "GOOG": 142.0, "MSFT": 415.0, "NVDA": 875.0, "TSLA": 248.0}
    total_trades = 0

    print(">>> Generating trades... (2,500 ticks)\n")
    for i in range(500):
        batch = []
        for sym, base_price in symbols.items():
            # Random walk with slight upward drift
            symbols[sym] *= 1 + (random.random() - 0.498) * 0.003
            batch.append({
                "symbol": sym,
                "price": round(symbols[sym], 2),
                "volume": random.randint(100, 50_000),
                "ts": int(time.time() * 1000),
            })

        db.insert("trades", batch)
        total_trades += len(batch)

        if i % 100 == 0 and i > 0:
            print(f"\n--- {total_trades:,} trades inserted ---")

        time.sleep(0.01)  # ~500 trades/sec

    # Let subscriptions drain
    time.sleep(0.5)

    # -- 6. Multi-format output from subscription data ---------------------
    # LaminarDB is streaming-first: data flows through subscriptions.
    # For ad-hoc queries use inline SQL. For streamed data, convert the
    # collected results to any format you need.

    print(f"\n{'=' * 55}")
    print("  Output Formats (from subscription data)")
    print(f"{'=' * 55}")

    # Ad-hoc inline SQL (works without sources)
    print("\n--- Inline SQL ---")
    result = db.sql("""
        SELECT * FROM (VALUES
            ('AAPL', 185.50, 1200),
            ('GOOG', 142.30, 800),
            ('MSFT', 415.00, 3500)
        ) AS t(symbol, price, volume)
    """)
    print(f"  fetchall() : {result.fetchall()}")
    print(f"  fetchone() : {result.fetchone()}")
    print(f"  to_dicts() : {result.to_dicts()}")
    print(f"  len()      : {len(result)}")
    print(f"  columns    : {result.columns}")

    try:
        print(f"\n  .df() (pandas):\n{result.df().to_string(index=False)}")
    except ImportError:
        print("  (pandas not installed)")

    try:
        print(f"\n  .pl() (polars):\n{result.pl()}")
    except ImportError:
        print("  (polars not installed)")

    try:
        print(f"\n  .arrow() (pyarrow):\n{result.arrow()}")
    except ImportError:
        print("  (pyarrow not installed)")

    # Show recent streamed trades
    print(f"\n--- Recent Trades (from subscription, last {len(recent_trades)}) ---")
    for t in list(recent_trades)[-5:]:
        print(f"  {t['symbol']:>5s}  ${t['price']:>8.2f}  vol={t['volume']:>6,}")

    # -- 7. Pipeline metrics & introspection -------------------------------
    m = db.metrics()
    print(f"\n--- Pipeline Metrics ---")
    print(f"  Events ingested : {m.total_events_ingested:,}")
    print(f"  Uptime          : {m.uptime_secs:.1f}s")
    print(f"  State           : {m.state}")

    print(f"\n--- Catalog ---")
    print(f"  Sources : {db.tables()}")
    print(f"  Streams : {db.materialized_views()}")

    print(f"\n--- Topology ---")
    topo = db.topology()
    for node in topo.nodes:
        print(f"  [{node.node_type}] {node.name}")
    for edge in topo.edges:
        print(f"    {edge.from_node} --> {edge.to_node}")

    # Clean up
    db.close()
    print(f"\n[+] Done! Processed {total_trades:,} trades, {alert_count} big-trade alerts.")


if __name__ == "__main__":
    main()
