#!/usr/bin/env bash
# OxiPulse — Install script
# Usage (generic):    curl -fsSL https://install.oxipulse.dev | bash
# Usage (SecuryBlack): curl -fsSL https://install.oxipulse.dev | bash -s -- --endpoint ingest.securyblack.com --token <TOKEN>
set -euo pipefail

SB_AGENT_LABEL="oxipulse"
LIB_URL="https://raw.githubusercontent.com/securyblack/sb-agent-core/main/scripts/install-lib.sh"
LIB_TMP="$(mktemp)"
curl -fsSL "$LIB_URL" -o "$LIB_TMP" || { echo "ERROR: could not fetch install-lib.sh from sb-agent-core" >&2; exit 1; }
# shellcheck source=/dev/null
source "$LIB_TMP"
rm -f "$LIB_TMP"

# ─── Constants ────────────────────────────────────────────────────────────────
GITHUB_REPO="securyblack/oxi-pulse"
BINARY_NAME="oxipulse"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/oxipulse"
CONFIG_FILE="${CONFIG_DIR}/config.toml"

# ─── Argument parsing ─────────────────────────────────────────────────────────
ENDPOINT=""
TOKEN=""
MODE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --endpoint) ENDPOINT="$2"; shift 2 ;;
    --token)    TOKEN="$2";    shift 2 ;;
    --mode)     MODE="$2";     shift 2 ;;
    *) sb_die "Unknown argument: $1" ;;
  esac
done

# ─── Banner ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}  ██████╗ ██╗  ██╗██╗██████╗ ██╗   ██╗██╗     ███████╗███████╗${RESET}"
echo -e "${BOLD}  ██╔═══██╗╚██╗██╔╝██║██╔══██╗██║   ██║██║     ██╔════╝██╔════╝${RESET}"
echo -e "${BOLD}  ██║   ██║ ╚███╔╝ ██║██████╔╝██║   ██║██║     ███████╗█████╗  ${RESET}"
echo -e "${BOLD}  ██║   ██║ ██╔██╗ ██║██╔═══╝ ██║   ██║██║     ╚════██║██╔══╝  ${RESET}"
echo -e "${BOLD}  ╚██████╔╝██╔╝ ██╗██║██║     ╚██████╔╝███████╗███████║███████╗${RESET}"
echo -e "${BOLD}   ╚═════╝ ╚═╝  ╚═╝╚═╝╚═╝      ╚═════╝ ╚══════╝╚══════╝╚══════╝${RESET}"
echo ""
sb_info "Server monitoring agent installer"
echo ""

sb_require_root
sb_require_cmds curl tar systemctl

TARGET="$(sb_detect_arch_linux)"
LATEST_VERSION="$(sb_fetch_latest_version "$GITHUB_REPO")"

ASSET_NAME="${BINARY_NAME}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${GITHUB_REPO}/releases/download/${LATEST_VERSION}/${ASSET_NAME}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

sb_download_and_verify "$DOWNLOAD_URL" "${TMP_DIR}/${ASSET_NAME}"
sb_install_binary "${TMP_DIR}/${ASSET_NAME}" "$BINARY_NAME" "$INSTALL_DIR"

# ─── Configuration ────────────────────────────────────────────────────────────
mkdir -p "$CONFIG_DIR"
chmod 700 "$CONFIG_DIR"

# Apply local_agent defaults
if [[ "${MODE:-}" == "local_agent" ]]; then
  ENDPOINT="${ENDPOINT:-http://localhost:4317}"
  sb_info "Mode: local_agent — OxiPulse will send metrics to localhost:4317"
fi

# Ask interactively if not provided via arguments
if [[ -z "$ENDPOINT" ]]; then
  echo ""
  read -rp "$(echo -e "${BOLD}  OTLP endpoint (e.g. https://ingest.example.com:4317):${RESET} ")" ENDPOINT </dev/tty
fi
if [[ -z "$TOKEN" ]]; then
  read -rsp "$(echo -e "${BOLD}  Auth token:${RESET} ")" TOKEN </dev/tty
  echo ""
fi

[[ -z "$ENDPOINT" ]] && sb_die "Endpoint cannot be empty"
[[ -z "$TOKEN" ]]    && sb_die "Token cannot be empty"

sb_info "Writing config to ${CONFIG_FILE}…"
cat > "$CONFIG_FILE" <<EOF
# OxiPulse configuration
# Do not share this file — it contains your auth token.
mode = "${MODE:-direct}"
endpoint = "${ENDPOINT}"
token = "${TOKEN}"
interval_secs = 30
buffer_max_size = 8640
EOF
chmod 600 "$CONFIG_FILE"
sb_success "Config written"

# ─── systemd service ──────────────────────────────────────────────────────────
sb_write_systemd_unit "oxipulse" "OxiPulse monitoring agent" "${INSTALL_DIR}/${BINARY_NAME}"
sb_enable_start_service "oxipulse"

# ─── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}  OxiPulse ${LATEST_VERSION} installed successfully!${RESET}"
echo ""
echo -e "  Status:  ${BOLD}systemctl status oxipulse${RESET}"
echo -e "  Logs:    ${BOLD}journalctl -fu oxipulse${RESET}"
echo -e "  Config:  ${BOLD}${CONFIG_FILE}${RESET}"
echo ""
