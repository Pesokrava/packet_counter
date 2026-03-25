#!/usr/bin/env bash
# dev/teardown.sh — Stop or destroy the packet-counter-dev Lima VM.
#
# Usage (from the project root):
#   ./dev/teardown.sh           # stop VM (keeps disk, fast to restart)
#   ./dev/teardown.sh --destroy # stop + delete VM and all its data
#   make vm-down                # alias for stop
#   make vm-destroy             # alias for destroy

set -euo pipefail

VM_NAME="packet-counter-dev"

# ---------------------------------------------------------------------------
# Colour helpers
# ---------------------------------------------------------------------------
if [[ -t 1 ]] && tput setaf 1 &>/dev/null; then
  BOLD=$(tput bold)
  CYAN=$(tput setaf 6)
  GREEN=$(tput setaf 2)
  YELLOW=$(tput setaf 3)
  RED=$(tput setaf 1)
  RESET=$(tput sgr0)
else
  BOLD="" CYAN="" GREEN="" YELLOW="" RED="" RESET=""
fi

info()    { echo "${CYAN}[teardown]${RESET} $*"; }
success() { echo "${GREEN}[teardown]${RESET} $*"; }
warn()    { echo "${YELLOW}[teardown]${RESET} $*"; }
fatal()   { echo "${RED}[teardown] ERROR:${RESET} $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
DESTROY=false
for arg in "$@"; do
  case "$arg" in
    --destroy) DESTROY=true ;;
    --help|-h)
      echo "Usage: ./dev/teardown.sh [--destroy]"
      echo ""
      echo "  (no flags)  Stop the VM, preserving disk state."
      echo "              Fast to restart with make vm-shell (~10-20s)."
      echo ""
      echo "  --destroy   Stop and permanently delete the VM and all its data."
      echo "              Next make vm-shell will fully reprovision (~3-5 min)."
      exit 0
      ;;
    *) fatal "Unknown argument: $arg. Use --help for usage." ;;
  esac
done

# ---------------------------------------------------------------------------
# Prerequisite check
# ---------------------------------------------------------------------------
if ! command -v limactl &>/dev/null; then
  fatal "limactl not found. Install Lima with: brew install lima"
fi

# ---------------------------------------------------------------------------
# Resolve current VM state
# ---------------------------------------------------------------------------
VM_STATUS=$(limactl list --format '{{.Name}} {{.Status}}' 2>/dev/null \
  | awk -v name="${VM_NAME}" '$1 == name { print $2 }')

if [[ -z "${VM_STATUS}" ]]; then
  warn "VM '${VM_NAME}' does not exist — nothing to do."
  exit 0
fi

info "VM '${VM_NAME}' is currently: ${VM_STATUS}"

# ---------------------------------------------------------------------------
# Stop
# ---------------------------------------------------------------------------
if [[ "${VM_STATUS}" == "Running" ]]; then
  info "Stopping VM '${VM_NAME}'..."
  limactl stop "${VM_NAME}"
  success "VM stopped."
elif [[ "${VM_STATUS}" == "Stopped" ]]; then
  info "VM is already stopped."
else
  warn "VM is in state '${VM_STATUS}' — attempting stop anyway..."
  limactl stop --force "${VM_NAME}" 2>/dev/null || true
fi

# ---------------------------------------------------------------------------
# Destroy (optional)
# ---------------------------------------------------------------------------
if [[ "${DESTROY}" == "true" ]]; then
  echo ""
  echo "${BOLD}${RED}WARNING: This will permanently delete VM '${VM_NAME}' and all its data.${RESET}"
  echo "  - Installed tools (rustup, bpf-linker, neovim plugins, node_modules, etc.)"
  echo "  - Cargo build cache at /var/tmp/cargo-target/"
  echo "  - Everything under the VM's /home directory"
  echo ""
  echo "Source code on your macOS filesystem is NOT affected."
  echo ""
  read -r -p "Type 'yes' to confirm destruction: " CONFIRM
  if [[ "${CONFIRM}" != "yes" ]]; then
    info "Aborted. VM stopped but not deleted."
    exit 0
  fi

  info "Deleting VM '${VM_NAME}'..."
  limactl delete "${VM_NAME}"
  success "VM '${VM_NAME}' deleted. Run 'make vm-shell' to reprovision."
else
  echo ""
  success "VM '${VM_NAME}' is stopped and its disk is preserved."
  info  "Restart with:  make vm-shell"
  info  "Full destroy:  make vm-destroy"
fi
