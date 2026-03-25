# AGENTS.md — packet_counter (workspace root)

## Project Overview

Linux eBPF/XDP packet counter built with Rust (Aya framework) and a React/TypeScript dashboard.
An XDP program counts TCP/UDP packets per destination port in a `PerCpuHashMap`.
The userspace binary (Axum) reads the BPF map and serves stats via a REST API.
The React SPA polls `/api/stats` and renders a bar chart + sortable table.

## Workspace Layout

```
packet-counter/          Userspace binary: Axum HTTP + eBPF loader      → AGENTS.md
packet-counter-common/   Shared #[repr(C)] types (kernel ↔ userspace)   → AGENTS.md
packet-counter-ebpf/     eBPF/XDP kernel program (#![no_std])           → AGENTS.md
web/                     React 18 + Vite 5 + TypeScript 5.5 frontend    → AGENTS.md
```

Each sub-directory has its own `AGENTS.md` with crate/project-specific context.

## Build / Check / Lint / Format

All Rust commands run **inside the Lima VM** (Ubuntu ARM64 with full BPF toolchain).
The frontend builds on the macOS host or inside the VM.

| Task | Command |
|---|---|
| Build userspace binary | `make build` |
| Type-check only (fast) | `make check` |
| Format (rustfmt) | `make fmt` |
| Lint (clippy, `-D warnings`) | `make lint` |
| Build frontend | `make build-web` |
| Build everything | `make build-all` |
| Run (debug, sudo) | `make run` (override: `IFACE=ens3 SKB=1 PORT=8080`) |
| Run (release) | `make run-release` |
| Dev mode (both) | `make dev` |
| Clean | `make clean` |

### VM Management (macOS host)

```sh
make vm-shell       # Create/start Lima VM + open shell
make vm-run         # Create/start VM, build, and run
make vm-down        # Stop VM (preserves state)
make vm-destroy     # Stop + permanently delete VM
```

### Tests

No tests exist yet. When adding them:
- Rust: `#[cfg(test)]` modules, run with `cargo test --target <TARGET> -p <crate>`
- Frontend: add vitest, run with `npx vitest run` (all) or `npx vitest run path/to/file` (one)

## Rust Toolchain

- **Nightly** (pinned in `rust-toolchain.toml`)
- Targets: `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`
- BPF linker: `bpf-linker` for `bpfel-unknown-none` target
- eBPF crate is **not** a default workspace member — cross-compiled by `aya-build` inside `packet-counter/build.rs`

## Shared Rust Code Style

### Formatting (`rustfmt.toml`)

- Edition 2021, max width **100** chars, 4-space indent
- `use_field_init_shorthand = true` — `StatEntry { port, count, .. }` not `StatEntry { port: port, .. }`
- `use_try_shorthand = true` — prefer `?` operator

### Imports

Three tiers separated by blank lines, alphabetized within each group:
1. `std::` / `core::`
2. External crates
3. Workspace crates (`packet_counter_common`)

Merge sub-items into nested `use`. Use `as _` for trait-only imports:
```rust
use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context as _;
use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use log::{info, warn};

use packet_counter_common::{PortKey, PROTO_TCP, PROTO_UDP};
```

### Naming

| Kind | Convention | Examples |
|---|---|---|
| Types / Structs | `PascalCase` | `PortKey`, `StatEntry`, `SharedStats` |
| Functions / variables | `snake_case` | `read_stats`, `shared_stats` |
| Constants | `SCREAMING_SNAKE_CASE` | `PROTO_TCP`, `PACKET_COUNTS` |
| Crate names (Cargo.toml) | `kebab-case` | `packet-counter-ebpf` |

### Comments

- `//!` module-level doc comments at the top of each file
- `///` doc comments on every public struct, field, function, and constant
- Section separators between logical blocks:
  ```rust
  // ---------------------------------------------------------------------------
  // Section Name
  // ---------------------------------------------------------------------------
  ```
- Inline comments explain **why**, not **what**
- `// Safety:` comments before every `unsafe` block

### Logging

`log` crate facade with `env_logger`. Keep output minimal:
- `info!()` — operational milestones (startup, attach, listen, shutdown)
- `warn!()` — recoverable failures
- Avoid `debug!`/`trace!` unless actively debugging

## Commit Style

Conventional Commits: `feat:`, `fix:`, `chore:`, `refactor:`, etc.
Optional scope in parentheses: `feat(api):`, `chore(dev):`.
