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

# Build headless (skip UI)
cargo build --no-default-features
```

## Testing

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p nazar-core
cargo test -p nazar-ai
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

## Releasing

1. Update `VERSION` file
2. Update version in all `Cargo.toml` files
3. Update `CHANGELOG.md`
4. Tag: `git tag $(cat VERSION)`
5. Push tag — CI builds and publishes release
6. AGNOS `ark-bundle.sh` will automatically pick up the new release
