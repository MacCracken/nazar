# ADR-001: Read /proc directly instead of using sysinfo crate

**Status**: Accepted
**Date**: 2026-03-16

## Context

Nazar needs to read CPU, memory, disk, and network metrics on Linux. The two
main options are:

1. Use the `sysinfo` crate — a popular cross-platform abstraction
2. Read `/proc` files directly and call `statvfs` via `libc`

## Decision

Read `/proc` directly.

## Rationale

- **Minimal dependencies** — `sysinfo` pulls in ~15 transitive crates. Direct
  `/proc` parsing uses only `libc` (already a transitive dep of tokio).
- **Full control** — we decide exactly what to parse, how to compute deltas,
  and what to expose. No fighting the crate's abstractions.
- **AGNOS convention** — other AGNOS components (phylax, daimon) read `/proc`
  directly for the same reasons.
- **Linux-only** — Nazar targets Linux exclusively. Cross-platform portability
  from `sysinfo` adds complexity without value.

## Trade-offs

- We maintain our own parsing code for `/proc/stat`, `/proc/meminfo`,
  `/proc/mounts`, `/proc/net/dev`, `/proc/net/tcp`.
- CPU usage requires delta-based calculation between two reads (handled by
  `ProcReader` storing previous CPU times).

## Implementation

- `ProcReader` struct in `nazar-api/src/proc_reader.rs` holds state for delta
  calculations.
- Methods: `read_cpu()`, `read_memory()`, `read_disk()`, `read_network()`,
  `snapshot()`.
- Disk space uses `libc::statvfs`; all other metrics are pure file reads.
