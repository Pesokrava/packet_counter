#![no_std]
#![no_main]

use aya_ebpf::{bindings::xdp_action, macros::xdp, programs::XdpContext};

// ---------------------------------------------------------------------------
// XDP entry point — stage 1 stub: pass all packets through unconditionally.
// Stage 2 will add header parsing and per-port counter maps.
// ---------------------------------------------------------------------------
#[xdp]
pub fn packet_counter(ctx: XdpContext) -> u32 {
    // Suppress unused-variable warning; ctx will be used in stage 2.
    let _ = ctx;
    xdp_action::XDP_PASS
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
// GPL license declaration required for BPF programs that use GPL-only helpers
// ---------------------------------------------------------------------------
#[link_section = "license"]
#[used]
static LICENSE: [u8; 4] = *b"GPL\0";
