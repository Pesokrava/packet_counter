# AGENTS.md — packet-counter-common (shared types)

## Purpose

Types and constants shared between the eBPF kernel program and the userspace binary.
This crate compiles for **both** targets:
- `bpfel-unknown-none` (no_std, no heap) — used by `packet-counter-ebpf`
- `aarch64-unknown-linux-gnu` (std) — used by `packet-counter` with `features = ["userspace"]`

## Critical Constraint: Dual-Target Compatibility

```rust
#![cfg_attr(not(feature = "userspace"), no_std)]
```

Everything in this crate must be `no_std`-compatible by default. The `userspace` feature
gates std-dependent code (like `aya::Pod` impls). When adding new types:
- Use only `core::` types (no `String`, `Vec`, `Box`, `HashMap`)
- All structs must be `#[repr(C)]` with fixed-width primitives
- Add explicit padding fields (`_pad`) — no hidden compiler padding allowed
- The BPF verifier compares map keys byte-for-byte; uninitialised padding causes misses

## Feature Gating

| Consumer | Feature | Meaning |
|---|---|---|
| `packet-counter-ebpf` | *(none)* | `no_std` — only `core::` available |
| `packet-counter` | `userspace` | enables `aya::Pod` impl, pulls in `aya` dep |

```toml
# In packet-counter-ebpf/Cargo.toml (no feature):
packet-counter-common = { path = "../packet-counter-common" }

# In packet-counter/Cargo.toml (with feature):
packet-counter-common = { path = "../packet-counter-common", features = ["userspace"] }
```

## File Structure

```
src/lib.rs   — single file (~64 lines), all types and constants
```

## Types

### `PortKey` — BPF map key

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortKey {
    pub protocol: u8,   // offset 0: IP protocol (6=TCP, 17=UDP)
    pub _pad: u8,       // offset 1: explicit zero padding
    pub port: u16,      // offset 2: destination port (host byte order)
}
```

- Total size: 4 bytes, no hidden padding
- `_pad` keeps `port` 2-byte aligned and is always zero-initialised via `PortKey::new()`
- `Clone + Copy` — required for BPF map operations
- `PortKey::new(protocol, port)` — convenience constructor, zero-initialises padding

### Pod impl (userspace only)

```rust
#[cfg(feature = "userspace")]
// Safety: #[repr(C)], only primitive fields, no hidden padding, Copy + 'static
unsafe impl aya::Pod for PortKey {}
```

This impl is required for `aya::maps::PerCpuHashMap<PortKey, u64>` to compile on
the userspace side. It must NOT be compiled for the eBPF target.

## Constants

```rust
pub const PROTO_TCP: u8 = 6;    // IP protocol number for TCP
pub const PROTO_UDP: u8 = 17;   // IP protocol number for UDP
```

## Adding New Shared Types

1. Define with `#[repr(C)]` and only fixed-width primitives (`u8`, `u16`, `u32`, `u64`)
2. Add explicit `_pad` fields to eliminate compiler-inserted padding
3. Derive `Clone, Copy, Debug, PartialEq, Eq, Hash`
4. Provide a `::new()` constructor that zero-initialises all padding
5. Gate the `aya::Pod` impl behind `#[cfg(feature = "userspace")]`
6. Add a `// Safety:` comment explaining why the `Pod` impl is sound
7. Document the memory layout (offsets, total size) in a doc comment
