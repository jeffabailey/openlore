#!/usr/bin/env bash
# render-dids.sh -- render the indexer's DID list from SSM into its config directory. Runs ON THE
# HOST as root (instance role), as the pass unit's ExecStartPre, so every pass sees the latest
# value (ADR-081; devops/infrastructure-integration.md §4). Installed to
# /pds/indexer/bin/render-dids.sh.
#
#   1. read the Standard String /openlore/prod/indexer/repo-dids (30 s bound) into a staging FILE
#      in the same directory (same filesystem, so the rename is atomic);
#   2. on success with a non-blank value: root-owned (only when running as root), mode 0444,
#      rename the FILE over repo-dids and touch .rendered-at;
#   3. on any failure (SSM error, timeout, blank value, chown/chmod/mv error): remove the staging
#      file, keep the last good repo-dids byte-identical, log indexer.dids.render_failed;
#   4. exit 0 always: availability is the host's job, validity is the binary's.
#
# The container mounts the DIRECTORY read-only and pins its inode, so the directory itself is
# never swapped, truncated or deleted (H1). DIDs are never validated here, and the value is never
# printed. A stale list (.rendered-at older than 2 h) pages through the health line (alarm A3).
#
# Env (tests): INDEXER_CONFIG_DIR (default /pds/indexer/config). aws, timeout and chown come from
# PATH.
set -uo pipefail
set +x

PARAM="${INDEXER_DIDS_PARAM:-/openlore/prod/indexer/repo-dids}"
CONFIG_DIR="${INDEXER_CONFIG_DIR:-/pds/indexer/config}"
REGION="${INDEXER_REGION:-us-east-1}"
LOG_GROUP=/openlore/prod/indexer
LOG_STREAM=host-dids

LIST="$CONFIG_DIR/repo-dids"
STAGING="$CONFIG_DIR/.repo-dids.new"

cleanup() { rm -f "$STAGING"; }
trap cleanup EXIT

ship() { # $1 = the JSON line; best effort, never fails the script
  local line="$1" msg ms events
  msg=${line//\\/\\\\}
  msg=${msg//\"/\\\"}
  ms=$(date +%s)000
  events="[{\"timestamp\":$ms,\"message\":\"$msg\"}]"
  if ! timeout 10 aws logs put-log-events --region "$REGION" --log-group-name "$LOG_GROUP" \
    --log-stream-name "$LOG_STREAM" --log-events "$events" >/dev/null 2>&1; then
    timeout 10 aws logs create-log-stream --region "$REGION" --log-group-name "$LOG_GROUP" \
      --log-stream-name "$LOG_STREAM" >/dev/null 2>&1 || true
    timeout 10 aws logs put-log-events --region "$REGION" --log-group-name "$LOG_GROUP" \
      --log-stream-name "$LOG_STREAM" --log-events "$events" >/dev/null 2>&1 ||
      echo "render-dids: could not ship the render_failed line" >&2
  fi
}

render_failed() { # $1 = closed-enum cause
  local line
  rm -f "$STAGING"
  line=$(printf '{"event":"indexer.dids.render_failed","ts":"%s","cause":"%s"}' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$1")
  echo "$line"
  ship "$line"
  exit 0
}

read_parameter() { # writes the value to $STAGING; prints the failure cause on error
  local status=0
  timeout 30 aws ssm get-parameter --region "$REGION" --name "$PARAM" \
    --query Parameter.Value --output text >"$STAGING" 2>/dev/null || status=$?
  case "$status" in
    0) return 0 ;;
    124) echo timeout ;;
    *) echo ssm_error ;;
  esac
  return 1
}

if ! cause=$(read_parameter); then
  render_failed "${cause:-ssm_error}"
fi

if ! grep -q '[^[:space:]]' "$STAGING" 2>/dev/null; then
  render_failed empty
fi

if [ "${EUID}" -eq 0 ]; then
  chown root:root "$STAGING" 2>/dev/null || render_failed chown_error
fi
chmod 0444 "$STAGING" 2>/dev/null || render_failed chmod_error

# rename(2): an atomic FILE replace; the directory inode never changes.
mv -f "$STAGING" "$LIST" 2>/dev/null || render_failed mv_error

touch "$CONFIG_DIR/.rendered-at" 2>/dev/null || true
echo "render-dids: rendered the DID list into $LIST"
