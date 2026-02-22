# Feature Specifications: Connectors (F-PY-011 through F-PY-023)

Connector feature specifications for the LaminarDB Python bindings. These features cover source connectors (data ingestion from external systems), sink connectors (data export to external systems), and the programmatic frameworks that manage their lifecycle.

All connectors follow a SQL-first design: sources and sinks are created via `CREATE SOURCE` / `CREATE SINK` DDL statements executed through `Connection.execute()`. The Python bindings expose catalog introspection (`sources()`, `sinks()`), lifecycle control (`start()`, `shutdown()`), and per-connector metrics (`source_metrics()`, `all_source_metrics()`).

Connector availability is governed by Cargo feature flags in the Python bindings crate (`Cargo.toml`). Each connector requires both the laminar-db feature and the corresponding laminar-connectors implementation.

---

# F-PY-011: Source Connector Framework

## Metadata
- **Phase**: Phase 1 (Core Infrastructure)
- **Priority**: P0
- **Dependencies**: `laminar-db` with `api` feature, `laminar-connectors`
- **Main Repo Module**: `laminar-connectors/src/source.rs`, `laminar-db/src/pipeline/`
- **Implementation Status**: Implemented

## Summary

The source connector framework provides the foundational infrastructure for ingesting data from external systems into LaminarDB's streaming pipeline. Sources are created declaratively via SQL DDL (`CREATE SOURCE ... WITH (connector = '...')`) and managed through the pipeline lifecycle (`start()`, `shutdown()`). The Python bindings expose this through `Connection.execute()` for DDL, `Connection.sources()` for catalog introspection, and `Connection.source_metrics()` for runtime observability. There is no programmatic `create_source()` method; the design is SQL-first.

Connectors auto-register based on enabled Cargo features. When a feature like `kafka` or `websocket` is enabled in the Python bindings' `Cargo.toml`, the corresponding connector becomes available in `CREATE SOURCE` statements without any additional Python-side registration.

## Python API

```python
import laminardb

# Open a connection
db = laminardb.open("mydb")

# Create a source via SQL DDL
db.execute("""
    CREATE SOURCE trades (
        symbol VARCHAR,
        price DOUBLE,
        ts TIMESTAMP
    ) WITH (
        connector = 'kafka',
        'bootstrap.servers' = 'localhost:9092',
        'topic' = 'trades',
        'group.id' = 'ldb'
    )
""")

# Catalog introspection
sources: list[laminardb.SourceInfo] = db.sources()
# Each SourceInfo has: name, schema (pyarrow.Schema), watermark_column

names: list[str] = db.list_tables()  # List source names

# Pipeline lifecycle
db.start()        # Start all connectors
db.shutdown()     # Graceful shutdown with drain

# Source metrics
metrics = db.source_metrics("trades")  # SourceMetrics | None
all_metrics = db.all_source_metrics()  # list[SourceMetrics]

# SourceMetrics properties:
# .name, .total_events, .pending, .capacity,
# .is_backpressured, .watermark, .utilization
```

## Usage Examples

```python
import laminardb

with laminardb.open("analytics") as db:
    # Create a source with a watermark column for event-time processing
    db.execute("""
        CREATE SOURCE sensor_data (
            device_id VARCHAR,
            temperature DOUBLE,
            humidity DOUBLE,
            event_time TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'sensors',
            'group.id' = 'analytics-pipeline',
            'format' = 'json'
        )
    """)

    # Create a derived stream
    db.execute("""
        CREATE STREAM high_temp AS
        SELECT device_id, temperature, event_time
        FROM sensor_data
        WHERE temperature > 100.0
    """)

    # Start the pipeline (activates all sources and streams)
    db.start()

    # Monitor source health
    for m in db.all_source_metrics():
        print(f"Source {m.name}: {m.total_events} events, "
              f"backpressured={m.is_backpressured}, "
              f"utilization={m.utilization:.1%}")

    # Inspect pipeline topology
    topo = db.topology()
    for node in topo.nodes:
        print(f"  {node.node_type}: {node.name}")
    for edge in topo.edges:
        print(f"  {edge.from_node} -> {edge.to_node}")

    # Graceful shutdown
    db.shutdown()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Connector type identifier (e.g., `'kafka'`, `'websocket'`, `'postgres-cdc'`) |
| `format` | No | Data format: `json`, `csv`, `avro`, `raw`, `debezium` (default varies by connector) |

Additional keys are connector-specific and documented in F-PY-012 through F-PY-017.

## Implementation Notes
- Sources are created via `Connection.execute()` which delegates to `laminar_db::api::Connection::execute()` and returns an `ExecuteResult` with `result_type == "ddl"`.
- The connector registry is static and populated at compile time based on enabled Cargo features. No runtime plugin loading.
- All catalog and metrics methods release the GIL via `py.allow_threads()` (see `connection.rs` lines 365-573).
- `SourceInfo` is a frozen pyclass wrapping `laminar_db::SourceInfo` with `name`, `schema` (as PyArrow Schema via `pyo3-arrow`), and `watermark_column` (see `catalog.rs`).
- `SourceMetrics` is a frozen pyclass wrapping `laminar_db::SourceMetrics` with `name`, `total_events`, `pending`, `capacity`, `is_backpressured`, `watermark`, and `utilization` (see `metrics.rs`).
- Pipeline lifecycle: `start()` begins event processing for all registered sources/streams/sinks; `shutdown()` performs a graceful drain of in-flight events.

## Testing Requirements
- Verify `db.execute("CREATE SOURCE ... WITH (connector = 'kafka', ...)")` returns `ExecuteResult` with `result_type == "ddl"` and `ddl_type == "CREATE SOURCE"`.
- Verify `db.sources()` returns `SourceInfo` with correct name, schema fields, and watermark column after source creation.
- Verify `db.list_tables()` includes newly created source names.
- Verify `db.source_metrics(name)` returns `None` for non-existent sources and a valid `SourceMetrics` after source creation and `start()`.
- Verify `db.all_source_metrics()` returns metrics for all registered sources.
- Verify `db.start()` and `db.shutdown()` succeed without error.
- Verify invalid connector names in `CREATE SOURCE` raise `ConnectorError`.
- Verify pipeline topology includes source nodes after source creation.

---

# F-PY-012: Kafka Source

## Metadata
- **Phase**: Phase 1 (Core Connectors)
- **Priority**: P0
- **Dependencies**: F-PY-011 (Source Connector Framework), `laminar-db` with `kafka` feature
- **Main Repo Module**: `laminar-connectors/src/kafka/source.rs`
- **Implementation Status**: Implemented

## Summary

The Kafka source connector ingests data from Apache Kafka topics into the LaminarDB streaming pipeline. It supports consumer groups for parallel consumption, multiple data formats (JSON, CSV, Avro with Schema Registry, raw bytes, Debezium CDC), configurable offset reset policies, and backpressure handling. The connector is enabled via the `kafka` Cargo feature, which is active in the Python bindings' `Cargo.toml`.

## Python API

```python
import laminardb

db = laminardb.open("mydb")

# Create a Kafka source
db.execute("""
    CREATE SOURCE trades (
        symbol VARCHAR,
        price DOUBLE,
        volume BIGINT,
        ts TIMESTAMP
    ) WITH (
        connector = 'kafka',
        'bootstrap.servers' = 'localhost:9092',
        'topic' = 'trades',
        'group.id' = 'laminar-trades',
        'format' = 'json',
        'auto.offset.reset' = 'earliest'
    )
""")

# Start ingestion
db.start()

# Monitor Kafka source
metrics = db.source_metrics("trades")
print(f"Events: {metrics.total_events}, Pending: {metrics.pending}")
```

## Usage Examples

```python
import laminardb

with laminardb.open("trading") as db:
    # JSON format with earliest offset reset
    db.execute("""
        CREATE SOURCE market_data (
            symbol VARCHAR,
            bid DOUBLE,
            ask DOUBLE,
            timestamp TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'broker1:9092,broker2:9092',
            'topic' = 'market-data',
            'group.id' = 'laminar-market',
            'format' = 'json',
            'auto.offset.reset' = 'earliest'
        )
    """)

    # Avro format with Schema Registry
    db.execute("""
        CREATE SOURCE orders (
            order_id VARCHAR,
            customer_id BIGINT,
            amount DOUBLE,
            created_at TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'broker1:9092',
            'topic' = 'orders',
            'group.id' = 'laminar-orders',
            'format' = 'avro',
            'schema.registry.url' = 'http://schema-registry:8081'
        )
    """)

    # Debezium CDC format (captures INSERT/UPDATE/DELETE from upstream DB)
    db.execute("""
        CREATE SOURCE user_changes (
            id BIGINT,
            name VARCHAR,
            email VARCHAR,
            updated_at TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'broker1:9092',
            'topic' = 'dbserver1.public.users',
            'group.id' = 'laminar-cdc',
            'format' = 'debezium'
        )
    """)

    # Create a real-time aggregation stream
    db.execute("""
        CREATE STREAM trade_summary AS
        SELECT
            symbol,
            COUNT(*) as trade_count,
            AVG(bid) as avg_bid,
            MAX(ask) as max_ask
        FROM market_data
        GROUP BY symbol
    """)

    db.start()

    # Subscribe to the aggregation stream
    for batch in db.subscribe_stream("trade_summary"):
        df = batch.to_pandas()
        print(df)
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'kafka'` |
| `bootstrap.servers` | Yes | Comma-separated list of Kafka broker addresses (e.g., `'broker1:9092,broker2:9092'`) |
| `topic` | Yes | Kafka topic to consume from |
| `group.id` | Yes | Consumer group ID for offset tracking and parallel consumption |
| `format` | No | Data format: `json` (default), `csv`, `avro`, `raw`, `debezium` |
| `auto.offset.reset` | No | Where to start consuming: `earliest` or `latest` (default: `latest`) |
| `schema.registry.url` | No | Confluent Schema Registry URL (required when `format = 'avro'`) |
| `security.protocol` | No | Security protocol: `PLAINTEXT`, `SSL`, `SASL_PLAINTEXT`, `SASL_SSL` |
| `sasl.mechanism` | No | SASL mechanism: `PLAIN`, `SCRAM-SHA-256`, `SCRAM-SHA-512` |
| `sasl.username` | No | SASL username (required with SASL mechanisms) |
| `sasl.password` | No | SASL password (required with SASL mechanisms) |

## Implementation Notes
- The `kafka` feature is enabled in `Cargo.toml` line 28: `"kafka"` in the `laminar-db` dependency features list.
- Kafka consumer lifecycle is managed by the pipeline: `db.start()` begins consuming, `db.shutdown()` commits offsets and gracefully disconnects.
- Consumer group offset tracking enables exactly-once semantics when combined with checkpointing (`LaminarConfig(checkpoint_interval_ms=...)`).
- Backpressure is handled by the source buffer: when the buffer fills (`is_backpressured == True` in `SourceMetrics`), the Kafka consumer pauses polling until space is available.
- Dead letter queue support: malformed messages that fail deserialization can be routed to a configurable DLQ topic rather than causing the pipeline to fail.
- The connector uses librdkafka under the hood via the `rdkafka` Rust crate.

## Testing Requirements
- Verify `CREATE SOURCE ... WITH (connector = 'kafka', ...)` succeeds and source appears in `db.sources()`.
- Verify required keys (`bootstrap.servers`, `topic`, `group.id`) cause `ConnectorError` when missing.
- Verify `db.source_metrics("kafka_source")` returns valid metrics after `start()`.
- Verify pipeline topology shows Kafka source as a source node.
- Integration test: with a running Kafka broker, verify end-to-end message consumption and query results.
- Verify Avro format with Schema Registry URL parses correctly.
- Verify Debezium format CDC messages are correctly deserialized.

---

# F-PY-013: WebSocket Source

## Metadata
- **Phase**: Phase 2 (Extended Connectors)
- **Priority**: P1
- **Dependencies**: F-PY-011 (Source Connector Framework), `laminar-db` with `websocket` feature
- **Main Repo Module**: `laminar-connectors/src/websocket/source.rs`
- **Implementation Status**: Newly Enabled

## Summary

The WebSocket source connector ingests data from WebSocket connections into the LaminarDB streaming pipeline. It supports two modes: client mode (connects to an external WebSocket server) and server mode (accepts incoming WebSocket connections). Features include automatic reconnection with exponential backoff, authentication (Bearer token, OAuth), event-time extraction from message payloads, configurable backpressure strategies, and ping/pong keepalive. The connector was recently enabled in the Python bindings via the `websocket` Cargo feature.

## Python API

```python
import laminardb

db = laminardb.open("mydb")

# Client mode: connect to a WebSocket server
db.execute("""
    CREATE SOURCE prices (
        symbol VARCHAR,
        bid DOUBLE,
        ask DOUBLE,
        timestamp TIMESTAMP
    ) WITH (
        connector = 'websocket',
        'url' = 'wss://feed.example.com/prices',
        'format' = 'json'
    )
""")

# Server mode: accept incoming WebSocket connections
db.execute("""
    CREATE SOURCE events (
        event_type VARCHAR,
        payload VARCHAR,
        received_at TIMESTAMP
    ) WITH (
        connector = 'websocket',
        'mode' = 'server',
        'bind_address' = '0.0.0.0:8080',
        'format' = 'json'
    )
""")

db.start()
```

## Usage Examples

```python
import laminardb

with laminardb.open("realtime") as db:
    # Connect to a cryptocurrency price feed with authentication
    db.execute("""
        CREATE SOURCE crypto_prices (
            symbol VARCHAR,
            price DOUBLE,
            volume DOUBLE,
            ts TIMESTAMP
        ) WITH (
            connector = 'websocket',
            'url' = 'wss://stream.crypto-exchange.com/v1/prices',
            'format' = 'json',
            'auth' = 'Bearer my-api-key-here',
            'reconnect.initial_delay_ms' = '1000',
            'reconnect.max_delay_ms' = '30000',
            'reconnect.max_retries' = '0'
        )
    """)

    # Create a real-time moving average stream
    db.execute("""
        CREATE STREAM price_avg AS
        SELECT
            symbol,
            AVG(price) as avg_price,
            COUNT(*) as tick_count
        FROM crypto_prices
        GROUP BY symbol
    """)

    db.start()

    # Subscribe to aggregated results
    sub = db.subscribe_stream("price_avg")
    for batch in sub:
        df = batch.to_pandas()
        print(df.head())

    db.shutdown()
```

```python
import laminardb

with laminardb.open("iot") as db:
    # Server mode: IoT devices connect and push telemetry
    db.execute("""
        CREATE SOURCE telemetry (
            device_id VARCHAR,
            temperature DOUBLE,
            humidity DOUBLE,
            battery_pct FLOAT
        ) WITH (
            connector = 'websocket',
            'mode' = 'server',
            'bind_address' = '0.0.0.0:9090',
            'format' = 'json'
        )
    """)

    # Alerting stream
    db.execute("""
        CREATE STREAM low_battery_alerts AS
        SELECT device_id, battery_pct
        FROM telemetry
        WHERE battery_pct < 20.0
    """)

    db.start()

    # Process alerts asynchronously
    async for batch in await db.subscribe_stream_async("low_battery_alerts"):
        for row in batch.to_dicts():
            print(f"Low battery: device {row['device_id']}, {row['battery_pct']}%")
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'websocket'` |
| `url` | Conditional | WebSocket URL (required for client mode, e.g., `'wss://...'` or `'ws://...'`) |
| `mode` | No | Connection mode: `client` (default) or `server` |
| `bind_address` | Conditional | Bind address for server mode (required when `mode = 'server'`, e.g., `'0.0.0.0:8080'`) |
| `format` | No | Data format: `json` (default), `csv`, `raw` |
| `auth` | No | Authentication header value (e.g., `'Bearer <token>'`) |
| `reconnect.initial_delay_ms` | No | Initial reconnect delay in milliseconds (default: `1000`) |
| `reconnect.max_delay_ms` | No | Maximum reconnect delay with exponential backoff (default: `30000`) |
| `reconnect.max_retries` | No | Maximum reconnect attempts, `0` for unlimited (default: `0`) |
| `ping_interval_ms` | No | WebSocket ping interval for keepalive (default: `30000`) |

## Implementation Notes
- The `websocket` feature is enabled in `Cargo.toml` line 33: `"websocket"` in the `laminar-db` dependency features list.
- This feature is **newly enabled** in the Python bindings. The connector code exists in `laminar-connectors` and has been tested at the Rust level, but Python-level integration testing is new.
- Client mode uses `tokio-tungstenite` for async WebSocket connections.
- Server mode uses `tokio-tungstenite` with a `TcpListener` to accept incoming connections.
- Auto-reconnect uses exponential backoff: delay doubles on each failure up to `max_delay_ms`, resets on successful connection.
- Backpressure strategy: when the source buffer fills, the WebSocket read loop pauses until space is available, relying on TCP flow control to propagate backpressure to the sender.
- Errors from the WebSocket connector are mapped to `ConnectorError` in the Python exception hierarchy (see `error.rs`).

## Testing Requirements
- Verify `CREATE SOURCE ... WITH (connector = 'websocket', 'url' = '...')` succeeds in client mode.
- Verify `CREATE SOURCE ... WITH (connector = 'websocket', 'mode' = 'server', 'bind_address' = '...')` succeeds in server mode.
- Verify missing `url` in client mode raises `ConnectorError`.
- Verify missing `bind_address` in server mode raises `ConnectorError`.
- Verify source appears in `db.sources()` and `db.list_tables()` after creation.
- Integration test: stand up a local WebSocket server, send JSON messages, verify they arrive via `db.query()`.
- Verify reconnect behavior: kill the WebSocket server, verify the source reconnects after restart.
- Verify `db.source_metrics("ws_source")` returns valid metrics.

---

# F-PY-014: CDC Sources (PostgreSQL, MySQL)

## Metadata
- **Phase**: Phase 2 (Extended Connectors)
- **Priority**: P0
- **Dependencies**: F-PY-011 (Source Connector Framework), `laminar-db` with `postgres-cdc` and/or `mysql-cdc` features
- **Main Repo Module**: `laminar-connectors/src/postgres_cdc/`, `laminar-connectors/src/mysql_cdc/`
- **Implementation Status**: PostgreSQL CDC: Implemented; MySQL CDC: Newly Enabled

## Summary

Change Data Capture (CDC) source connectors replicate changes from relational databases into the LaminarDB streaming pipeline in real time. Two CDC connectors are available:

- **PostgreSQL CDC** (`postgres-cdc`): Uses PostgreSQL logical replication with the `pgoutput` plugin to capture INSERT, UPDATE, and DELETE operations. Tracks Log Sequence Numbers (LSNs) for reliable replication. Supports automatic schema detection from the upstream table.
- **MySQL CDC** (`mysql-cdc`): Uses MySQL binary log (binlog) replication to capture row-level changes. Supports GTID-based replication for reliable positioning. Uses rustls for TLS connections. Supports automatic schema detection.

Both connectors produce a unified change stream that can be queried with standard SQL in LaminarDB.

## Python API

```python
import laminardb

db = laminardb.open("replication")

# PostgreSQL CDC source
db.execute("""
    CREATE SOURCE users (
        id BIGINT,
        name VARCHAR,
        email VARCHAR,
        updated_at TIMESTAMP
    ) WITH (
        connector = 'postgres-cdc',
        'connection' = 'postgresql://user:pass@pghost:5432/mydb',
        'table' = 'public.users',
        'slot.name' = 'laminar_users'
    )
""")

# MySQL CDC source
db.execute("""
    CREATE SOURCE orders (
        order_id BIGINT,
        customer_id BIGINT,
        total DOUBLE,
        status VARCHAR,
        created_at TIMESTAMP
    ) WITH (
        connector = 'mysql-cdc',
        'connection' = 'mysql://user:pass@mysqlhost:3306/shop',
        'table' = 'orders',
        'server_id' = '1001'
    )
""")

db.start()
```

## Usage Examples

```python
import laminardb

with laminardb.open("cdc_pipeline") as db:
    # Replicate PostgreSQL users table
    db.execute("""
        CREATE SOURCE pg_users (
            id BIGINT,
            name VARCHAR,
            email VARCHAR,
            role VARCHAR,
            updated_at TIMESTAMP
        ) WITH (
            connector = 'postgres-cdc',
            'connection' = 'postgresql://replicator:secret@db.example.com:5432/production',
            'table' = 'public.users',
            'slot.name' = 'laminar_users_slot',
            'publication.name' = 'laminar_pub'
        )
    """)

    # Replicate MySQL orders table
    db.execute("""
        CREATE SOURCE mysql_orders (
            order_id BIGINT,
            customer_id BIGINT,
            total_amount DOUBLE,
            status VARCHAR,
            created_at TIMESTAMP
        ) WITH (
            connector = 'mysql-cdc',
            'connection' = 'mysql://cdc_user:secret@mysql.example.com:3306/ecommerce',
            'table' = 'orders',
            'server_id' = '2001'
        )
    """)

    # Join PostgreSQL and MySQL data in real time
    db.execute("""
        CREATE STREAM enriched_orders AS
        SELECT
            o.order_id,
            o.total_amount,
            o.status,
            u.name as customer_name,
            u.email as customer_email
        FROM mysql_orders o
        JOIN pg_users u ON o.customer_id = u.id
    """)

    db.start()

    # Subscribe to the enriched stream
    for batch in db.subscribe_stream("enriched_orders"):
        df = batch.to_pandas()
        # Process enriched orders (e.g., send notifications)
        new_orders = df[df["status"] == "new"]
        if len(new_orders) > 0:
            print(f"New orders: {len(new_orders)}")

    db.shutdown()
```

## Configuration Keys

### PostgreSQL CDC
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'postgres-cdc'` |
| `connection` | Yes | PostgreSQL connection URI (e.g., `'postgresql://user:pass@host:5432/db'`) |
| `table` | Yes | Fully qualified table name (e.g., `'public.users'`) |
| `slot.name` | No | Replication slot name (auto-generated if not specified) |
| `publication.name` | No | Publication name for logical replication (auto-created if not specified) |

### MySQL CDC
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'mysql-cdc'` |
| `connection` | Yes | MySQL connection URI (e.g., `'mysql://user:pass@host:3306/db'`) |
| `table` | Yes | Table name to replicate (e.g., `'orders'`) |
| `server_id` | No | Unique server ID for binlog replication (auto-assigned if not specified) |

## Implementation Notes
- PostgreSQL CDC (`postgres-cdc`) is enabled in `Cargo.toml` line 29: `"postgres-cdc"` in the `laminar-db` dependency features list. It has been available since early versions.
- MySQL CDC (`mysql-cdc`) is enabled in `Cargo.toml` line 34: `"mysql-cdc"` in the `laminar-db` dependency features list. It is **newly enabled**.
- PostgreSQL CDC uses the `pgoutput` logical decoding plugin (built into PostgreSQL 10+). Requires `wal_level = logical` in `postgresql.conf` and a user with `REPLICATION` privilege.
- MySQL CDC uses binlog replication. Requires `binlog_format = ROW` and a user with `REPLICATION SLAVE` and `REPLICATION CLIENT` privileges.
- LSN tracking (PostgreSQL) and GTID tracking (MySQL) enable resumable replication after restarts, especially when combined with LaminarDB checkpointing.
- Schema detection: both connectors can introspect the upstream table schema, but the `CREATE SOURCE` DDL must still declare the expected column types for validation.
- MySQL CDC uses `rustls` for TLS, avoiding the OpenSSL dependency.
- MongoDB CDC is **not available** -- there is no MongoDB connector crate in the main repository.
- Errors from CDC connectors are mapped to `ConnectorError` in the Python exception hierarchy.

## Testing Requirements
- Verify `CREATE SOURCE ... WITH (connector = 'postgres-cdc', ...)` succeeds and source appears in catalog.
- Verify `CREATE SOURCE ... WITH (connector = 'mysql-cdc', ...)` succeeds and source appears in catalog.
- Verify missing `connection` key raises `ConnectorError`.
- Verify missing `table` key raises `ConnectorError`.
- Integration test (PostgreSQL): with a running PostgreSQL instance, insert rows into the upstream table and verify they appear in LaminarDB queries.
- Integration test (MySQL): with a running MySQL instance, insert/update/delete rows and verify change events are captured.
- Verify `db.source_metrics("pg_source")` returns valid metrics for CDC sources.
- Verify pipeline topology shows CDC sources as source nodes.
- Verify schema mismatch between DDL and upstream table raises appropriate error.

---

# F-PY-015: RabbitMQ Source

## Metadata
- **Phase**: Future
- **Priority**: P2
- **Dependencies**: F-PY-011 (Source Connector Framework), RabbitMQ connector crate (does not exist)
- **Main Repo Module**: N/A
- **Implementation Status**: Not Available

## Summary

A RabbitMQ source connector would ingest messages from RabbitMQ queues into the LaminarDB streaming pipeline. This connector is **not available** because there is no RabbitMQ connector crate in the `laminar-connectors` package in the main repository. Implementation would require building a new connector crate in the main repo before it could be exposed through the Python bindings.

## Python API

```python
# PROPOSED API — NOT YET AVAILABLE

import laminardb

db = laminardb.open("mydb")

db.execute("""
    CREATE SOURCE events (
        event_type VARCHAR,
        payload VARCHAR,
        timestamp TIMESTAMP
    ) WITH (
        connector = 'rabbitmq',
        'uri' = 'amqp://guest:guest@localhost:5672',
        'queue' = 'events',
        'format' = 'json'
    )
""")
```

## Usage Examples

```python
# PROPOSED — NOT YET AVAILABLE

import laminardb

with laminardb.open("messaging") as db:
    # Consume from a RabbitMQ queue
    db.execute("""
        CREATE SOURCE task_queue (
            task_id VARCHAR,
            task_type VARCHAR,
            payload VARCHAR,
            priority INT,
            created_at TIMESTAMP
        ) WITH (
            connector = 'rabbitmq',
            'uri' = 'amqp://user:pass@rabbitmq.example.com:5672/vhost',
            'queue' = 'tasks',
            'format' = 'json',
            'prefetch_count' = '100',
            'ack_mode' = 'auto'
        )
    """)

    db.start()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'rabbitmq'` |
| `uri` | Yes | AMQP connection URI |
| `queue` | Yes | Queue name to consume from |
| `format` | No | Data format: `json` (default), `csv`, `raw` |
| `prefetch_count` | No | Consumer prefetch count for flow control |
| `ack_mode` | No | Acknowledgement mode: `auto` or `manual` |

## Implementation Notes
- **Not available.** No RabbitMQ connector crate exists in the main monorepo (`laminardb/crates/laminar-connectors/`).
- Implementation would require:
  1. A new `laminar-connectors/src/rabbitmq/` module implementing the `SourceConnector` trait.
  2. A new `rabbitmq` feature flag in `laminar-connectors` and `laminar-db`.
  3. A corresponding feature flag in the Python bindings' `Cargo.toml`.
- Likely Rust dependency: `lapin` crate for async AMQP 0.9.1 support.
- Would follow the same SQL-first pattern as existing connectors.

## Testing Requirements
- N/A (not implemented). When implemented:
  - Verify `CREATE SOURCE ... WITH (connector = 'rabbitmq', ...)` succeeds.
  - Integration test with a running RabbitMQ instance.
  - Verify message acknowledgement and prefetch behavior.

---

# F-PY-016: Data Lake Sources (Delta Lake)

## Metadata
- **Phase**: Phase 2 (Extended Connectors)
- **Priority**: P1
- **Dependencies**: F-PY-011 (Source Connector Framework), `laminar-db` with `delta-lake` feature
- **Main Repo Module**: `laminar-connectors/src/delta_lake/source.rs`
- **Implementation Status**: Newly Enabled

## Summary

The Delta Lake source connector reads data from Delta Lake tables and polls for new table versions, enabling LaminarDB to ingest data from cloud-based data lakes. It uses the `delta-rs` Rust library for Arrow-native reads with zero-copy data access. Cloud storage backends (S3, Azure Blob Storage, GCS) are supported via optional feature flags. The connector periodically polls for new Delta Lake table versions and ingests new data as it becomes available.

Note: An Iceberg source connector is **not available** -- only an Iceberg sink exists (see F-PY-022).

## Python API

```python
import laminardb

db = laminardb.open("lake")

# Read from a local Delta Lake table
db.execute("""
    CREATE SOURCE inventory (
        product_id BIGINT,
        warehouse VARCHAR,
        quantity INT,
        last_updated TIMESTAMP
    ) WITH (
        connector = 'delta-lake',
        'table_uri' = '/data/delta/inventory',
        'poll_interval_ms' = '5000'
    )
""")

# Read from S3 (requires delta-lake-s3 feature)
db.execute("""
    CREATE SOURCE events (
        event_id VARCHAR,
        event_type VARCHAR,
        payload VARCHAR,
        ts TIMESTAMP
    ) WITH (
        connector = 'delta-lake',
        'table_uri' = 's3://my-bucket/delta/events',
        'poll_interval_ms' = '10000'
    )
""")

db.start()
```

## Usage Examples

```python
import laminardb

with laminardb.open("lakehouse") as db:
    # Poll a Delta Lake table on S3 for new data
    db.execute("""
        CREATE SOURCE sales (
            transaction_id VARCHAR,
            store_id BIGINT,
            product_id BIGINT,
            quantity INT,
            revenue DOUBLE,
            sale_date DATE
        ) WITH (
            connector = 'delta-lake',
            'table_uri' = 's3://analytics-lake/delta/sales',
            'poll_interval_ms' = '30000'
        )
    """)

    # Create a real-time aggregation over the lake data
    db.execute("""
        CREATE STREAM daily_revenue AS
        SELECT
            store_id,
            sale_date,
            SUM(revenue) as total_revenue,
            COUNT(*) as transaction_count
        FROM sales
        GROUP BY store_id, sale_date
    """)

    db.start()

    # Query the aggregation
    result = db.query("SELECT * FROM daily_revenue")
    df = result.to_pandas()
    print(df)

    db.shutdown()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'delta-lake'` |
| `table_uri` | Yes | Delta Lake table URI (local path, `s3://...`, `az://...`, `gs://...`) |
| `poll_interval_ms` | No | Polling interval for new table versions in milliseconds (default: `5000`) |

Cloud storage authentication is configured via environment variables (e.g., `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` for S3) or cloud provider defaults.

## Implementation Notes
- The `delta-lake` feature is enabled in `Cargo.toml` line 32: `"delta-lake"` in the `laminar-db` dependency features list. This feature is **newly enabled**.
- Cloud storage backends require additional feature flags defined in `Cargo.toml` lines 51-57:
  - `delta-lake-s3`: S3 access via AWS SDK
  - `delta-lake-azure`: Azure Blob Storage
  - `delta-lake-gcs`: Google Cloud Storage
  - `delta-lake-unity`: Databricks Unity Catalog
  - `delta-lake-glue`: AWS Glue Catalog
  - `cloud-all`: Enables all cloud backends (`delta-lake-s3`, `delta-lake-azure`, `delta-lake-gcs`)
- These cloud storage features require the optional `laminar-connectors` dependency (`Cargo.toml` line 41).
- The connector uses `delta-rs` for Arrow-native reads, enabling zero-copy data transfer into the LaminarDB pipeline.
- Polling mechanism: the connector checks the Delta Lake `_delta_log/` for new JSON commits at the configured interval and reads only the new data files.
- Iceberg source is **not available**. Only Iceberg sink exists (see F-PY-022).

## Testing Requirements
- Verify `CREATE SOURCE ... WITH (connector = 'delta-lake', 'table_uri' = '...')` succeeds.
- Verify missing `table_uri` raises `ConnectorError`.
- Verify source appears in `db.sources()` and `db.list_tables()`.
- Integration test: create a local Delta Lake table, write data, verify it is ingested into LaminarDB.
- Verify polling: append new data to the Delta Lake table and verify it appears in subsequent queries.
- Verify `db.source_metrics("delta_source")` returns valid metrics.
- Verify cloud storage URIs (S3, Azure, GCS) are accepted when the corresponding feature is enabled.

---

# F-PY-017: Custom Python Source (Callback)

## Metadata
- **Phase**: Future
- **Priority**: P2
- **Dependencies**: F-PY-011 (Source Connector Framework), new Rust bridging code
- **Main Repo Module**: N/A (new code required in Python bindings)
- **Implementation Status**: Not Started

## Summary

A custom Python source would allow users to define data sources as Python generators or callback functions, enabling ingestion from any Python-accessible data source without writing Rust connector code. The Python function would yield data (as dicts, Arrow batches, or DataFrames) that gets converted to Arrow and fed into the LaminarDB streaming pipeline. This feature is **not started** and would require new Rust code in the Python bindings to bridge Python generators into the connector framework.

## Python API

```python
# PROPOSED API — NOT YET IMPLEMENTED

import laminardb
import pyarrow as pa

# Decorator-based source definition
@laminardb.source(schema=pa.schema([
    ("symbol", pa.utf8()),
    ("price", pa.float64()),
    ("ts", pa.timestamp("ms")),
]))
def market_feed():
    """Generator that yields rows for the streaming pipeline."""
    import websocket
    ws = websocket.create_connection("wss://feed.example.com")
    while True:
        msg = json.loads(ws.recv())
        yield {
            "symbol": msg["s"],
            "price": float(msg["p"]),
            "ts": msg["t"],
        }

# Register and use
db = laminardb.open("mydb")
db.register_source("prices", market_feed)
db.start()
```

## Usage Examples

```python
# PROPOSED — NOT YET IMPLEMENTED

import laminardb
import pyarrow as pa

# Batch-oriented source (yields RecordBatches)
@laminardb.source(schema=pa.schema([
    ("sensor_id", pa.utf8()),
    ("value", pa.float64()),
    ("ts", pa.timestamp("ms")),
]))
def sensor_poller():
    """Poll an HTTP API every second and yield batches."""
    import requests
    import time
    while True:
        resp = requests.get("https://api.sensors.example.com/latest")
        data = resp.json()
        yield [
            {"sensor_id": d["id"], "value": d["val"], "ts": d["time"]}
            for d in data["readings"]
        ]
        time.sleep(1.0)

# Async source
@laminardb.source(schema=my_schema)
async def async_source():
    """Async generator source."""
    import aiohttp
    async with aiohttp.ClientSession() as session:
        async with session.ws_connect("wss://...") as ws:
            async for msg in ws:
                yield json.loads(msg.data)

# Register with the connection
db = laminardb.open("mydb")
db.register_source("sensors", sensor_poller)
db.register_source("async_feed", async_source)

# Sources can now be referenced in SQL
db.execute("""
    CREATE STREAM alerts AS
    SELECT sensor_id, value FROM sensors WHERE value > 100
""")

db.start()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| N/A | N/A | Custom sources are configured programmatically, not via SQL WITH clauses |

## Implementation Notes
- **Not started.** This feature requires significant new Rust code in the Python bindings.
- Implementation approach:
  1. A `@laminardb.source(schema=...)` decorator in Python that wraps a generator function.
  2. A `Connection.register_source(name, source_fn)` method that registers the Python source.
  3. Rust bridging code that:
     - Spawns a background thread (not async, since Python generators are synchronous).
     - Calls the Python generator's `__next__()` (reacquiring the GIL each time).
     - Converts yielded data to Arrow `RecordBatch` via `conversion::python_to_batches()`.
     - Feeds batches into the pipeline's source channel.
  4. For async generators, use `pyo3-async-runtimes` to drive the Python async iterator from the Tokio runtime.
- Key challenge: GIL management. The background thread must acquire the GIL to call Python, but release it during Rust pipeline operations to avoid deadlocks.
- The `Writer` API (`db.writer("source_name")`) already provides a manual version of this pattern, but without the generator/decorator ergonomics.

## Testing Requirements
- N/A (not implemented). When implemented:
  - Verify `@laminardb.source` decorator correctly captures schema and generator.
  - Verify `db.register_source()` makes the source available for SQL queries.
  - Verify generator data flows through the pipeline and is queryable.
  - Verify generator errors are surfaced as `ConnectorError`.
  - Verify GIL release during pipeline processing (no deadlocks under load).
  - Verify async generator sources work with `pyo3-async-runtimes`.
  - Verify source metrics are reported for custom sources.

---

# F-PY-018: Sink Connector Framework

## Metadata
- **Phase**: Phase 1 (Core Infrastructure)
- **Priority**: P0
- **Dependencies**: `laminar-db` with `api` feature, `laminar-connectors`
- **Main Repo Module**: `laminar-connectors/src/sink.rs`, `laminar-db/src/pipeline/`
- **Implementation Status**: Implemented

## Summary

The sink connector framework provides the foundational infrastructure for exporting data from the LaminarDB streaming pipeline to external systems. Sinks are created declaratively via SQL DDL (`CREATE SINK ... AS SELECT ... WITH (connector = '...')`) and managed through the pipeline lifecycle. Each sink is defined as a continuous query whose results are automatically pushed to the configured external system. The Python bindings expose this through `Connection.execute()` for DDL, `Connection.sinks()` for catalog introspection, and `Connection.list_sinks()` for name listing.

The sink connector trait in the main repo supports multiple delivery guarantees: exactly-once (epoch-based two-phase commit), at-least-once, upsert mode, changelog mode, and schema evolution.

## Python API

```python
import laminardb

db = laminardb.open("mydb")

# Create a sink via SQL DDL
db.execute("""
    CREATE SINK alerts AS
    SELECT symbol, price, ts
    FROM trades
    WHERE price > 1000.0
    WITH (
        connector = 'kafka',
        'bootstrap.servers' = 'localhost:9092',
        'topic' = 'price-alerts'
    )
""")

# Catalog introspection
sinks: list[laminardb.SinkInfo] = db.sinks()
# Each SinkInfo has: name

names: list[str] = db.list_sinks()  # List sink names

# Pipeline lifecycle (sources and sinks start together)
db.start()
db.shutdown()
```

## Usage Examples

```python
import laminardb

with laminardb.open("pipeline") as db:
    # Create sources
    db.execute("""
        CREATE SOURCE sensor_data (
            device_id VARCHAR,
            temperature DOUBLE,
            ts TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'sensors',
            'group.id' = 'pipeline',
            'format' = 'json'
        )
    """)

    # Create a derived stream
    db.execute("""
        CREATE STREAM high_temp AS
        SELECT device_id, temperature, ts
        FROM sensor_data
        WHERE temperature > 80.0
    """)

    # Create a sink that writes high temperature alerts to Kafka
    db.execute("""
        CREATE SINK temp_alerts AS
        SELECT * FROM high_temp
        WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'temperature-alerts',
            'format' = 'json'
        )
    """)

    # Create a sink that archives all data to Delta Lake
    db.execute("""
        CREATE SINK archive AS
        SELECT * FROM sensor_data
        WITH (
            connector = 'delta-lake',
            'table_uri' = 's3://data-lake/sensor_archive',
            'write_mode' = 'append'
        )
    """)

    # Inspect the pipeline
    print(f"Sources: {db.list_tables()}")
    print(f"Streams: {db.list_streams()}")
    print(f"Sinks: {db.list_sinks()}")
    print(f"Sink count: {db.sink_count}")

    # Start everything
    db.start()

    # Monitor
    metrics = db.metrics()
    print(f"Pipeline state: {metrics.state}")
    print(f"Events ingested: {metrics.total_events_ingested}")
    print(f"Events emitted: {metrics.total_events_emitted}")

    db.shutdown()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Connector type identifier (e.g., `'kafka'`, `'websocket'`, `'delta-lake'`, `'iceberg'`) |
| `format` | No | Output data format (connector-specific, e.g., `json`, `csv`, `avro`) |

Additional keys are connector-specific and documented in F-PY-019 through F-PY-023.

## Implementation Notes
- Sinks are created via `Connection.execute()` with `CREATE SINK ... AS SELECT ... WITH (...)` syntax. The `AS SELECT` clause defines the continuous query whose results feed the sink.
- `SinkInfo` is a frozen pyclass wrapping `laminar_db::SinkInfo` with a `name` property (see `catalog.rs` lines 55-77).
- The sink connector trait (`SinkConnector`) in the main repo supports:
  - **Exactly-once**: Epoch-based two-phase commit protocol.
  - **At-least-once**: Messages may be redelivered on failure.
  - **Upsert mode**: Key-based updates (insert or update).
  - **Changelog mode**: Propagates INSERT/UPDATE/DELETE operations.
  - **Schema evolution**: Handles schema changes in the upstream query.
- Pipeline metrics (`PipelineMetrics.total_events_emitted`) track the total number of events written to all sinks.
- `Connection.sink_count` property returns the number of registered sinks.
- All catalog and metrics calls release the GIL.

## Testing Requirements
- Verify `db.execute("CREATE SINK ... AS SELECT ... WITH (connector = '...')")` returns `ExecuteResult` with `result_type == "ddl"`.
- Verify `db.sinks()` returns `SinkInfo` with correct name after sink creation.
- Verify `db.list_sinks()` includes newly created sink names.
- Verify `db.sink_count` increments after sink creation.
- Verify invalid connector names in `CREATE SINK` raise `ConnectorError`.
- Verify pipeline topology includes sink nodes after sink creation.
- Verify `db.metrics().total_events_emitted` increases as the sink processes data.

---

# F-PY-019: Kafka Sink

## Metadata
- **Phase**: Phase 1 (Core Connectors)
- **Priority**: P0
- **Dependencies**: F-PY-018 (Sink Connector Framework), `laminar-db` with `kafka` feature
- **Main Repo Module**: `laminar-connectors/src/kafka/sink.rs`
- **Implementation Status**: Implemented

## Summary

The Kafka sink connector writes query results from the LaminarDB streaming pipeline to Apache Kafka topics. It supports multiple data formats (JSON, CSV, Avro with Schema Registry), configurable partitioning strategies, delivery guarantees (at-least-once, exactly-once), and idempotent production. The sink is created as a continuous query whose results are automatically serialized and published to the configured Kafka topic.

## Python API

```python
import laminardb

db = laminardb.open("mydb")

# Create a source and sink
db.execute("""
    CREATE SOURCE trades (
        symbol VARCHAR, price DOUBLE, ts TIMESTAMP
    ) WITH (
        connector = 'kafka',
        'bootstrap.servers' = 'kafka:9092',
        'topic' = 'trades',
        'group.id' = 'ldb'
    )
""")

db.execute("""
    CREATE SINK price_alerts AS
    SELECT symbol, price, ts FROM trades WHERE price > 1000
    WITH (
        connector = 'kafka',
        'bootstrap.servers' = 'kafka:9092',
        'topic' = 'alerts',
        'format' = 'json'
    )
""")

db.start()
```

## Usage Examples

```python
import laminardb

with laminardb.open("streaming") as db:
    # Source from one Kafka topic
    db.execute("""
        CREATE SOURCE raw_events (
            event_id VARCHAR,
            user_id BIGINT,
            action VARCHAR,
            ts TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'user-events',
            'group.id' = 'enrichment',
            'format' = 'json'
        )
    """)

    # Aggregation stream
    db.execute("""
        CREATE STREAM user_activity AS
        SELECT
            user_id,
            COUNT(*) as event_count,
            MAX(ts) as last_seen
        FROM raw_events
        GROUP BY user_id
    """)

    # Sink aggregated results to another Kafka topic with Avro format
    db.execute("""
        CREATE SINK activity_sink AS
        SELECT * FROM user_activity
        WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'user-activity-summary',
            'format' = 'avro',
            'schema.registry.url' = 'http://schema-registry:8081',
            'delivery.guarantee' = 'exactly-once'
        )
    """)

    # Sink with key-based partitioning
    db.execute("""
        CREATE SINK partitioned_sink AS
        SELECT user_id, action, ts FROM raw_events
        WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'partitioned-events',
            'format' = 'json',
            'partitioner' = 'key-hash'
        )
    """)

    db.start()

    # Monitor sink throughput via pipeline metrics
    import time
    for _ in range(5):
        time.sleep(2)
        m = db.metrics()
        print(f"Events emitted to sinks: {m.total_events_emitted}")

    db.shutdown()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'kafka'` |
| `bootstrap.servers` | Yes | Comma-separated list of Kafka broker addresses |
| `topic` | Yes | Kafka topic to produce to |
| `format` | No | Output format: `json` (default), `csv`, `avro` |
| `schema.registry.url` | No | Confluent Schema Registry URL (required when `format = 'avro'`) |
| `partitioner` | No | Partitioning strategy: `key-hash`, `round-robin`, `sticky` (default: `round-robin`) |
| `delivery.guarantee` | No | Delivery guarantee: `at-least-once` (default), `exactly-once` |
| `security.protocol` | No | Security protocol: `PLAINTEXT`, `SSL`, `SASL_PLAINTEXT`, `SASL_SSL` |
| `sasl.mechanism` | No | SASL mechanism: `PLAIN`, `SCRAM-SHA-256`, `SCRAM-SHA-512` |
| `sasl.username` | No | SASL username |
| `sasl.password` | No | SASL password |

## Implementation Notes
- The `kafka` feature covers both source and sink connectors. Enabled in `Cargo.toml` line 28.
- Exactly-once delivery uses Kafka's transactional producer with epoch-based two-phase commit coordinated by the LaminarDB checkpoint system.
- Idempotent production (`enable.idempotence = true` in librdkafka) is automatically enabled when `delivery.guarantee = 'exactly-once'`.
- Schema Registry integration: when `format = 'avro'`, the sink serializes Arrow batches to Avro using the registered schema, with automatic schema evolution support.
- `key-hash` partitioning extracts a partition key from the first column of the query result (or a designated key column).
- The sink processes batches from the pipeline's streaming execution engine and publishes them asynchronously. Backpressure from Kafka (e.g., slow broker) propagates back through the pipeline.

## Testing Requirements
- Verify `CREATE SINK ... WITH (connector = 'kafka', ...)` succeeds and sink appears in `db.sinks()`.
- Verify required keys (`bootstrap.servers`, `topic`) cause errors when missing.
- Verify sink appears in pipeline topology as a sink node.
- Integration test: with a running Kafka broker, verify messages are produced to the configured topic.
- Verify Avro format with Schema Registry produces correctly serialized messages.
- Verify `exactly-once` delivery guarantee with transactional producer.
- Verify `db.metrics().total_events_emitted` increases as the sink produces messages.

---

# F-PY-020: WebSocket Sink

## Metadata
- **Phase**: Phase 2 (Extended Connectors)
- **Priority**: P1
- **Dependencies**: F-PY-018 (Sink Connector Framework), `laminar-db` with `websocket` feature
- **Main Repo Module**: `laminar-connectors/src/websocket/sink.rs`
- **Implementation Status**: Newly Enabled

## Summary

The WebSocket sink connector pushes query results from the LaminarDB streaming pipeline to WebSocket clients. It supports two modes: server mode (listens for incoming connections and fans out results to all connected clients) and client mode (connects to an external WebSocket server and pushes results). Features include fan-out to multiple clients, per-client backpressure handling, configurable slow-client policies (drop or disconnect), and a replay buffer for late-joining clients.

## Python API

```python
import laminardb

db = laminardb.open("mydb")

# Server mode: serve query results via WebSocket
db.execute("""
    CREATE SINK dashboard AS
    SELECT symbol, price, volume
    FROM trades
    WITH (
        connector = 'websocket',
        'mode' = 'server',
        'bind_address' = '0.0.0.0:8080',
        'format' = 'json'
    )
""")

# Client mode: push results to an external WebSocket server
db.execute("""
    CREATE SINK push_notifications AS
    SELECT alert_type, message, ts
    FROM alerts
    WITH (
        connector = 'websocket',
        'mode' = 'client',
        'url' = 'wss://notifications.example.com/ingest',
        'format' = 'json'
    )
""")

db.start()
```

## Usage Examples

```python
import laminardb

with laminardb.open("dashboard_backend") as db:
    # Create a source
    db.execute("""
        CREATE SOURCE market_data (
            symbol VARCHAR,
            bid DOUBLE,
            ask DOUBLE,
            ts TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'market-data',
            'group.id' = 'dashboard',
            'format' = 'json'
        )
    """)

    # Create a real-time aggregation
    db.execute("""
        CREATE STREAM market_summary AS
        SELECT
            symbol,
            AVG(bid) as avg_bid,
            AVG(ask) as avg_ask,
            COUNT(*) as tick_count
        FROM market_data
        GROUP BY symbol
    """)

    # Serve the aggregation to browser clients via WebSocket
    # Clients connect to ws://host:8080 and receive JSON updates
    db.execute("""
        CREATE SINK live_dashboard AS
        SELECT * FROM market_summary
        WITH (
            connector = 'websocket',
            'mode' = 'server',
            'bind_address' = '0.0.0.0:8080',
            'format' = 'json'
        )
    """)

    db.start()

    # The WebSocket server is now running on port 8080.
    # Browser clients can connect and receive real-time updates.
    # Monitor with:
    print(f"Sinks: {db.list_sinks()}")
    print(f"Pipeline state: {db.pipeline_state}")

    import time
    time.sleep(60)  # Keep running
    db.shutdown()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'websocket'` |
| `mode` | No | Connection mode: `server` (default) or `client` |
| `bind_address` | Conditional | Bind address for server mode (required when `mode = 'server'`, e.g., `'0.0.0.0:8080'`) |
| `url` | Conditional | WebSocket URL for client mode (required when `mode = 'client'`) |
| `format` | No | Output format: `json` (default), `csv`, `raw` |
| `slow_client_policy` | No | Policy for slow clients: `drop` (drop messages, default) or `disconnect` |
| `replay_buffer_size` | No | Number of recent messages to replay to newly connected clients (default: `0`) |

## Implementation Notes
- The `websocket` feature is enabled in `Cargo.toml` line 33. This feature is **newly enabled** in the Python bindings and covers both source and sink.
- Server mode uses `tokio-tungstenite` with a `TcpListener`. Each connected client gets its own send channel. The sink fans out each batch to all connected clients.
- Client mode connects to a single upstream WebSocket server and pushes batches as messages.
- Per-client backpressure: each client has a bounded send buffer. When the buffer fills, the `slow_client_policy` determines behavior:
  - `drop`: Newest messages for that client are dropped (other clients are unaffected).
  - `disconnect`: The slow client is disconnected.
- Replay buffer: server mode can maintain a ring buffer of recent messages to send to newly connected clients, ensuring they receive recent state.
- The sink serializes Arrow `RecordBatch` data into the configured format (JSON, CSV, or raw bytes) before sending.

## Testing Requirements
- Verify `CREATE SINK ... WITH (connector = 'websocket', 'mode' = 'server', ...)` succeeds.
- Verify `CREATE SINK ... WITH (connector = 'websocket', 'mode' = 'client', ...)` succeeds.
- Verify missing `bind_address` in server mode raises error.
- Verify missing `url` in client mode raises error.
- Verify sink appears in `db.sinks()` and `db.list_sinks()`.
- Integration test (server mode): connect a WebSocket client to the sink's port and verify JSON messages are received.
- Integration test (client mode): set up a WebSocket server, create a client-mode sink, and verify messages arrive.
- Verify slow client handling: connect a slow client and verify the policy is applied.
- Verify `db.metrics().total_events_emitted` increases as data is pushed.

---

# F-PY-021: RabbitMQ Sink

## Metadata
- **Phase**: Future
- **Priority**: P2
- **Dependencies**: F-PY-018 (Sink Connector Framework), RabbitMQ connector crate (does not exist)
- **Main Repo Module**: N/A
- **Implementation Status**: Not Available

## Summary

A RabbitMQ sink connector would publish query results from the LaminarDB streaming pipeline to RabbitMQ exchanges and queues. This connector is **not available** because there is no RabbitMQ connector crate in the `laminar-connectors` package in the main repository. Implementation would require building a new connector crate in the main repo before it could be exposed through the Python bindings.

## Python API

```python
# PROPOSED API — NOT YET AVAILABLE

import laminardb

db = laminardb.open("mydb")

db.execute("""
    CREATE SINK notifications AS
    SELECT user_id, message, ts FROM alerts
    WITH (
        connector = 'rabbitmq',
        'uri' = 'amqp://guest:guest@localhost:5672',
        'exchange' = 'notifications',
        'routing_key' = 'alerts',
        'format' = 'json'
    )
""")
```

## Usage Examples

```python
# PROPOSED — NOT YET AVAILABLE

import laminardb

with laminardb.open("messaging") as db:
    db.execute("""
        CREATE SOURCE orders (
            order_id BIGINT, status VARCHAR, ts TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'orders',
            'group.id' = 'notifier',
            'format' = 'json'
        )
    """)

    # Publish completed orders to RabbitMQ
    db.execute("""
        CREATE SINK order_completed AS
        SELECT order_id, ts FROM orders WHERE status = 'completed'
        WITH (
            connector = 'rabbitmq',
            'uri' = 'amqp://user:pass@rabbitmq:5672/vhost',
            'exchange' = 'order-events',
            'routing_key' = 'order.completed',
            'format' = 'json',
            'mandatory' = 'true'
        )
    """)

    db.start()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'rabbitmq'` |
| `uri` | Yes | AMQP connection URI |
| `exchange` | Yes | Exchange name to publish to |
| `routing_key` | No | Routing key for message routing |
| `format` | No | Output format: `json` (default), `csv`, `raw` |
| `mandatory` | No | Whether messages must be routable: `true` or `false` (default: `false`) |

## Implementation Notes
- **Not available.** No RabbitMQ connector crate exists in the main monorepo.
- Implementation would require:
  1. A new `laminar-connectors/src/rabbitmq/` module implementing the `SinkConnector` trait.
  2. A new `rabbitmq` feature flag in `laminar-connectors` and `laminar-db`.
  3. A corresponding feature flag in the Python bindings' `Cargo.toml`.
- Likely Rust dependency: `lapin` crate for async AMQP 0.9.1 support.
- Would support publisher confirms for at-least-once delivery.

## Testing Requirements
- N/A (not implemented). When implemented:
  - Verify `CREATE SINK ... WITH (connector = 'rabbitmq', ...)` succeeds.
  - Integration test with a running RabbitMQ instance.
  - Verify publisher confirms and message routing.

---

# F-PY-022: Data Lake Sinks (Delta Lake, Iceberg)

## Metadata
- **Phase**: Phase 2 (Extended Connectors)
- **Priority**: P1
- **Dependencies**: F-PY-018 (Sink Connector Framework), `laminar-db` with `delta-lake` and/or `iceberg` features
- **Main Repo Module**: `laminar-connectors/src/delta_lake/sink.rs`, `laminar-connectors/src/iceberg/sink.rs`
- **Implementation Status**: Newly Enabled

## Summary

Data lake sink connectors write streaming query results from the LaminarDB pipeline to open table format storage systems. Two connectors are available:

- **Delta Lake Sink**: Writes to Delta Lake tables with ACID transaction guarantees, epoch-to-version mapping, partitioning, background compaction, schema evolution, and vacuum (retention cleanup). Supports append, overwrite, and upsert (CDC MERGE) write modes. Cloud storage backends (S3, Azure, GCS) are supported via optional feature flags.
- **Iceberg Sink**: Writes to Apache Iceberg tables with snapshot isolation, hidden partitioning (DAYS, BUCKET, TRUNCATE), catalog support (REST, Glue, Hive), and multiple file formats (Parquet, ORC, Avro). Supports append and upsert (equality deletes) write modes.

## Python API

```python
import laminardb

db = laminardb.open("mydb")

# Delta Lake sink: append mode
db.execute("""
    CREATE SINK archive AS
    SELECT * FROM sensor_data
    WITH (
        connector = 'delta-lake',
        'table_uri' = 's3://data-lake/sensors',
        'write_mode' = 'append'
    )
""")

# Iceberg sink: append mode with Parquet format
db.execute("""
    CREATE SINK warehouse AS
    SELECT * FROM enriched_orders
    WITH (
        connector = 'iceberg',
        'catalog_uri' = 'https://iceberg-rest-catalog.example.com',
        'table' = 'analytics.enriched_orders',
        'write_mode' = 'append'
    )
""")

db.start()
```

## Usage Examples

```python
import laminardb

with laminardb.open("lakehouse_pipeline") as db:
    # Kafka source
    db.execute("""
        CREATE SOURCE transactions (
            tx_id VARCHAR,
            account_id BIGINT,
            amount DOUBLE,
            tx_type VARCHAR,
            ts TIMESTAMP
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'transactions',
            'group.id' = 'lakehouse',
            'format' = 'json'
        )
    """)

    # Delta Lake sink with upsert mode (CDC MERGE on tx_id)
    db.execute("""
        CREATE SINK delta_transactions AS
        SELECT * FROM transactions
        WITH (
            connector = 'delta-lake',
            'table_uri' = 's3://data-lake/transactions',
            'write_mode' = 'upsert'
        )
    """)

    # Iceberg sink with hidden date partitioning
    db.execute("""
        CREATE SINK iceberg_transactions AS
        SELECT * FROM transactions
        WITH (
            connector = 'iceberg',
            'catalog_uri' = 'https://glue.us-east-1.amazonaws.com',
            'table' = 'production.transactions',
            'write_mode' = 'append'
        )
    """)

    db.start()

    import time
    while True:
        time.sleep(10)
        m = db.metrics()
        print(f"Ingested: {m.total_events_ingested}, "
              f"Emitted to sinks: {m.total_events_emitted}")
        if m.total_events_emitted > 10000:
            break

    db.shutdown()
```

```python
import laminardb

with laminardb.open("overwrite_example") as db:
    # Source
    db.execute("""
        CREATE SOURCE daily_summary (
            date DATE,
            region VARCHAR,
            total_sales DOUBLE,
            order_count BIGINT
        ) WITH (
            connector = 'kafka',
            'bootstrap.servers' = 'kafka:9092',
            'topic' = 'daily-summaries',
            'group.id' = 'overwrite-pipeline',
            'format' = 'json'
        )
    """)

    # Delta Lake sink with overwrite mode (replaces table on each epoch)
    db.execute("""
        CREATE SINK summary_snapshot AS
        SELECT * FROM daily_summary
        WITH (
            connector = 'delta-lake',
            'table_uri' = '/data/delta/summary_snapshot',
            'write_mode' = 'overwrite'
        )
    """)

    db.start()
```

## Configuration Keys

### Delta Lake Sink
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'delta-lake'` |
| `table_uri` | Yes | Delta Lake table URI (local path, `s3://...`, `az://...`, `gs://...`) |
| `write_mode` | No | Write mode: `append` (default), `overwrite`, `upsert` |

### Iceberg Sink
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'iceberg'` |
| `catalog_uri` | Yes | Iceberg catalog URI (REST catalog URL, Glue endpoint, Hive metastore URI) |
| `table` | Yes | Fully qualified table name (e.g., `'database.table'`) |
| `write_mode` | No | Write mode: `append` (default), `upsert` |

Cloud storage authentication for both connectors is configured via environment variables or cloud provider defaults.

## Implementation Notes
- **Delta Lake** sink: enabled via `delta-lake` feature in `Cargo.toml` line 32. **Newly enabled.**
  - ACID transactions: each pipeline epoch maps to a Delta Lake commit, ensuring atomic writes.
  - Upsert mode (CDC MERGE): uses Delta Lake's MERGE operation keyed on specified columns.
  - Background compaction: small files produced by streaming writes are periodically compacted into larger files.
  - Schema evolution: new columns in the streaming query are automatically added to the Delta Lake table schema.
  - Vacuum: old files beyond the retention period are cleaned up.
  - Cloud storage features: `delta-lake-s3`, `delta-lake-azure`, `delta-lake-gcs` (Cargo.toml lines 51-57).
- **Iceberg** sink: enabled via `iceberg` feature in `Cargo.toml` line 35. **Newly enabled.**
  - Snapshot isolation: each commit creates a new Iceberg snapshot.
  - Hidden partitioning: transform functions (DAYS, HOURS, BUCKET, TRUNCATE) applied to partition columns without requiring explicit partition columns in the data.
  - Catalog support: REST catalog, AWS Glue, Hive metastore.
  - File formats: Parquet (default), ORC, Avro.
  - Upsert mode: uses equality deletes for key-based updates.
- Both sinks use Arrow-native writes, avoiding serialization overhead.
- The `postgres-sink` feature is also enabled in `Cargo.toml` line 30 but is not a data lake connector and is not covered in this spec.

## Testing Requirements
- Verify `CREATE SINK ... WITH (connector = 'delta-lake', ...)` succeeds and sink appears in `db.sinks()`.
- Verify `CREATE SINK ... WITH (connector = 'iceberg', ...)` succeeds and sink appears in `db.sinks()`.
- Verify missing `table_uri` for Delta Lake raises error.
- Verify missing `catalog_uri` or `table` for Iceberg raises error.
- Integration test (Delta Lake): write data via sink, read back from Delta Lake table, verify correctness.
- Integration test (Delta Lake upsert): write conflicting keys, verify MERGE semantics.
- Integration test (Iceberg): write data via sink, verify Iceberg snapshots are created.
- Verify `db.metrics().total_events_emitted` increases as data is written to the lake.
- Verify cloud storage URIs are accepted when corresponding features are enabled.
- Verify schema evolution: add a column to the streaming query and verify Delta Lake/Iceberg tables are updated.

---

# F-PY-023: Custom Python Sink (Callback)

## Metadata
- **Phase**: Future
- **Priority**: P2
- **Dependencies**: F-PY-018 (Sink Connector Framework), new Rust bridging code
- **Main Repo Module**: N/A (new code required in Python bindings)
- **Implementation Status**: Not Started

## Summary

A custom Python sink would allow users to define data sinks as Python callback functions, enabling export to any Python-accessible system without writing Rust connector code. The Python function would receive Arrow RecordBatches (or converted DataFrames/dicts) from the pipeline and process them in user-defined logic. This feature is **not started** and would require new Rust code in the Python bindings to bridge the pipeline's sink output to Python callbacks.

## Python API

```python
# PROPOSED API — NOT YET IMPLEMENTED

import laminardb
import pyarrow as pa

# Decorator-based sink definition
@laminardb.sink(name="http_push")
def push_to_api(batch: pa.RecordBatch):
    """Process each batch from the pipeline."""
    import requests
    data = batch.to_pydict()
    requests.post("https://api.example.com/ingest", json=data)

# Function-based sink registration
def send_to_slack(batch: pa.RecordBatch):
    """Send alerts to Slack."""
    for row in batch.to_pylist():
        slack_client.chat_postMessage(
            channel="#alerts",
            text=f"Alert: {row['message']}"
        )

db = laminardb.open("mydb")
db.register_sink("slack_alerts", send_to_slack)

# Use in SQL
db.execute("""
    CREATE SINK slack_sink AS
    SELECT message, severity FROM alerts WHERE severity = 'critical'
    WITH (connector = 'python', 'name' = 'slack_alerts')
""")

db.start()
```

## Usage Examples

```python
# PROPOSED — NOT YET IMPLEMENTED

import laminardb
import pyarrow as pa

# Batch processing sink with error handling
@laminardb.sink(name="warehouse_loader")
def load_to_warehouse(batch: pa.RecordBatch):
    """Load each batch into a data warehouse."""
    import sqlalchemy
    engine = sqlalchemy.create_engine("postgresql://...")
    df = batch.to_pandas()
    df.to_sql("fact_events", engine, if_exists="append", index=False)

# Async sink callback
@laminardb.sink(name="async_push")
async def async_push(batch: pa.RecordBatch):
    """Async callback for non-blocking I/O."""
    import aiohttp
    async with aiohttp.ClientSession() as session:
        await session.post(
            "https://api.example.com/batch",
            json=batch.to_pydict()
        )

# Sink with metrics/logging
@laminardb.sink(name="logged_sink")
def logged_sink(batch: pa.RecordBatch):
    """Sink with custom logging."""
    import logging
    logger = logging.getLogger("laminardb.sink")
    logger.info(f"Processing batch: {batch.num_rows} rows")
    # Process the batch...
    logger.info("Batch processed successfully")

db = laminardb.open("mydb")

# Register sinks
db.register_sink("warehouse", load_to_warehouse)
db.register_sink("async_push", async_push)

# Create SQL sinks that use the Python callbacks
db.execute("""
    CREATE SINK to_warehouse AS
    SELECT * FROM enriched_data
    WITH (connector = 'python', 'name' = 'warehouse')
""")

db.start()
```

## Configuration Keys
| Key | Required | Description |
|-----|----------|-------------|
| `connector` | Yes | Must be `'python'` |
| `name` | Yes | Name of the registered Python sink callback |

## Implementation Notes
- **Not started.** This feature requires significant new Rust code in the Python bindings.
- Implementation approach:
  1. A `@laminardb.sink(name=...)` decorator in Python that wraps a callback function.
  2. A `Connection.register_sink(name, callback)` method that registers the Python callback.
  3. Rust bridging code that:
     - Implements the `SinkConnector` trait.
     - Receives `RecordBatch` from the pipeline.
     - Acquires the GIL and calls the Python callback with the batch (converted to `pyarrow.RecordBatch` via `pyo3-arrow`).
     - Handles errors from the Python callback and propagates them as `ConnectorError`.
  4. For async callbacks, use `pyo3-async-runtimes` to drive the Python coroutine.
- Key challenge: GIL management and throughput. Each callback invocation requires acquiring the GIL, which serializes Python callback execution. For high-throughput pipelines, this could become a bottleneck.
- Mitigation strategies:
  - Batch delivery: the callback receives full `RecordBatch` objects (not individual rows), amortizing GIL acquisition overhead.
  - Background thread pool: multiple Python threads could process batches in parallel (with the GIL, they would still serialize, but I/O-bound callbacks would benefit).
- The existing `Subscription` / `StreamSubscription` APIs already provide a pull-based alternative where Python code iterates over pipeline output. The custom sink adds a push-based, declarative alternative.

## Testing Requirements
- N/A (not implemented). When implemented:
  - Verify `@laminardb.sink` decorator correctly captures the callback.
  - Verify `db.register_sink()` makes the sink available for `CREATE SINK ... WITH (connector = 'python', ...)`.
  - Verify callback receives correctly converted `pyarrow.RecordBatch` data.
  - Verify callback errors are surfaced as `ConnectorError` in the pipeline.
  - Verify GIL release during non-callback pipeline processing (no deadlocks).
  - Verify async callback sinks work with `pyo3-async-runtimes`.
  - Verify `db.metrics().total_events_emitted` includes events processed by Python sinks.
  - Stress test: high-throughput pipeline with Python sink to verify no GIL contention issues.

---

# Connector Feature Summary

| Feature | Type | Connector | Status | Cargo Feature |
|---------|------|-----------|--------|---------------|
| F-PY-011 | Framework | Source Framework | Implemented | `api` |
| F-PY-012 | Source | Kafka | Implemented | `kafka` |
| F-PY-013 | Source | WebSocket | Newly Enabled | `websocket` |
| F-PY-014 | Source | PostgreSQL CDC | Implemented | `postgres-cdc` |
| F-PY-014 | Source | MySQL CDC | Newly Enabled | `mysql-cdc` |
| F-PY-015 | Source | RabbitMQ | Not Available | N/A |
| F-PY-016 | Source | Delta Lake | Newly Enabled | `delta-lake` |
| F-PY-017 | Source | Custom Python | Not Started | N/A |
| F-PY-018 | Framework | Sink Framework | Implemented | `api` |
| F-PY-019 | Sink | Kafka | Implemented | `kafka` |
| F-PY-020 | Sink | WebSocket | Newly Enabled | `websocket` |
| F-PY-021 | Sink | RabbitMQ | Not Available | N/A |
| F-PY-022 | Sink | Delta Lake | Newly Enabled | `delta-lake` |
| F-PY-022 | Sink | Iceberg | Newly Enabled | `iceberg` |
| F-PY-023 | Sink | Custom Python | Not Started | N/A |

### Enabled Cargo Features (Cargo.toml)

```toml
laminar-db = { path = "laminardb/crates/laminar-db", features = [
    "api",
    "jit",
    "kafka",            # F-PY-012, F-PY-019
    "postgres-cdc",     # F-PY-014
    "postgres-sink",    # (not covered in connector specs)
    "rocksdb",          # (state backend, not a connector)
    "delta-lake",       # F-PY-016, F-PY-022
    "websocket",        # F-PY-013, F-PY-020
    "mysql-cdc",        # F-PY-014
    "iceberg",          # F-PY-022
    "parquet-lookup",   # (not a connector)
] }
```

### Optional Cloud Storage Features

```toml
delta-lake-s3 = ["dep:laminar-connectors", "laminar-connectors/delta-lake-s3"]
delta-lake-azure = ["dep:laminar-connectors", "laminar-connectors/delta-lake-azure"]
delta-lake-gcs = ["dep:laminar-connectors", "laminar-connectors/delta-lake-gcs"]
delta-lake-unity = ["dep:laminar-connectors", "laminar-connectors/delta-lake-unity"]
delta-lake-glue = ["dep:laminar-connectors", "laminar-connectors/delta-lake-glue"]
cloud-all = ["delta-lake-s3", "delta-lake-azure", "delta-lake-gcs"]
```
