#!/usr/bin/env python3
"""
LaminarDB Trading Signals: Multi-Pipeline Technical Analysis

Demonstrates:
  - Multi-stage streaming SQL pipeline (multiple CREATE STREAM stages)
  - Moving average crossover detection via Python + SQL hybrid
  - Volume anomaly detection
  - Spread monitoring
  - Python decision layer on top of SQL-filtered streams
  - Rich terminal dashboard with live updates

Pipeline:

  trades (raw ticks)
      |
      +---> big_movers     (price moves > 0.3% from first seen)
      +---> vol_spikes     (single trades > 30,000 shares)
      +---> wide_spreads   (bid-ask spread > 50 bps)
      |
      (Python-side analytics on each stream)
      |
      +---> signal_log     (aggregated trading signals)
      +---> Rich dashboard (live terminal UI)

Run:     python trading_signals.py
Requires: pip install laminardb pandas rich
"""

import laminardb
from laminardb import ChangeEvent
import time
import random
import threading
from datetime import datetime, timezone, timedelta
from collections import deque

try:
    from rich.console import Console
    from rich.table import Table
    from rich.live import Live
    from rich.panel import Panel
    from rich.layout import Layout

    HAS_RICH = True
except ImportError:
    HAS_RICH = False


def main():
    console = Console() if HAS_RICH else None

    def log(msg: str) -> None:
        if console:
            console.print(msg)
        else:
            print(msg)

    # ==================================================================
    # 1. SETUP: Connect + Create Sources
    # ==================================================================

    db = laminardb.open(":memory:")

    # Market data source
    db.execute("""
        CREATE SOURCE trades (
            symbol  VARCHAR,
            price   DOUBLE,
            volume  BIGINT,
            bid     DOUBLE,
            ask     DOUBLE,
            ts      BIGINT
        )
    """)

    # Reference prices source (for tracking moves)
    db.execute("""
        CREATE SOURCE ref_prices (
            symbol     VARCHAR,
            ref_price  DOUBLE,
            updated_ts BIGINT
        )
    """)

    # ==================================================================
    # 2. STREAMING SQL PIPELINE
    # ==================================================================

    # -- Stage 1: Volume spike detection -----------------------------------
    db.execute("""
        CREATE STREAM vol_spikes AS
        SELECT symbol, price, volume, ts
        FROM trades
        WHERE volume > 30000
    """)

    # -- Stage 2: Wide spread detection ------------------------------------
    db.execute("""
        CREATE STREAM wide_spreads AS
        SELECT
            symbol,
            bid,
            ask,
            price,
            (ask - bid) / ((ask + bid) / 2.0) * 10000.0 AS spread_bps,
            ts
        FROM trades
        WHERE bid > 0.0 AND ask > 0.0
          AND (ask - bid) / ((ask + bid) / 2.0) * 10000.0 > 50.0
    """)

    # -- Stage 3: All high-activity trades (volume > 10k OR price > 500) ---
    db.execute("""
        CREATE STREAM active_trades AS
        SELECT symbol, price, volume, ts
        FROM trades
        WHERE volume > 10000 OR price > 500.0
    """)

    # Start the pipeline
    db.start()
    log("[bold green][+] Pipeline started with 3 streaming stages[/]" if HAS_RICH else "[+] Pipeline started with 3 streaming stages")

    # ==================================================================
    # 3. PYTHON SIGNAL LAYER -- Intelligence on top of SQL
    # ==================================================================

    signal_log: deque = deque(maxlen=50)
    vol_spike_log: deque = deque(maxlen=100)
    spread_log: deque = deque(maxlen=100)
    metrics = {
        "trades_ingested": 0,
        "signals_generated": 0,
        "volume_spikes": 0,
        "spread_alerts": 0,
    }

    # Track price history for Python-side moving average crossover
    price_history: dict[str, deque] = {}
    MA_SHORT = 10
    MA_LONG = 30

    def compute_signal(symbol: str, price: float, volume: int) -> str | None:
        """
        Python-side moving average crossover detection.

        SQL handles the filtering (big trades, wide spreads).
        Python computes the technical indicators and makes decisions.
        This is the hybrid pattern: SQL does the heavy lifting at native
        speed, Python adds the intelligence layer.
        """
        if symbol not in price_history:
            price_history[symbol] = deque(maxlen=MA_LONG + 5)

        price_history[symbol].append(price)
        hist = list(price_history[symbol])

        if len(hist) < MA_LONG:
            return None

        sma_short = sum(hist[-MA_SHORT:]) / MA_SHORT
        sma_long = sum(hist[-MA_LONG:]) / MA_LONG

        # Check for crossover
        if len(hist) > MA_LONG:
            prev_short = sum(hist[-(MA_SHORT + 1) : -1]) / MA_SHORT
            prev_long = sum(hist[-(MA_LONG + 1) : -1]) / MA_LONG

            if prev_short <= prev_long and sma_short > sma_long:
                return "BUY"
            elif prev_short >= prev_long and sma_short < sma_long:
                return "SELL"

        return "HOLD"

    # -- Subscription callbacks ---

    mv_vol = laminardb.mv(db, "vol_spikes")
    mv_spread = laminardb.mv(db, "wide_spreads")
    mv_active = laminardb.mv(db, "active_trades")

    def on_vol_spike(event: ChangeEvent) -> None:
        for row in event:
            metrics["volume_spikes"] += 1
            vol_spike_log.append(row.to_dict())
            signal_log.append({
                "time": datetime.now().strftime("%H:%M:%S.%f")[:12],
                "symbol": row["symbol"],
                "signal": "VOL SPIKE",
                "price": row["price"],
                "change_pct": 0.0,
                "volume": row["volume"],
            })
            metrics["signals_generated"] += 1

    def on_wide_spread(event: ChangeEvent) -> None:
        for row in event:
            metrics["spread_alerts"] += 1
            spread_log.append(row.to_dict())
            signal_log.append({
                "time": datetime.now().strftime("%H:%M:%S.%f")[:12],
                "symbol": row["symbol"],
                "signal": f"SPREAD {row['spread_bps']:.0f}bp",
                "price": row["price"],
                "change_pct": 0.0,
                "volume": 0,
            })
            metrics["signals_generated"] += 1

    def on_active_trade(event: ChangeEvent) -> None:
        for row in event:
            signal = compute_signal(row["symbol"], row["price"], row["volume"])
            if signal and signal != "HOLD":
                signal_log.append({
                    "time": datetime.now().strftime("%H:%M:%S.%f")[:12],
                    "symbol": row["symbol"],
                    "signal": signal,
                    "price": row["price"],
                    "change_pct": 0.0,
                    "volume": row["volume"],
                })
                metrics["signals_generated"] += 1

    # Start background subscription threads
    t1 = mv_vol.subscribe(handler=on_vol_spike)
    t2 = mv_spread.subscribe(handler=on_wide_spread)
    t3 = mv_active.subscribe(handler=on_active_trade)

    log("[bold green][+] Subscribed to 3 streams[/]" if HAS_RICH else "[+] Subscribed to 3 streams")

    # ==================================================================
    # 4. MARKET DATA SIMULATOR
    # ==================================================================

    class MarketSimulator:
        """Generates realistic-ish market data with regime changes."""

        def __init__(self, symbols: dict[str, float]):
            self.symbols = dict(symbols)
            self.volatility = {s: 0.001 for s in symbols}
            self.trend = {s: 0.0 for s in symbols}
            self.tick_count = 0

        def generate_batch(self, batch_size: int = 10) -> list[dict]:
            trades = []
            now_ms = int(time.time() * 1000)

            for sym, price in self.symbols.items():
                for j in range(batch_size):
                    # Regime change every ~2000 ticks
                    if random.random() < 0.0005:
                        self.volatility[sym] = random.uniform(0.0005, 0.005)
                        self.trend[sym] = random.uniform(-0.0002, 0.0002)

                    # Price evolution: drift + noise + occasional jumps
                    ret = self.trend[sym] + self.volatility[sym] * random.gauss(0, 1)
                    if random.random() < 0.001:  # Fat tail event
                        ret += random.gauss(0, 0.01)

                    self.symbols[sym] = max(price * (1 + ret), 0.01)
                    price = self.symbols[sym]
                    spread = price * random.uniform(0.0001, 0.002)

                    trades.append({
                        "symbol": sym,
                        "price": round(price, 4),
                        "volume": int(random.lognormvariate(7, 1.5)),
                        "bid": round(price - spread / 2, 4),
                        "ask": round(price + spread / 2, 4),
                        "ts": now_ms + j,
                    })

            self.tick_count += len(trades)
            return trades

    sim = MarketSimulator({
        "AAPL": 185.50,
        "GOOG": 175.20,
        "MSFT": 415.00,
        "NVDA": 875.00,
        "TSLA": 248.00,
        "AMZN": 178.90,
        "META": 520.00,
        "JPM": 195.00,
    })

    # ==================================================================
    # 5. RICH TERMINAL DASHBOARD
    # ==================================================================

    start_time = time.time()

    def build_dashboard() -> Layout | str:
        elapsed = time.time() - start_time

        if not HAS_RICH:
            lines = [
                f"=== LaminarDB Trading Signals | {elapsed:.0f}s ===",
                f"Trades: {metrics['trades_ingested']:,} | "
                f"Signals: {metrics['signals_generated']} | "
                f"Vol Spikes: {metrics['volume_spikes']} | "
                f"Spread Alerts: {metrics['spread_alerts']}",
            ]
            for sig in list(signal_log)[-5:]:
                lines.append(
                    f"  {sig['time']} {sig['symbol']:>5s} "
                    f"{sig['signal']:>12s} "
                    f"${sig['price']:>8.2f}  vol={sig['volume']:>8,}"
                )
            return "\n".join(lines)

        layout = Layout()
        layout.split_column(
            Layout(name="header", size=3),
            Layout(name="body"),
            Layout(name="footer", size=3),
        )
        layout["body"].split_row(
            Layout(name="signals", ratio=3),
            Layout(name="metrics", ratio=1),
        )

        # Header
        layout["header"].update(
            Panel(
                "[bold cyan]LaminarDB Trading Signals[/] -- "
                "Multi-Pipeline Technical Analysis",
                style="bold white on blue",
            )
        )

        # Signals table
        sig_table = Table(title="Live Trading Signals", expand=True)
        sig_table.add_column("Time", style="dim", width=12)
        sig_table.add_column("Symbol", width=6)
        sig_table.add_column("Signal", width=14)
        sig_table.add_column("Price", justify="right", width=10)
        sig_table.add_column("Volume", justify="right", width=12)

        for sig in list(signal_log)[-20:]:
            style = ""
            if "BUY" in sig["signal"]:
                style = "green"
            elif "SELL" in sig["signal"]:
                style = "red"
            elif "SPIKE" in sig["signal"]:
                style = "yellow"
            elif "SPREAD" in sig["signal"]:
                style = "magenta"

            sig_table.add_row(
                sig["time"],
                sig["symbol"],
                f"[{style}]{sig['signal']}[/{style}]" if style else sig["signal"],
                f"${sig['price']:.2f}" if sig["price"] else "--",
                f"{sig['volume']:,}" if sig["volume"] else "--",
            )

        layout["signals"].update(Panel(sig_table))

        # Metrics panel
        rate = metrics["trades_ingested"] / max(1, elapsed)
        metrics_text = (
            f"[bold]Trades:[/]  {metrics['trades_ingested']:,}\n"
            f"[bold]Signals:[/] {metrics['signals_generated']:,}\n"
            f"[bold]Vol:[/]     {metrics['volume_spikes']:,}\n"
            f"[bold]Spread:[/]  {metrics['spread_alerts']:,}\n"
            f"\n[dim]Rate: ~{rate:,.0f}/s[/]\n"
            f"[dim]Elapsed: {elapsed:.0f}s[/]"
        )
        layout["metrics"].update(Panel(metrics_text, title="Metrics"))

        # Footer
        layout["footer"].update(
            Panel(
                "[dim]Pipeline: trades -> vol_spikes, wide_spreads, active_trades | "
                f"Sources: {db.tables()} | Streams: {db.materialized_views()}[/]"
            )
        )

        return layout

    # ==================================================================
    # 6. MAIN LOOP
    # ==================================================================

    log("\n[bold cyan]Starting market simulation...[/]" if HAS_RICH else "\nStarting market simulation...")

    if HAS_RICH:
        with Live(build_dashboard(), refresh_per_second=4, console=console) as live:
            try:
                while True:
                    batch = sim.generate_batch(batch_size=20)
                    db.insert("trades", batch)
                    metrics["trades_ingested"] += len(batch)
                    live.update(build_dashboard())
                    time.sleep(0.05)
            except KeyboardInterrupt:
                pass
    else:
        try:
            for _ in range(200):
                batch = sim.generate_batch(batch_size=20)
                db.insert("trades", batch)
                metrics["trades_ingested"] += len(batch)

                if metrics["trades_ingested"] % 5000 < 200:
                    print(build_dashboard())
                    print()

                time.sleep(0.05)
        except KeyboardInterrupt:
            pass

    # Let subscriptions drain
    time.sleep(0.5)
    elapsed = time.time() - start_time

    # -- Cleanup & Final Report --
    log("\n[bold]Session Summary[/]" if HAS_RICH else "\n=== Session Summary ===")
    log(f"Duration: {elapsed:.1f}s")
    log(f"Trades processed: {metrics['trades_ingested']:,}")
    log(f"Throughput: {metrics['trades_ingested'] / elapsed:,.0f} trades/sec")
    log(f"Signals generated: {metrics['signals_generated']}")

    # Final report uses Python-accumulated subscription data.
    # Data flows through streams -> subscriptions -> Python collections.

    # Volume spikes as Polars DataFrame
    if vol_spike_log:
        try:
            import polars as pl

            log("\n[bold]Volume Spikes (polars):[/]" if HAS_RICH else "\n--- Volume Spikes (polars) ---")
            spikes_df = pl.DataFrame(list(vol_spike_log))
            log(str(
                spikes_df.select("symbol", "price", "volume")
                .sort("volume", descending=True)
                .head(10)
            ))
        except ImportError:
            log(f"\n  Top spikes: {list(vol_spike_log)[-5:]}")

    # Wide spreads as pandas DataFrame
    if spread_log:
        try:
            import pandas as pd

            log("\n[bold]Wide Spreads (pandas):[/]" if HAS_RICH else "\n--- Wide Spreads (pandas) ---")
            spread_df = pd.DataFrame(list(spread_log))
            log(str(
                spread_df[["symbol", "spread_bps", "price"]]
                .sort_values("spread_bps", ascending=False)
                .head(10)
                .to_string(index=False)
            ))
        except ImportError:
            log(f"\n  Top spreads: {list(spread_log)[-5:]}")

    # Price summary from tracked history
    if price_history:
        log("\n[bold]Price Summary (from MA tracker):[/]" if HAS_RICH else "\n--- Price Summary ---")
        for sym in sorted(price_history):
            prices = list(price_history[sym])
            if prices:
                log(f"  {sym:>5s}: last=${prices[-1]:.2f}  "
                    f"low=${min(prices):.2f}  high=${max(prices):.2f}  "
                    f"ticks={len(prices)}")

    # Pipeline metrics
    m = db.metrics()
    log(f"\n--- Pipeline ---")
    log(f"  Events: {m.total_events_ingested:,} | Uptime: {m.uptime_secs:.1f}s | State: {m.state}")

    # Topology
    topo = db.topology()
    log(f"  Nodes: {[n.name for n in topo.nodes]}")
    log(f"  Edges: {[(e.from_node, e.to_node) for e in topo.edges]}")

    db.close()
    log("\n[green][+] Done![/]" if HAS_RICH else "\n[+] Done!")


if __name__ == "__main__":
    main()
