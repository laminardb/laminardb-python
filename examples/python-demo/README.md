# LaminarDB Python Demos

Real-time quantitative trading analytics powered by streaming SQL.

## Quick Start

```bash
pip install laminardb
python quickstart.py
```

For the full experience:

```bash
pip install laminardb pandas polars rich plotly scipy numpy jupyterlab
```

## Demos

| Demo | Time | Complexity | What You'll See |
|------|------|-----------|-----------------|
| `quickstart.py` | 2 min | Beginner | Connect, SQL, streaming filters, subscriptions |
| `trading_signals.py` | 5 min | Intermediate | Multi-pipeline signals, MA crossover, Rich dashboard |
| `portfolio_risk.py` | 10 min | Advanced | Multi-asset risk, VaR (scipy), correlation (numpy), checkpointing |
| `notebook_exploration.ipynb` | Interactive | All levels | Jupyter, plotly charts, pandas/polars/arrow interop |

### Demo 1: `quickstart.py`

Zero dependencies beyond LaminarDB. Creates a streaming source, inserts 5,000
simulated trades, subscribes to high-volume trade alerts, and demonstrates
every output format (`.df()`, `.pl()`, `.arrow()`, `.fetchall()`, `.to_dicts()`).

```bash
python quickstart.py
```

### Demo 2: `trading_signals.py`

Three streaming SQL pipelines (volume spikes, wide spreads, active trades) feed
a Python-side moving average crossover detector. A Rich terminal dashboard shows
live signals, metrics, and pipeline topology.

```bash
pip install rich pandas polars
python trading_signals.py
# Press Ctrl+C to stop
```

### Demo 3: `portfolio_risk.py`

Enterprise-grade risk monitoring. Multiple asset classes with correlated price
movements. Python computes rolling correlation matrices (numpy), historical VaR
(scipy), and unrealized P&L. SQL handles the filtering and aggregation.
Checkpointing persists window state and source offsets to disk so the pipeline
can resume from where it left off.

```bash
pip install rich pandas polars numpy scipy
python portfolio_risk.py
# Press Ctrl+C to stop
```

### Demo 4: `notebook_exploration.ipynb`

Interactive Jupyter notebook. Side-by-side comparison with DuckDB-style APIs,
plotly visualizations, polars lazy evaluation on query results, and schema
introspection.

```bash
pip install jupyterlab plotly pandas polars
jupyter lab notebook_exploration.ipynb
```

## What Makes This Different?

**Traditional approach** (Kafka + Flink + PostgreSQL + Python):

```
Kafka --> Flink (filter/aggregate) --> PostgreSQL --> Python (pandas query) --> Dashboard
       3 systems to maintain, seconds of latency, $$ infrastructure
```

**LaminarDB approach** (`pip install laminardb`):

```python
db = laminardb.open(":memory:")
db.execute("CREATE SOURCE trades (symbol VARCHAR, price DOUBLE, volume BIGINT, ts BIGINT)")
db.execute("CREATE STREAM big_trades AS SELECT * FROM trades WHERE volume > 25000")
db.start()

mv = laminardb.mv(db, "big_trades")
mv.subscribe(handler=my_callback)
# That's it. Real-time. Zero infrastructure.
```

## API Highlights

```python
import laminardb
from laminardb import ChangeEvent

# Embedded -- no server needed
db = laminardb.open(":memory:")

# Create sources (streaming ingestion points)
db.execute("CREATE SOURCE sensors (ts BIGINT, device VARCHAR, value DOUBLE)")
db.insert("sensors", [{"ts": 1, "device": "a", "value": 42.0}])

# Create streams (continuous SQL queries over sources)
db.execute("CREATE STREAM hot AS SELECT * FROM sensors WHERE value > 40")
db.start()  # Activate the streaming pipeline

# Subscribe to real-time events -- this is the magic
mv = laminardb.mv(db, "hot")
mv.subscribe(handler=lambda event: print(f"Got {len(event)} hot readings!"))

# Inline SQL queries (via DataFusion)
result = db.sql("SELECT 42 AS answer, 'hello' AS greeting")
result.df()        # -> pandas DataFrame
result.pl()        # -> polars DataFrame
result.arrow()     # -> PyArrow Table
result.fetchall()  # -> list[tuple]
result.to_dicts()  # -> dict[str, list]
result.show()      # -> pretty-print in terminal

# Introspection
db.tables()              # -> ["sensors"]
db.materialized_views()  # -> ["hot"]
db.metrics()             # -> PipelineMetrics
db.topology()            # -> PipelineTopology (nodes + edges)
```

**Data flows through the pipeline:** source -> stream -> subscription.
Sources are write-only ingestion points. Streams are continuous queries.
Subscriptions deliver results in real time.

## Requirements

- Python 3.11+
- `pip install laminardb` (includes native Rust engine)
- Optional: pandas, polars, rich, plotly, scipy, numpy, jupyterlab
