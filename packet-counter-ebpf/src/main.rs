#![no_std]
#![no_main]

use core::mem;

use aya_ebpf::{
    bindings::xdp_action, macros::map, macros::xdp, maps::PerCpuHashMap, programs::XdpContext,
};
use network_types::{
    eth::{EthHdr, EtherType},
    ip::{IpProto, Ipv4Hdr, Ipv6Hdr},
    tcp::TcpHdr,
    udp::UdpHdr,
};
use packet_counter_common::{PortKey, PROTO_TCP, PROTO_UDP};

// ---------------------------------------------------------------------------
// BPF map — per-CPU packet counter keyed by (protocol, destination port).
//
// PerCpuHashMap: each CPU has its own independent counter slot for every key,
// so increments require no locking — the BPF verifier guarantees that each
// CPU only ever touches its own slot.  The userspace side sums across CPUs
// when reading.
//
// max_entries: 65535 covers the full port range for both TCP and UDP.
// In practice only a small subset of ports will ever see traffic.
// ---------------------------------------------------------------------------
#[map]
static PACKET_COUNTS: PerCpuHashMap<PortKey, u64> = PerCpuHashMap::with_max_entries(65535, 0);

// ---------------------------------------------------------------------------
// Bounds-checked pointer helper
// ---------------------------------------------------------------------------
/// Returns a pointer to a `T`-sized region at `ctx.data() + offset`, or
/// `None` if the region would exceed `ctx.data_end()`.
///
/// All arithmetic uses `checked_add` so that a wrapping offset cannot bypass
/// the bounds check (required by the BPF verifier).
#[inline(always)]
unsafe fn ptr_at<T>(ctx: &XdpContext, offset: usize) -> Option<*const T> {
    let start = ctx.data() as usize;
    let end = ctx.data_end() as usize;
    let size = mem::size_of::<T>();
    let ptr = start.checked_add(offset)?;
    let region_end = ptr.checked_add(size)?;
    if region_end > end {
        return None;
    }
    Some(ptr as *const T)
}

// ---------------------------------------------------------------------------
// Counter helper
// ---------------------------------------------------------------------------
/// Increment the per-CPU counter for `key`.
///
/// Uses `get_ptr_mut` to obtain a mutable reference to the existing counter
/// and increments it in-place.  If no entry exists yet, inserts 1.
///
/// This is safe for per-CPU maps: the BPF runtime guarantees each CPU only
/// accesses its own slot, so there is no data race.
#[inline(always)]
fn increment(key: &PortKey) {
    // Fast path: entry already exists — increment in place.
    if let Some(count) = PACKET_COUNTS.get_ptr_mut(key) {
        unsafe { *count += 1 };
        return;
    }
    // Slow path: first packet for this (protocol, port) pair.
    // Ignore insertion errors (map full) — we never drop packets.
    let _ = PACKET_COUNTS.insert(key, &1u64, 0);
}

// ---------------------------------------------------------------------------
// XDP entry point
// ---------------------------------------------------------------------------
#[xdp]
pub fn packet_counter(ctx: XdpContext) -> u32 {
    // Parse failure → pass the packet through unchanged.
    match try_count(&ctx) {
        Ok(action) => action,
        Err(_) => xdp_action::XDP_PASS,
    }
}

fn try_count(ctx: &XdpContext) -> Result<u32, ()> {
    // ------------------------------------------------------------------
    // 1. Ethernet header
    // ------------------------------------------------------------------
    let eth: *const EthHdr = unsafe { ptr_at(ctx, 0) }.ok_or(())?;
    let ether_type = unsafe { (*eth).ether_type() };

    match ether_type {
        // ------------------------------------------------------------------
        // 2a. IPv4
        // ------------------------------------------------------------------
        Ok(EtherType::Ipv4) => {
            let ipv4: *const Ipv4Hdr = unsafe { ptr_at(ctx, EthHdr::LEN) }.ok_or(())?;

            // `ihl()` returns the header length in bytes (min 20, max 60).
            // The transport header starts immediately after the IP header.
            let ip_hdr_len = unsafe { (*ipv4).ihl() } as usize;
            let transport_offset = EthHdr::LEN + ip_hdr_len;

            match unsafe { (*ipv4).proto } {
                IpProto::Tcp => {
                    let tcp: *const TcpHdr = unsafe { ptr_at(ctx, transport_offset) }.ok_or(())?;
                    let dst_port = u16::from_be_bytes(unsafe { (*tcp).dest });
                    increment(&PortKey::new(PROTO_TCP, dst_port));
                }
                IpProto::Udp => {
                    let udp: *const UdpHdr = unsafe { ptr_at(ctx, transport_offset) }.ok_or(())?;
                    let dst_port = unsafe { (*udp).dst_port() };
                    increment(&PortKey::new(PROTO_UDP, dst_port));
                }
                // Other protocols (ICMP, etc.) are not counted.
                _ => {}
            }
        }

        // ------------------------------------------------------------------
        // 2b. IPv6
        // ------------------------------------------------------------------
        Ok(EtherType::Ipv6) => {
            let ipv6: *const Ipv6Hdr = unsafe { ptr_at(ctx, EthHdr::LEN) }.ok_or(())?;
            // IPv6 fixed header is always 40 bytes; no IHL field.
            let transport_offset = EthHdr::LEN + Ipv6Hdr::LEN;

            match unsafe { (*ipv6).next_hdr } {
                IpProto::Tcp => {
                    let tcp: *const TcpHdr = unsafe { ptr_at(ctx, transport_offset) }.ok_or(())?;
                    let dst_port = u16::from_be_bytes(unsafe { (*tcp).dest });
                    increment(&PortKey::new(PROTO_TCP, dst_port));
                }
                IpProto::Udp => {
                    let udp: *const UdpHdr = unsafe { ptr_at(ctx, transport_offset) }.ok_or(())?;
                    let dst_port = unsafe { (*udp).dst_port() };
                    increment(&PortKey::new(PROTO_UDP, dst_port));
                }
                // IPv6 extension headers and other next-headers are skipped.
                _ => {}
            }
        }

        // Other EtherTypes (ARP, VLAN, etc.) — pass through, not counted.
        _ => {}
    }

    Ok(xdp_action::XDP_PASS)
}

// ---------------------------------------------------------------------------
// Required panic handler for no_std BPF targets
// ---------------------------------------------------------------------------
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// ---------------------------------------------------------------------------
// GPL license declaration required for BPF programs using GPL-only helpers
// ---------------------------------------------------------------------------
#[link_section = "license"]
#[used]
static LICENSE: [u8; 4] = *b"GPL\0";
