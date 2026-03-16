# ADR-003: Nazar HTTP API via axum

**Status**: Accepted
**Date**: 2026-03-16

## Context

Nazar needs its own HTTP API (separate from daimon) so that external tools,
scripts, and AGNOS agents can query system metrics without going through the GUI.

## Decision

Expose a lightweight REST API on port 8095 (configurable via `--port`) using
axum, running on the same tokio runtime as the collector.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Nazar health + uptime + version |
| GET | `/v1/snapshot` | Latest full SystemSnapshot as JSON |
| GET | `/v1/alerts` | Current anomaly alerts |
| GET | `/v1/predict` | Resource exhaustion predictions |

## Rationale

- **axum** is already a workspace dependency (used by nazar-mcp).
- Shares the tokio runtime — no extra threads.
- JSON responses use the same serde types from nazar-core, ensuring
  consistency between the API and MCP tools.
- The `/health` endpoint enables monitoring nazar itself.

## Trade-offs

- Port 8095 must not conflict with other AGNOS services (daimon=8090,
  hoosh=8088).
- No authentication — assumes trusted local network (same as daimon).
