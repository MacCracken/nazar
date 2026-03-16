# ADR-002: Shared state via Arc<RwLock<MonitorState>>

**Status**: Accepted
**Date**: 2026-03-16

## Context

Nazar has three concurrent consumers of metrics data:

1. **Collector loop** — polls `/proc` and daimon on an interval, writes snapshots
2. **GUI dashboard** — reads latest snapshot + history every frame (~1s)
3. **HTTP API** — serves snapshot/alerts/predictions on demand

These run on different threads (collector on tokio, GUI on the main thread).

## Decision

Use `Arc<RwLock<MonitorState>>` as the shared state handle, type-aliased as
`SharedState` in `nazar-core`.

## Rationale

- **Single writer, multiple readers** — `RwLock` allows the collector to hold
  an exclusive write lock briefly while all readers can overlap.
- **Simplicity** — no channels, no message passing, no actor model. The state
  is just a struct behind a lock.
- **Testability** — MCP tool handlers, HTTP API, and UI can all be tested by
  constructing a `SharedState` directly and populating it.

## Trade-offs

- Writer starvation is theoretically possible but unlikely at 5s poll intervals.
- If the GUI holds the read lock too long, the collector blocks. Mitigated by
  keeping the read lock only for the duration of rendering (not IO).
- `RwLock` poison on panic — acceptable since a panic in the collector is fatal.

## Alternatives considered

- **`tokio::sync::RwLock`** — async-aware but requires `.await` in UI code
  which doesn't run in an async context. Would need `try_read()` everywhere.
- **Channels (mpsc)** — more complex, requires the UI to maintain its own copy
  of state, duplicating memory.
- **`dashmap`** — overkill for a single-key state; adds a dependency.
