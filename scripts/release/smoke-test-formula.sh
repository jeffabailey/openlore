#!/usr/bin/env bash
# Regression smoke-test for the Homebrew tap (feature homebrew-binary-distribution).
#
# Reproduces the exact user-facing install flow against the LIVE tap + current
# GitHub Release, and asserts a working `openlore` lands on PATH. Catches the two
# failure modes that shipped undocumented:
#   - the in-repo tap needs an explicit URL (Homebrew shorthand → homebrew-openlore)
#   - Homebrew 6.0.0+ ignores untrusted custom-remote taps until `brew trust <tap>`
#
# Runs locally and in CI (.github/workflows/formula-smoke.yml). Idempotent: untaps +
# uninstalls first and on exit, so re-runs are clean.
set -euo pipefail

TAP="jeffabailey/openlore"
TAP_URL="https://github.com/jeffabailey/openlore"
FORMULA="jeffabailey/openlore/openlore"

export HOMEBREW_NO_AUTO_UPDATE=1

cleanup() {
  brew uninstall openlore >/dev/null 2>&1 || true
  brew untap "$TAP" >/dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup  # start from a clean slate

echo "==> 1/4 tap (explicit URL — points at THIS repo, no homebrew-openlore repo)"
brew tap "$TAP" "$TAP_URL"

echo "==> 2/4 trust (Homebrew 6.0.0+ requires it for custom-remote taps; no-op on older)"
brew trust "$TAP" 2>/dev/null || echo "    (brew trust unavailable — older Homebrew, no trust needed)"

echo "==> 3/4 install"
brew install "$FORMULA"

echo "==> 4/4 assert 'openlore --version' reports a semver"
out="$(openlore --version)"
echo "    $out"
if ! printf '%s\n' "$out" | grep -qE '^openlore [0-9]+\.[0-9]+\.[0-9]+'; then
  echo "FAIL: unexpected 'openlore --version' output: '$out'" >&2
  exit 1
fi

echo "==> SMOKE TEST PASSED: brew tap → trust → install → $out"
