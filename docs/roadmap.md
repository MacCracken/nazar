# Nazar Development Roadmap

> **Status**: v1 MVP complete, v2-v4 largely complete | **Version**: 2026.3.17

---

## v1 — MVP System Monitor (Complete)

All 23 items shipped. See [CHANGELOG.md](../CHANGELOG.md) for details.

- /proc readers: CPU (delta-based), memory, disk (statvfs), network (delta-based)
- Metrics pipeline: tokio collector, shared state, anomaly detection, predictions
- HTTP API: `/health`, `/v1/snapshot`, `/v1/alerts`, `/v1/predict` on port 8095
- MCP tool handlers: `nazar_dashboard`, `nazar_alerts`, `nazar_predict`, `nazar_history`, `nazar_config`
- GUI dashboard: CPU/memory sparklines, disk/network panels, alerts with timestamps, live service status, top processes
- Per-process CPU/memory: delta-based CPU%, RSS memory, state, thread count via /proc/[pid]/stat + statm
- Temperature sensors, disk I/O throughput, per-interface network sparklines
- GPU monitoring: AMD amdgpu (sysfs) + NVIDIA (nvidia-smi fallback)
- Multi-metric capacity planning with confidence intervals
- Cross-metric correlation detection (Pearson)
- SQLite persistence (~/.local/share/nazar/metrics.db)
- Prometheus export (`GET /metrics`)
- MCP stdio transport (JSON-RPC 2.0)
- 87 tests, 0 clippy warnings

---

## v2 — Enhanced Monitoring

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Per-process CPU/memory | **Done** | Delta-based CPU%, RSS memory, threads via /proc/[pid]/stat + statm |
| 2 | Disk I/O throughput | **Done** | Delta-based read/write bytes from /proc/diskstats |
| 3 | Network traffic time series | **Done** | Per-interface rx/tx rate sparklines with history |
| 4 | GPU monitoring | **Done** | AMD amdgpu via sysfs, NVIDIA via nvidia-smi fallback |
| 5 | Temperature sensors | **Done** | /sys/class/thermal + /sys/class/hwmon with labels and critical thresholds |
| 6 | Agent detail view | Not started | Click agent in UI to see per-agent resource breakdown (requires AGNOS) |
| 7 | MCP transport (stdio) | **Done** | JSON-RPC 2.0 over stdin/stdout via `--mcp` flag |
| 8 | AGNOS MCP tool registration | Not started | Register nazar_* tools in daimon's tool registry (requires AGNOS) |
| 9 | AGNOS agnoshi intents | Not started | NL commands: "show system status", "predict memory" (requires AGNOS) |

## v3 — AI Features

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | LLM-assisted triage | Not started | Route alerts through hoosh for NL explanation (requires AGNOS) |
| 2 | Process recommendations | Not started | "Agent X is using 3x normal memory — likely leak" (requires AGNOS) |
| 3 | Capacity planning | **Done** | Multi-metric (CPU, memory, disk) prediction with 95% confidence intervals |
| 4 | Correlation detection | **Done** | Pearson r for CPU/disk_io, CPU/net_tx, memory/swap, memory/net_rx |

## v4 — Polish

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Configurable dashboard layouts | Not started | Drag-and-drop widget arrangement |
| 2 | Alert notifications | Not started | Desktop notifications via aethersafha (requires AGNOS) |
| 3 | Historical data persistence | **Done** | SQLite WAL mode, auto-prune 30 days, `--db-path` CLI flag |
| 4 | Export to Prometheus | **Done** | `GET /metrics` endpoint in Prometheus text exposition format |

---

## Remaining (requires AGNOS ecosystem)

| Item | Depends on |
|------|------------|
| Agent detail view | daimon agent data |
| MCP tool registration | daimon tool registry |
| agnoshi intents | agnoshi NL engine |
| LLM-assisted triage | hoosh LLM integration |
| Process recommendations | hoosh + agent correlation |
| Alert notifications | aethersafha |
| Dashboard layouts | UX design |
