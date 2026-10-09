#!/usr/bin/env bash
# deploy.sh -- deploy the OpenLore indexer to the PDS host (ADR-075, ADR-080..083;
# docs/feature/indexer-deployment/devops/infrastructure-integration.md §6). Run from the
# operator's laptop; CI never deploys.
#
#   deploy.sh install                    host files, systemd units, Caddy site (no container change)
#   deploy.sh deploy <git-sha|digest>    verify CI + cosign, then Recreate-deploy that digest
#   deploy.sh redeploy                   install + recreate the current digest (after a replacement)
#   deploy.sh rollback [--reset-index]   redeploy the previous digest from the host's releases
#   deploy.sh stop | start               stop/start the pass timer and the container
#   deploy.sh trigger                    run one pass now and print the unit result
#   deploy.sh status                     freshness, exit codes, skips, last health line (Logs only)
#   deploy.sh host-status                container, timers, unit results, logs, releases (over SSM)
#
# DIGEST ONLY. <git-sha> must be the full 40-hex commit; it is resolved to the digest CI pushed
# for it, and from then on only `image@sha256:...` is used. A `sha256:<64 hex>` digest is also
# accepted; its signed org.opencontainers.image.revision label names the commit whose CI must be
# green. Anything else -- a tag, a branch, a short sha -- is refused before any tool runs.
#
# Rollback is designed first: the host keeps an append-only releases file
# (`<utc> <sha> <digest> ready_s=<n>`). If the new digest does not answer /healthz through Caddy
# within 60 s (INDEXER_READY_WAIT_S=<s> on the laptop overrides it: the FIRST deploy also waits
# for Caddy to issue index.'s certificate, so give it longer, e.g. 300), the host restarts the previous digest by itself and this exits 1. There is no
# data restore: the index is a rebuildable cache with no schema change between releases;
# `rollback --reset-index` moves it aside and lets the next passes rebuild it.
#
# Laptop needs: AWS credentials (e.g. AWS_PROFILE=jeff), gh (signed in), cosign, jq, and crane
# or docker buildx. The same file runs on the host (as `deploy.sh host <mode>`) via SSM Run
# Command, which carries only these repo files and the digest -- never a secret.
set -euo pipefail

IMAGE=ghcr.io/jeffabailey/openlore-indexer
REGION=us-east-1
INDEX_HOST=index.openlore.jeffbailey.us
PDS_URL=https://openlore.jeffbailey.us
APP_URL=https://app.openlore.jeffbailey.us
LOG_GROUP=/openlore/prod/indexer
SEARCH_PATH=/xrpc/org.openlore.appview.searchClaims
CERT_IDENTITY_RE='^https://github.com/jeffabailey/openlore/.github/workflows/ci.yml@refs/heads/main$'
OIDC_ISSUER=https://token.actions.githubusercontent.com

CLEANUP_PATHS=()
POLL_PID=""

cleanup() {
  if [ -n "$POLL_PID" ]; then kill "$POLL_PID" 2>/dev/null || true; fi
  if [ "${#CLEANUP_PATHS[@]}" -gt 0 ]; then rm -rf "${CLEANUP_PATHS[@]}"; fi
}
trap cleanup EXIT

die() {
  echo "deploy.sh: $*" >&2
  exit 2
}

is_digest() { [[ "$1" =~ ^sha256:[0-9a-f]{64}$ ]]; }
is_git_sha() { [[ "$1" =~ ^[0-9a-f]{40}$ ]]; }
is_wait() { [[ "$1" =~ ^[0-9]+$ ]] && [ "$1" -ge 2 ]; }

# --------------------------------------------------------------------------------------------
# Laptop side
# --------------------------------------------------------------------------------------------

need() {
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || die "missing tool: $tool"
  done
}

resolve_digest() { # $1 = git sha -> prints the digest CI pushed as sha-<sha>
  local ref="$IMAGE:sha-$1" digest
  if command -v crane >/dev/null 2>&1; then
    digest=$(crane digest "$ref")
  else
    digest=$(docker buildx imagetools inspect "$ref" --format '{{json .Manifest}}' | jq -r .digest)
  fi
  is_digest "$digest" || die "could not resolve a digest for $ref (got '$digest')"
  echo "$digest"
}

image_revision() { # $1 = digest -> the commit in the signed image's org.opencontainers.image.revision
  local ref="$IMAGE@$1" config rev
  if command -v crane >/dev/null 2>&1; then
    config=$(crane config "$ref")
  else
    config=$(docker buildx imagetools inspect "$ref" --format '{{json .Image}}')
  fi
  rev=$(jq -r '(.config.Labels // {})["org.opencontainers.image.revision"] // empty' <<<"$config" 2>/dev/null || true)
  is_git_sha "$rev" || die "refusing $ref: no 40-hex org.opencontainers.image.revision label (got '$rev')"
  echo "$rev"
}

ci_green() { # $1 = git sha
  local conclusion
  conclusion=$(gh run list --repo jeffabailey/openlore --workflow ci.yml --commit "$1" \
    --branch main --json conclusion --jq '.[0].conclusion // "none"')
  [ "$conclusion" = "success" ] || die "ci.yml on main for $1 is '$conclusion', not success"
}

verify_signature() { # $1 = digest
  echo "verifying the signature on $IMAGE@$1"
  cosign verify "$IMAGE@$1" \
    --certificate-identity-regexp "$CERT_IDENTITY_RE" \
    --certificate-oidc-issuer "$OIDC_ISSUER" >/dev/null ||
    die "refusing $IMAGE@$1: no valid CI signature"
}

instance_id() {
  if [ -n "${INDEXER_INSTANCE_ID:-}" ]; then
    echo "$INDEXER_INSTANCE_ID"
  else
    tofu -chdir="$(dirname "$0")/../tofu/environments/prod" output -raw instance_id
  fi
}

remote() { # $@ = host mode and args; ships host/ + this script, runs it as root over SSM
  need aws jq tar base64
  local here payload b64 iid params cmd_id status
  here=$(cd "$(dirname "$0")" && pwd)
  payload=$(mktemp -d)
  CLEANUP_PATHS+=("$payload")
  cp "$here"/host/compose.yaml "$here"/host/index.caddy "$here"/host/*.sh \
    "$here"/host/openlore-indexer-*.service "$here"/host/openlore-indexer-*.timer \
    "$here/deploy.sh" "$payload/"
  b64=$(COPYFILE_DISABLE=1 tar -C "$payload" -cz . | base64 | tr -d '\n')
  iid=$(instance_id)
  params=$(jq -n --arg b64 "$b64" --arg args "$*" '{
    commands: [
      "set -eu",
      "d=$(mktemp -d)",
      "trap '\''rm -rf \"$d\"'\'' EXIT",
      ("echo " + $b64 + " | base64 -d | tar -xz -C \"$d\""),
      ("bash \"$d/deploy.sh\" host " + $args)
    ],
    executionTimeout: ["2400"]
  }')
  cmd_id=$(aws ssm send-command --region "$REGION" --instance-ids "$iid" \
    --document-name AWS-RunShellScript --comment "openlore-indexer $1" \
    --parameters "$params" --query Command.CommandId --output text)
  echo "SSM command $cmd_id on $iid ($*)"
  while :; do
    sleep 5
    status=$(aws ssm get-command-invocation --region "$REGION" --command-id "$cmd_id" \
      --instance-id "$iid" --query Status --output text 2>/dev/null || echo Pending)
    case "$status" in Pending | InProgress | Delayed) continue ;; *) ;; esac
    break
  done
  aws ssm get-command-invocation --region "$REGION" --command-id "$cmd_id" --instance-id "$iid" \
    --query '[StandardOutputContent,StandardErrorContent]' --output text
  if [ "$status" != "Success" ]; then
    echo "deploy.sh: host command ended $status" >&2
    exit 1
  fi
}

http_status() { # $@ = curl args; prints the HTTP status (000 when unreachable)
  curl -sS -o /dev/null -m 10 -w '%{http_code}' "$@" 2>/dev/null || true
}

search_body() { # $1 = total body size in bytes; a valid search whose value pads it out
  local prefix='{"dimension":"subject","value":"' suffix='"}' pad
  pad=$(($1 - ${#prefix} - ${#suffix}))
  printf '%s%s%s' "$prefix" "$(head -c "$pad" /dev/zero | tr '\0' 'a')" "$suffix"
}

pre_checks() {
  local code
  code=$(http_status "$PDS_URL/xrpc/_health")
  [ "$code" = 200 ] || die "the PDS _health answers $code; not deploying beside an unhealthy PDS"
  code=$(http_status "$APP_URL/healthz")
  case "$code" in
    200) echo "pre-checks passed: PDS _health 200, review app /healthz 200" ;;
    000 | 404 | 503) echo "pre-checks passed: PDS _health 200 (review app not deployed: $code)" ;;
    *) die "the review app /healthz answers $code" ;;
  esac
}

start_pds_poller() { # samples the PDS _health every 2 s for the whole deploy (KPI-IXD-6)
  local log
  log=$(mktemp)
  CLEANUP_PATHS+=("$log")
  PDS_POLL_LOG="$log"
  (
    while :; do
      if curl -fsS -o /dev/null -m 5 "$PDS_URL/xrpc/_health" 2>/dev/null; then
        echo ok >>"$log"
      else
        echo fail >>"$log"
      fi
      sleep 2
    done
  ) &
  POLL_PID=$!
}

stop_pds_poller() {
  local total ok
  if [ -n "$POLL_PID" ]; then kill "$POLL_PID" 2>/dev/null || true; fi
  POLL_PID=""
  total=$(wc -l <"$PDS_POLL_LOG" | tr -d ' ')
  ok=$(grep -c '^ok$' "$PDS_POLL_LOG" || true)
  echo "PDS _health: $ok/$total ok during deploy"
}

post_checks() {
  local base="https://$INDEX_HOST" body code tmp
  body=$(curl -fsS -m 10 "$base/healthz") || die "post-check: GET /healthz failed"
  [[ "${body//[[:space:]]/}" == *'"status":"ok"'* ]] || die "post-check: /healthz is not ok"
  tmp=$(mktemp)
  CLEANUP_PATHS+=("$tmp")
  search_body 120 >"$tmp"
  curl -fsS -m 10 -X POST -H 'content-type: application/json' --data-binary "@$tmp" \
    "$base$SEARCH_PATH" | jq -e '.results | type == "array"' >/dev/null ||
    die "post-check: a minimal POST search did not return 200 JSON"
  code=$(http_status "$base/xrpc/com.atproto.repo.createRecord")
  [ "$code" = 404 ] || die "post-check: GET createRecord answered $code, not 404"
  code=$(http_status -X POST "$base/healthz")
  [ "$code" = 404 ] || die "post-check: POST /healthz answered $code, not 404"
  search_body 8000 >"$tmp"
  code=$(http_status -X POST -H 'content-type: application/json' --data-binary "@$tmp" "$base$SEARCH_PATH")
  [ "$code" != 413 ] || die "post-check: an 8 KB body was refused with 413"
  search_body 9000 >"$tmp"
  code=$(http_status -X POST -H 'content-type: application/json' --data-binary "@$tmp" "$base$SEARCH_PATH")
  [ "$code" = 413 ] || die "post-check: a 9 KB body answered $code, not 413"
  code=$(http_status "$PDS_URL/xrpc/_health")
  [ "$code" = 200 ] || die "post-check: the PDS _health answers $code"
  echo "post-checks passed: healthz, search, createRecord 404, POST /healthz 404, 8 KB ok, 9 KB 413, PDS _health"
}

laptop_deploy() { # $1 = git sha or digest
  local ref="${1:-}" sha=unknown digest wait_s="${INDEXER_READY_WAIT_S:-60}"
  is_wait "$wait_s" || die "refusing INDEXER_READY_WAIT_S='$wait_s': whole seconds, at least 2"
  if is_digest "$ref"; then
    digest="$ref"
  elif is_git_sha "$ref"; then
    sha="$ref"
  else
    die "refusing '$ref': deploy by full 40-hex git sha or sha256:<64 hex> digest only (no tags)"
  fi
  need gh cosign jq
  if [ "$sha" != unknown ]; then
    ci_green "$sha"
    digest=$(resolve_digest "$sha")
    verify_signature "$digest"
  else # a digest: once its signature verifies, its revision label is CI's; that commit must be green
    verify_signature "$digest"
    sha=$(image_revision "$digest")
    ci_green "$sha"
  fi
  pre_checks
  start_pds_poller
  remote deploy "$digest" "$sha" "$wait_s"
  post_checks
  stop_pds_poller
  echo "deployed $IMAGE@$digest"
}

# Logs Insights over the shipped log group: no host shell, no claim content (AC-005.1..4).
insights_start() { # $1 = window seconds, $2 = query -> prints the query id
  local now
  now=$(date +%s)
  aws logs start-query --region "$REGION" --log-group-name "$LOG_GROUP" \
    --start-time "$((now - $1))" --end-time "$now" --query-string "$2" \
    --query queryId --output text
}

insights_wait() { # $1 = query id -> prints the rows as JSON objects, one per line
  local out i
  for i in $(seq 1 20); do
    out=$(aws logs get-query-results --region "$REGION" --query-id "$1" --output json)
    case "$(jq -r .status <<<"$out")" in
      Complete)
        jq -c '.results[] | map({(.field): .value}) | add' <<<"$out"
        return 0
        ;;
      Failed | Cancelled | Timeout) die "query $1 ended without results" ;;
      *) sleep "$((i < 5 ? 1 : 2))" ;;
    esac
  done
  die "query $1 did not finish"
}

age_of() { # $1 = Logs Insights @timestamp ("2026-10-07 07:03:41.000") -> "<n> min ago"
  local epoch
  epoch=$(jq -rn --arg t "$1" '$t[0:19] | sub(" "; "T") + "Z" | fromdateiso8601')
  echo "$((($(date +%s) - epoch) / 60)) min ago"
}

laptop_status() {
  need aws jq
  local summary="filter event = \"indexer.ingest.pass_summary\" and pass_id not like /^TEST/"
  local q_ok q_recent q_health q_config ok recent health config pass_id ts q_skips
  q_ok=$(insights_start 172800 "$summary and exit_code = 0 | fields @timestamp, pass_id, configured, own_pds, fallback, skipped, purged_authors, duration_ms | sort @timestamp desc | limit 1")
  q_recent=$(insights_start 172800 "$summary | fields @timestamp, exit_code, cause, pass_id | sort @timestamp desc | limit 8")
  q_health=$(insights_start 3600 'filter event = "indexer.host.health" | fields @message | sort @timestamp desc | limit 1')
  q_config=$(insights_start 172800 'filter event = "indexer.config.loaded" | fields @timestamp, repo_did_count, repo_dids_age_secs | sort @timestamp desc | limit 1')
  ok=$(insights_wait "$q_ok")
  recent=$(insights_wait "$q_recent")
  health=$(insights_wait "$q_health")
  config=$(insights_wait "$q_config")

  echo "--- last successful pass (exit 0)"
  if [ -n "$ok" ]; then
    ts=$(jq -r '."@timestamp"' <<<"$ok")
    echo "$ts UTC ($(age_of "$ts"))"
    jq -r 'del(."@timestamp", ."@ptr") | to_entries[] | "  \(.key) = \(.value)"' <<<"$ok"
    pass_id=$(jq -r .pass_id <<<"$ok")
  else
    echo "(none in the last 48 h)"
    pass_id=""
  fi
  echo "--- last 8 passes (newest first)"
  if [ -n "$recent" ]; then
    ts=$(head -n1 <<<"$recent" | jq -r '."@timestamp"')
    echo "last pass of any kind: $ts UTC ($(age_of "$ts"))"
    jq -r '"  \(."@timestamp")  exit \(.exit_code)  \(.cause // "")  \(.pass_id)"' <<<"$recent"
  else
    echo "(none in the last 48 h)"
  fi
  if [ -n "$pass_id" ]; then
    echo "--- skips and purges in $pass_id"
    q_skips=$(insights_start 172800 "filter event in [\"indexer.ingest.source_skipped\", \"indexer.ingest.author_purged\", \"indexer.ingest.pass_refused\"] and pass_id = \"$pass_id\" | fields event, did, reason, cause, claims_removed")
    insights_wait "$q_skips" | jq -r '"  \(.event)  \(.did // "")  \(.reason // .cause // "")  \(.claims_removed // "")"'
  fi
  echo "--- DID list as last loaded"
  if [ -n "$config" ]; then jq -r '"  repo_did_count = \(.repo_did_count)  repo_dids_age_secs = \(.repo_dids_age_secs)"' <<<"$config"; else echo "  (no config load in 48 h)"; fi
  echo "--- latest indexer.host.health"
  if [ -n "$health" ]; then jq -r '."@message"' <<<"$health"; else echo "(none in the last hour: A3 treats this as not live)"; fi
}

# --------------------------------------------------------------------------------------------
# Host side (root, via SSM). Paths per infrastructure-integration.md §1.
# --------------------------------------------------------------------------------------------

BASE="${INDEXER_BASE_DIR:-/pds/indexer}"
DATA=$BASE/data
CONFIG=$BASE/config
STATE=$BASE/state
RELEASES=$STATE/releases
UNITS=/etc/systemd/system
SITES="${INDEXER_CADDY_SITES_DIR:-/pds/caddy/sites}"
compose() { docker compose -f "$BASE/compose.yaml" "$@"; }

# >>> shared isolation contract: byte-identical in deploy/indexer/deploy.sh and
# deploy/review-app/deploy.sh (xtask both_apps_refuse_with_the_same_isolation_contract).
# Needs SITES (the host dir the PDS Caddy serves as /etc/caddy/sites) and die.
# curlimages/curl:8.10.1, multi-arch index digest (read from the registry with `crane digest`).
IMDS_PROBE_IMAGE="${IMDS_PROBE_IMAGE:-curlimages/curl@sha256:d9b4541e214bcd85196d6e92e2753ac6d0ea699f0af5741f8c6cccbfcf00ef4b}"
CADDY_SITES=/etc/caddy/sites
CADDYFILE=/etc/caddy/Caddyfile
NEEDS_MODULE="apply tofu-aws-pds v1.7.0 (R-REPLACE) first"

pds_compose() { docker compose -f /pds/compose.yaml "$@"; }
pds_caddy() { pds_compose exec -T caddy caddy "$@"; }

imds_probe() { # $@ = curl args, run by curl in the pinned probe image on the PDS network
  docker run --rm --network pds_default "$IMDS_PROBE_IMAGE" "$@" >/dev/null 2>&1
}

# A site file in $SITES is only served if the PDS Caddy bind-mounts THAT directory at
# /etc/caddy/sites and its Caddyfile imports it on a live (uncommented) line.
refuse_unless_caddy_imports_sites() {
  local caddy mounts caddyfile
  caddy=$(pds_compose ps -q caddy 2>/dev/null) || caddy=""
  [ -n "$caddy" ] || die "refusing: the PDS Caddy container is not running; $NEEDS_MODULE"
  mounts=$(docker inspect --format '{{range .Mounts}}{{.Source}}:{{.Destination}}{{println}}{{end}}' "$caddy" 2>/dev/null) ||
    die "refusing: cannot inspect the PDS Caddy container's mounts; $NEEDS_MODULE"
  grep -qxF "$SITES:$CADDY_SITES" <<<"$mounts" ||
    die "refusing: the PDS Caddy does not mount $SITES at $CADDY_SITES; $NEEDS_MODULE"
  caddyfile=$(pds_compose exec -T caddy cat "$CADDYFILE" 2>/dev/null) ||
    die "refusing: cannot read $CADDYFILE in the PDS Caddy container; $NEEDS_MODULE"
  grep -qE '^[[:space:]]*import[[:space:]]+/etc/caddy/sites/\*\.caddy' <<<"$caddyfile" ||
    die "refusing: $CADDYFILE does not import $CADDY_SITES/*.caddy; $NEEDS_MODULE"
}

# §8: never start a public container that could reach the host role. Fails CLOSED: only a
# probe that provably ran curl on pds_default and then could not connect (7) or timed out (28)
# proves isolation; an answer (0, or 22 for an HTTP error) or any other outcome refuses.
# Runs before any host file is written.
refuse_unless_isolated() {
  local rc=0
  [ -d "$SITES" ] || die "refusing: $SITES does not exist; $NEEDS_MODULE"
  [[ "$IMDS_PROBE_IMAGE" =~ @sha256:[0-9a-f]{64}$ ]] ||
    die "refusing: IMDS_PROBE_IMAGE '$IMDS_PROBE_IMAGE' is not pinned by digest"
  refuse_unless_caddy_imports_sites
  imds_probe --version ||
    die "refusing: the IMDS probe cannot run curl on pds_default (positive control failed); isolation unproven"
  imds_probe -sS -m 3 -X PUT http://169.254.169.254/latest/api/token \
    -H 'X-aws-ec2-metadata-token-ttl-seconds: 60' || rc=$?
  case "$rc" in
    7 | 28) echo "IMDS unreachable from pds_default (curl exit $rc)" ;;
    0 | 22) die "refusing: a container can reach IMDS (hop limit is not 1; run R-REPLACE first)" ;;
    *) die "refusing: the IMDS probe was inconclusive (exit $rc); isolation unproven" ;;
  esac
}

# Places one app's site file, then reloads Caddy only once the adapted config serves the
# app's host and the whole Caddyfile validates; otherwise removes the file and refuses.
install_caddy_site() { # $1 = site file, $2 = the host it serves
  local site adapted
  site="$SITES/$(basename "$1")"
  install -m 0644 -o root -g root "$1" "$site"
  adapted=$(pds_caddy adapt --config "$CADDYFILE" 2>/dev/null) || adapted=""
  if [[ "$adapted" != *"\"$2\""* ]]; then
    rm -f "$site"
    die "refusing: Caddy does not serve $2 with $site in place; removed it, the PDS sites are unchanged"
  fi
  if ! pds_caddy validate --config "$CADDYFILE"; then
    rm -f "$site"
    die "refusing: Caddy rejected $site; removed it, the PDS sites are unchanged"
  fi
  pds_caddy reload --config "$CADDYFILE"
}
# <<< shared isolation contract

host_install() {
  local src
  src=$(cd "$(dirname "$0")" && pwd)
  refuse_unless_isolated
  install -d -m 0755 -o root -g root "$BASE" "$BASE/bin" "$CONFIG"
  install -d -m 0700 -o 65532 -g 65532 "$DATA"
  install -d -m 0700 -o root -g root "$STATE"
  install -m 0644 -o root -g root "$src/compose.yaml" "$BASE/compose.yaml"
  install -m 0755 -o root -g root "$src/render-dids.sh" "$src/health-timer.sh" "$BASE/bin/"
  install -m 0644 -o root -g root "$src"/openlore-indexer-*.service "$src"/openlore-indexer-*.timer "$UNITS/"
  systemctl daemon-reload
  systemctl enable --now openlore-indexer-health.timer
  install_caddy_site "$src/index.caddy" "$INDEX_HOST"
  echo "host files installed"
}

wait_ready() { # $1 = seconds to wait (default 60); prints the seconds taken once /healthz is ok through Caddy
  local i body
  for i in $(seq 1 $((${1:-60} / 2))); do
    body=$(curl -fsS -m 5 --resolve "$INDEX_HOST:443:127.0.0.1" "https://$INDEX_HOST/healthz" 2>/dev/null || true)
    if [[ "${body//[[:space:]]/}" == *'"status":"ok"'* ]]; then
      echo "$((i * 2))"
      return 0
    fi
    sleep 2
  done
  return 1
}

start_digest() { # $1 = digest
  is_digest "$1" || die "refusing to start non-digest '$1'"
  printf 'INDEXER_DIGEST=%s\n' "$1" >"$BASE/.env"
  chmod 0644 "$BASE/.env"
  compose up -d --force-recreate indexer
}

stop_indexer() {
  if [ -f "$BASE/.env" ]; then
    compose stop indexer || true
  fi
}

releases_digest() { # $1 = 1 for the current release, 2 for the one before it (field 3 = digest)
  [ -f "$RELEASES" ] || return 0
  tail -n "$1" "$RELEASES" | head -n1 | awk '{print $3}'
}

record_release() { # $@ = the fields after the timestamp
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $*" >>"$RELEASES"
  chmod 0600 "$RELEASES"
}

passes_on() {
  systemctl enable --now openlore-indexer-pass.timer
  systemctl start --no-block openlore-indexer-pass.service
}

host_deploy() { # $1 = digest, $2 = git sha (or "unknown"), $3 = readiness wait in seconds (default 60)
  local digest="$1" sha="${2:-unknown}" wait_s="${3:-60}" prev ready_s
  is_digest "$digest" || die "refusing non-digest '$digest'"
  is_wait "$wait_s" || die "refusing readiness wait '$wait_s': whole seconds, at least 2"
  host_install
  "$BASE/bin/render-dids.sh"
  [ -f "$CONFIG/repo-dids" ] || die "no DID list: put /openlore/prod/indexer/repo-dids first"
  docker pull "$IMAGE@$digest" # while the old version still serves
  prev=$(releases_digest 1)
  start_digest "$digest"
  if ready_s=$(wait_ready "$wait_s"); then
    record_release "$sha $digest ready_s=$ready_s"
    passes_on
    # The moved-aside index from an earlier --reset-index is no longer a rollback aid.
    find "$DATA" -mindepth 1 -maxdepth 1 -type d -name 'reset-*' -exec rm -rf {} + 2>/dev/null || true
    echo "deployed $digest (ready after ~${ready_s} s)"
    return 0
  fi
  echo "NOT READY: rolling back automatically" >&2
  compose logs --no-color --tail 50 indexer >&2 || true
  if [ -z "$prev" ] || [ "$prev" = "$digest" ]; then
    stop_indexer
    echo "no previous release; the indexer is stopped and index. answers 503" >&2
    exit 1
  fi
  start_digest "$prev"
  if wait_ready >/dev/null; then
    echo "previous release $prev is serving again" >&2
  else
    echo "previous release still not ready; see deploy.sh host-status" >&2
  fi
  exit 1
}

reset_index() { # move the index aside; the next passes rebuild it from the authors' PDSes
  local aside
  aside="$DATA/reset-$(date -u +%Y%m%dT%H%M%SZ)"
  install -d -m 0700 -o 65532 -g 65532 "$aside"
  local f
  for f in index.duckdb index.duckdb.wal indexed_claims; do
    if [ -e "$DATA/$f" ]; then mv "$DATA/$f" "$aside/"; fi
  done
  echo "moved the index aside to $aside"
}

host_rollback() { # $1 = --reset-index or empty
  local prev
  prev=$(releases_digest 2)
  [ -n "$prev" ] || die "no previous release in $RELEASES"
  systemctl stop openlore-indexer-pass.timer || true
  stop_indexer
  if [ "${1:-}" = "--reset-index" ]; then reset_index; fi
  start_digest "$prev"
  if ! wait_ready >/dev/null; then
    echo "rollback target not ready; try 'deploy.sh rollback --reset-index'" >&2
    exit 1
  fi
  record_release "rollback $prev"
  passes_on
  echo "rolled back to $prev"
}

host_redeploy() {
  local cur
  cur=$(releases_digest 1)
  [ -n "$cur" ] || die "nothing deployed yet; use deploy.sh deploy <git-sha>"
  host_install
  "$BASE/bin/render-dids.sh"
  start_digest "$cur"
  wait_ready >/dev/null || {
    echo "redeploy not ready; see deploy.sh host-status" >&2
    exit 1
  }
  passes_on
  echo "redeployed $cur"
}

host_stop() {
  systemctl stop openlore-indexer-pass.timer || true
  stop_indexer
  echo "stopped (index. answers 503; the health timer keeps reporting)"
}

host_start() {
  local cur
  cur=$(releases_digest 1)
  [ -n "$cur" ] || die "nothing deployed yet; use deploy.sh deploy <git-sha>"
  start_digest "$cur"
  wait_ready >/dev/null || {
    echo "start not ready; see deploy.sh host-status" >&2
    exit 1
  }
  passes_on
  echo "started $cur"
}

host_trigger() {
  local result=0
  systemctl start openlore-indexer-pass.service || result=$?
  systemctl show openlore-indexer-pass.service -p Result -p ExecMainStatus --no-pager || true
  journalctl -u openlore-indexer-pass.service -n 5 -o cat --no-pager || true
  return "$result"
}

host_status() {
  compose ps -a || true
  systemctl list-timers 'openlore-indexer-*' --all --no-pager || true
  systemctl show openlore-indexer-pass.service -p Result -p ExecMainStatus --no-pager || true
  journalctl -u openlore-indexer-pass.service -n 10 -o cat --no-pager || true
  compose logs --no-color --tail 30 indexer || true
  echo "--- releases (last 10)"
  tail -n 10 "$RELEASES" 2>/dev/null || echo "(none)"
  echo "--- last indexer.host.health"
  journalctl -u openlore-indexer-health -n 1 -o cat --no-pager 2>/dev/null || true
}

host_main() {
  local mode="${1:-}"
  shift || true
  case "$mode" in
    install) host_install ;;
    deploy) host_deploy "$@" ;;
    redeploy) host_redeploy ;;
    rollback) host_rollback "$@" ;;
    stop) host_stop ;;
    start) host_start ;;
    trigger) host_trigger ;;
    host-status) host_status ;;
    *) die "unknown host mode '$mode'" ;;
  esac
}

# --------------------------------------------------------------------------------------------

cmd="${1:-}"
shift || true
case "$cmd" in
  deploy) laptop_deploy "${1:-}" ;;
  install | redeploy | stop | start | trigger | host-status) remote "$cmd" ;;
  rollback)
    case "${1:-}" in "" | --reset-index) ;; *) die "usage: deploy.sh rollback [--reset-index]" ;; esac
    remote rollback "$@"
    ;;
  status) laptop_status ;;
  host) host_main "$@" ;;
  *)
    sed -n '2,15p' "$0" >&2
    exit 2
    ;;
esac
