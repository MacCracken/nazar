# Nazar Architecture

## Overview

Nazar is an AI-native system monitor for AGNOS. It reads system metrics from
`/proc` (local) and the daimon REST API (remote), applies AI-powered analysis,
and presents results through a real-time GUI dashboard.

## Crate Structure

```
nazar (workspace root + binary)
├── nazar-core     — Shared types, metrics structs, time series, config
├── nazar-api      — HTTP client for daimon + /proc readers
├── nazar-ui       — egui/eframe GUI dashboard
├── nazar-ai       — Anomaly detection, linear regression, predictions
└── nazar-mcp      — MCP tool definitions for AGNOS agent integration
```

## Data Flow

```
/proc/stat ──┐
/proc/meminfo ┤
/proc/diskstats┤──→ nazar-api ──→ nazar-core (SystemSnapshot)
               │                       │
daimon:8090 ───┘                       ├──→ nazar-ai (alerts, predictions)
                                       └──→ nazar-ui (dashboard)
```

## Key Design Decisions

1. **Reads /proc directly** — no dependency on sysinfo crate; minimal overhead
2. **Polls daimon API** — leverages existing agent metrics, anomaly detection, phylax status
3. **egui for GUI** — same toolkit as other AGNOS desktop apps (Aequi uses Tauri, but egui is lighter for a monitor)
4. **Separate MCP crate** — tool definitions can be registered with daimon without pulling in the full UI
5. **Headless mode** — for server/edge deployments where GUI is unavailable
