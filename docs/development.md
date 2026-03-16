# Development Guide

## Prerequisites

- Rust 1.85+ (2024 edition)
- Linux (for /proc access and egui/wayland support)

## Building

```bash
# Check everything compiles
cargo check --workspace

# Build debug
cargo build

# Build release
cargo build --release
```

## Running

```bash
# Start with GUI (collector + API + dashboard)
cargo run

# Headless mode (collector + API only)
cargo run -- --headless

# Custom daimon endpoint
cargo run -- --api-url http://192.168.1.100:8090

# Custom nazar API port
cargo run -- --port 9095
```

Once running, the HTTP API is available at `http://localhost:8095`:

```bash
curl http://localhost:8095/health
curl http://localhost:8095/v1/snapshot
curl http://localhost:8095/v1/alerts
curl http://localhost:8095/v1/predict
```

## Testing

```bash
# Run all tests (44 tests across 5 crates)
cargo test --workspace

# Run specific crate tests
cargo test -p nazar-core
cargo test -p nazar-api
cargo test -p nazar-ai
cargo test -p nazar-mcp
cargo test -p nazar-ui
```

## Code Quality

```bash
# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Check format
cargo fmt --all -- --check
```

## Project Structure

```
nazar/
├── src/main.rs              — CLI, collector loop, HTTP API
├── crates/
│   ├── nazar-core/          — Types, metrics, config, shared state
│   ├── nazar-api/           — Daimon client + ProcReader
│   │   └── src/
│   │       ├── lib.rs       — ApiClient (HTTP)
│   │       └── proc_reader.rs — /proc filesystem readers
│   ├── nazar-ui/            — egui dashboard + charts
│   ├── nazar-ai/            — Anomaly detection + prediction
│   └── nazar-mcp/           — MCP tool definitions + handlers
├── docs/
│   ├── architecture.md      — System design
│   ├── mcp-tools.md         — Tool reference
│   ├── development.md       — This file
│   ├── roadmap.md           — Milestones
│   └── adr/                 — Architecture decision records
├── CHANGELOG.md
└── VERSION
```

## Releasing

1. Update `VERSION` file
2. Update version in all `Cargo.toml` files
3. Update `CHANGELOG.md`
4. Tag: `git tag $(cat VERSION)`
5. Push tag — CI builds and publishes release
6. AGNOS `ark-bundle.sh` will automatically pick up the new release
