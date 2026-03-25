use aya_build::{Package, Toolchain};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile the eBPF program at build time and embed the resulting BPF ELF
    // object into the userspace binary via `include_bytes_aligned!`.
    aya_build::build_ebpf(
        [Package {
            name: "packet-counter-ebpf",
            root_dir: "../packet-counter-ebpf",
            ..Default::default()
        }],
        Toolchain::default(),
    )?;
    Ok(())
}
