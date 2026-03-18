# Nazar Development Roadmap

> **Version**: 2026.3.17 | **Status**: v1–v4 complete

---

## Shipped

All planned milestones delivered. See [CHANGELOG.md](../CHANGELOG.md) for full details.

**v1 — MVP System Monitor** (23 items)
/proc readers, metrics pipeline, HTTP API, MCP tools, egui dashboard, anomaly detection, predictions

**v2 — Enhanced Monitoring** (9 items)
Per-process CPU/memory, disk I/O, network sparklines, GPU monitoring, temperature sensors, agent detail view, MCP stdio transport, daimon tool registration, agnoshi discoverability

**v3 — AI Features** (4 items)
LLM-assisted alert triage, process recommendations, multi-metric capacity planning, correlation detection

**v4 — Polish** (3 items)
Alert notifications via daimon event bus, SQLite persistence, Prometheus export

**Stats**: 6 crates, 89 tests, 0 clippy warnings

---

## Future Ideas

| Item | Notes |
|------|-------|
| Configurable dashboard layouts | Drag-and-drop panel arrangement in egui |
| GPU history time series | Sparklines for GPU utilization and VRAM over time |
| Per-process history tracking | Track CPU/memory trends for individual processes |
| Disk I/O predictions | Extend capacity planning to predict I/O bottlenecks |
| Authentication for HTTP API | Bearer token support for production deployments |
| WebSocket live updates | Real-time metric streaming to browser clients |
