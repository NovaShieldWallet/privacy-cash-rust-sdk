#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# ═══════════════════════════════════════════════════════════════
#                    ANSI Color Definitions
# ═══════════════════════════════════════════════════════════════
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

# Spinner characters
SPINNER_CHARS='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'

# ═══════════════════════════════════════════════════════════════
#                    Helper Functions
# ═══════════════════════════════════════════════════════════════

print_header() {
  echo
  echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════╗${RESET}"
  echo -e "${CYAN}║${RESET}       ${BOLD}${WHITE}PRIVACY CASH${RESET} - ${MAGENTA}Send Test Script${RESET}                      ${CYAN}║${RESET}"
  echo -e "${CYAN}║${RESET}       ${DIM}Pure Rust SDK with ZK Proofs${RESET}                          ${CYAN}║${RESET}"
  echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════╝${RESET}"
  echo
}

print_success() {
  echo -e "${GREEN}✓${RESET} $1"
}

print_error() {
  echo -e "${RED}✗${RESET} $1" >&2
}

print_warning() {
  echo -e "${YELLOW}⚠${RESET} $1"
}

print_info() {
  echo -e "${BLUE}ℹ${RESET} $1"
}

print_step() {
  echo -e "${CYAN}▸${RESET} $1"
}

# Spinner that runs while a command executes
# Usage: spin "Message" command args...
spin() {
  local msg="$1"
  shift
  local pid
  local i=0
  local spin_len=${#SPINNER_CHARS}
  
  # Start the command in background
  "$@" > /tmp/privacy-cash-build.log 2>&1 &
  pid=$!
  
  # Show spinner while command runs
  while kill -0 "$pid" 2>/dev/null; do
    local char="${SPINNER_CHARS:i++%spin_len:1}"
    printf "\r${CYAN}%s${RESET} %s" "$char" "$msg"
    sleep 0.1
  done
  
  # Wait for command and get exit code
  wait "$pid"
  local exit_code=$?
  
  # Clear the spinner line
  printf "\r\033[K"
  
  if [[ $exit_code -eq 0 ]]; then
    print_success "$msg"
  else
    print_error "$msg"
    echo
    echo -e "${DIM}Build output:${RESET}"
    cat /tmp/privacy-cash-build.log | head -50
    return $exit_code
  fi
}

print_config() {
  echo -e "${WHITE}${BOLD}Configuration:${RESET}"
  echo -e "  ${DIM}Repo:${RESET}      ${WHITE}$ROOT${RESET}"
  echo -e "  ${DIM}Recipient:${RESET} ${WHITE}$RECIPIENT${RESET}"
  echo -e "  ${DIM}Amount:${RESET}    ${GREEN}$AMOUNT${RESET} ${YELLOW}$TOKEN${RESET}"
  echo -e "  ${DIM}Mode:${RESET}      ${MAGENTA}$([ "$SIMULATE" == "true" ] && echo "SIMULATION" || echo "LIVE")${RESET}"
  if [[ -n "${RPC:-}" ]]; then
    echo -e "  ${DIM}RPC:${RESET}       ${WHITE}$RPC${RESET}"
  fi
  echo
}

usage() {
  cat <<EOF
${BOLD}${WHITE}Send test for the Privacy Cash Rust SDK${RESET}

${BOLD}Usage:${RESET}
  ${CYAN}bash scripts/send-test.sh${RESET} ${GREEN}<recipient>${RESET} [amount] [token] [--simulate true|false]

${BOLD}Examples:${RESET}
  ${DIM}# Send 0.02 SOL${RESET}
  bash scripts/send-test.sh DZk343QuE... 0.02 sol

  ${DIM}# Send 2 USDC with custom RPC${RESET}
  SOLANA_RPC_URL="https://api.mainnet-beta.solana.com" bash scripts/send-test.sh <recipient> 2 usdc

  ${DIM}# Simulation mode (no real transactions)${RESET}
  bash scripts/send-test.sh DZk343QuE... --simulate true

${BOLD}Environment variables:${RESET}
  ${YELLOW}SOLANA_PRIVATE_KEY${RESET}  (required unless --simulate) base58-encoded keypair
  ${YELLOW}SOLANA_RPC_URL${RESET}      (optional) defaults to mainnet inside the SDK
  ${YELLOW}AMOUNT${RESET}              (optional) default: 0.02
  ${YELLOW}TOKEN${RESET}               (optional) default: sol
  ${YELLOW}RECIPIENT${RESET}           (optional) used if <recipient> arg omitted
  ${YELLOW}SIMULATE${RESET}            (optional) default: false

${BOLD}Notes:${RESET}
  ${RED}⚠${RESET}  This will MOVE FUNDS on-chain. Start with a tiny amount.
  ${BLUE}ℹ${RESET}  Circuit files required at: circuit/transaction2.{wasm,zkey}
  ${GREEN}✓${RESET}  Tip: put SOLANA_PRIVATE_KEY in .env.local (gitignored)
EOF
}

# ═══════════════════════════════════════════════════════════════
#                    Parse Arguments
# ═══════════════════════════════════════════════════════════════

SIMULATE="${SIMULATE:-false}"
POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --simulate|--simualte)
      if [[ "${2:-}" =~ ^(true|false)$ ]]; then
        SIMULATE="$2"
        shift 2
      else
        SIMULATE="true"
        shift
      fi
      ;;
    --simulate=*|--simualte=*)
      value="${1#*=}"
      if [[ "$value" =~ ^(true|false)$ ]]; then
        SIMULATE="$value"
      else
        print_error "--simulate expects true|false (got: $value)"
        exit 2
      fi
      shift
      ;;
    *)
      POSITIONAL+=("$1")
      shift
      ;;
  esac
done
set -- "${POSITIONAL[@]}"

# ═══════════════════════════════════════════════════════════════
#                    Validation
# ═══════════════════════════════════════════════════════════════

print_header

# Check circuit files
if [[ ! -f "circuit/transaction2.wasm" || ! -f "circuit/transaction2.zkey" ]]; then
  print_error "Missing circuit files!"
  echo
  echo -e "  ${DIM}Expected:${RESET}"
  echo -e "    circuit/transaction2.wasm"
  echo -e "    circuit/transaction2.zkey"
  echo
  echo -e "  ${DIM}Download them from your Privacy Cash circuit distribution.${RESET}"
  exit 1
fi
print_success "Circuit files found"

RECIPIENT="${RECIPIENT:-}"
AMOUNT="${AMOUNT:-0.02}"
TOKEN="${TOKEN:-sol}"

if [[ $# -ge 1 && -n "${1:-}" ]]; then
  RECIPIENT="$1"
fi
if [[ $# -ge 2 && -n "${2:-}" ]]; then
  AMOUNT="$2"
fi
if [[ $# -ge 3 && -n "${3:-}" ]]; then
  TOKEN="$3"
fi

if [[ -z "${RECIPIENT:-}" ]]; then
  print_error "Recipient is required."
  echo
  usage
  exit 2
fi

# ═══════════════════════════════════════════════════════════════
#                    Load Environment
# ═══════════════════════════════════════════════════════════════

if [[ -z "${SOLANA_PRIVATE_KEY:-}" || -z "${SOLANA_RPC_URL:-}" ]]; then
  for envfile in ".env.local" ".env"; do
    [[ -f "$envfile" ]] || continue
    while IFS= read -r line || [[ -n "$line" ]]; do
      line="${line%%#*}"
      line="${line#"${line%%[![:space:]]*}"}"
      line="${line%"${line##*[![:space:]]}"}"
      [[ -z "$line" ]] && continue

      case "$line" in
        SOLANA_RPC_URL=*)
          key="${line%%=*}"
          value="${line#*=}"
          ;;
        SOLANA_PRIVATE_KEY=*)
          if [[ "$SIMULATE" == "true" ]]; then
            continue
          fi
          key="${line%%=*}"
          value="${line#*=}"
          ;;
        *)
          continue
          ;;
      esac

      value="${value#"${value%%[![:space:]]*}"}"
      value="${value%"${value##*[![:space:]]}"}"
      if [[ ( "$value" == \"*\" && "$value" == *\" ) || ( "$value" == \'*\' && "$value" == *\' ) ]]; then
        value="${value:1:${#value}-2}"
      fi
      if [[ -z "${!key:-}" ]]; then
        export "$key=$value"
      fi
    done < "$envfile"
  done
fi

RPC="${SOLANA_RPC_URL:-}"

echo
print_config

# ═══════════════════════════════════════════════════════════════
#                    Simulation Mode
# ═══════════════════════════════════════════════════════════════

if [[ "$SIMULATE" == "true" ]]; then
  echo -e "${YELLOW}${BOLD}━━━ SIMULATION MODE ━━━${RESET}"
  echo -e "${DIM}No transactions will be submitted to the blockchain.${RESET}"
  echo
  
  # Build with spinner (suppress warnings)
  print_step "Building release binary..."
  if spin "Compiled send_privately example" cargo build --release --example send_privately 2>&1; then
    echo
    print_success "Build successful!"
    echo
    echo -e "${WHITE}${BOLD}Simulation Summary:${RESET}"
    echo -e "  ${DIM}Would send:${RESET}    ${GREEN}$AMOUNT${RESET} ${YELLOW}$TOKEN${RESET}"
    echo -e "  ${DIM}To:${RESET}            ${WHITE}$RECIPIENT${RESET}"
    echo
    echo -e "${CYAN}Command that would run:${RESET}"
    echo -e "  ${DIM}SOLANA_PRIVATE_KEY=\"***\" cargo run --release --example send_privately -- \"$AMOUNT\" \"$TOKEN\" \"$RECIPIENT\"${RESET}"
    echo
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${RESET}"
    echo -e "${GREEN}║${RESET}              ${BOLD}${GREEN}✓ SIMULATION COMPLETE${RESET}                          ${GREEN}║${RESET}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${RESET}"
    echo
    print_info "Run without --simulate to execute the real transaction."
  else
    echo
    print_error "Build failed. Check the output above."
    exit 1
  fi
  exit 0
fi

# ═══════════════════════════════════════════════════════════════
#                    Live Mode
# ═══════════════════════════════════════════════════════════════

echo -e "${RED}${BOLD}━━━ LIVE MODE ━━━${RESET}"
echo -e "${YELLOW}⚠ Real transactions will be submitted to the blockchain!${RESET}"
echo

if [[ -z "${SOLANA_PRIVATE_KEY:-}" ]]; then
  echo -e "${CYAN}Enter your Solana private key (base58, hidden):${RESET}"
  read -rsp "> " SOLANA_PRIVATE_KEY
  echo
  export SOLANA_PRIVATE_KEY
fi

print_success "Private key loaded"
echo

echo -e "${YELLOW}${BOLD}Confirm transaction:${RESET}"
echo -e "  ${DIM}Amount:${RESET}    ${GREEN}$AMOUNT${RESET} ${YELLOW}$TOKEN${RESET}"
echo -e "  ${DIM}Recipient:${RESET} ${WHITE}$RECIPIENT${RESET}"
echo
read -r -p "$(echo -e "${BOLD}Continue and submit transactions? ${DIM}(y/N)${RESET} ")" confirm
case "${confirm}" in
  y|Y|yes|YES) 
    echo
    print_success "Confirmed. Starting transaction..."
    echo
    ;;
  *) 
    echo
    print_warning "Aborted by user."
    exit 0 
    ;;
esac

# Run with warnings suppressed
exec env RUSTFLAGS="-Awarnings" cargo run --release --example send_privately -- "$AMOUNT" "$TOKEN" "$RECIPIENT"
