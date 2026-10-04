#!/usr/bin/env bash
# health-timer.sh -- the review app's host health check. Runs ON THE HOST as root (instance
# role), once a minute, from the systemd timer `review-app-health` (devops/observability-design.md
# §4, monitoring-alerting.md A-3).
#
#   health-timer.sh install   write + enable the systemd service and timer (idempotent)
#   health-timer.sh run       one check (what the timer runs)
#
# Each run:
#   1. inspects the app container (running, restart count, OOM kill, memory);
#   2. probes /healthz and /readyz through Caddy on the host's loopback (real TLS, real route);
#   3. if the container is running but /healthz has failed FAIL_LIMIT runs in a row (a hung
#      process that docker's restart policy cannot see), restarts it and counts that restart;
#   4. writes one JSON `host.health` line to the journal and to the `host-health` stream of
#      /openlore/prod/review-app. Alarm A-3 is a log metric filter on that line, so it publishes
#      no custom metrics.
set -euo pipefail

INSTALL_PATH=/pds/app/bin/health-timer.sh
STATE=/run/review-app-health.state
LOG_GROUP=/openlore/prod/review-app
LOG_STREAM=host-health
REGION=us-east-1
HOST=app.openlore.jeffbailey.us
FAIL_LIMIT=3

install_units() {
  cat >/etc/systemd/system/review-app-health.service <<UNIT
[Unit]
Description=OpenLore review app host health check
After=docker.service
Wants=docker.service

[Service]
Type=oneshot
ExecStart=$INSTALL_PATH run
UNIT
  cat >/etc/systemd/system/review-app-health.timer <<'UNIT'
[Unit]
Description=Run the OpenLore review app health check every minute

[Timer]
OnCalendar=*-*-* *:*:00
AccuracySec=5s

[Install]
WantedBy=timers.target
UNIT
  systemctl daemon-reload
  systemctl enable --now review-app-health.timer
}

probe() { # $1 = path; 0 if the public route answers 2xx through Caddy on loopback
  curl -fsS -o /dev/null -m 5 --resolve "$HOST:443:127.0.0.1" "https://$HOST$1"
}

emit() { # $1 = the JSON line
  local line="$1" msg ms events
  echo "$line"
  msg=${line//\\/\\\\}
  msg=${msg//\"/\\\"}
  ms=$(date +%s%3N)
  events="[{\"timestamp\":$ms,\"message\":\"$msg\"}]"
  if ! aws logs put-log-events --region "$REGION" --log-group-name "$LOG_GROUP" \
    --log-stream-name "$LOG_STREAM" --log-events "$events" >/dev/null 2>&1; then
    aws logs create-log-stream --region "$REGION" --log-group-name "$LOG_GROUP" \
      --log-stream-name "$LOG_STREAM" >/dev/null 2>&1 || true
    aws logs put-log-events --region "$REGION" --log-group-name "$LOG_GROUP" \
      --log-stream-name "$LOG_STREAM" --log-events "$events" >/dev/null ||
      echo "health-timer: could not ship the host.health line" >&2
  fi
}

run_once() {
  local cid running=0 ready=0 restarts=0 oom=0 mem_pct=0 count=0 fails=0 swapin_pages
  local prev_cid="" prev_count=0 prev_oom=0 prev_fails=0 prev_swapin=""

  if [ -r "$STATE" ]; then
    read -r prev_cid prev_count prev_oom prev_fails prev_swapin <"$STATE" || true
  fi

  cid=$(docker ps -aq --no-trunc \
    --filter label=com.docker.compose.project=review-app \
    --filter label=com.docker.compose.service=review-app | head -n1)

  if [ -n "$cid" ]; then
    [ "$(docker inspect -f '{{.State.Running}}' "$cid")" = "true" ] && running=1
    count=$(docker inspect -f '{{.RestartCount}}' "$cid")
    [ "$(docker inspect -f '{{.State.OOMKilled}}' "$cid")" = "true" ] && oom=1
    # A new container id (a deploy) restarts the count; only same-container deltas count.
    if [ "$cid" = "$prev_cid" ] && [ "$count" -gt "$prev_count" ]; then
      restarts=$((count - prev_count))
    fi
    if [ "$oom" = 1 ] && [ "$prev_oom" != 1 ]; then
      restarts=$((restarts + 1))
    fi
    if [ "$running" = 1 ]; then
      mem_pct=$(docker stats --no-stream --format '{{.MemPerc}}' "$cid" | tr -d '%' | cut -d. -f1)
      [[ "$mem_pct" =~ ^[0-9]+$ ]] || mem_pct=0
    fi
  fi

  if [ "$running" = 1 ]; then
    if probe /healthz; then
      fails=0
    else
      fails=$((prev_fails + 1))
      if [ "$fails" -ge "$FAIL_LIMIT" ]; then
        echo "health-timer: /healthz failed $fails runs in a row; restarting the app" >&2
        docker restart "$cid" >/dev/null
        restarts=$((restarts + 1))
        fails=0
        count=$(docker inspect -f '{{.RestartCount}}' "$cid")
      fi
    fi
    probe /readyz && ready=1
  fi

  local host_mem_mb swap_in_kb=0 pds_free_mb
  host_mem_mb=$(awk '/^MemAvailable:/ { printf "%d", $2 / 1024 }' /proc/meminfo)
  swapin_pages=$(awk '/^pswpin / { print $2 }' /proc/vmstat)
  if [ -n "$prev_swapin" ] && [ "$swapin_pages" -ge "$prev_swapin" ]; then
    swap_in_kb=$(((swapin_pages - prev_swapin) * 4))
  fi
  pds_free_mb=$(df -Pm /pds | awk 'NR == 2 { print $4 }')

  echo "$cid $count $oom $fails $swapin_pages" >"$STATE"

  emit "$(printf '{"event":"host.health","ts":"%s","app_running":%d,"ready":%d,"restarts":%d,"oom_killed":%d,"app_mem_pct":%d,"host_mem_available_mb":%d,"swap_in_kb":%d,"pds_free_mb":%d}' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$running" "$ready" "$restarts" "$oom" \
    "${mem_pct:-0}" "${host_mem_mb:-0}" "$swap_in_kb" "${pds_free_mb:-0}")"
}

case "${1:-run}" in
  install) install_units ;;
  run) run_once ;;
  *)
    echo "usage: $0 [install|run]" >&2
    exit 2
    ;;
esac
