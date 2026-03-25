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

.PHONY: help vm-up vm-shell vm-down vm-destroy build build-ebpf build-web build-all check fmt lint run run-release dev dev-backend dev-frontend clean

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
	@echo "    make build-web   Build the React frontend (outputs to web/dist/)"
	@echo "    make build-all   Build frontend then Rust binary (sequentially)"
	@echo "    make check       cargo check (fast, no codegen)"
	@echo "    make fmt         Run rustfmt across the workspace"
	@echo "    make lint        Run clippy across the workspace"
	@echo "    make run         Build and run with sudo; passes --static-dir web/dist if it exists"
	@echo "    make run IFACE=ens3 SKB=1        Run on a specific interface in SKB mode"
	@echo "    make run PORT=8080               Override the HTTP listen port"
	@echo "    make dev         Start backend in VM + Vite on host (from macOS host)"
	@echo "    make dev-backend Start only the Rust backend (run inside the VM)"
	@echo "    make dev-frontend Start only the Vite dev server (run on macOS host)"
	@echo "    make clean       Remove build artifacts and web/dist/"
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

# HTTP listen address (override with: make run PORT=8080)
PORT ?= 3001

# Build and run with privilege escalation (required for BPF + XDP attach).
# The run-privileged.sh wrapper passes only RUST_LOG and RUST_BACKTRACE to sudo.
# If web/dist/ exists (i.e. frontend has been built), serve it via --static-dir.
STATIC_DIR_FLAG :=
ifneq ($(wildcard web/dist/index.html),)
  STATIC_DIR_FLAG := --static-dir web/dist
endif

run: build
	./run-privileged.sh \
		"$${CARGO_TARGET_DIR:-target}/$(TARGET)/debug/packet-counter" \
		--iface $(IFACE) $(SKB_FLAG) \
		--listen "0.0.0.0:$(PORT)" \
		$(STATIC_DIR_FLAG)

# Build in release mode and run.
run-release:
	cargo build --release --target $(TARGET) -p packet-counter
	./run-privileged.sh \
		"$${CARGO_TARGET_DIR:-target}/$(TARGET)/release/packet-counter" \
		--iface $(IFACE) $(SKB_FLAG) \
		--listen "0.0.0.0:$(PORT)" \
		$(STATIC_DIR_FLAG)

# ---------------------------------------------------------------------------
# Frontend build
# ---------------------------------------------------------------------------

# Build the React SPA. Output goes to web/dist/.
build-web:
	cd web && npm install && npm run build

# Build frontend first, then the Rust binary. No Cargo/build.rs coupling —
# the frontend is a pure Makefile prerequisite.
build-all: build-web build

# ---------------------------------------------------------------------------
# Dev mode (frontend proxies /api to the Rust backend on :3001)
# ---------------------------------------------------------------------------

# VM name used by limactl (matches --name passed to limactl start).
VM_NAME ?= packet-counter-dev

# Start the Rust backend inside the Lima VM (forwarded to host :3001) and
# run the Vite dev server on the macOS host.  The Vite proxy config
# (web/vite.config.ts) forwards /api requests to localhost:3001.
#
# Run each command in a separate terminal, or background the VM command:
#   Terminal 1 (inside VM):  make dev-backend
#   Terminal 2 (host):       make dev-frontend
#
# Or run both from the host with:
#   make dev
#
dev:
	limactl shell $(VM_NAME) -- bash -lc \
		'cd $(shell pwd) && make dev-backend' &
	npm --prefix web run dev

# Build and start only the Rust backend (run this inside the VM).
dev-backend: build
	./run-privileged.sh \
		"$${CARGO_TARGET_DIR:-target}/$(TARGET)/debug/packet-counter" \
		--iface $(IFACE) $(SKB_FLAG) \
		--listen "0.0.0.0:$(PORT)"

# Start only the Vite dev server (run this on the macOS host).
dev-frontend:
	npm --prefix web run dev

# ---------------------------------------------------------------------------
# Clean
# ---------------------------------------------------------------------------

clean:
	cargo clean
	rm -rf web/dist
	@echo "Note: VM-local build cache at /var/tmp/cargo-target/ is not removed."
	@echo "      To clear it: rm -rf /var/tmp/cargo-target/packet-counter"
