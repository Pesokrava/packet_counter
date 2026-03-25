# =============================================================================
# Makefile — packet_counter project shortcuts
#
# All build and run targets are designed to be executed *inside* the Lima VM
# (which has the full Rust + bpf-linker + Linux kernel headers toolchain).
# The VM targets (vm-*) run on the macOS host.
#
# Quick start:
#   make vm-shell   — create/start VM and open an interactive shell
#   (inside VM) make build
#   (inside VM) make run
# =============================================================================

.PHONY: help vm-up vm-shell vm-down vm-destroy build build-ebpf check fmt lint run clean

# Default target
help:
	@echo ""
	@echo "  packet_counter — Makefile targets"
	@echo ""
	@echo "  VM management (run on macOS host):"
	@echo "    make vm-shell    Create/start the Lima VM and open a shell"
	@echo "    make vm-down     Stop the VM (preserves disk state)"
	@echo "    make vm-destroy  Stop and permanently delete the VM"
	@echo ""
	@echo "  Build & run (run inside the Lima VM):"
	@echo "    make build       Build the userspace binary (embeds eBPF)"
	@echo "    make check       cargo check (fast, no codegen)"
	@echo "    make fmt         Run rustfmt across the workspace"
	@echo "    make lint        Run clippy across the workspace"
	@echo "    make run         Build and run with sudo (requires root/CAP_BPF)"
	@echo "    make run IFACE=ens3 SKB=1   Run on a specific interface in SKB mode"
	@echo "    make clean       Remove build artifacts"
	@echo ""

# ---------------------------------------------------------------------------
# VM management (macOS host)
# ---------------------------------------------------------------------------

# Create the VM on first run, start if stopped, then open an interactive shell.
vm-shell:
	./dev/dev.sh

# Alias kept for documentation clarity (dev.sh does the same thing)
vm-up: vm-shell

# Stop the VM, preserving disk state.
vm-down:
	./dev/teardown.sh

# Stop and permanently delete the VM and all its data.
vm-destroy:
	./dev/teardown.sh --destroy

# ---------------------------------------------------------------------------
# Build targets (run inside the Lima VM)
# ---------------------------------------------------------------------------

# Target triple for the Linux host inside the VM.
TARGET ?= aarch64-unknown-linux-gnu

# Network interface to attach to (override with: make run IFACE=ens3)
IFACE ?= eth0

# Set SKB=1 to attach in SKB (generic) mode instead of native XDP.
SKB ?= 0

SKB_FLAG :=
ifeq ($(SKB),1)
  SKB_FLAG := --skb-mode
endif

# Build the userspace binary. aya-build compiles the eBPF crate automatically.
build:
	cargo build --target $(TARGET) -p packet-counter

# Fast type/borrow check without codegen.
check:
	cargo check --target $(TARGET) -p packet-counter
	cargo check --target $(TARGET) -p packet-counter-common

# Format the entire workspace.
fmt:
	cargo fmt --all

# Lint the entire workspace (non-eBPF crates only — clippy can't target bpfel).
lint:
	cargo clippy --target $(TARGET) -p packet-counter -p packet-counter-common -- -D warnings

# Build and run with privilege escalation (required for BPF + XDP attach).
# The run-privileged.sh wrapper passes only RUST_LOG and RUST_BACKTRACE to sudo.
run: build
	./run-privileged.sh \
		"$${CARGO_TARGET_DIR:-target}/$(TARGET)/debug/packet-counter" \
		--iface $(IFACE) $(SKB_FLAG)

# Build in release mode and run.
run-release:
	cargo build --release --target $(TARGET) -p packet-counter
	./run-privileged.sh \
		"$${CARGO_TARGET_DIR:-target}/$(TARGET)/release/packet-counter" \
		--iface $(IFACE) $(SKB_FLAG)

# ---------------------------------------------------------------------------
# Clean
# ---------------------------------------------------------------------------

clean:
	cargo clean
	@echo "Note: VM-local build cache at /var/tmp/cargo-target/ is not removed."
	@echo "      To clear it: rm -rf /var/tmp/cargo-target/packet-counter"
