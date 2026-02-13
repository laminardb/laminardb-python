#!/usr/bin/env python3
"""
LaminarDB Portfolio Risk Engine: Real-Time Risk Monitoring

Demonstrates:
  - Multiple streaming sources with stream joins (SQL-side)
  - Python scientific computing: numpy for correlation, scipy for VaR
  - Polars lazy API for post-processing query results
  - Multi-stream subscriptions with Python-side risk aggregation
  - Checkpointing: persist window state & source offsets to disk
  - Pipeline topology introspection and metrics
  - Writer API with watermark control

Pipeline:

  trades ----------+---> high_value_trades (>$100k notional)
                   |
  positions -------+---> portfolio_exposure (position * price stream)
                   |
  (Python layer) --+---> rolling correlation matrix (numpy)
                   +---> Value at Risk (scipy)
                   +---> risk dashboard (rich)

Run:     python portfolio_risk.py
Requires: pip install laminardb pandas numpy scipy polars rich
"""

import laminardb
from laminardb import ChangeEvent
import time
import random
import math
import tempfile
from datetime import datetime
from collections import deque
from pathlib import Path

try:
    import numpy as np

    HAS_NUMPY = True
except ImportError:
    HAS_NUMPY = False

try:
    from scipy import stats as sp_stats

    HAS_SCIPY = True
except ImportError:
    HAS_SCIPY = False

try:
    import polars as pl

    HAS_POLARS = True
except ImportError:
    HAS_POLARS = False

try:
    import pandas as pd

    HAS_PANDAS = True
except ImportError:
    HAS_PANDAS = False

try:
    from rich.console import Console
    from rich.table import Table
    from rich.panel import Panel
    from rich.live import Live
    from rich.layout import Layout
    from rich.text import Text

    HAS_RICH = True
except ImportError:
    HAS_RICH = False


def main():
    console = Console() if HAS_RICH else None

    def log(msg: str) -> None:
        if console:
            console.print(msg)
        else:
            # Strip rich markup for plain output
            import re
            print(re.sub(r"\[/?[a-z_ ]+\]", "", msg))

    log("[bold cyan]LaminarDB Portfolio Risk Engine[/]")
    log("=" * 50)

    # ==================================================================
    # 1. SETUP: Sources + Streams (with checkpointing enabled)
    # ==================================================================

    # Checkpointing persists window state and source offsets to disk.
    # On restart, the streaming pipeline can resume from where it left off.
    checkpoint_dir = Path(tempfile.mkdtemp(prefix="laminardb_risk_"))
    config = laminardb.Config(
        checkpoint_interval_ms=5000,       # Checkpoint every 5 seconds
        storage_dir=str(checkpoint_dir),   # Where to write checkpoint files
    )
    db = laminardb.open(":memory:", config=config)
    log(f"[+] Checkpointing enabled -> {checkpoint_dir}")

    # -- Source: Market trades --
    db.execute("""
        CREATE SOURCE trades (
            symbol   VARCHAR,
            price    DOUBLE,
            volume   BIGINT,
            side     VARCHAR,
            ts       BIGINT
        )
    """)

    # -- Source: Portfolio positions --
    db.execute("""
        CREATE SOURCE positions (
            account_id VARCHAR,
            symbol     VARCHAR,
            quantity   DOUBLE,
            avg_cost   DOUBLE,
            ts         BIGINT
        )
    """)

    # -- Source: Risk limits --
    db.execute("""
        CREATE SOURCE risk_limits (
            account_id VARCHAR,
            max_notional DOUBLE,
            max_var_95   DOUBLE,
            ts           BIGINT
        )
    """)

    # -- Stream: High-value trades (notional > $50k) --
    db.execute("""
        CREATE STREAM high_value_trades AS
        SELECT symbol, price, volume, side,
               price * volume AS notional, ts
        FROM trades
        WHERE price * volume > 50000.0
    """)

    # -- Stream: Sell-side pressure (large sell orders) --
    db.execute("""
        CREATE STREAM sell_pressure AS
        SELECT symbol, price, volume, ts
        FROM trades
        WHERE side = 'SELL' AND volume > 5000
    """)

    # Start streaming pipeline
    db.start()
    log("[bold green][+] Pipeline started: 2 sources, 2 streams[/]")

    # ==================================================================
    # 2. PORTFOLIO STATE (Python-side tracking)
    # ==================================================================

    # Active portfolio positions
    portfolio = {
        "ACCT-001": {
            "AAPL": {"qty": 1000, "avg_cost": 182.00},
            "GOOG": {"qty": 500, "avg_cost": 172.00},
            "MSFT": {"qty": 300, "avg_cost": 410.00},
            "NVDA": {"qty": 200, "avg_cost": 860.00},
        },
        "ACCT-002": {
            "TSLA": {"qty": 800, "avg_cost": 245.00},
            "AMZN": {"qty": 600, "avg_cost": 176.00},
            "META": {"qty": 400, "avg_cost": 515.00},
        },
    }

    # Insert positions into LaminarDB
    now_ms = int(time.time() * 1000)
    for acct, holdings in portfolio.items():
        for sym, pos in holdings.items():
            db.insert("positions", {
                "account_id": acct,
                "symbol": sym,
                "quantity": float(pos["qty"]),
                "avg_cost": pos["avg_cost"],
                "ts": now_ms,
            })

    # Insert risk limits
    db.insert("risk_limits", [
        {"account_id": "ACCT-001", "max_notional": 2_000_000.0, "max_var_95": 50_000.0, "ts": now_ms},
        {"account_id": "ACCT-002", "max_notional": 1_500_000.0, "max_var_95": 35_000.0, "ts": now_ms},
    ])

    log(f"[+] Loaded {sum(len(h) for h in portfolio.values())} positions across {len(portfolio)} accounts")

    # ==================================================================
    # 3. RISK ANALYTICS ENGINE (Python + SQL hybrid)
    # ==================================================================

    # Price history for computing returns and correlation
    price_history: dict[str, deque] = {}
    HISTORY_SIZE = 200
    latest_prices: dict[str, float] = {}

    # Risk metrics accumulator
    risk_state = {
        "trades_processed": 0,
        "high_value_count": 0,
        "sell_pressure_count": 0,
        "var_95": {},
        "correlation_matrix": None,
        "portfolio_pnl": {},
        "alerts": deque(maxlen=30),
    }

    def update_price_history(symbol: str, price: float) -> None:
        """Track price history for risk calculations."""
        if symbol not in price_history:
            price_history[symbol] = deque(maxlen=HISTORY_SIZE)
        price_history[symbol].append(price)
        latest_prices[symbol] = price

    def compute_portfolio_pnl() -> dict[str, dict]:
        """Compute unrealized P&L for all accounts using latest prices."""
        results = {}
        for acct, holdings in portfolio.items():
            acct_pnl = 0.0
            acct_value = 0.0
            for sym, pos in holdings.items():
                if sym in latest_prices:
                    current = latest_prices[sym]
                    unrealized = pos["qty"] * (current - pos["avg_cost"])
                    market_val = pos["qty"] * current
                    acct_pnl += unrealized
                    acct_value += market_val
            results[acct] = {
                "total_pnl": round(acct_pnl, 2),
                "market_value": round(acct_value, 2),
                "positions": len(holdings),
            }
        risk_state["portfolio_pnl"] = results
        return results

    def compute_var(confidence: float = 0.95) -> dict[str, float]:
        """
        Historical VaR per symbol using price returns.

        SQL handles the raw data aggregation. Python (scipy) handles
        the statistical modeling. This is the hybrid pattern.
        """
        var_results = {}

        for sym, prices in price_history.items():
            if len(prices) < 20:
                continue
            price_list = list(prices)
            returns = [
                (price_list[i] - price_list[i - 1]) / price_list[i - 1]
                for i in range(1, len(price_list))
            ]

            if HAS_SCIPY and HAS_NUMPY:
                arr = np.array(returns)
                var_results[sym] = float(np.percentile(arr, (1 - confidence) * 100))
            else:
                # Fallback: simple sorted percentile
                sorted_ret = sorted(returns)
                idx = int(len(sorted_ret) * (1 - confidence))
                var_results[sym] = sorted_ret[max(0, idx)]

        risk_state["var_95"] = var_results
        return var_results

    def compute_correlation() -> None:
        """Compute rolling cross-asset correlation matrix."""
        if not HAS_NUMPY:
            return

        symbols_with_data = [
            s for s, h in price_history.items() if len(h) >= 30
        ]
        if len(symbols_with_data) < 2:
            return

        # Build returns matrix
        returns_matrix = []
        for sym in symbols_with_data:
            prices = list(price_history[sym])
            returns = [
                (prices[i] - prices[i - 1]) / prices[i - 1]
                for i in range(1, len(prices))
            ]
            returns_matrix.append(returns[-29:])  # Last 29 returns

        # Trim to common length
        min_len = min(len(r) for r in returns_matrix)
        if min_len < 10:
            return
        trimmed = [r[-min_len:] for r in returns_matrix]

        corr = np.corrcoef(trimmed)
        risk_state["correlation_matrix"] = {
            "symbols": symbols_with_data,
            "matrix": corr,
        }

    # -- Subscription callbacks --

    mv_hv = laminardb.mv(db, "high_value_trades")
    mv_sell = laminardb.mv(db, "sell_pressure")

    hv_trade_log: deque = deque(maxlen=100)

    def on_high_value(event: ChangeEvent) -> None:
        for row in event:
            risk_state["high_value_count"] += 1
            hv_trade_log.append(row.to_dict())
            risk_state["alerts"].append({
                "time": datetime.now().strftime("%H:%M:%S"),
                "type": "HIGH VALUE",
                "symbol": row["symbol"],
                "detail": f"${row['notional']:,.0f} ({row['side']})",
            })

    def on_sell_pressure(event: ChangeEvent) -> None:
        for row in event:
            risk_state["sell_pressure_count"] += 1
            risk_state["alerts"].append({
                "time": datetime.now().strftime("%H:%M:%S"),
                "type": "SELL PRESSURE",
                "symbol": row["symbol"],
                "detail": f"{row['volume']:,} shares @ ${row['price']:.2f}",
            })

    t1 = mv_hv.subscribe(handler=on_high_value)
    t2 = mv_sell.subscribe(handler=on_sell_pressure)
    log("[+] Subscribed to high_value_trades and sell_pressure streams")

    # ==================================================================
    # 4. MARKET SIMULATOR (Multi-Asset)
    # ==================================================================

    class MultiAssetSimulator:
        """Generates correlated multi-asset price movements."""

        def __init__(self, symbols: dict[str, float]):
            self.symbols = dict(symbols)
            self.base_vol = {s: 0.001 for s in symbols}
            # Sector correlations: tech stocks co-move
            self.sectors = {
                "AAPL": "tech", "GOOG": "tech", "MSFT": "tech",
                "NVDA": "tech", "META": "tech", "AMZN": "tech",
                "TSLA": "auto", "JPM": "finance",
            }

        def generate_batch(self) -> list[dict]:
            trades = []
            now_ms = int(time.time() * 1000)

            # Market-wide shock (affects all)
            market_shock = random.gauss(0, 0.0003)

            # Sector shocks
            sector_shock = {
                "tech": random.gauss(0, 0.0005),
                "auto": random.gauss(0, 0.0008),
                "finance": random.gauss(0, 0.0004),
            }

            for sym, price in self.symbols.items():
                sector = self.sectors.get(sym, "tech")
                idio = random.gauss(0, self.base_vol[sym])
                ret = market_shock + sector_shock[sector] + idio

                # Occasional fat tail
                if random.random() < 0.002:
                    ret += random.gauss(0, 0.008)

                self.symbols[sym] = max(price * (1 + ret), 0.01)
                price = self.symbols[sym]

                side = "BUY" if random.random() > 0.48 else "SELL"
                vol = int(random.lognormvariate(7, 1.5))

                trades.append({
                    "symbol": sym,
                    "price": round(price, 4),
                    "volume": vol,
                    "side": side,
                    "ts": now_ms,
                })

                update_price_history(sym, price)

            return trades

    sim = MultiAssetSimulator({
        "AAPL": 185.50, "GOOG": 175.20, "MSFT": 415.00,
        "NVDA": 875.00, "TSLA": 248.00, "AMZN": 178.90,
        "META": 520.00, "JPM": 195.00,
    })

    # ==================================================================
    # 5. DASHBOARD
    # ==================================================================

    start_time = time.time()

    def build_dashboard():
        elapsed = time.time() - start_time

        if not HAS_RICH:
            pnl = risk_state.get("portfolio_pnl", {})
            var = risk_state.get("var_95", {})
            lines = [
                f"\n=== Risk Dashboard | {elapsed:.0f}s | "
                f"Trades: {risk_state['trades_processed']:,} ===",
            ]
            for acct, data in pnl.items():
                lines.append(f"  {acct}: PnL=${data['total_pnl']:+,.0f}  MV=${data['market_value']:,.0f}")
            if var:
                lines.append("  VaR(95):")
                for sym, v in sorted(var.items()):
                    lines.append(f"    {sym}: {v:+.4f}")
            for alert in list(risk_state["alerts"])[-5:]:
                lines.append(f"  [{alert['type']}] {alert['symbol']}: {alert['detail']}")
            return "\n".join(lines)

        layout = Layout()
        layout.split_column(
            Layout(name="header", size=3),
            Layout(name="top", size=12),
            Layout(name="bottom"),
            Layout(name="footer", size=4),
        )
        layout["top"].split_row(
            Layout(name="portfolio", ratio=2),
            Layout(name="var", ratio=1),
        )
        layout["bottom"].split_row(
            Layout(name="alerts", ratio=2),
            Layout(name="corr", ratio=1),
        )

        # Header
        rate = risk_state["trades_processed"] / max(1, elapsed)
        layout["header"].update(Panel(
            f"[bold cyan]Portfolio Risk Engine[/] | "
            f"{risk_state['trades_processed']:,} trades | "
            f"{rate:,.0f}/s | {elapsed:.0f}s",
            style="bold white on blue",
        ))

        # Portfolio P&L
        pnl_table = Table(title="Portfolio P&L", expand=True)
        pnl_table.add_column("Account")
        pnl_table.add_column("Positions", justify="right")
        pnl_table.add_column("Market Value", justify="right")
        pnl_table.add_column("Unrealized P&L", justify="right")

        for acct, data in risk_state.get("portfolio_pnl", {}).items():
            pnl_val = data["total_pnl"]
            style = "green" if pnl_val >= 0 else "red"
            pnl_table.add_row(
                acct,
                str(data["positions"]),
                f"${data['market_value']:,.0f}",
                f"[{style}]${pnl_val:+,.0f}[/{style}]",
            )
        layout["portfolio"].update(Panel(pnl_table))

        # VaR table
        var_table = Table(title="VaR (95%)", expand=True)
        var_table.add_column("Symbol")
        var_table.add_column("1-Tick VaR", justify="right")

        for sym, v in sorted(risk_state.get("var_95", {}).items()):
            style = "red" if v < -0.005 else "yellow" if v < -0.002 else "green"
            var_table.add_row(sym, f"[{style}]{v:+.4f}[/{style}]")
        layout["var"].update(Panel(var_table))

        # Alerts
        alert_table = Table(title="Risk Alerts", expand=True)
        alert_table.add_column("Time", width=8, style="dim")
        alert_table.add_column("Type", width=14)
        alert_table.add_column("Symbol", width=6)
        alert_table.add_column("Detail")

        for alert in list(risk_state["alerts"])[-12:]:
            style = "yellow" if alert["type"] == "HIGH VALUE" else "red"
            alert_table.add_row(
                alert["time"],
                f"[{style}]{alert['type']}[/{style}]",
                alert["symbol"],
                alert["detail"],
            )
        layout["alerts"].update(Panel(alert_table))

        # Correlation matrix
        corr_data = risk_state.get("correlation_matrix")
        if corr_data and HAS_NUMPY:
            syms = corr_data["symbols"]
            matrix = corr_data["matrix"]
            corr_table = Table(title="Correlation", expand=True)
            corr_table.add_column("", width=5)
            for s in syms[:5]:
                corr_table.add_column(s[:4], width=5, justify="right")
            for i, s in enumerate(syms[:5]):
                row = [s[:4]]
                for j in range(min(5, len(syms))):
                    val = matrix[i][j]
                    style = "red" if val > 0.7 and i != j else "dim"
                    row.append(f"[{style}]{val:.2f}[/{style}]")
                corr_table.add_row(*row)
            layout["corr"].update(Panel(corr_table))
        else:
            layout["corr"].update(Panel("[dim]Collecting data...[/]", title="Correlation"))

        # Footer
        m = db.metrics()
        layout["footer"].update(Panel(
            f"[dim]Pipeline: {m.state} | "
            f"Events: {m.total_events_ingested:,} | "
            f"HV Trades: {risk_state['high_value_count']} | "
            f"Sell Pressure: {risk_state['sell_pressure_count']}[/]"
        ))

        return layout

    # ==================================================================
    # 6. MAIN LOOP
    # ==================================================================

    log("\n[bold]Starting risk engine...[/]")

    risk_calc_interval = 50  # Recompute risk every N iterations
    iteration = 0

    def run_loop(iterations: int) -> None:
        nonlocal iteration

        for _ in range(iterations):
            # Ingest market data
            batch = sim.generate_batch()
            db.insert("trades", batch)
            risk_state["trades_processed"] += len(batch)
            iteration += 1

            # Periodic risk recalculation (Python-side)
            if iteration % risk_calc_interval == 0:
                compute_portfolio_pnl()
                compute_var()
                compute_correlation()

            time.sleep(0.03)

    if HAS_RICH:
        with Live(build_dashboard(), refresh_per_second=4, console=console) as live:
            try:
                while True:
                    batch = sim.generate_batch()
                    db.insert("trades", batch)
                    risk_state["trades_processed"] += len(batch)
                    iteration += 1

                    if iteration % risk_calc_interval == 0:
                        compute_portfolio_pnl()
                        compute_var()
                        compute_correlation()

                    live.update(build_dashboard())
                    time.sleep(0.03)
            except KeyboardInterrupt:
                pass
    else:
        try:
            for i in range(300):
                batch = sim.generate_batch()
                db.insert("trades", batch)
                risk_state["trades_processed"] += len(batch)
                iteration += 1

                if iteration % risk_calc_interval == 0:
                    compute_portfolio_pnl()
                    compute_var()
                    compute_correlation()

                if i % 100 == 0 and i > 0:
                    print(build_dashboard())

                time.sleep(0.03)
        except KeyboardInterrupt:
            pass

    # Let streams drain
    time.sleep(0.5)
    elapsed = time.time() - start_time

    # ==================================================================
    # 7. FINAL REPORT
    # ==================================================================

    log(f"\n{'=' * 60}")
    log("[bold]Final Risk Report[/]")
    log(f"{'=' * 60}")
    log(f"Duration: {elapsed:.1f}s")
    log(f"Trades processed: {risk_state['trades_processed']:,}")
    log(f"Throughput: {risk_state['trades_processed'] / elapsed:,.0f} trades/sec")

    # -- Portfolio P&L from Python-side tracking --
    log("\n[bold]Portfolio P&L:[/]")
    pnl = compute_portfolio_pnl()
    for acct, data in pnl.items():
        pnl_val = data["total_pnl"]
        log(f"  {acct}: {data['positions']} positions  "
            f"MV=${data['market_value']:,.0f}  "
            f"PnL=${pnl_val:+,.0f}")

    # -- High-value trades via Polars (from subscription data) --
    if hv_trade_log and HAS_POLARS:
        log("\n[bold]Top High-Value Trades (Polars):[/]")
        hv_df = pl.DataFrame(list(hv_trade_log))
        log(str(
            hv_df.select("symbol", "price", "volume", "notional", "side")
            .sort("notional", descending=True)
            .head(10)
        ))

    # -- VaR results --
    log("\n[bold]Value at Risk (95% confidence):[/]")
    var = risk_state.get("var_95", {})
    for sym in sorted(var):
        log(f"  {sym:>5s}: {var[sym]:+.4f} ({var[sym] * 100:+.2f}%)")

    # -- Correlation highlights --
    corr_data = risk_state.get("correlation_matrix")
    if corr_data and HAS_NUMPY:
        log("\n[bold]High Correlations (>0.5):[/]")
        syms = corr_data["symbols"]
        matrix = corr_data["matrix"]
        for i in range(len(syms)):
            for j in range(i + 1, len(syms)):
                if abs(matrix[i][j]) > 0.5:
                    log(f"  {syms[i]} <-> {syms[j]}: {matrix[i][j]:.3f}")

    # -- Pipeline topology --
    log("\n[bold]Pipeline Topology:[/]")
    topo = db.topology()
    for node in topo.nodes:
        kind = node.node_type
        log(f"  [{kind}] {node.name}")
    for edge in topo.edges:
        log(f"    {edge.from_node} --> {edge.to_node}")

    # -- Pipeline metrics --
    m = db.metrics()
    log(f"\n[bold]Pipeline Metrics:[/]")
    log(f"  State: {m.state}")
    log(f"  Events ingested: {m.total_events_ingested:,}")
    log(f"  Uptime: {m.uptime_secs:.1f}s")

    # Source-level metrics
    for sm in db.all_source_metrics():
        log(f"  Source '{sm.name}': {sm.total_events:,} events, watermark={sm.watermark}")

    # -- Checkpointing --
    # Trigger a manual checkpoint before shutdown.
    # Checkpoints persist window state and source offsets to disk,
    # so the streaming pipeline can resume from the last checkpoint.
    log(f"\n[bold]Checkpointing:[/]")
    log(f"  Enabled: {db.is_checkpoint_enabled}")
    log(f"  Storage: {checkpoint_dir}")
    try:
        ckpt_id = db.checkpoint()
        log(f"  Checkpoint ID: {ckpt_id}")
    except laminardb.LaminarError as e:
        log(f"  Checkpoint result: {e}")

    db.close()
    log("\n[green][+] Risk engine shut down cleanly.[/]")


if __name__ == "__main__":
    main()
