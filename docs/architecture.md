# Nazar Architecture

## Overview

Nazar is an AI-native system monitor for AGNOS. It reads system metrics from
`/proc` (local) and the daimon REST API (remote), applies AI-powered analysis,
and presents results through a real-time GUI dashboard and HTTP API.

## Crate Structure

```
nazar (workspace root + binary)
├── nazar-core     — Shared types, metrics structs, time series, config, shared state
├── nazar-api      — HTTP client for daimon + ProcReader for /proc metrics
├── nazar-ui       — egui/eframe GUI dashboard with charts (egui_plot)
├── nazar-ai       — Anomaly detection, linear regression, predictions
└── nazar-mcp      — MCP tool definitions + handlers for AGNOS agent integration
```

## Data Flow

```
                    ┌─────────────────────────────────────┐
                    │          Collector Loop              │
                    │  (tokio interval task, every 5s)     │
                    └──────────┬──────────────────────────┘
                               │
         ┌─────────────────────┼──────────────────────┐
         │                     │                      │
    /proc/stat            /proc/meminfo          /proc/net/dev
    /proc/loadavg         /proc/mounts           /proc/net/tcp
         │               + statvfs                    │
         └─────────────────────┼──────────────────────┘
                               │
                        ProcReader.snapshot()
                               │
                               ▼
                    ┌──────────────────────┐
                    │   AnomalyDetector    │
                    │   (check + predict)  │
                    └──────────┬───────────┘
                               │
                               ▼
                  ┌────────────────────────┐
                  │  Arc<RwLock<           │
                  │    MonitorState>>      │
                  │  (SharedState)         │
                  └──┬──────┬──────┬──────┘
                     │      │      │
              ┌──────┘      │      └──────┐
              ▼             ▼             ▼
        ┌──────────┐  ┌──────────┐  ┌──────────┐
        │  GUI     │  │  HTTP    │  │  MCP     │
        │  (egui)  │  │  API     │  │  Tools   │
        │ :desktop │  │  :8095   │  │ (5 tools)│
        └──────────┘  └──────────┘  └──────────┘
```

## Key Components

### ProcReader (`nazar-api`)

Stateful reader for Linux `/proc` filesystem. Holds previous CPU time
counters to compute delta-based usage percentages between polls.

- `read_cpu()` — `/proc/stat` + `/proc/loadavg` (delta-based, per-core)
- `read_memory()` — `/proc/meminfo`
- `read_disk()` — `/proc/mounts` + `statvfs()` (real filesystems only)
- `read_network()` — `/proc/net/dev` + `/proc/net/tcp`
- `snapshot()` — assembles all readers into a `SystemSnapshot`

### MonitorState (`nazar-core`)

Shared state struct written by the collector and read by all consumers:

- Latest `SystemSnapshot`
- Alert history (capped at 100)
- Prediction results
- Time series for CPU, memory, disk (per-mount), network rx/tx
- Runtime config

### HTTP API (`src/main.rs`)

Lightweight axum server on port 8095:

| Endpoint | Returns |
|----------|---------|
| `GET /health` | Version, uptime, sample count |
| `GET /v1/snapshot` | Full latest SystemSnapshot |
| `GET /v1/alerts` | Current anomaly alerts |
| `GET /v1/predict` | Resource exhaustion predictions |

### MCP Tool Handlers (`nazar-mcp`)

5 tools with full backend implementations reading from SharedState:

- `nazar_dashboard` — snapshot summary
- `nazar_alerts` — alerts with optional severity filter
- `nazar_predict` — exhaustion predictions
- `nazar_history` — time series data for any metric
- `nazar_config` — get/set runtime configuration

## Key Design Decisions

See [ADRs](adr/) for detailed rationale.

1. **Reads /proc directly** — no sysinfo crate; minimal deps, full control ([ADR-001](adr/001-proc-readers-over-sysinfo.md))
2. **Shared state via RwLock** — single writer, multiple readers; simple and testable ([ADR-002](adr/002-shared-state-rwlock.md))
3. **Own HTTP API** — axum on port 8095 for external access ([ADR-003](adr/003-axum-http-api.md))
4. **egui for GUI** — lightweight, same Rust stack, no web overhead
5. **Separate MCP crate** — tool registration without pulling in UI
6. **Headless mode** — collector + API without GUI for server/edge
