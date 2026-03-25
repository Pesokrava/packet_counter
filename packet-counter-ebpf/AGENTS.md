# AGENTS.md — packet-counter-ebpf (XDP kernel program)

## Purpose

eBPF/XDP program that runs in the Linux kernel. Attached to a network interface,
it parses every incoming packet's Ethernet → IP → TCP/UDP headers and increments
a per-CPU counter keyed by `(protocol, destination_port)`. It never drops packets —
all packets are passed through with `XDP_PASS`.

## Critical Constraints

This crate is **`#![no_std]` and `#![no_main]`**. It targets `bpfel-unknown-none`.

- **No heap allocation** — no `Vec`, `String`, `Box`, `HashMap`, etc.
- **No standard library** — only `core::` is available.
- **No panics in production** — the `#[panic_handler]` is an infinite loop. Any panic
  halts the program. Use `Result<u32, ()>` and propagate errors with `?`.
- **BPF verifier must accept the program** — all memory accesses must be bounds-checked.
  The verifier runs at load time and rejects programs with unbounded reads.
- **No floating point** — BPF programs cannot use floats.
- **Stack limit: 512 bytes** — keep local variables minimal.

## Build

This crate is **not** built directly. It is cross-compiled by `aya-build` inside
`packet-counter/build.rs`. It is excluded from the workspace `default-members`.

```sh
# These commands build the eBPF crate indirectly:
make build     # builds packet-counter, which triggers build.rs → aya-build
make check     # cargo check on the userspace crate (does not check eBPF directly)
```

To check eBPF code in isolation (rarely needed):
```sh
cargo build -p packet-counter-ebpf --target bpfel-unknown-none -Z build-std=core
```

## Key Dependencies

- **aya-ebpf** — BPF map types, XDP context, entry point macros
- **aya-log-ebpf** — BPF-side logging (available but not currently used)
- **network-types 0.1.0** — `EthHdr`, `Ipv4Hdr`, `Ipv6Hdr`, `TcpHdr`, `UdpHdr`
- **packet-counter-common** — `PortKey`, `PROTO_TCP`, `PROTO_UDP` (without `userspace` feature)

## File Structure

```
src/main.rs   — single file (~166 lines), the entire XDP program
build.rs      — minimal, no custom logic
```

`main.rs` layout:
1. `#![no_std]`, `#![no_main]`
2. Imports (`core::mem`, aya-ebpf, network-types, common)
3. BPF map definition (`PACKET_COUNTS: PerCpuHashMap<PortKey, u64>`)
4. `ptr_at<T>()` — bounds-checked pointer helper
5. `increment()` — per-CPU counter upsert
6. `packet_counter()` — XDP entry point (public, `#[xdp]`)
7. `try_count()` — internal packet parser
8. `#[panic_handler]`, `LICENSE` boilerplate

## BPF Map

```rust
#[map]
static PACKET_COUNTS: PerCpuHashMap<PortKey, u64> =
    PerCpuHashMap::with_max_entries(65535, 0);
```

- `PerCpuHashMap`: each CPU has an independent slot per key — no locking needed.
- `max_entries: 65535` covers the full port range for TCP + UDP.
- Userspace reads this map via `aya::maps::PerCpuHashMap` and sums across CPUs.

## Error Handling — NEVER DROP PACKETS

The entry point converts any parse error to `XDP_PASS`:
```rust
#[xdp]
pub fn packet_counter(ctx: XdpContext) -> u32 {
    match try_count(&ctx) {
        Ok(action) => action,
        Err(_) => xdp_action::XDP_PASS,  // parse failure → pass through
    }
}
```

Inside `try_count()`, use `Result<u32, ()>` with `.ok_or(())?` for bounds checks.
Map insertion errors are intentionally ignored with `let _ = ...`:
```rust
// Ignore insertion errors (map full) — we never drop packets.
let _ = PACKET_COUNTS.insert(key, &1u64, 0);
```

## Memory Safety Patterns

### Bounds-checked pointer access (required by BPF verifier)

Every packet header read goes through `ptr_at<T>()`:
```rust
#[inline(always)]
unsafe fn ptr_at<T>(ctx: &XdpContext, offset: usize) -> Option<*const T> {
    let start = ctx.data() as usize;
    let end = ctx.data_end() as usize;
    let ptr = start.checked_add(offset)?;
    let region_end = ptr.checked_add(mem::size_of::<T>())?;
    if region_end > end { return None; }
    Some(ptr as *const T)
}
```

- Uses `checked_add` to prevent wrapping.
- Returns `Option` — caller converts to `Result` with `.ok_or(())?`.
- Always `#[inline(always)]` — BPF programs have no function call overhead budget.

### Counter increment (lock-free)

```rust
#[inline(always)]
fn increment(key: &PortKey) {
    // Fast path: entry exists — increment in place
    if let Some(count) = PACKET_COUNTS.get_ptr_mut(key) {
        unsafe { *count += 1 };
        return;
    }
    // Slow path: first packet — insert 1
    let _ = PACKET_COUNTS.insert(key, &1u64, 0);
}
```

## Packet Parsing Flow

```
Ethernet → ether_type match:
  ├─ IPv4 → read IHL for variable header length → proto match:
  │   ├─ TCP → read TcpHdr.dest → increment(PROTO_TCP, dst_port)
  │   ├─ UDP → read UdpHdr.dst_port → increment(PROTO_UDP, dst_port)
  │   └─ other → skip
  ├─ IPv6 → fixed 40-byte header → next_hdr match:
  │   ├─ TCP / UDP → same as IPv4
  │   └─ other → skip (extension headers not chased)
  └─ other (ARP, VLAN, etc.) → skip
→ always return XDP_PASS
```

## Required Boilerplate

Every eBPF binary must include:
```rust
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }

#[link_section = "license"]
#[used]
static LICENSE: [u8; 4] = *b"GPL\0";
```

The GPL license is required for BPF programs that use GPL-only kernel helpers.
