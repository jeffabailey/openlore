#!/usr/bin/env bash
#
# run.sh — build and launch the OpenLore read-only viewer (`openlore ui`) with
# every requirement satisfied: builds the binary, resolves a self-contained
# data home, runs the idempotent `init` bootstrap, and serves the viewer on
# 127.0.0.1. Optionally seeds a few demo claims and starts the network indexer
# so the `/search` surface has a backend.
#
# Usage:
#   ./run.sh [--port N] [--seed] [--with-indexer] [--release] [--help]
#
# Options:
#   --port N         Loopback port for the viewer (default: 8788).
#   --seed           Seed a few demo claims on first run so the store isn't empty.
#   --with-indexer   Also start `openlore-indexer serve` and point /search at it.
#   --release        Build in release mode (default: debug).
#   -h, --help       Show this help and exit.
#
# Environment (all optional — sensible dev defaults are applied if unset):
#   OPENLORE_HOME           Data/config root (default: ./.openlore-home in the repo).
#                           Set this to "$HOME" to use your real store instead.
#   OPENLORE_DID            Signing DID stub for `init` (default: did:plc:local-dev).
#   OPENLORE_KEY_SEED_HEX   Ed25519 seed hex for local signing (default: 64 zeros).
#
# The viewer binds 127.0.0.1 ONLY (loopback, never remote). Ctrl-C stops it (and
# the indexer, if started).

set -euo pipefail

# --- resolve repo root (this script's directory) ---------------------------
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

# --- defaults + flag parsing ------------------------------------------------
PORT=8788
PROFILE="debug"
CARGO_PROFILE_FLAG=""
SEED=0
WITH_INDEXER=0

usage() { sed -n '2,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port)         PORT="${2:?--port needs a value}"; shift 2 ;;
    --port=*)       PORT="${1#*=}"; shift ;;
    --seed)         SEED=1; shift ;;
    --with-indexer) WITH_INDEXER=1; shift ;;
    --release)      PROFILE="release"; CARGO_PROFILE_FLAG="--release"; shift ;;
    -h|--help)      usage; exit 0 ;;
    *) echo "run.sh: unknown option '$1' (try --help)" >&2; exit 2 ;;
  esac
done

# --- prerequisites ----------------------------------------------------------
command -v cargo >/dev/null 2>&1 || {
  echo "run.sh: 'cargo' not found. Install the Rust toolchain from https://rustup.rs" >&2
  exit 1
}

# --- environment (respect anything already exported) ------------------------
export OPENLORE_HOME="${OPENLORE_HOME:-$REPO_ROOT/.openlore-home}"
export OPENLORE_DID="${OPENLORE_DID:-did:plc:local-dev}"
# 64 hex zeros → a deterministic local dev key so `init`/`claim add` can sign
# without an OS keychain entry. NOT for real identities.
export OPENLORE_KEY_SEED_HEX="${OPENLORE_KEY_SEED_HEX:-$(printf '0%.0s' {1..64})}"

mkdir -p "$OPENLORE_HOME"

BIN="$REPO_ROOT/target/$PROFILE/openlore"
INDEXER_BIN="$REPO_ROOT/target/$PROFILE/openlore-indexer"

# --- build ------------------------------------------------------------------
echo "==> Building openlore ($PROFILE)…"
BUILD_TARGETS=(-p cli --bin openlore)
if [[ "$WITH_INDEXER" == "1" ]]; then
  BUILD_TARGETS+=(-p openlore-indexer --bin openlore-indexer)
fi
cargo build $CARGO_PROFILE_FLAG "${BUILD_TARGETS[@]}"

# --- bootstrap (idempotent: short-circuits if already initialized) ----------
echo "==> Initializing store at $OPENLORE_HOME (idempotent)…"
"$BIN" init --handle "local-dev.openlore" --app-password "local-dev-password"

# --- optional: seed a few demo claims on first run --------------------------
if [[ "$SEED" == "1" && ! -f "$OPENLORE_HOME/.seeded" ]]; then
  echo "==> Seeding demo claims…"
  seed_claim() {
    # A single newline on stdin confirms the "Press Enter to sign locally"
    # prompt; EOF afterward declines the publish prompt (local-only claim).
    printf '\n' | "$BIN" claim add \
      --subject "$1" --predicate "$2" --object "$3" \
      --confidence "$4" --evidence "$5" >/dev/null
  }
  seed_claim "github:rust-lang/rust" "embodiesPhilosophy" \
    "org.openlore.philosophy.memory-safety" "0.85" "https://github.com/rust-lang/rust"
  seed_claim "github:denoland/deno" "embodiesPhilosophy" \
    "org.openlore.philosophy.dependency-pinning" "0.70" "https://github.com/denoland/deno"
  touch "$OPENLORE_HOME/.seeded"
fi

# --- optional: start the network indexer for /search ------------------------
INDEXER_PID=""
cleanup() {
  if [[ -n "$INDEXER_PID" ]] && kill -0 "$INDEXER_PID" 2>/dev/null; then
    echo "==> Stopping indexer (pid $INDEXER_PID)…"
    kill "$INDEXER_PID" 2>/dev/null || true
    wait "$INDEXER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

if [[ "$WITH_INDEXER" == "1" ]]; then
  INDEXER_ADDR="127.0.0.1:8789"
  echo "==> Starting indexer on http://$INDEXER_ADDR …"
  OPENLORE_INDEXER_INDEX_PATH="$OPENLORE_HOME/indexer.duckdb" \
  OPENLORE_INDEXER_LISTEN_ADDR="$INDEXER_ADDR" \
    "$INDEXER_BIN" serve &
  INDEXER_PID=$!
  # Point the viewer's read-only /search at the indexer (unreachable → the
  # viewer degrades /search to "Unavailable"; it never blocks startup).
  export OPENLORE_INDEXER_URL="http://$INDEXER_ADDR"
  # Give the serve loop a moment to bind before the viewer soft-probes it.
  for _ in $(seq 1 20); do
    if nc -z 127.0.0.1 8789 2>/dev/null; then break; fi
    sleep 0.25
  done
fi

# --- launch the viewer (foreground; Ctrl-C to stop) -------------------------
echo ""
echo "==> OpenLore viewer starting on http://127.0.0.1:$PORT"
echo "    data home : $OPENLORE_HOME"
echo "    /search   : $([[ "$WITH_INDEXER" == "1" ]] && echo "backed by local indexer" || echo "Unavailable (run with --with-indexer to enable)")"
echo "    press Ctrl-C to stop"
echo ""
exec "$BIN" ui --port "$PORT"
