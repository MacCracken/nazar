# Nazar — AI-Native System Monitor

> Arabic/Persian: نظر (watchful eye)

[![License](https://img.shields.io/badge/license-GPLv3-blue)](LICENSE)
[![Status](https://img.shields.io/badge/status-development-yellow)]()

**Nazar** is an AI-powered system monitor for [AGNOS](https://github.com/MacCracken/agnosticos). It provides real-time visualization of system metrics, AI-driven anomaly detection, and resource exhaustion prediction.

## Features

- **Real-time dashboard** — CPU, memory, disk, network monitoring with live charts
- **Agent monitoring** — Per-agent resource usage from daimon API
- **Anomaly detection** — Threshold-based alerts with configurable sensitivity
- **Resource prediction** — Linear trend analysis for memory/disk exhaustion forecasting
- **Service status** — Live status of AGNOS services (daimon, hoosh, phylax)
- **MCP tools** — 5 native tools for agent-driven monitoring queries
- **Headless mode** — Run without GUI for server/edge deployments

## Architecture

```
nazar
├── nazar-core    — Types, metrics, time series, config
├── nazar-api     — Daimon API client + /proc reader
├── nazar-ui      — egui/eframe GUI dashboard
├── nazar-ai      — Anomaly detection, prediction, recommendations
└── nazar-mcp     — MCP tool definitions for AGNOS integration
```

## Usage

```bash
# Start with GUI
nazar

# Headless mode (metrics collection only)
nazar --headless

# Custom daimon endpoint
nazar --api-url http://192.168.1.100:8090

# Custom port for nazar's own API
nazar --port 8095
```

## AGNOS Integration

Nazar integrates with AGNOS through:

- **daimon API** (port 8090) — agent metrics, anomaly alerts, health checks
- **hoosh API** (port 8088) — LLM-assisted alert triage
- **phylax** — threat scanner status
- **MCP tools** — `nazar_dashboard`, `nazar_alerts`, `nazar_predict`, `nazar_history`, `nazar_config`
- **agnoshi intents** — "show system status", "predict memory usage", "list alerts"

## License

GPL-3.0
