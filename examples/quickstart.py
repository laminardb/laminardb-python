"""Quickstart: DuckDB-style API for LaminarDB.

Demonstrates the new DuckDB-style convenience methods.

Run: python examples/quickstart.py
"""

import laminardb

# ── Connect ──
conn = laminardb.open(":memory:")

# ── Create a table ──
conn.execute("CREATE SOURCE sensors (ts BIGINT, device VARCHAR, value DOUBLE)")

# ── Insert data ──
conn.insert("sensors", [
    {"ts": 1, "device": "sensor_a", "value": 42.0},
    {"ts": 2, "device": "sensor_b", "value": 43.5},
    {"ts": 3, "device": "sensor_a", "value": 44.1},
    {"ts": 4, "device": "sensor_c", "value": 41.2},
    {"ts": 5, "device": "sensor_b", "value": 45.8},
])

# ── Query with .sql() — DuckDB-style ──
result = conn.sql("SELECT * FROM sensors WHERE value > 43.0")

# ── Show results in the terminal ──
print("=== result.show() ===")
result.show()

# ── Convert to Pandas DataFrame ──
print("\n=== result.df() ===")
try:
    df = result.df()
    print(df)
except Exception:
    print("(pandas not installed, skipping df())")

# ── Convert to PyArrow Table ──
print("\n=== result.arrow() ===")
try:
    table = result.arrow()
    print(table)
except Exception:
    print("(pyarrow not installed, skipping arrow())")

# ── Fetch rows as tuples ──
print("\n=== result.fetchall() ===")
for row in conn.sql("SELECT * FROM sensors LIMIT 3").fetchall():
    print(row)

# ── Use len() ──
print(f"\n=== len(result) = {len(result)} ===")

# ── Iterate directly ──
print("\n=== for row in result ===")
for row in result:
    print(row)

# ── List tables ──
print(f"\n=== conn.tables() = {conn.tables()} ===")

# ── Module-level sql() (uses default connection) ──
print("\n=== laminardb.sql() ===")
r = laminardb.sql("SELECT 1 + 1 AS answer")
r.show()

# ── Clean up ──
conn.close()
print("\nDone!")
