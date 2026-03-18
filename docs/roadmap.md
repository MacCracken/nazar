# Nazar Development Roadmap

> **Status**: v1 MVP complete | **Version**: 2026.3.17

---

## v1 — MVP System Monitor (Complete)

All 23 items shipped. See [CHANGELOG.md](../CHANGELOG.md) for details.

- /proc readers: CPU (delta-based), memory, disk (statvfs), network (delta-based)
- Metrics pipeline: tokio collector, shared state, anomaly detection, predictions
- HTTP API: `/health`, `/v1/snapshot`, `/v1/alerts`, `/v1/predict` on port 8095
- MCP tool handlers: `nazar_dashboard`, `nazar_alerts`, `nazar_predict`, `nazar_history`, `nazar_config`
- GUI dashboard: CPU/memory sparklines, disk/network panels, alerts with timestamps, live service status
- 56 tests, 0 clippy warnings

---

## v2 — Enhanced Monitoring

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Per-process CPU/memory | Not started | Read /proc/[pid]/stat for top-N processes |
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

Low-severity items from code audit. Address as time permits.

| # | Item | Notes |
|---|------|-------|
| 1 | Monitor spawned task JoinHandles | Collector/API tasks are fire-and-forget; silent panic = silent failure |
| 2 | Evaluate RwLock poison recovery vs panic | Recovering hides bugs; poisoned state may be inconsistent |
| 3 | Wire `show_anomalies`/`show_agents` config flags | Config fields exist and are settable via MCP but no code reads them |
| 4 | Populate real agent data from daimon API | `AgentSummary` is always zeroed; needs daimon `/v1/agents` integration |
| 5 | CORS headers on HTTP API | Add `tower_http::cors::CorsLayer` if browser-based consumption is needed |
| 6 | Dynamic poll interval updates | `poll_interval_secs` change via MCP requires restart |
| 7 | /proc/mounts octal escape decoding | Mount points with spaces use `\040` escaping; currently passed raw |
| 8 | Alert deduplication / cooldown | Sustained threshold breaches generate identical alerts every tick |
| 9 | Config persistence to file | `NazarConfig` is serializable but never saved/loaded; changes lost on restart |
| 10 | Stricter /proc/stat cpu line matching | `starts_with("cpu")` could match unexpected future kernel lines |
