# Changelog

All notable changes to Nazar will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [2026.3.16] - 2026-03-16

### Added — Initial Release

- **nazar-core**: System metrics types (CPU, memory, disk, network, agents), time series buffer, config
- **nazar-api**: Daimon API client (health, metrics, agents, anomaly, scan, edge), /proc readers (meminfo, loadavg)
- **nazar-ui**: egui/eframe GUI dashboard with CPU/memory progress bars, service status panel
- **nazar-ai**: Threshold-based anomaly detection, linear regression resource prediction
- **nazar-mcp**: 5 MCP tool definitions (dashboard, alerts, predict, history, config)
- **CLI**: `--headless`, `--api-url`, `--port` flags
- **CI/CD**: GitHub Actions for check, test, clippy, fmt, release (amd64 + arm64)
- **27 tests** across all crates, 0 warnings
