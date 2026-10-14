#!/usr/bin/env bash
# Release notes for a tag (release.yml `release` job): the annotated tag's message leads,
# then how to verify a download, then GitHub's generated change list.
#
#   scripts/release/release-notes.sh <tag> <repo owner/name>   # writes the notes to stdout
#
# The change list is generated against the previous release tag that still exists, so its
# "Full Changelog" link never points at a deleted pre-release tag: a GA release compares with
# the last GA tag, a pre-release with the last tag of any kind. Needs the tags fetched and `gh`
# authenticated (GH_TOKEN).
set -euo pipefail

tag="${1:?usage: release-notes.sh <tag> <owner/repo>}"
repo="${2:?usage: release-notes.sh <tag> <owner/repo>}"

# The previous release tag that exists: versions sorted newest first, the one after <tag>.
previous_tag() {
  local tags
  if [[ "$tag" == *-* ]]; then
    tags="$(git -c versionsort.suffix=- tag --list 'v*' --sort=-v:refname)"
  else
    tags="$(git -c versionsort.suffix=- tag --list 'v*' --sort=-v:refname | grep -v -- '-' || true)"
  fi
  printf '%s\n' "$tags" | awk -v t="$tag" 'found { print; exit } $0 == t { found = 1 }'
}

# The annotated tag's message, without a signature block (a lightweight tag gives the
# commit message instead).
git tag --list --format='%(contents)' "$tag" | sed '/^-----BEGIN PGP SIGNATURE-----$/,$d'

sed "s|@REPO@|$repo|g" <<'EOF'

## Verify a download (optional, recommended)
```sh
# 1. checksum
shasum -a 256 -c openlore-*-<triple>.tar.gz.sha256

# 2. cosign keyless signature (anchored to this workflow's OIDC identity)
cosign verify-blob \
  --certificate openlore-*-<triple>.tar.gz.cert \
  --signature   openlore-*-<triple>.tar.gz.sig \
  --certificate-identity-regexp 'https://github.com/@REPO@/.github/workflows/release.yml@.*' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  openlore-*-<triple>.tar.gz

# 3. review the dependency SBOM (the `openlore` binary's crates)
#    cyclonedx analyze --input-file sbom.cdx.json
```

EOF

previous="$(previous_tag)"
args=(-f "tag_name=$tag")
if [ -n "$previous" ]; then
  args+=(-f "previous_tag_name=$previous")
fi
gh api "repos/$repo/releases/generate-notes" "${args[@]}" --jq .body
