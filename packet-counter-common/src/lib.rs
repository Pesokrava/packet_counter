#![no_std]

//! Types shared between the eBPF kernel program and the userspace loader.
//!
//! All structs use `#[repr(C)]` with fixed-width primitives so that the memory
//! layout is identical in both contexts.  Field ordering is chosen to achieve
//! natural alignment without hidden padding bytes.
//!
//! Stage 1: stub with placeholder types only.
//! Stage 2: `PortKey` and packet-count map key/value types will be added here.

/// IP protocol numbers used as the `protocol` field of `PortKey`.
pub const PROTO_TCP: u8 = 6;
pub const PROTO_UDP: u8 = 17;
