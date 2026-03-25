#!/usr/bin/env bash
# dev/dev.sh — Start the Lima eBPF dev VM and open an interactive shell.
#
# Usage (from the project root):
#   ./dev/dev.sh
#   make vm-shell
#
# What it does:
#   - Creates and provisions the VM on first run (~3-5 min)
#   - Starts the VM if it is stopped (~10-20s)
#   - Drops you into a bash shell inside the VM, already cd'd to the
#     project directory with cargo on PATH.
#
# Prerequisites (one-time on macOS):
#   brew install lima

set -euo pipefail

VM_NAME="packet-counter-dev"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
YAML="${SCRIPT_DIR}/ebpf-dev.yaml"

# --no-shell: start/provision the VM but do not open an interactive shell.
# Used by Makefile vm-run targets which manage the shell themselves.
NO_SHELL=0
for arg in "$@"; do
  [[ "$arg" == "--no-shell" ]] && NO_SHELL=1
done

# ---------------------------------------------------------------------------
# Colour helpers
# ---------------------------------------------------------------------------
if [[ -t 1 ]] && tput setaf 1 &>/dev/null; then
  BOLD=$(tput bold)
  CYAN=$(tput setaf 6)
  GREEN=$(tput setaf 2)
  YELLOW=$(tput setaf 3)
  RESET=$(tput sgr0)
else
  BOLD="" CYAN="" GREEN="" YELLOW="" RESET=""
fi

info()    { echo "${CYAN}[dev]${RESET} $*"; }
success() { echo "${GREEN}[dev]${RESET} $*"; }
warn()    { echo "${YELLOW}[dev]${RESET} $*"; }
header()  { echo "${BOLD}${CYAN}$*${RESET}"; }

# ---------------------------------------------------------------------------
# Prerequisite checks
# ---------------------------------------------------------------------------
if ! command -v limactl &>/dev/null; then
  echo "Error: limactl not found. Install Lima with: brew install lima" >&2
  exit 1
fi

if [[ ! -f "$YAML" ]]; then
  echo "Error: VM template not found at ${YAML}" >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# check_provision_status — fail loudly if cloud-init provisioning errored.
# ---------------------------------------------------------------------------
check_provision_status() {
  local log
  log=$(limactl shell "${VM_NAME}" -- sudo cat /var/log/cloud-init-output.log 2>/dev/null)

  if echo "${log}" | grep -q 'WARNING: Failed to execute'; then
    echo "" >&2
    echo "${BOLD}${YELLOW}[dev] WARNING: VM provisioning had failures.${RESET}" >&2
    echo "" >&2
    echo "${YELLOW}--- Provisioning errors ---${RESET}" >&2
    echo "${log}" | grep -E 'WARNING: Failed|error:|Error:|command not found|E: Package|curl: \(' >&2
    echo "" >&2
    echo "${YELLOW}Full log: limactl shell ${VM_NAME} -- sudo cat /var/log/cloud-init-output.log${RESET}" >&2
    echo "" >&2
    exit 1
  fi
}

# ---------------------------------------------------------------------------
# VM lifecycle management
# ---------------------------------------------------------------------------
VM_STATUS=$(limactl list --format '{{.Name}} {{.Status}}' 2>/dev/null \
  | awk -v name="${VM_NAME}" '$1 == name { print $2 }')

if [[ -z "${VM_STATUS}" ]]; then
  header "============================================================"
  header "  First-time setup: creating VM '${VM_NAME}'"
  header "  This takes 3-5 minutes. Grab a coffee."
  header "============================================================"
  limactl start --name="${VM_NAME}" --timeout=20m --progress "${YAML}"
  check_provision_status
  success "VM '${VM_NAME}' created and provisioned."
elif [[ "${VM_STATUS}" == "Running" ]]; then
  success "VM '${VM_NAME}' is already running."
elif [[ "${VM_STATUS}" == "Stopped" ]]; then
  info "Starting VM '${VM_NAME}'..."
  limactl start --timeout=20m "${VM_NAME}"
  success "VM started."
else
  warn "VM '${VM_NAME}' is in state '${VM_STATUS}'. Attempting to start..."
  limactl start --timeout=20m "${VM_NAME}" || {
    echo "Error: could not start VM. Run 'limactl list' for details." >&2
    exit 1
  }
fi

# ---------------------------------------------------------------------------
# Enter the VM (skipped when --no-shell is passed)
# ---------------------------------------------------------------------------
if [[ "$NO_SHELL" == "1" ]]; then
  success "VM '${VM_NAME}' is ready."
  exit 0
fi

info "Opening shell in '${VM_NAME}'..."
echo ""
exec limactl shell "${VM_NAME}"
