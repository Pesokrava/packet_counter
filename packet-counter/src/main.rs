use std::time::Duration;

use anyhow::Context as _;
use aya::{
    maps::PerCpuHashMap,
    programs::{Xdp, XdpFlags},
    Ebpf,
};
use aya_log::EbpfLogger;
use clap::Parser;
use log::{info, warn};
use packet_counter_common::{PortKey, PROTO_TCP, PROTO_UDP};
use tokio::{signal, time};

/// eBPF packet counter — attaches an XDP program to a network interface,
/// counts packets per destination port (TCP + UDP), and prints per-port
/// statistics to stdout every second.
///
/// Communication with the eBPF program happens through a `PerCpuHashMap`
/// BPF map named `PACKET_COUNTS`.  The XDP program increments a per-CPU
/// counter for each `(protocol, dst_port)` pair it observes.  Userspace
/// reads the map on demand and sums the per-CPU values to get the total.
#[derive(Debug, Parser)]
struct Opt {
    /// Network interface to attach to (e.g. eth0, lo, ens3)
    #[clap(short, long, default_value = "eth0")]
    iface: String,

    /// Attach using SKB (generic) mode instead of native XDP mode.
    ///
    /// Native mode (default) runs before the kernel networking stack — lowest
    /// latency. Use this flag when the driver does not support native XDP
    /// (e.g. virtual or container interfaces).
    #[clap(long)]
    skb_mode: bool,

    /// How often (in seconds) to print the stats table to stdout.
    #[clap(long, default_value = "1")]
    interval: u64,
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
    // Wire up aya-log so that aya_log_ebpf messages are forwarded to log/env_logger.
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
            "Attaching in SKB (generic) mode — worse performance than native XDP. \
             Use only when the driver does not support native XDP."
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
        "XDP program attached to '{}'. Printing stats every {}s. Press Ctrl-C to stop.",
        opt.iface, opt.interval
    );

    // -----------------------------------------------------------------------
    // Periodic stats loop — reads the PerCpuHashMap, sums per-CPU values,
    // and prints a sorted table.  Runs until Ctrl-C.
    // -----------------------------------------------------------------------
    let mut interval = time::interval(Duration::from_secs(opt.interval));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                print_stats(&bpf);
            }
            _ = signal::ctrl_c() => {
                info!("Received Ctrl-C, detaching and exiting.");
                break;
            }
        }
    }

    Ok(())
}

/// Read `PACKET_COUNTS` from the BPF map, aggregate per-CPU values, and
/// print a sorted stats table to stdout.
fn print_stats(bpf: &Ebpf) {
    // Borrow the map.  Returns None if the map name doesn't exist — shouldn't
    // happen if the eBPF program was loaded correctly.
    let map = match bpf.map("PACKET_COUNTS") {
        Some(m) => m,
        None => {
            warn!("PACKET_COUNTS map not found");
            return;
        }
    };

    let counts: PerCpuHashMap<_, PortKey, u64> = match PerCpuHashMap::try_from(map) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to open PACKET_COUNTS map: {}", e);
            return;
        }
    };

    // Collect and sum per-CPU values for each key.
    let mut entries: Vec<(PortKey, u64)> = counts
        .iter()
        .filter_map(|result| {
            let (key, per_cpu_values) = result.ok()?;
            // Sum across all CPUs.
            let total: u64 = per_cpu_values.iter().sum();
            Some((key, total))
        })
        .collect();

    if entries.is_empty() {
        info!("No packets counted yet.");
        return;
    }

    // Sort by count descending, then by port ascending for stable output.
    entries.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.port.cmp(&b.0.port)));

    println!("\n{:<10} {:<8} {:>15}", "PROTOCOL", "PORT", "PACKETS");
    println!("{}", "-".repeat(35));
    for (key, count) in &entries {
        let proto = match key.protocol {
            p if p == PROTO_TCP => "TCP",
            p if p == PROTO_UDP => "UDP",
            _ => "OTHER",
        };
        println!("{:<10} {:<8} {:>15}", proto, key.port, count);
    }
    println!();
}
