# Nazar — AI-Native System Monitor

> Arabic/Persian: نظر (watchful eye)

[![License](https://img.shields.io/badge/license-GPLv3-blue)](LICENSE)
[![Status](https://img.shields.io/badge/status-development-yellow)]()

**Nazar** is an AI-powered system monitor for [AGNOS](https://github.com/MacCracken/agnosticos). It provides real-time visualization of system metrics, AI-driven anomaly detection, and resource exhaustion prediction.

## Features

- **Real-time dashboard** — CPU (per-core), memory, disk, network with live sparkline charts
- **Anomaly detection** — threshold-based alerts for CPU, memory, disk usage
- **Resource prediction** — linear regression for memory exhaustion forecasting
- **HTTP API** — REST endpoints on port 8095 for external access
- **MCP tools** — 5 tools with full handlers for agent-driven monitoring queries
- **Agent monitoring** — per-agent resource usage from daimon API
- **Headless mode** — collector + API without GUI for server/edge deployments

## Architecture

```
nazar
├── nazar-core    — Types, metrics, time series, config, shared state
├── nazar-api     — Daimon API client + ProcReader (/proc metrics)
├── nazar-ui      — egui/eframe GUI dashboard with egui_plot charts
├── nazar-ai      — Anomaly detection, prediction, recommendations
└── nazar-mcp     — MCP tool definitions + handlers
```

## Usage

```bash
# Start with GUI
nazar

# Headless mode (collector + API only)
nazar --headless

# Custom daimon endpoint
nazar --api-url http://192.168.1.100:8090

# Custom port for nazar's own API
nazar --port 8095
```

## HTTP API

```bash
curl http://localhost:8095/health
curl http://localhost:8095/v1/snapshot
curl http://localhost:8095/v1/alerts
curl http://localhost:8095/v1/predict
```

## AGNOS Integration

Nazar integrates with AGNOS through:

- **daimon API** (port 8090) — agent metrics, anomaly alerts, health checks
- **hoosh API** (port 8088) — LLM-assisted alert triage (v3)
- **phylax** — threat scanner status
- **MCP tools** — `nazar_dashboard`, `nazar_alerts`, `nazar_predict`, `nazar_history`, `nazar_config`

## License

GPL-3.0
