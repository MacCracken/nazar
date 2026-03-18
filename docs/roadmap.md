# Nazar Development Roadmap

> **Status**: v1 MVP complete | **Version**: 2026.3.17

---

## v1 — MVP System Monitor

A fully functional system monitor that reads real metrics, displays them in a
live dashboard, detects anomalies, predicts resource exhaustion, and exposes
an HTTP API and MCP tools.

### /proc Readers

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | CPU metrics from /proc/stat | **Done** | Delta-based total % and per-core usage between two reads |
| 2 | Process/thread counts from /proc/stat | **Done** | Parse `processes` and `procs_running` lines |
| 3 | Disk space from /proc/mounts + statvfs | **Done** | Mount point, device, filesystem, total/used/available |
| 4 | Network interfaces from /proc/net/dev | **Done** | Per-interface rx/tx bytes, packets, errors, up/down |

### Metrics Pipeline

| # | Item | Status | Notes |
|---|------|--------|-------|
| 5 | SystemSnapshot assembly | **Done** | ProcReader.snapshot() combines all /proc readers |
| 6 | Metrics collection loop | **Done** | Tokio interval task: poll, snapshot, feed to detector/history |
| 7 | Shared state (Arc<RwLock<>>) | **Done** | MonitorState in nazar-core; collector writes, UI/API/MCP read |
| 8 | Anomaly detector integration | **Done** | AnomalyDetector wired into collection loop, alerts logged |

### HTTP API (axum)

| # | Item | Status | Notes |
|---|------|--------|-------|
| 9 | GET /health | **Done** | Version, uptime, sample count |
| 10 | GET /v1/snapshot | **Done** | Latest SystemSnapshot as JSON |
| 11 | GET /v1/alerts | **Done** | Current anomaly alerts |
| 12 | GET /v1/predict | **Done** | Memory/disk exhaustion predictions |

### MCP Tool Handlers

| # | Item | Status | Notes |
|---|------|--------|-------|
| 13 | nazar_dashboard handler | **Done** | Returns snapshot summary from shared state |
| 14 | nazar_alerts handler | **Done** | Returns alerts, optional severity filter |
| 15 | nazar_predict handler | **Done** | Returns prediction results from detector |
| 16 | nazar_history handler | **Done** | Returns time series points for any metric |
| 17 | nazar_config handler | **Done** | Get/set NazarConfig fields at runtime |

### GUI Dashboard

| # | Item | Status | Notes |
|---|------|--------|-------|
| 18 | Real CPU % display | **Done** | Delta-based usage from ProcReader, per-core bars |
| 19 | Disk usage panel | **Done** | Per-mount progress bars with used/total |
| 20 | Network panel | **Done** | Per-interface rx/tx with error counts |
| 21 | Alerts panel | **Done** | Bottom panel with severity-colored alert list |
| 22 | Time series charts | **Done** | egui_plot sparklines for CPU and memory history |
| 23 | Service status (live) | **Done** | Probes daimon/hoosh health endpoints every ~30s |

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
| 7 | AGNOS MCP tool registration | Not started | Register 5 nazar_* tools in daimon's tool registry |
| 8 | AGNOS agnoshi intents | Not started | NL commands: "show system status", "predict memory", etc. |

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
