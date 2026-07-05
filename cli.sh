#!/usr/bin/env bash
#
# cli.sh — thin wrapper around the `openlore` CLI. Builds the binary if needed,
# applies the SAME data home + dev identity that run.sh uses (so CLI writes show
# up in the viewer), then forwards every argument to `openlore`.
#
# Usage:
#   ./cli.sh <verb> [args…]
#
# Examples:
#   ./cli.sh init --handle local-dev.openlore --app-password local-dev-password
#   ./cli.sh claim add --subject github:rust-lang/rust \
#       --predicate embodiesPhilosophy \
#       --object org.openlore.philosophy.memory-safety --confidence 0.85
#   ./cli.sh claim --help
#   ./cli.sh graph query --subject github:rust-lang/rust
#
# Environment (optional — same defaults as run.sh, so they share one store):
#   OPENLORE_HOME           Data/config root (default: ./.openlore-home).
#   OPENLORE_DID            Signing DID stub (default: did:plc:local-dev).
#   OPENLORE_KEY_SEED_HEX   Ed25519 seed hex (default: 64 zeros — dev key).
#   PROFILE                 Cargo profile: debug (default) or release.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

if [[ $# -eq 0 || "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  sed -n '2,26p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  [[ $# -eq 0 ]] && exit 2 || exit 0
fi

command -v cargo >/dev/null 2>&1 || {
  echo "cli.sh: 'cargo' not found. Install the Rust toolchain from https://rustup.rs" >&2
  exit 1
}

# Same data home + dev identity as run.sh, so `./cli.sh claim add …` writes to
# the store `./run.sh` serves.
export OPENLORE_HOME="${OPENLORE_HOME:-$REPO_ROOT/.openlore-home}"
export OPENLORE_DID="${OPENLORE_DID:-did:plc:local-dev}"
export OPENLORE_KEY_SEED_HEX="${OPENLORE_KEY_SEED_HEX:-$(printf '0%.0s' {1..64})}"
mkdir -p "$OPENLORE_HOME"

PROFILE="${PROFILE:-debug}"
PROFILE_FLAG=""; [[ "$PROFILE" == "release" ]] && PROFILE_FLAG="--release"

# Build quietly (a no-op when already up to date; surfaces warnings/errors).
cargo build -q $PROFILE_FLAG -p cli --bin openlore

exec "$REPO_ROOT/target/$PROFILE/openlore" "$@"
