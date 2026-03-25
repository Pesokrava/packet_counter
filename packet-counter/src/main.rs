use anyhow::Context as _;
use aya::{
    programs::{Xdp, XdpFlags},
    Ebpf,
};
use aya_log::EbpfLogger;
use clap::Parser;
use log::{info, warn};
use tokio::signal;

/// eBPF packet counter — attaches an XDP program to a network interface and
/// counts packets per destination port (TCP + UDP).
///
/// Stage 1 stub: attaches the XDP program (pass-through only) and waits for
/// Ctrl-C.  Stage 2 will add map reading and per-port stats output.
#[derive(Debug, Parser)]
struct Opt {
    /// Network interface to attach to (e.g. eth0, lo, ens3)
    #[clap(short, long, default_value = "eth0")]
    iface: String,

    /// Attach using SKB (generic) mode instead of native XDP mode.
    ///
    /// Native mode (default) runs before the kernel networking stack processes
    /// the packet — lowest latency, highest performance.  Use this flag when
    /// the interface driver does not support native XDP (e.g. virtual or
    /// container interfaces).
    #[clap(long)]
    skb_mode: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    env_logger::init();

    // -----------------------------------------------------------------------
    // Load the compiled eBPF bytecode embedded at build time.
    // -----------------------------------------------------------------------
    let mut bpf = Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/packet-counter-ebpf"
    )))?;

    // -----------------------------------------------------------------------
    // Wire up aya-log so that `aya_log_ebpf` messages from the kernel side
    // are forwarded to the `log` crate (env_logger).
    // -----------------------------------------------------------------------
    if let Err(e) = EbpfLogger::init(&mut bpf) {
        warn!("Failed to initialise eBPF logger: {}", e);
    }

    // -----------------------------------------------------------------------
    // Attach the XDP program to the requested interface.
    // -----------------------------------------------------------------------
    let program: &mut Xdp = bpf
        .program_mut("packet_counter")
        .context("BPF program 'packet_counter' not found in object file")?
        .try_into()?;

    program.load()?;

    let xdp_flags = if opt.skb_mode {
        warn!(
            "Attaching in SKB (generic) mode — performance will be worse than native XDP. \
             Use only when the interface driver does not support native XDP."
        );
        XdpFlags::SKB_MODE
    } else {
        XdpFlags::default()
    };

    program.attach(&opt.iface, xdp_flags).with_context(|| {
        format!(
            "Failed to attach XDP program to '{}' in {} mode. \
                 If the driver does not support native XDP, retry with --skb-mode.",
            opt.iface,
            if opt.skb_mode { "SKB" } else { "native" }
        )
    })?;

    info!(
        "XDP program attached to '{}'. Press Ctrl-C to stop.",
        opt.iface
    );

    // -----------------------------------------------------------------------
    // Block until Ctrl-C; Aya cleans up on Drop.
    // -----------------------------------------------------------------------
    signal::ctrl_c().await?;
    info!("Received Ctrl-C, detaching and exiting.");

    Ok(())
}
