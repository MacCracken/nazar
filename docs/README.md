# Nazar Documentation

## Architecture

- [architecture.md](architecture.md) — System design, data flow, crate responsibilities
- [mcp-tools.md](mcp-tools.md) — MCP tool reference with example responses

## Development

- [development.md](development.md) — Building, testing, releasing
- [roadmap.md](roadmap.md) — Development phases and milestones

## Architecture Decision Records

- [ADR-001](adr/001-proc-readers-over-sysinfo.md) — Read /proc directly instead of sysinfo crate
- [ADR-002](adr/002-shared-state-rwlock.md) — Shared state via Arc<RwLock<MonitorState>>
- [ADR-003](adr/003-axum-http-api.md) — Nazar HTTP API via axum
