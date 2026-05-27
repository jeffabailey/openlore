#!/usr/bin/env bash
# check-probes.sh — friendly wrapper around `cargo xtask check-probes`.
#
# Invoked by:
#   - the pre-commit hook at .githooks/pre-commit
#   - CI workflows that want a one-liner with consistent output framing
#   - developers running it ad-hoc to sanity-check adapter changes
#
# Exits 0 on clean check, 1 on any probe violation, 2 on an internal
# error (e.g. workspace layout broken, syn parse failure). These match
# the exit codes documented in `xtask/src/check_probes.rs::run`.
#
# Step 06-06 (ADR-009 D-10 layer-(b) structural enforcement).

set -euo pipefail

# Locate the workspace root by walking up from this script's directory.
# Keeps the hook usable regardless of where `git` invokes it from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$WORKSPACE_ROOT"

echo "[check-probes] Running cargo xtask check-probes from $WORKSPACE_ROOT"

# --quiet suppresses cargo's compile chatter; the xtask itself still
# prints its own one-line OK/violation summary to stderr.
if cargo run --quiet -p xtask --bin xtask -- check-probes; then
    echo "[check-probes] OK"
    exit 0
else
    rc=$?
    echo "[check-probes] FAILED (exit $rc) — see violations above" >&2
    echo "[check-probes] One or more adapter probe() bodies are stubs." >&2
    echo "[check-probes] Earned Trust (ADR-002 §self-application) requires" >&2
    echo "[check-probes] each adapter to demonstrate readiness, not assert it." >&2
    exit "$rc"
fi
