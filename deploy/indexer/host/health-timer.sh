#!/usr/bin/env bash
# health-timer.sh -- the indexer's host health check. Runs ON THE HOST as root (instance role)
# every 2 minutes from openlore-indexer-health.timer (devops/observability-design.md §4,
# monitoring-alerting.md A3). Installed to /pds/indexer/bin/health-timer.sh.
#
#   health-timer.sh run   one check (what the timer runs); the default
#
# Each run writes ONE JSON `indexer.host.health` line to stdout (the journal) and to the
# `host-health` stream of /openlore/prod/indexer. Alarm A3 is a log metric filter on its
# `not_live` field, so the script publishes no custom metric:
#
#   not_live = 1 if running = 0         (container openlore-indexer absent, stopped or OOM-killed)
#             or healthz_ok = 0         (/healthz through Caddy not 200 with "status":"ok";
#                                        a 503 for an unusable store counts)
#             or summary_45m = 0        (no indexer.ingest.pass_summary in the SHIPPED log group in
#                                        the last 45 min; any FilterLogEvents error sets
#                                        summary_check = error and summary_45m = 0: fail closed)
#             or dids_age_s > 7200      (.rendered-at older than 2 h; missing = infinitely old)
#              else 0.
#
# Hung-process guard (as the review-app timer): if the container runs but /healthz has failed
# FAIL_LIMIT runs in a row, `docker restart openlore-indexer` and count the restart.
#
# The check never fails its own unit: it always exits 0.
#
# Every aws call is bounded by `timeout` (INDEXER_AWS_TIMEOUT_S, default 15 s; four calls at
# most stay inside the unit's TimeoutStartSec=90s), so a slow FilterLogEvents can never cost the
# health line: a timed-out FilterLogEvents is summary_check = error (not live), with a line.
#
# Env (tests): INDEXER_CONFIG_DIR (default /pds/indexer/config), INDEXER_STATE_DIR (default
# /pds/indexer/state), INDEXER_AWS_TIMEOUT_S. docker, curl, aws and timeout come from PATH. A
# host without /proc reports 0 for the host memory fields.
set -uo pipefail

CONTAINER=openlore-indexer
CONFIG_DIR="${INDEXER_CONFIG_DIR:-/pds/indexer/config}"
STATE_DIR="${INDEXER_STATE_DIR:-/pds/indexer/state}"
STATE="$STATE_DIR/health.state"
LOG_GROUP=/openlore/prod/indexer
LOG_STREAM=host-health
REGION=us-east-1
HOST=index.openlore.jeffbailey.us
SEARCH_PATH=/xrpc/org.openlore.appview.searchClaims
# A fixed canned body: no user data, a value that matches nothing.
SEARCH_BODY='{"dimension":"subject","value":"https://openlore.invalid/health-probe"}'
FAIL_LIMIT=3
SUMMARY_WINDOW_S=2700
DIDS_MAX_AGE_S=7200
NEVER_RENDERED_AGE_S=999999999
AWS_TIMEOUT_S="${INDEXER_AWS_TIMEOUT_S:-15}"

now_s() { date +%s; }

is_uint() { [[ "${1:-}" =~ ^[0-9]+$ ]]; }

aws_bounded() { # $@ = aws args; killed after AWS_TIMEOUT_S (exit 124 counts as a failure)
  timeout "$AWS_TIMEOUT_S" aws "$@"
}

uint_or_zero() {
  if is_uint "${1:-}"; then echo "$1"; else echo 0; fi
}

# --------------------------------------------------------------------------------------------
# Probes (each prints its result; none can fail the script)
# --------------------------------------------------------------------------------------------

container_id() {
  docker ps -aq --no-trunc --filter "name=^${CONTAINER}$" 2>/dev/null | head -n1
}

inspect() { # $1 = container id, $2 = Go template
  docker inspect -f "$2" "$1" 2>/dev/null || true
}

memory_mb() { # $1 = container id; "61MiB / 128MiB" -> 61
  docker stats --no-stream --format '{{.MemUsage}}' "$1" 2>/dev/null | awk '
    NR == 1 {
      v = $1; n = v + 0
      if (v ~ /GiB$|GB$/) n = n * 1024
      else if (v ~ /KiB$|kB$/) n = n / 1024
      else if (v !~ /MiB$|MB$/) n = 0
      printf "%d", n
    }'
}

healthz_ok() { # 1 if /healthz answers 200 with "status":"ok" through Caddy on loopback
  local body
  body=$(curl -fsS -m 5 --resolve "$HOST:443:127.0.0.1" "https://$HOST/healthz" 2>/dev/null) || {
    echo 0
    return
  }
  if [[ "${body//[[:space:]]/}" == *'"status":"ok"'* ]]; then echo 1; else echo 0; fi
}

search_probe() { # prints "<search_ok> <search_ms>"
  local seconds
  if seconds=$(curl -fsS -o /dev/null -m 5 -w '%{time_total}' \
    --resolve "$HOST:443:127.0.0.1" -X POST -H 'content-type: application/json' \
    --data "$SEARCH_BODY" "https://$HOST$SEARCH_PATH" 2>/dev/null); then
    if [[ "$seconds" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
      echo "1 $(awk -v s="$seconds" 'BEGIN { printf "%d", s * 1000 }')"
    else
      echo "1 0"
    fi
  else
    echo "0 0"
  fi
}

summary_check() { # prints "<summary_45m> <ok|error>"; any API error fails closed
  local start_ms out
  start_ms=$((($(now_s) - SUMMARY_WINDOW_S) * 1000))
  if ! out=$(aws_bounded logs filter-log-events --region "$REGION" --log-group-name "$LOG_GROUP" \
    --filter-pattern '{ $.event = "indexer.ingest.pass_summary" }' \
    --start-time "$start_ms" --max-items 1 --output json \
    --cli-connect-timeout 5 --cli-read-timeout 20 2>/dev/null); then
    echo "0 error"
    return
  fi
  if [[ "$out" == *'"message"'* ]]; then echo "1 ok"; else echo "0 ok"; fi
}

dids_age_s() { # seconds since .rendered-at; a missing file is infinitely old
  local marker="$CONFIG_DIR/.rendered-at" mtime
  mtime=$(stat -c %Y "$marker" 2>/dev/null || stat -f %m "$marker" 2>/dev/null || true)
  if is_uint "$mtime"; then
    echo $(($(now_s) - mtime))
  else
    echo "$NEVER_RENDERED_AGE_S"
  fi
}

host_mem_available_mb() {
  awk '/^MemAvailable:/ { printf "%d", $2 / 1024 }' /proc/meminfo 2>/dev/null || true
}

swapin_pages() {
  awk '/^pswpin / { print $2 }' /proc/vmstat 2>/dev/null || true
}

pds_free_mb() {
  df -Pm /pds 2>/dev/null | awk 'NR == 2 { print $4 }' || true
}

# --------------------------------------------------------------------------------------------
# One run
# --------------------------------------------------------------------------------------------

emit() { # $1 = the JSON line: stdout (journal) and the host-health stream (best effort)
  local line="$1" msg ms events
  echo "$line"
  msg=${line//\\/\\\\}
  msg=${msg//\"/\\\"}
  ms=$(($(now_s) * 1000))
  events="[{\"timestamp\":$ms,\"message\":\"$msg\"}]"
  if ! aws_bounded logs put-log-events --region "$REGION" --log-group-name "$LOG_GROUP" \
    --log-stream-name "$LOG_STREAM" --log-events "$events" >/dev/null 2>&1; then
    aws_bounded logs create-log-stream --region "$REGION" --log-group-name "$LOG_GROUP" \
      --log-stream-name "$LOG_STREAM" >/dev/null 2>&1 || true
    aws_bounded logs put-log-events --region "$REGION" --log-group-name "$LOG_GROUP" \
      --log-stream-name "$LOG_STREAM" --log-events "$events" >/dev/null 2>&1 ||
      echo "health-timer: could not ship the indexer.host.health line" >&2
  fi
}

run_once() {
  local cid running=0 healthz=0 search_ok=0 search_ms=0 restarts=0 oom=0 mem_mb=0 count=0
  local fails=0 summary_45m=0 summary_state=error dids_age not_live=0
  local prev_cid="" prev_count=0 prev_oom=0 prev_fails=0 prev_swapin=""
  local host_mem swapin swap_in_kb=0 pds_free

  if [ -r "$STATE" ]; then
    read -r prev_cid prev_count prev_oom prev_fails prev_swapin <"$STATE" || true
  fi
  is_uint "$prev_count" || prev_count=0
  is_uint "$prev_fails" || prev_fails=0

  cid=$(container_id)
  if [ -n "$cid" ]; then
    if [ "$(inspect "$cid" '{{.State.Running}}')" = "true" ]; then running=1; fi
    count=$(inspect "$cid" '{{.RestartCount}}')
    is_uint "$count" || count=0
    if [ "$(inspect "$cid" '{{.State.OOMKilled}}')" = "true" ]; then oom=1; fi
    # A new container id (a deploy) restarts the count; only same-container deltas count.
    if [ "$cid" = "$prev_cid" ] && [ "$count" -gt "$prev_count" ]; then
      restarts=$((count - prev_count))
    fi
    if [ "$oom" = 1 ] && [ "$prev_oom" != 1 ]; then
      restarts=$((restarts + 1))
    fi
    if [ "$running" = 1 ]; then
      mem_mb=$(memory_mb "$cid")
    fi
  fi

  if [ "$running" = 1 ]; then
    healthz=$(healthz_ok)
    if [ "$healthz" = 1 ]; then
      fails=0
    else
      fails=$((prev_fails + 1))
      if [ "$fails" -ge "$FAIL_LIMIT" ]; then
        echo "health-timer: /healthz failed $fails runs in a row; restarting $CONTAINER" >&2
        docker restart "$CONTAINER" >/dev/null 2>&1 || true
        restarts=$((restarts + 1))
        fails=0
        count=$(inspect "$cid" '{{.RestartCount}}')
        is_uint "$count" || count=0
      fi
    fi
    read -r search_ok search_ms <<<"$(search_probe)"
  fi

  read -r summary_45m summary_state <<<"$(summary_check)"
  dids_age=$(dids_age_s)

  if [ "$running" != 1 ] || [ "$healthz" != 1 ] || [ "$summary_45m" != 1 ] ||
    [ "$dids_age" -gt "$DIDS_MAX_AGE_S" ]; then
    not_live=1
  fi

  host_mem=$(host_mem_available_mb)
  swapin=$(swapin_pages)
  if is_uint "$swapin" && is_uint "$prev_swapin" && [ "$swapin" -ge "$prev_swapin" ]; then
    swap_in_kb=$(((swapin - prev_swapin) * 4))
  fi
  pds_free=$(pds_free_mb)

  if mkdir -p "$STATE_DIR" 2>/dev/null; then
    echo "${cid:--} $count $oom $fails ${swapin:-}" >"$STATE" 2>/dev/null || true
  fi

  emit "$(printf '{"event":"indexer.host.health","ts":"%s","running":%d,"healthz_ok":%d,"search_ok":%d,"search_ms":%d,"summary_45m":%d,"summary_check":"%s","dids_age_s":%d,"restarts":%d,"oom_killed":%d,"indexer_mem_mb":%d,"host_mem_available_mb":%d,"swap_in_kb":%d,"pds_free_mb":%d,"not_live":%d}' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$running" "$healthz" "${search_ok:-0}" "${search_ms:-0}" \
    "${summary_45m:-0}" "${summary_state:-error}" "$dids_age" "$restarts" "$oom" \
    "$(uint_or_zero "$mem_mb")" \
    "$(uint_or_zero "$host_mem")" "$swap_in_kb" \
    "$(uint_or_zero "$pds_free")" "$not_live")"
}

case "${1:-run}" in
  run) run_once ;;
  *) echo "usage: $0 [run]" >&2 ;;
esac
exit 0
