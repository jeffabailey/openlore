#!/usr/bin/env bash
#
# review-app-preview.sh — a dev-only local preview of the hosted review app on
# the FAKE Bluesky / PDS / GitHub network. Nothing leaves this machine.
#
# Starts the fake ATProto directory + PDS (also the OAuth server; it approves
# at once), the fake GitHub accounts and the DEBUG review app on 127.0.0.1,
# prints the origin and the handle to type at sign-in, then blocks until
# Ctrl-C (or stdin EOF) and tears everything down.
#
# Usage:
#   ./review-app-preview.sh
#
# It runs the #[ignore]d launcher test:
#   cargo test -p openlore-review-app --test review_app_local_preview \
#     -- --ignored --nocapture --exact launch_local_preview
#
# The development-only REVIEW_APP_ALLOW_LOOPBACK_HTTP seam makes this work; a
# release build refuses it. The session cookie is `__Host-` + Secure: Chrome
# and Firefox keep it on http://127.0.0.1 (loopback is a secure context).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  # The leading comment block (after the shebang), up to the first other line.
  awk 'NR == 1 { next } !/^#/ { exit } { sub(/^# ?/, ""); print }' "${BASH_SOURCE[0]}"
  exit 0
fi

command -v cargo >/dev/null 2>&1 || {
  echo "review-app-preview.sh: 'cargo' not found. Install the Rust toolchain from https://rustup.rs" >&2
  exit 1
}

exec cargo test -q -p openlore-review-app --test review_app_local_preview \
  -- --ignored --nocapture --exact launch_local_preview
