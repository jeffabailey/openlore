#!/usr/bin/env bash
# Operator wrapper for the two app deploy scripts: sets up the laptop environment they need and
# runs one of them with the given arguments.
#
#   deploy/ops.sh review-app <mode> [args...]   # runs deploy/review-app/deploy.sh
#   deploy/ops.sh indexer <mode> [args...]      # runs deploy/indexer/deploy.sh
#
# The setup it does (found during go-live on 2026-10-11):
# - AWS: the openlore account in us-east-1, whatever the shell's default profile and region are.
#   Override with OPENLORE_AWS_PROFILE / OPENLORE_AWS_REGION.
# - GHCR: crane and cosign read through an empty, throwaway DOCKER_CONFIG. Both images are public
#   (go-live step 8), so anonymous reads work, and they never touch the macOS keychain credential
#   helper, which fails with "the current session does not allow user interaction" when locked.
# Every other variable (INDEXER_READY_WAIT_S, ...) passes through unchanged.
set -euo pipefail

usage() {
  echo "usage: deploy/ops.sh review-app|indexer <mode> [args...]" >&2
  exit 2
}

[ $# -ge 2 ] || usage
app=$1
shift
case "$app" in
  review-app | indexer) ;;
  *) usage ;;
esac

export AWS_PROFILE="${OPENLORE_AWS_PROFILE:-jeff}"
export AWS_REGION="${OPENLORE_AWS_REGION:-us-east-1}"
DOCKER_CONFIG=$(mktemp -d)
export DOCKER_CONFIG
trap 'rm -rf "$DOCKER_CONFIG"' EXIT

"$(cd "$(dirname "$0")" && pwd)/$app/deploy.sh" "$@"
