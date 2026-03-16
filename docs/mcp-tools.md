# Nazar MCP Tools Reference

Nazar exposes 5 MCP tools with full backend handlers. These can be registered
with daimon's tool registry and called by any AGNOS agent.

All tools read from (or write to) the shared `MonitorState` — the same state
that powers the GUI and HTTP API.

## Tools

### nazar_dashboard

Get a system monitoring dashboard snapshot.

**Parameters**: None

**Returns**:
```json
{
  "timestamp": "2026-03-16T12:00:00Z",
  "cpu": { "total_percent": 23.5, "load_average": [1.2, 0.9, 0.8], "cores": 8 },
  "memory": { "used_percent": 62.3, "used_gb": 9.97, "total_gb": 16.0 },
  "disk": [{ "mount": "/", "used_percent": 45.2, "used_gb": 226.0, "total_gb": 500.0 }],
  "network": { "rx_mb": 1024.5, "tx_mb": 512.3, "connections": 42 },
  "agents": { "total": 3, "running": 2, "error": 0 },
  "alerts_count": 1
}
```

### nazar_alerts

Get current anomaly alerts.

**Parameters**:
- `severity` (optional): Filter by `info`, `warning`, `critical`

**Returns**:
```json
{
  "count": 1,
  "alerts": [{
    "severity": "WARNING",
    "component": "memory",
    "message": "Memory usage at 87.3% (threshold: 85.0%)",
    "timestamp": "2026-03-16T12:00:00Z"
  }]
}
```

### nazar_predict

Predict resource exhaustion based on trends.

**Parameters**:
- `metric` (optional): `memory`, `disk`, `cpu`

**Returns**:
```json
{
  "predictions": [{
    "metric": "memory",
    "current_percent": 62.3,
    "target_percent": 95.0,
    "minutes_until": 180,
    "trend": "Rising"
  }]
}
```

Requires 10+ samples before predictions are available.

### nazar_history

Get historical time series data for a metric.

**Parameters**:
- `metric` (required): `cpu`, `memory`, `network_rx`, `network_tx`, or `disk:<mount>` (e.g. `disk:/`)
- `points` (optional): Number of data points to return (default: 60)

**Returns**:
```json
{
  "metric": "cpu",
  "unit": "%",
  "count": 60,
  "points": [
    { "timestamp": "2026-03-16T11:55:00Z", "value": 23.5 },
    { "timestamp": "2026-03-16T11:55:05Z", "value": 24.1 }
  ]
}
```

### nazar_config

Get or update Nazar monitor configuration at runtime.

**Parameters**:
- `action` (required): `get` or `set`
- `key` (optional, for set): `poll_interval_secs`, `show_anomalies`, `show_agents`, `ui_refresh_ms`
- `value` (optional, for set): New value as string

**Get returns**:
```json
{
  "api_url": "http://127.0.0.1:8090",
  "poll_interval_secs": 5,
  "max_history_points": 720,
  "show_anomalies": true,
  "show_agents": true,
  "ui_refresh_ms": 1000
}
```

**Set returns**:
```json
{ "updated": "poll_interval_secs", "value": "10" }
```
