# Nazar Development Roadmap

> **Status**: v1 MVP complete | **Version**: 2026.3.17

---

## v1 — MVP System Monitor (Complete)

All 23 items shipped. See [CHANGELOG.md](../CHANGELOG.md) for details.

- /proc readers: CPU (delta-based), memory, disk (statvfs), network (delta-based)
- Metrics pipeline: tokio collector, shared state, anomaly detection, predictions
- HTTP API: `/health`, `/v1/snapshot`, `/v1/alerts`, `/v1/predict` on port 8095
- MCP tool handlers: `nazar_dashboard`, `nazar_alerts`, `nazar_predict`, `nazar_history`, `nazar_config`
- GUI dashboard: CPU/memory sparklines, disk/network panels, alerts with timestamps, live service status, top processes
- Per-process CPU/memory: delta-based CPU%, RSS memory, state, thread count via /proc/[pid]/stat + statm
- 67 tests, 0 clippy warnings

---

## v2 — Enhanced Monitoring

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Per-process CPU/memory | **Done** | Delta-based CPU%, RSS memory, threads via /proc/[pid]/stat + statm |
| 2 | Disk I/O throughput | Not started | Read /proc/diskstats for IOPS and read/write rates |
| 3 | Network traffic time series | Not started | Per-interface rx/tx rate graphs over time |
| 4 | GPU monitoring | Not started | NVIDIA (nvidia-smi) and AMD (amdgpu) |
| 5 | Temperature sensors | Not started | /sys/class/hwmon/ and /sys/class/thermal/ |
| 6 | Agent detail view | Not started | Click agent in UI to see per-agent resource breakdown |
| 7 | MCP transport (stdio/HTTP) | Not started | Wire `nazar-mcp` handlers to a live MCP server |
| 8 | AGNOS MCP tool registration | Not started | Register nazar_* tools in daimon's tool registry |
| 9 | AGNOS agnoshi intents | Not started | NL commands: "show system status", "predict memory", etc. |

## v3 — AI Features

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | LLM-assisted triage | Not started | Route alerts through hoosh for NL explanation |
| 2 | Process recommendations | Not started | "Agent X is using 3x normal memory — likely leak" |
| 3 | Capacity planning | Not started | Multi-metric prediction with confidence intervals |
| 4 | Correlation detection | Not started | "Disk I/O spike correlates with agent Y's file operations" |

## v4 — Polish

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Configurable dashboard layouts | Not started | Drag-and-drop widget arrangement |
| 2 | Alert notifications | Not started | Desktop notifications via aethersafha |
| 3 | Historical data persistence | Not started | SQLite for long-term metric storage |
| 4 | Export to Prometheus | Not started | `/metrics` endpoint in Prometheus format |

---

## Engineering Backlog

No outstanding items.
