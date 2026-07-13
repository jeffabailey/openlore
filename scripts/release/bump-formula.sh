#!/usr/bin/env bash
# Render Formula/openlore.rb from Formula/openlore.rb.tmpl for a release.
#
# Feature homebrew-binary-distribution, slice-02 (OD-HB-2). Invoked by the
# `bump-formula` job in .github/workflows/release.yml after the release job has
# published the tarballs + .sha256 companions, so the tap formula tracks each
# release with a verified checksum. Also runnable by hand from the repo root.
#
# Usage: scripts/release/bump-formula.sh <version> <sha_dir>
#   <version>  release tag without a leading 'v' (e.g. 0.1.0, or 0.1.0-rc6)
#   <sha_dir>  directory holding the 4 openlore-<version>-<triple>.tar.gz.sha256 files
#              (as produced by release.yml / `gh release download --pattern '*.sha256'`)
set -euo pipefail

VERSION="${1:?usage: bump-formula.sh <version> <sha_dir>}"
SHA_DIR="${2:?usage: bump-formula.sh <version> <sha_dir>}"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMPL="$ROOT/Formula/openlore.rb.tmpl"
OUT="$ROOT/Formula/openlore.rb"

# Read the checksum (first field) from a triple's .sha256 file.
read_sha() {
  local triple="$1"
  local file="$SHA_DIR/openlore-${VERSION}-${triple}.tar.gz.sha256"
  [ -f "$file" ] || { echo "ERROR: missing checksum file $file" >&2; exit 1; }
  awk '{print $1; exit}' "$file"
}

SHA_AARCH64_APPLE="$(read_sha aarch64-apple-darwin)"
SHA_X86_64_APPLE="$(read_sha x86_64-apple-darwin)"
SHA_AARCH64_LINUX="$(read_sha aarch64-unknown-linux-gnu)"
SHA_X86_64_LINUX="$(read_sha x86_64-unknown-linux-gnu)"

sed \
  -e "s|__VERSION__|${VERSION}|g" \
  -e "s|__SHA_AARCH64_APPLE__|${SHA_AARCH64_APPLE}|g" \
  -e "s|__SHA_X86_64_APPLE__|${SHA_X86_64_APPLE}|g" \
  -e "s|__SHA_AARCH64_LINUX__|${SHA_AARCH64_LINUX}|g" \
  -e "s|__SHA_X86_64_LINUX__|${SHA_X86_64_LINUX}|g" \
  "$TMPL" > "$OUT"

# Fail loudly if any placeholder survived (typo / new placeholder).
if grep -q "__[A-Z0-9_]*__" "$OUT"; then
  echo "ERROR: unsubstituted placeholder(s) in $OUT:" >&2
  grep -o "__[A-Z0-9_]*__" "$OUT" | sort -u >&2
  exit 1
fi

echo "Rendered Formula/openlore.rb for ${VERSION}"
