"""Binance BTC trade signals in 30 lines of Python.

WebSocket → Stream → Signal — zero infrastructure, pure SQL.

Run: python examples/binance_signals.py
"""

import laminardb

db = laminardb.open(":memory:")

# 1. Ingest live BTC trades straight from Binance WebSocket
db.execute("""
    CREATE SOURCE trades (
        s VARCHAR, p DOUBLE, q DOUBLE, "T" BIGINT,
        WATERMARK FOR "T" AS "T" - INTERVAL '0' SECOND
    ) FROM WEBSOCKET (
        url    = 'wss://stream.binance.com:9443/ws/btcusdt@trade',
        format = 'json'
    )
""")

# 2. 10-second VWAP windows with BUY/SELL/HOLD signal
db.execute("""
    CREATE STREAM signals AS
    SELECT
        s                            AS symbol,
        SUM(p * q) / SUM(q)          AS vwap,
        AVG(p)                       AS avg_price,
        COUNT(*)                     AS trades,
        CASE
            WHEN AVG(p) > SUM(p * q) / SUM(q) * 1.001 THEN 'SELL'
            WHEN AVG(p) < SUM(p * q) / SUM(q) * 0.999 THEN 'BUY'
            ELSE 'HOLD'
        END AS signal
    FROM trades
    GROUP BY s, TUMBLE("T", INTERVAL '10' SECOND)
    EMIT ON WINDOW CLOSE
""")

# 3. Start pipeline and print signals as they arrive
db.start()

print("Listening for BTC signals...\n")
sub = db.subscribe_stream("signals")

try:
    while True:
        batch = sub.next()
        if batch:
            for row in batch.fetchall():
                symbol, vwap, avg_price, num_trades, signal = row
                print(f"  {signal:4s}  {symbol}  vwap=${vwap:,.2f}  avg=${avg_price:,.2f}  ({num_trades} trades)")
except KeyboardInterrupt:
    print("\nStopped.")
finally:
    sub.cancel()
    db.shutdown()
    db.close()
