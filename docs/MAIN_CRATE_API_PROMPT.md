# Main Crate API Changes — Agent Prompt

Copy the section below (between the `---` markers) and paste it as a prompt in a Claude Code session opened in the `laminardb` main repository.

---

## Task

Add passthrough methods to `api::Connection` in `crates/laminar-db/src/api/connection.rs` and re-export types from `crates/laminar-db/src/api/mod.rs` so that the Python bindings can access the full LaminarDB API surface.

The `api` module is the FFI-friendly wrapper around `LaminarDB`. It already exposes `open`, `execute`, `query`, `query_stream`, `writer`, `insert`, `get_schema`, `list_sources`, `list_streams`, `list_sinks`, `start`, `close`, `is_closed`, `checkpoint`, and `is_checkpoint_enabled`. The Python bindings (in the `laminardb-python` repo) consume **only** this `api` module.

The following methods exist on `LaminarDB` (in `src/db.rs`) but are NOT yet exposed through `api::Connection`. We need thin passthrough wrappers that delegate to `self.inner`.

### Changes to `crates/laminar-db/src/api/connection.rs`

Add these methods to `impl Connection`:

```rust
// ── Catalog info ──

/// List source info with schemas and watermark columns.
pub fn source_info(&self) -> Vec<crate::SourceInfo> {
    self.inner.sources()
}

/// List sink info.
pub fn sink_info(&self) -> Vec<crate::SinkInfo> {
    self.inner.sinks()
}

/// List stream info with SQL.
pub fn stream_info(&self) -> Vec<crate::StreamInfo> {
    self.inner.streams()
}

/// List active/completed query info.
pub fn query_info(&self) -> Vec<crate::QueryInfo> {
    self.inner.queries()
}

// ── Pipeline topology & state ──

/// Get the pipeline topology graph.
pub fn pipeline_topology(&self) -> crate::PipelineTopology {
    self.inner.pipeline_topology()
}

/// Get the pipeline state as a string ("Created", "Running", "Stopped", etc.).
#[must_use]
pub fn pipeline_state(&self) -> String {
    self.inner.pipeline_state().to_string()
}

/// Get the global pipeline watermark.
#[must_use]
pub fn pipeline_watermark(&self) -> i64 {
    self.inner.pipeline_watermark()
}

/// Get total events processed across all sources.
#[must_use]
pub fn total_events_processed(&self) -> u64 {
    self.inner.total_events_processed()
}

/// Get the number of registered sources.
#[must_use]
pub fn source_count(&self) -> usize {
    self.inner.source_count()
}

/// Get the number of registered sinks.
#[must_use]
pub fn sink_count(&self) -> usize {
    self.inner.sink_count()
}

/// Get the number of active queries.
#[must_use]
pub fn active_query_count(&self) -> usize {
    self.inner.active_query_count()
}

// ── Metrics ──

/// Get pipeline-wide metrics snapshot.
pub fn metrics(&self) -> crate::PipelineMetrics {
    self.inner.metrics()
}

/// Get metrics for a specific source.
pub fn source_metrics(&self, name: &str) -> Option<crate::SourceMetrics> {
    self.inner.source_metrics(name)
}

/// Get metrics for all sources.
pub fn all_source_metrics(&self) -> Vec<crate::SourceMetrics> {
    self.inner.all_source_metrics()
}

/// Get metrics for a specific stream.
pub fn stream_metrics(&self, name: &str) -> Option<crate::StreamMetrics> {
    self.inner.stream_metrics(name)
}

/// Get metrics for all streams.
pub fn all_stream_metrics(&self) -> Vec<crate::StreamMetrics> {
    self.inner.all_stream_metrics()
}

// ── Query control ──

/// Cancel a running query by ID.
pub fn cancel_query(&self, query_id: u64) -> Result<(), ApiError> {
    self.inner.cancel_query(query_id).map_err(ApiError::from)
}

// ── Shutdown ──

/// Gracefully shut down the streaming pipeline.
///
/// Unlike `close()`, this waits for in-flight events to drain.
pub fn shutdown(&self) -> Result<(), ApiError> {
    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        std::thread::scope(|s| {
            s.spawn(|| {
                let inner = Arc::clone(&self.inner);
                handle.block_on(async move { inner.shutdown().await })
            })
            .join()
            .unwrap()
        })
    } else {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| ApiError::internal(format!("Runtime error: {e}")))?;
        rt.block_on(self.inner.shutdown())
    };
    result.map_err(ApiError::from)
}

// ── True subscription ──

/// Subscribe to a continuous query, returning an ArrowSubscription.
///
/// This creates a streaming query via `execute()`, extracts the QueryStream,
/// and bridges it into an ArrowSubscription for proper channel-based delivery
/// with timeout support.
///
/// Note: For v1, this delegates to query_stream internally. A future version
/// should use LaminarDB::subscribe() directly once the API supports it.
pub fn subscribe_stream(&self, sql: &str) -> Result<super::subscription::ArrowSubscription, ApiError> {
    // For now, create via query_stream and wrap.
    // The ArrowSubscription needs a laminar_core::streaming::Subscription<ArrowRecord>,
    // but we only have a QueryStream. This is a known gap — see implementation note below.
    //
    // Temporary: return QueryStream-based subscription.
    // TODO: Bridge to true ArrowSubscription when LaminarDB.subscribe() supports
    // untyped Arrow subscriptions natively.
    Err(ApiError::internal("subscribe_stream not yet implemented — use query_stream()".into()))
}
```

### Changes to `crates/laminar-db/src/api/mod.rs`

Add these re-exports so Python bindings can reference the types:

```rust
// Add after existing re-exports
pub use crate::{
    SourceInfo, SinkInfo, StreamInfo, QueryInfo,
    PipelineTopology, PipelineNode, PipelineEdge, PipelineNodeType,
    PipelineMetrics, PipelineState, SourceMetrics, StreamMetrics,
};
```

### Implementation Note: `subscribe_stream()`

The `ArrowSubscription` type wraps `laminar_core::streaming::Subscription<ArrowRecord>`, which is a channel-based subscription from the streaming pipeline. Creating one requires:

1. The query to be registered as a streaming query (not a one-shot DataFusion query)
2. A `Subscription<ArrowRecord>` channel from the stream's sink

`LaminarDB::subscribe()` exists but takes a `name: &str` (stream name, not SQL). To bridge SQL → ArrowSubscription:

**Option A**: Add a `subscribe_sql()` method to `LaminarDB` that:
1. Executes the SQL to create a temporary stream
2. Gets the stream's subscription channel
3. Returns an `ArrowSubscription`

**Option B**: Leave `subscribe_stream()` as a stub for now. The Python bindings already work using `query_stream()` which returns a `QueryStream` (finite result set iterated via `next()`). True continuous subscriptions from streaming pipelines can be added later when the use case is clearer.

**Recommendation**: Implement Option B (stub) for now. The Python bindings' current `subscribe()` → `query_stream()` pattern works for the documented use cases. Add the real implementation when streaming pipeline subscriptions are needed.

### Tests

Add tests in the existing `#[cfg(test)] mod tests` block at the bottom of `connection.rs`:

```rust
#[test]
fn test_source_info() {
    let conn = Connection::open().unwrap();
    conn.execute("CREATE SOURCE test_info (id BIGINT, name VARCHAR)").unwrap();
    let info = conn.source_info();
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].name, "test_info");
    assert_eq!(info[0].schema.fields().len(), 2);
}

#[test]
fn test_pipeline_state() {
    let conn = Connection::open().unwrap();
    let state = conn.pipeline_state();
    assert!(!state.is_empty());
}

#[test]
fn test_metrics() {
    let conn = Connection::open().unwrap();
    let m = conn.metrics();
    assert_eq!(m.total_events_ingested, 0);
}

#[test]
fn test_source_count() {
    let conn = Connection::open().unwrap();
    assert_eq!(conn.source_count(), 0);
    conn.execute("CREATE SOURCE cnt_test (x BIGINT)").unwrap();
    assert_eq!(conn.source_count(), 1);
}

#[test]
fn test_cancel_query_invalid() {
    let conn = Connection::open().unwrap();
    // Cancelling a non-existent query should return an error
    let result = conn.cancel_query(999);
    assert!(result.is_err());
}

#[test]
fn test_shutdown() {
    let conn = Connection::open().unwrap();
    assert!(conn.shutdown().is_ok());
}
```

### Verification

After making changes:
```bash
cargo check -p laminar-db --features api
cargo test -p laminar-db --features api
cargo clippy -p laminar-db --features api -- -D warnings
```

### Summary of changes

| File | Change |
|------|--------|
| `src/api/connection.rs` | ~120 lines: 17 new methods on `impl Connection` |
| `src/api/mod.rs` | ~5 lines: re-export catalog/metrics/pipeline types |
| `src/api/connection.rs` (tests) | ~50 lines: 6 new tests |

Total: ~175 lines of thin passthrough code. No new logic, no architectural changes.

---
