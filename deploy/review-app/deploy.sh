#!/usr/bin/env bash
# deploy.sh -- deploy the OpenLore review app to the PDS host (ADR-075;
# devops/infrastructure-integration.md §7). Run from the operator's laptop; CI never deploys.
#
#   deploy.sh install                  install/refresh host files, systemd timer, Caddy site
#   deploy.sh deploy <git-sha|digest>  verify CI + cosign, then Recreate-deploy that digest
#   deploy.sh redeploy                 re-render secrets, restart the current digest
#   deploy.sh rollback [--restore-db]  redeploy the previous digest from the host's releases
#   deploy.sh stop                     stop the app (Caddy then serves its 503 page)
#   deploy.sh status                   container state, last logs, releases, last host.health
#
# DIGEST ONLY. <git-sha> must be the full 40-hex commit; it is resolved to the digest CI pushed
# for it, and from then on only `image@sha256:...` is used. A `sha256:<64 hex>` digest is also
# accepted; its signed org.opencontainers.image.revision label names the commit whose CI must be
# green. Anything else -- a tag, a branch, a short sha -- is refused before any tool runs.
#
# Rollback is designed first: the host keeps an append-only releases file
# (`<utc> <sha> <digest>`) and a pre-deploy copy of the DuckDB file. If the new digest is not
# ready within 90 s the host rolls back to the previous digest automatically and this exits 1.
#
# Laptop needs: AWS credentials (e.g. AWS_PROFILE=jeff), gh (signed in), cosign, jq, and crane
# or docker buildx. The same file runs on the host (as `deploy.sh host <mode>`) via SSM Run
# Command, which carries only these repo files and the digest -- never a secret.
set -euo pipefail

IMAGE=ghcr.io/jeffabailey/openlore-review-app
REGION=us-east-1
APP_HOST=app.openlore.jeffbailey.us
CERT_IDENTITY_RE='^https://github.com/jeffabailey/openlore/.github/workflows/ci.yml@refs/heads/main$'
OIDC_ISSUER=https://token.actions.githubusercontent.com

die() {
  echo "deploy.sh: $*" >&2
  exit 2
}

is_digest() { [[ "$1" =~ ^sha256:[0-9a-f]{64}$ ]]; }
is_git_sha() { [[ "$1" =~ ^[0-9a-f]{40}$ ]]; }

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

instance_id() {
  if [ -n "${REVIEW_APP_INSTANCE_ID:-}" ]; then
    echo "$REVIEW_APP_INSTANCE_ID"
  else
    tofu -chdir="$(dirname "$0")/../tofu/environments/prod" output -raw instance_id
  fi
}

remote() { # $@ = host mode and args; ships host/ + this script, runs it as root over SSM
  need aws jq tar base64
  local here payload b64 iid params cmd_id status
  here=$(cd "$(dirname "$0")" && pwd)
  payload=$(mktemp -d)
  # shellcheck disable=SC2064 # expand now: the path is fixed
  trap "rm -rf '$payload'" EXIT
  cp "$here"/host/compose.yaml "$here"/host/app.caddy "$here"/host/*.sh "$here/deploy.sh" "$payload/"
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
    executionTimeout: ["900"]
  }')
  cmd_id=$(aws ssm send-command --region "$REGION" --instance-ids "$iid" \
    --document-name AWS-RunShellScript --comment "review-app $1" \
    --parameters "$params" --query Command.CommandId --output text)
  echo "SSM command $cmd_id on $iid ($*)"
  while :; do
    sleep 5
    status=$(aws ssm get-command-invocation --region "$REGION" --command-id "$cmd_id" \
      --instance-id "$iid" --query Status --output text 2>/dev/null || echo Pending)
    case "$status" in Pending | InProgress | Delayed) continue ;; esac
    break
  done
  aws ssm get-command-invocation --region "$REGION" --command-id "$cmd_id" --instance-id "$iid" \
    --query '[StandardOutputContent,StandardErrorContent]' --output text
  [ "$status" = "Success" ] || {
    echo "deploy.sh: host command ended $status" >&2
    exit 1
  }
}

post_checks() {
  local base="https://$APP_HOST" meta
  curl -fsS -o /dev/null -m 10 "$base/healthz"
  curl -fsS -o /dev/null -m 10 "$base/readyz"
  meta=$(curl -fsS -m 10 "$base/oauth/client-metadata.json")
  [ "$(jq -r .client_id <<<"$meta")" = "$base/oauth/client-metadata.json" ] ||
    die "client_id in client-metadata.json is not its own URL"
  curl -fsS -m 10 "$base/oauth/jwks.json" | jq -e '[.keys[] | has("d")] | any | not' >/dev/null ||
    die "jwks.json carries a private key member"
  echo "post-checks passed: healthz, readyz, client-metadata, jwks"
}

laptop_deploy() { # $1 = git sha or digest
  local ref="${1:-}" sha=unknown digest
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
  fi
  echo "verifying the signature on $IMAGE@$digest"
  cosign verify "$IMAGE@$digest" \
    --certificate-identity-regexp "$CERT_IDENTITY_RE" \
    --certificate-oidc-issuer "$OIDC_ISSUER" >/dev/null
  if [ "$sha" = unknown ]; then # a digest: its signed revision label names the commit; CI must be green
    sha=$(image_revision "$digest")
    ci_green "$sha"
  fi
  remote deploy "$digest" "$sha"
  post_checks
}

# --------------------------------------------------------------------------------------------
# Host side (root, via SSM). Paths per infrastructure-integration.md §1.
# --------------------------------------------------------------------------------------------

APP="${REVIEW_APP_BASE_DIR:-/pds/app}"
RELEASES=$APP/state/releases
DB=$APP/data/review-app.duckdb
# Created by tofu-aws-pds v1.7.0, never by this script: its absence means the module is not applied.
SITES="${REVIEW_APP_CADDY_SITES_DIR:-/pds/caddy/sites}"

compose() { docker compose -f "$APP/compose.yaml" "$@"; }

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
  install -d -m 0755 -o root -g root "$APP" "$APP/bin"
  install -d -m 0700 -o 65532 -g 65532 "$APP/data"
  install -d -m 0700 -o root -g root "$APP/state"
  install -m 0644 -o root -g root "$src/compose.yaml" "$APP/compose.yaml"
  install -m 0755 -o root -g root "$src/render-secrets.sh" "$src/health-timer.sh" "$APP/bin/"
  "$APP/bin/health-timer.sh" install
  install_caddy_site "$src/app.caddy" "$APP_HOST"
  echo "host files installed"
}

wait_ready() {
  local i
  for i in $(seq 1 45); do
    if curl -fsS -o /dev/null -m 5 --resolve "$APP_HOST:443:127.0.0.1" "https://$APP_HOST/readyz"; then
      echo "ready after ~$((i * 2)) s"
      return 0
    fi
    sleep 2
  done
  return 1
}

start_digest() { # $1 = digest
  is_digest "$1" || die "refusing to start non-digest '$1'"
  printf 'REVIEW_APP_DIGEST=%s\n' "$1" >"$APP/.env"
  chmod 0644 "$APP/.env"
  compose up -d --force-recreate review-app
}

stop_app() {
  if [ -f "$APP/.env" ]; then
    compose stop review-app || true
  fi
}

releases_digest() { # $1 = 1 for the current release, 2 for the one before it
  [ -f "$RELEASES" ] || return 0
  tail -n "$1" "$RELEASES" | head -n1 | awk '{print $3}'
}

restore_db() {
  local pre
  pre=$(find "$APP/data" -maxdepth 1 -name 'review-app.duckdb.pre-*' ! -name '*.wal' -printf '%T@ %p\n' |
    sort -n | tail -n1 | cut -d' ' -f2-)
  [ -n "$pre" ] || die "no pre-deploy DuckDB copy to restore"
  echo "restoring $pre"
  cp -p "$pre" "$DB"
  if [ -f "$pre.wal" ]; then cp -p "$pre.wal" "$DB.wal"; else rm -f "$DB.wal"; fi
}

host_deploy() { # $1 = digest, $2 = git sha (or "unknown")
  local digest="$1" sha="${2:-unknown}" prev pre
  is_digest "$digest" || die "refusing non-digest '$digest'"
  host_install
  "$APP/bin/render-secrets.sh"
  docker pull "$IMAGE@$digest" # while the old version still serves
  prev=$(releases_digest 1)
  stop_app
  if [ -f "$DB" ]; then
    pre="$DB.pre-${sha:0:12}"
    cp -p "$DB" "$pre"
    if [ -f "$DB.wal" ]; then cp -p "$DB.wal" "$pre.wal"; fi
  fi
  start_digest "$digest"
  if wait_ready; then
    echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) $sha $digest" >>"$RELEASES"
    chmod 0600 "$RELEASES"
    # Keep only this deploy's pre-copy (the rollback aid for the NEXT deploy is taken then).
    find "$APP/data" -maxdepth 1 -name 'review-app.duckdb.pre-*' ! -name "${pre##*/}*" -delete 2>/dev/null || true
    echo "deployed $digest"
    return 0
  fi
  echo "NOT READY: rolling back automatically" >&2
  compose logs --no-color --tail 50 review-app >&2 || true
  if [ -z "$prev" ]; then
    stop_app
    echo "no previous release; the app is stopped" >&2
    exit 1
  fi
  start_digest "$prev"
  if ! wait_ready; then
    if compose logs --no-color --tail 100 review-app 2>&1 | grep -q 'schema_version'; then
      stop_app
      restore_db
      start_digest "$prev"
      wait_ready || echo "previous release still not ready; see deploy.sh status" >&2
    fi
  fi
  exit 1
}

host_rollback() { # $1 = --restore-db or empty
  local prev
  prev=$(releases_digest 2)
  [ -n "$prev" ] || die "no previous release in $RELEASES"
  stop_app
  if [ "${1:-}" = "--restore-db" ]; then restore_db; fi
  start_digest "$prev"
  wait_ready || {
    echo "rollback target not ready; try 'deploy.sh rollback --restore-db'" >&2
    exit 1
  }
  echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) rollback $prev" >>"$RELEASES"
  echo "rolled back to $prev"
}

host_redeploy() {
  local cur
  cur=$(releases_digest 1)
  [ -n "$cur" ] || die "nothing deployed yet; use deploy.sh deploy <git-sha>"
  host_install
  "$APP/bin/render-secrets.sh"
  start_digest "$cur"
  wait_ready || {
    echo "redeploy not ready; see deploy.sh status" >&2
    exit 1
  }
}

host_status() {
  compose ps -a || true
  compose logs --no-color --tail 50 review-app || true
  echo "--- releases (last 10)"
  tail -n 10 "$RELEASES" 2>/dev/null || echo "(none)"
  echo "--- last host.health"
  journalctl -u review-app-health -n 1 -o cat --no-pager 2>/dev/null || true
}

host_main() {
  local mode="${1:-}"
  shift || true
  case "$mode" in
    install) host_install ;;
    deploy) host_deploy "$@" ;;
    redeploy) host_redeploy ;;
    rollback) host_rollback "$@" ;;
    stop) stop_app && echo "stopped" ;;
    status) host_status ;;
    *) die "unknown host mode '$mode'" ;;
  esac
}

# --------------------------------------------------------------------------------------------

cmd="${1:-}"
shift || true
case "$cmd" in
  deploy) laptop_deploy "${1:-}" ;;
  install | redeploy | stop | status) remote "$cmd" ;;
  rollback)
    case "${1:-}" in "" | --restore-db) ;; *) die "usage: deploy.sh rollback [--restore-db]" ;; esac
    remote rollback "$@"
    ;;
  host) host_main "$@" ;;
  *)
    sed -n '2,13p' "$0" >&2
    exit 2
    ;;
esac
