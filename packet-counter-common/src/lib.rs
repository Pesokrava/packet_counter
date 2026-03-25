#![cfg_attr(not(feature = "userspace"), no_std)]

//! Types shared between the eBPF kernel program and the userspace loader.
//!
//! All structs are `#[repr(C)]` with fixed-width primitives and explicit
//! padding so the memory layout is identical on both sides of the BPF map
//! boundary.  The compiler is not allowed to insert hidden padding bytes.
//!
//! # PortKey alignment rules
//!
//! BPF map keys must satisfy two constraints:
//!   1. The total size is fixed and known at map-definition time.
//!   2. There must be no uninitialised padding bytes — the kernel compares
//!      keys byte-for-byte, so hidden padding would cause spurious misses.
//!
//! `PortKey` is laid out as: protocol (1 B) + _pad (1 B) + port (2 B) = 4 B
//! with _pad always zero, giving natural 2-byte alignment for `port` and a
//! total size that is a multiple of 4 (common BPF alignment requirement).

/// IP protocol numbers stored in `PortKey::protocol`.
pub const PROTO_TCP: u8 = 6;
pub const PROTO_UDP: u8 = 17;

/// BPF map key: identifies a unique (protocol, destination-port) pair.
///
/// Used as the key of `PACKET_COUNTS: PerCpuHashMap<PortKey, u64>`.
///
/// Layout (4 bytes, no hidden padding):
///   offset 0: protocol  — IP protocol number (6 = TCP, 17 = UDP)
///   offset 1: _pad      — always zero; keeps `port` 2-byte aligned
///   offset 2: port      — destination port in host byte order
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortKey {
    /// IP protocol number: `PROTO_TCP` (6) or `PROTO_UDP` (17).
    pub protocol: u8,
    /// Explicit padding — must be zero-initialised; never read.
    pub _pad: u8,
    /// Destination port in **host byte order**.
    /// The XDP program converts from network byte order before storing.
    pub port: u16,
}

impl PortKey {
    /// Convenience constructor that zero-initialises the padding byte.
    #[inline]
    pub const fn new(protocol: u8, port: u16) -> Self {
        Self {
            protocol,
            _pad: 0,
            port,
        }
    }
}

// ---------------------------------------------------------------------------
// Pod impl — required so aya's PerCpuHashMap<PortKey, u64> compiles on the
// userspace side.  Only compiled when the "userspace" feature is enabled
// (the eBPF crate never enables it).
// ---------------------------------------------------------------------------
#[cfg(feature = "userspace")]
// Safety: PortKey is #[repr(C)], contains only primitive fields, has no
// padding bytes beyond the explicit `_pad` field, and is Copy + 'static.
unsafe impl aya::Pod for PortKey {}
