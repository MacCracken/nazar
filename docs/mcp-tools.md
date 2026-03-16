# Nazar MCP Tools Reference

Nazar exposes 5 MCP tools that can be registered with daimon's tool registry.

## Tools

### nazar_dashboard
Get a system monitoring dashboard snapshot.

**Parameters**: None

**Returns**: CPU, memory, disk, network metrics + agent summary

### nazar_alerts
Get current anomaly alerts and warnings.

**Parameters**:
- `severity` (optional): Filter by `info`, `warning`, `critical`

### nazar_predict
Predict resource exhaustion based on trends.

**Parameters**:
- `metric` (optional): `memory`, `disk`, `cpu`

### nazar_history
Get historical metrics for charting.

**Parameters**:
- `metric` (required): `cpu`, `memory`, `disk`, `network`
- `points` (optional): Number of data points (default: 60)

### nazar_config
Get or update monitor configuration.

**Parameters**:
- `action` (required): `get` or `set`
- `key` (optional): Config key
- `value` (optional): Config value (for set)
