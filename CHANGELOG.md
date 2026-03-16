# Changelog

All notable changes to Nazar will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased] — v1 MVP

### Added

- **ProcReader** — stateful `/proc` reader with delta-based CPU usage calculation
  - `/proc/stat` for total + per-core CPU percentages
  - `/proc/stat` process/thread counts (`processes`, `procs_running`)
  - `/proc/mounts` + `statvfs` for disk space (filters to real filesystems)
  - `/proc/net/dev` for per-interface network metrics
  - `/proc/net/tcp{,6}` for active connection count
  - `snapshot()` method assembles all metrics into a `SystemSnapshot`
- **Metrics pipeline** — tokio interval task collects metrics every 5s
  - Feeds snapshots to `AnomalyDetector` for threshold-based alerting
  - Writes to `SharedState` (`Arc<RwLock<MonitorState>>`) for all consumers
  - Time series history for CPU, memory, disk (per-mount), network rx/tx
- **HTTP API** — axum server on port 8095
  - `GET /health` — version, uptime, sample count
  - `GET /v1/snapshot` — latest full `SystemSnapshot` as JSON
  - `GET /v1/alerts` — current anomaly alerts
  - `GET /v1/predict` — resource exhaustion predictions
- **MCP tool handlers** — 5 tools now have full backend implementations
  - `nazar_dashboard` — snapshot summary from shared state
  - `nazar_alerts` — alerts with optional severity filter
  - `nazar_predict` — exhaustion predictions
  - `nazar_history` — time series data for any metric (cpu, memory, network, disk)
  - `nazar_config` — get/set runtime configuration
- **GUI dashboard** — enhanced egui interface
  - Real CPU usage percentage (delta-based, not just load average)
  - Per-core CPU bars
  - CPU and memory sparkline charts (egui_plot)
  - Disk usage panel with per-mount progress bars
  - Network panel with per-interface stats
  - Alerts panel with severity-colored badges
  - Predictions panel showing time-to-exhaustion
  - Live service status (placeholder until daimon connection)
- **Shared state** — `MonitorState` in nazar-core with `SharedState` type alias
  - Alert, AlertSeverity, PredictionResult, Trend types moved from nazar-ai to nazar-core
  - All consumers (UI, API, MCP) read from the same state
- **Documentation** — ADRs, updated architecture, MCP tool reference with examples
- **44 tests** across all crates, clean clippy, 0 warnings

### Changed

- nazar-api refactored: `ApiClient` for HTTP, new `ProcReader` for /proc
- nazar-ui now takes `SharedState` instead of polling directly
- nazar-mcp tools have full handlers (previously definitions only)
- nazar-ai types (Alert, PredictionResult, Trend) moved to nazar-core

### Dependencies

- Added `libc` (statvfs for disk metrics)
- Added `egui_plot` 0.34 (time series sparklines)

## [2026.3.16] - 2026-03-16

### Added — Initial Scaffold

- **nazar-core**: System metrics types (CPU, memory, disk, network, agents), time series buffer, config
- **nazar-api**: Daimon API client (health, metrics, agents, anomaly, scan, edge), /proc readers (meminfo, loadavg)
- **nazar-ui**: egui/eframe GUI dashboard with CPU/memory progress bars, service status panel
- **nazar-ai**: Threshold-based anomaly detection, linear regression resource prediction
- **nazar-mcp**: 5 MCP tool definitions (dashboard, alerts, predict, history, config)
- **CLI**: `--headless`, `--api-url`, `--port` flags
- **CI/CD**: GitHub Actions for check, test, clippy, fmt, release (amd64 + arm64)
- **27 tests** across all crates, 0 warnings
