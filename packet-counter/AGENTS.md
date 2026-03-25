# AGENTS.md — packet-counter (userspace binary)

## Purpose

Axum HTTP server + eBPF loader. Loads the compiled XDP program, attaches it to a
network interface, runs a background task that periodically reads `PACKET_COUNTS`
from the BPF map, caches aggregated stats behind `Arc<RwLock<>>`, and serves them
via a REST API. Optionally serves a pre-built React SPA from `--static-dir`.

## API Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/api/health` | Liveness probe — `{ "status": "ok" }` |
| GET | `/api/stats` | JSON array of `{ protocol, port, count }` sorted by count desc |

## Key Dependencies

- **aya** — eBPF loader (`Ebpf`, `PerCpuHashMap`, `Xdp`, `XdpFlags`)
- **axum 0.7** — HTTP framework (Router, State extractor, Json)
- **tokio** — async runtime (`#[tokio::main]`, spawn, RwLock, signal)
- **tower-http 0.5** — CORS layer, `ServeDir`/`ServeFile` for SPA serving
- **clap 4** (derive) — CLI parsing (`Opt` struct)
- **anyhow** — error handling with `.context()` / `.with_context()`
- **serde + serde_json** — JSON serialization
- **log + env_logger** — logging

## Build & Run

```sh
make build              # cargo build --target aarch64-unknown-linux-gnu -p packet-counter
make check              # cargo check (fast, no codegen)
make run                # build + sudo run (override: IFACE=ens3 SKB=1 PORT=8080)
make run-release        # release build + sudo run
```

The `build.rs` calls `aya_build::build_ebpf()` to cross-compile `packet-counter-ebpf`
to `bpfel-unknown-none` and embed the BPF ELF in the binary via `include_bytes_aligned!`.
Changes to the eBPF crate are automatically picked up by cargo's build script.

## File Structure

```
src/main.rs   — single-file binary (~300 lines)
build.rs      — invokes aya-build to compile the eBPF crate
```

`main.rs` is organized top-down with section separators:
1. Module docs (`//!`)
2. Imports (three-tier grouping)
3. CLI definition (`struct Opt` with `#[derive(Parser)]`)
4. Shared state types (`StatEntry`, `type SharedStats`)
5. `main()` — orchestrator
6. `shutdown_signal()` — graceful shutdown
7. `read_stats()` — BPF map reader (synchronous, called from async task)
8. HTTP handlers (`health_handler`, `stats_handler`)

## Error Handling

- `main()` returns `anyhow::Result<()>`. Every fallible operation has `.context()` or
  `.with_context()` with a descriptive message including the failing value.
- Non-fatal BPF map errors: `warn!()` + return `Vec::new()`. Never panic on transient
  map read failures — the background task keeps running.
- Pattern: match on `Option`/`Result`, log the error, return a safe default.

```rust
let map = match bpf.map("PACKET_COUNTS") {
    Some(m) => m,
    None => {
        warn!("PACKET_COUNTS map not found");
        return Vec::new();
    }
};
```

## Async Patterns

- `#[tokio::main]` multi-threaded runtime
- Background polling: `tokio::spawn(async move { loop { ticker.tick().await; ... } })`
- Shared state: `Arc<RwLock<Vec<StatEntry>>>` — tokio's async `RwLock`
- Reads use `.read().await`, writes use `.write().await`
- Graceful shutdown: `signal::ctrl_c().await` passed to `axum::serve().with_graceful_shutdown()`
- `read_stats()` is synchronous (short BPF map iteration) — called inline from async task

## Types

- `StatEntry`: `#[derive(Debug, Clone, Serialize)]` — the API response shape
- `SharedStats`: type alias for `Arc<RwLock<Vec<StatEntry>>>`
- `Opt`: `#[derive(Debug, Parser)]` — CLI arguments with `/// ` doc comments on each field
  (clap uses doc comments as `--help` text)

## Static File Serving

When `--static-dir` is provided, `tower_http::services::ServeDir` is mounted at `/`
with `ServeFile` fallback to `index.html` for SPA routing. API routes take precedence
via `api_router.fallback_service(serve_dir)`.
