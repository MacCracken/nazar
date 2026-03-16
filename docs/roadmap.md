# Nazar Development Roadmap

> **Status**: Phase 1 in progress | **Version**: 2026.3.16

---

## Phase 1 — Core Monitoring (Current)

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Core types and metrics | **Done** | SystemSnapshot, CpuMetrics, MemoryMetrics, DiskMetrics, NetworkMetrics |
| 2 | /proc readers | **Done** | CPU load average, memory from /proc/meminfo |
| 3 | Daimon API client | **Done** | Health, metrics, agents, anomaly alerts, scan status, edge dashboard |
| 4 | Anomaly detection | **Done** | Threshold-based alerts for CPU, memory, disk |
| 5 | Resource prediction | **Done** | Linear regression for memory exhaustion forecasting |
| 6 | Basic GUI | **Done** | egui dashboard with CPU/memory bars, service status |
| 7 | MCP tool definitions | **Done** | 5 tools: dashboard, alerts, predict, history, config |
| 8 | CI/CD pipeline | **Done** | GitHub Actions: check, test, clippy, fmt, release |
| 9 | AGNOS marketplace recipe | **Done** | `recipes/marketplace/nazar.toml` in agnosticos |
| 10 | AGNOS MCP tools | Not started | 5 nazar_* tools registered in daimon |
| 11 | AGNOS agnoshi intents | Not started | NL commands for monitoring |

## Phase 2 — Enhanced Monitoring

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Per-process CPU/memory | Not started | Read /proc/[pid]/stat for top processes |
| 2 | Disk I/O monitoring | Not started | Read /proc/diskstats for IOPS/throughput |
| 3 | Network traffic graphs | Not started | Per-interface rx/tx time series |
| 4 | GPU monitoring | Not started | NVIDIA (nvidia-smi) and AMD (amdgpu) |
| 5 | Temperature sensors | Not started | /sys/class/hwmon/ or /sys/class/thermal/ |
| 6 | Agent detail view | Not started | Click agent → see resource breakdown |

## Phase 3 — AI Features

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | LLM-assisted triage | Not started | Route alerts through hoosh for NL explanation |
| 2 | Process recommendations | Not started | "Agent X is using 3x normal memory — likely leak" |
| 3 | Capacity planning | Not started | Multi-metric prediction with confidence intervals |
| 4 | Correlation detection | Not started | "Disk I/O spike correlates with agent Y's file operations" |

## Phase 4 — Polish

| # | Item | Status | Notes |
|---|------|--------|-------|
| 1 | Configurable dashboard layouts | Not started | Drag-and-drop widget arrangement |
| 2 | Alert notifications | Not started | Desktop notifications via aethersafha |
| 3 | Historical data persistence | Not started | SQLite for long-term metric storage |
| 4 | Export to Prometheus | Not started | `/metrics` endpoint in Prometheus format |
