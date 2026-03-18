# Changelog

All notable changes to Nazar will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [2026.3.18] - 2026-03-18

### Added

- **TUI mode** — terminal dashboard via `nazar --tui`, like btop/htop but for nazar
  - **nazar-tui crate**: built on ratatui + crossterm for cross-platform terminal rendering
  - Live-updating panels: CPU (gauge + sparkline + per-core bars), Memory (gauge + sparkline + swap), Disk (table with usage bars), Network (RX/TX sparklines + per-interface stats), GPU (utilization + VRAM gauges), Temperatures (color-coded readings)
  - **AGNOS-aware layout**: auto-detects AGNOS (via agents/daimon service) and reorganizes layout to show Services + Agents panels alongside system metrics
  - **Standard layout**: btop-style arrangement for non-AGNOS systems (CPU/Mem top, Disk/Net mid, GPU/Temps, tabbed bottom panel)
  - **Tabbed bottom panel**: 6 tabs — Processes (sortable), Alerts (scrollable, severity-colored), Predictions (with trend indicators + confidence intervals), Agents (per-agent CPU/memory table), Services (state indicators + uptime), AI Insights (triage + recommendations)
  - **Process table sorting**: cycle sort column (CPU/Mem/PID/Name) with `s`, reverse with `r`, visual sort indicators
  - **Keyboard navigation**: `Tab`/`Shift+Tab` cycle tabs, `1-6` jump to tab, `Up/Down` scroll, `q`/`Esc` quit, `?` help overlay
  - **Adaptive layout**: degrades gracefully on small terminals, collapses empty panels (no GPU, no temps)
  - **Reuses SharedState**: same collector feeds GUI, TUI, headless, and MCP modes — zero data duplication
  - HTTP API runs in background during TUI mode
- **`--tui` CLI flag** for launching terminal dashboard mode

## [2026.3.17] - 2026-03-17

### Added

- **AGNOS integration** — full integration with daimon and hoosh
  - **MCP tool registration**: registers 5 nazar tools with daimon's `/v1/mcp/tools` on startup, with HTTP callback endpoint `/v1/mcp/call`
  - **Agent detail view**: UI panel showing per-agent CPU/memory breakdown from daimon `/v1/agents`
  - **LLM-assisted triage**: forwards alerts to hoosh `/v1/chat/completions` for NL explanation
  - **Process recommendations**: sends top processes to hoosh for analysis every ~5 min
  - **Alert notifications**: publishes alerts to daimon event bus (`/v1/events/publish`, topic `nazar.alerts`) for desktop notification
  - **agnoshi discoverability**: MCP tools registered in daimon are discoverable by agnoshi shell
  - All integrations gracefully degrade when services are unavailable
  - Auto-retries MCP registration if daimon starts after nazar
- **AI Insights UI panel** — displays LLM triage explanations and process recommendations when available
- **Multi-metric capacity planning** — predicts exhaustion for CPU, memory, and all disk mounts
  - Generalized `predict_metric()` with configurable target thresholds
  - 95% confidence intervals via standard error of the regression slope
  - `PredictionResult` gains `confidence_low` / `confidence_high` fields
  - `predict_all()` replaces single-metric `predict_memory_exhaustion()`
- **Correlation detection** — Pearson cross-metric correlation analysis
  - `CorrelationDetector` tracks 4 metric pairs: cpu/disk_io, cpu/net_tx, memory/swap, memory/net_rx
  - `CorrelationResult` struct with coefficient, strength (Strong/Moderate/Weak), sample count
  - Computed every 12th tick (~1 min), stored in `MonitorState.correlations`
  - `GET /v1/correlations` HTTP endpoint
- **Prometheus export** — `GET /metrics` endpoint in Prometheus text exposition format
  - CPU, memory, swap, per-disk usage/IO, network rx/tx, GPU utilization/VRAM/temp, temperatures, alerts count
  - Labeled metrics for multi-instance resources (disks, GPUs, sensors)
  - `Content-Type: text/plain; version=0.0.4`
- **SQLite persistence** — `nazar-store` crate for long-term metric storage
  - WAL mode, auto-prune rows older than 30 days on startup
  - Stores snapshots, alerts, and predictions every ~1 min (12th tick)
  - Loads 100 most recent snapshots on startup to prime detectors
  - `--db-path` CLI flag, defaults to `~/.local/share/nazar/metrics.db`
- **MCP stdio transport** — JSON-RPC 2.0 server over stdin/stdout
  - `--mcp` CLI flag runs nazar as an MCP server
  - Handles `initialize`, `tools/list`, `tools/call` methods
  - All 5 nazar tools callable via MCP protocol
  - Tracing goes to stderr, stdout reserved for JSON-RPC
- **GPU monitoring** — AMD and NVIDIA GPU metrics
  - `GpuMetrics` struct: id, driver, name, utilization%, VRAM used/total, temp, power, clock
  - **AMD amdgpu**: reads sysfs — `gpu_busy_percent`, `mem_info_vram_*`, hwmon temp/power/clock
  - **NVIDIA**: fallback via `nvidia-smi --query-gpu` CSV output (utilization, VRAM, temp, power, clock)
  - Auto-detects GPU driver from `/sys/class/drm/card*/device/driver` symlink
  - GUI: "GPU" panel with utilization + VRAM progress bars, temp/power/clock inline
  - MCP: `nazar_dashboard` includes `gpu` array
  - Included in `/v1/snapshot` JSON response
- **Temperature sensors** — reads from `/sys/class/thermal/thermal_zone*/` and `/sys/class/hwmon/hwmon*/`
  - `ThermalInfo` struct: label, temp_celsius, critical_celsius
  - Reads thermal zone type, hwmon labels, and critical trip points
  - GUI: "Temperatures" panel with color-coded readings (yellow >70C, red >90% of critical)
  - MCP: `nazar_dashboard` includes `temperatures` array
  - Included in `/v1/snapshot` JSON response
- **Disk I/O throughput** — delta-based read/write bytes per device
  - Parses `/proc/diskstats` for sector counts, converts to bytes (512B sectors)
  - `DiskMetrics.read_bytes`/`write_bytes` now populated (were always 0)
  - GUI: disk panel shows "R: X KB  W: Y KB" per mount
  - MCP: dashboard disk entries include `read_kb`/`write_kb`
- **Network traffic time series** — per-interface rx/tx rate sparklines
  - `MonitorState.net_iface_history`: per-interface `(TimeSeries, TimeSeries)` for rx/tx B/s
  - Collector tracks history for all non-loopback interfaces, prunes disappeared interfaces
  - GUI: inline sparkline chart per interface showing RX/TX KB/s over time
- **Per-process CPU/memory monitoring** — top-N processes by CPU usage
  - `ProcessInfo` struct: pid, name, state, cpu_percent (delta-based), memory_bytes (RSS), memory_percent, threads
  - `ProcReader::read_processes()` enumerates `/proc/[pid]/stat` and `/proc/[pid]/statm`
  - Delta-based CPU% per process using same pattern as per-core CPU
  - Tracks all PIDs' CPU times for accurate ranking across ticks
  - Only reads memory (`/proc/[pid]/statm`) for top-N processes (performance)
  - System-wide thread count populated from process enumeration
  - GUI: "Top Processes" grid panel with PID, name, CPU%, memory, state, threads
  - HTTP API: `GET /v1/processes` endpoint + included in `/v1/snapshot`
  - MCP: `nazar_dashboard` includes `top_processes` array
  - Config: `top_processes` (default 10, range 1-50) settable via MCP
- **Live service status** — probes daimon (8090) and hoosh (8088) health endpoints every ~30s
  - `ServiceChecker` struct with reusable HTTP client and host validation
  - Shows Running/Failed/Stopped state with uptime and port in GUI
- **Panic-safe RwLock helpers** — `read_state()`/`write_state()` recover from poisoned locks
- **Alert timestamps in UI** — alerts show relative age ("5s ago", "3m ago", "2h ago")
- **Configurable anomaly thresholds** — `cpu_threshold`, `memory_threshold`, `disk_threshold` in `NazarConfig`
  - `AnomalyDetector::from_config()` constructor
  - MCP `nazar_config` supports get/set of thresholds with range validation (0.0–100.0, finite)
  - Collector re-reads thresholds from config each tick
- **CLI `--bind` flag** — control HTTP API bind address (defaults to `127.0.0.1`)
- **CORS headers** — permissive CORS layer on HTTP API for browser-based consumers
- **Alert deduplication** — 60-second cooldown per component prevents duplicate alerts from sustained threshold breaches
- **Dynamic poll interval** — `poll_interval_secs` changes via MCP take effect immediately (no restart needed)
- **`show_anomalies` config flag** — when false, suppresses alert generation in the collector
- **`AgentSummary` derives `Default`** — cleaner construction, future-proofed for daimon integration
- **Octal escape decoding** — `/proc/mounts` paths with `\040` (space), `\011` (tab) etc. are decoded correctly
- **Agent data from daimon** — `ServiceChecker::fetch_agents()` queries daimon `/v1/agents` for real agent counts, CPU, and memory usage. Falls back to defaults when unreachable
- **Config persistence** — `NazarConfig::load()`/`save()` to `~/.config/nazar/config.json`. Loaded on startup (CLI `--api-url` overrides). MCP config `set` auto-persists changes
- **89 tests** across 6 crates (up from 27)
  - Config validation: zero poll interval, low refresh rate, out-of-range thresholds, NaN, unknown keys, boolean validation
  - Service checker: host validation, known services, async probing, agent fetch fallback
  - Network delta computation, TimeSeries zero-max-points edge case
  - Case-insensitive severity filter, octal escape decoding
  - Config save/load round-trip, missing file fallback, invalid JSON fallback

### Changed

- **HTTP API binds to 127.0.0.1** by default (was 0.0.0.0). Use `--bind 0.0.0.0` for external access
- **HTTP API returns proper status codes** — `GET /v1/snapshot` returns 503 when no data, 500 on serialization failure
- **TimeSeries uses VecDeque** — O(1) push/pop instead of O(n) `Vec::remove(0)` at capacity
- **AnomalyDetector history uses VecDeque** — same O(1) improvement
- **Network metrics are delta-based** — `ProcReader` tracks previous counters, reports bytes-since-last-read. History stores B/s rate. UI shows KB/s
- **Interface up/down detection** reads `/sys/class/net/<name>/operstate` instead of checking byte counts
- **Single /proc/stat read per tick** — merged `parse_proc_stat()` and `parse_proc_stat_counts()` into one read
- **CPU `processes` field** now shows running process count (was showing total forks since boot)
- **UI clones data before rendering** — RwLock read guard dropped before draw calls to prevent writer starvation
- **Disk history pruned** — entries for unmounted filesystems are removed each tick
- **Prediction math corrected** — uses `(target - current_value) / slope` for remaining intervals (was using regression intercept)
- **MCP config validation** — `poll_interval_secs >= 1`, `ui_refresh_ms >= 100`, thresholds finite and 0–100, booleans must be "true"/"false"
- **MCP alerts filter** — case-insensitive severity matching
- **MCP history schema** — corrected metric names to `cpu, memory, network_rx, network_tx, disk:<mount>`
- **MCP dashboard network fields** — `rx_bytes_delta`/`tx_bytes_delta` (was misleading `rx_mb`/`tx_mb`)
- **RwLock poison logging** escalated from `warn!` to `error!` with clearer diagnostic message
- **Spawned task monitoring** — headless mode uses `tokio::select!` to detect collector/API panics and log errors
- **`poll_interval_secs`** — MCP set takes effect immediately (collector re-creates interval on config change)

### Removed

- **Dead `ApiClient`** — unused HTTP client for daimon removed from `nazar-api`
- **Dead `AnomalyAlert`** struct removed from `nazar-core`
- **`nazar-mcp` removed from binary deps** — crate is compiled in workspace but not wired to a transport yet (planned for v2)
- **Unused dependencies** — `uuid`, `anyhow` removed from workspace; trimmed ~12 unused deps across crates

### Refactored

- **`src/main.rs` split** into `src/http.rs` (API router + handlers) and `src/collector.rs` (metrics loop)
- **UI `update()` decomposed** into 8 panel methods: `draw_header`, `draw_alerts_panel`, `draw_cpu_panel`, `draw_memory_panel`, `draw_disk_panel`, `draw_network_panel`, `draw_services_panel`, `draw_predictions_panel`
- **Graceful shutdown** — GUI mode calls `rt.shutdown_timeout(2s)` on window close; API bind failure logs error instead of panicking
- **`unsafe` statvfs** — added `// SAFETY:` documentation

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
