#!/usr/bin/env bash
# run-privileged.sh — minimal privilege escalation wrapper for the packet counter.
#
# Passes only the env vars the binary needs (RUST_LOG, RUST_BACKTRACE) into
# the privileged context instead of forwarding the entire ambient environment
# with `sudo -E` (which leaks credentials, tokens, proxy settings, etc.).
#
# The eBPF loader must run as root (or with CAP_BPF + CAP_NET_ADMIN) to:
#   - Load BPF programs into the kernel
#   - Attach XDP programs to network interfaces
#
# Usage (invoked automatically by `cargo run --target <linux-target>`):
#   run-privileged.sh <binary> [args...]

set -euo pipefail

BINARY="${1:?Usage: run-privileged.sh <binary> [args...]}"
shift

exec sudo \
    RUST_LOG="${RUST_LOG:-info}" \
    RUST_BACKTRACE="${RUST_BACKTRACE:-1}" \
    "$BINARY" "$@"
