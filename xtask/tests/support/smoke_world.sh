# shellcheck shell=bash
# The world lambda-smoke.sh runs in, for scale_to_zero_ci.rs: `docker`, `curl` and
# `python3`, each a stub that sets STATE and BROKEN, sources this and calls its function.
# It answers what a good image and a good store answer. BROKEN names the one thing that
# goes wrong, so a run shows the smoke stopping at that thing.
#
# The smoke starts one function per step, so the count of starts says which step this is:
#   1 search, 2 search with no store, 3 a pass, 4 the probe, 5 and 6 the loopback seam,
#   7 no-publish, 8 a pass that publishes, 9 a pass that is raced, 10 a misspelled event.

broken() { [ "$BROKEN" = "$1" ]; }

record() { printf '%s\n' "$1" >>"$STATE/calls.log"; }

starts() { cat "$STATE/starts" 2>/dev/null || echo 0; }

value_of() { cat "$STATE/$1" 2>/dev/null || echo "$2"; }

log_event() { printf '{"event":"%s"%s}\n' "$1" "${2:+,$2}"; }

log_summary() { log_event indexer.ingest.pass_summary "$1"; }

log_refused_start() {
  echo 'Runtime.ExitError'
  log_event health.startup.refused
}

function_log() { # $1 = which start
  case "$1" in
    2) broken search-starts || log_refused_start ;;
    3) log_summary '"exit_code":3,"pass_id":"p3"' ;;
    4) log_event indexer.host.health "\"not_live\":$(if broken probe-says-live; then echo 0; else echo 1; fi)" ;;
    5 | 6)
      log_refused_start
      broken refused-for-another-reason || echo 'OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP is set'
      ;;
    7) log_summary '"exit_code":3,"pass_id":"p7","no_publish":true' ;;
    8)
      if broken published-after-the-summary; then
        log_summary '"exit_code":3,"pass_id":"p8"'
        log_event indexer.lambda.published
      else
        log_event indexer.lambda.published
        log_summary '"exit_code":3,"pass_id":"p8"'
      fi
      ;;
    9)
      log_event indexer.lambda.publish_conflict
      log_summary '"exit_code":2,"pass_id":"p9","cause":"publish_conflict"'
      ;;
    10) if broken refused-event-ran; then log_summary '"exit_code":0,"pass_id":"p10"'; fi ;;
  esac
}

docker_stub() {
  record "docker $*"
  case "$1" in
    inspect) if broken working-directory; then echo /home/nonroot; else echo /; fi ;;
    logs) function_log "$(starts)" ;;
    run)
      case "$*" in
        *"--name openlore-lambda-smoke "*)
          local start previous="" word
          start=$(($(starts) + 1))
          echo "$start" >"$STATE/starts"
          for word in "$@"; do
            if [ "$previous" = --env-file ]; then cp "$word" "$STATE/env-$start"; fi
            previous=$word
          done
          ;;
      esac
      ;;
  esac
}

object_is() { # $1 = the pass id of the new current version
  echo "$1" >"$STATE/object"
  echo $(($(value_of version 0) + 1)) >"$STATE/version"
}

object_headers() {
  printf 'etag: "seed"\r\nx-amz-version-id: v%s\r\n' "$(value_of version 0)"
  printf 'x-amz-meta-published-at: 2026-01-01T00:00:00Z\r\nx-amz-meta-pass-id: %s\r\n' \
    "$(value_of object none)"
}

object_versions() {
  if [ -f "$STATE/object" ]; then
    printf '<Version><Key>index/index.duckdb</Key><VersionId>v%s</VersionId></Version>\n' \
      "$(value_of version 0)"
  else
    echo '<ListVersionsResult/>'
  fi
}

answer_search() { # $1 = the answer file
  local ask
  echo ask >>"$STATE/asks"
  ask=$(($(wc -l <"$STATE/asks")))
  case "$ask" in
    1) printf '%s' '{"statusCode":200,"body":"{\"status\":\"ok\",\"last_successful_pass_at\":null}"}' ;;
    2)
      if broken empty-bucket-answers-rows; then
        printf '%s' '{"statusCode":200,"body":"{\"results\":[{}]}"}'
      else
        printf '%s' '{"statusCode":200,"body":"{\"results\":[]}"}'
      fi
      ;;
    3) printf '%s' '{"statusCode":404,"body":""}' ;;
    4) printf '%s' '{"statusCode":200,"body":"{\"results\":[]}"}' ;;
    5)
      if broken large-body-taken; then
        printf '%s' '{"statusCode":200,"body":"{\"results\":[]}"}'
      else
        printf '%s' '{"statusCode":413,"body":""}'
      fi
      ;;
  esac >"$1"
}

# The race shim's log, beside the answer file in the smoke's work directory.
shim_logs() { # $1 = the answer file, $2 = one line
  printf '%s\n' "$2" >>"$(dirname "$1")/calls.jsonl"
}

answer_invoke() { # $1 = the answer file
  local index='"/openlore-lambda-smoke/index/index.duckdb"'
  case "$(starts)" in
    1) answer_search "$1" ;;
    2 | 5 | 6) return 52 ;;
    3)
      echo 3 >"$STATE/last-pass"
      if broken pass-of-nothing-publishes; then object_is p3; fi
      printf '%s' '{"exit_code":3,"published":false}' >"$1"
      ;;
    4) printf '%s' '{}' >"$1" ;;
    7)
      if broken no-publish-moves-a-version; then object_is p7; fi
      printf '%s' '{"mode":"no-publish","published":false,"would_publish":true}' >"$1"
      ;;
    8)
      echo 3 >"$STATE/last-pass"
      object_is p8
      if broken publish-without-if-match; then
        shim_logs "$1" "{\"method\":\"PUT\",\"path\":$index,\"if_match\":null}"
      else
        shim_logs "$1" "{\"method\":\"PUT\",\"path\":$index,\"if_match\":\"\\\"seed\\\"\"}"
      fi
      printf '%s' '{"exit_code":3,"published":true}' >"$1"
      ;;
    9)
      echo 2 >"$STATE/last-pass"
      object_is race-shim
      if broken stale-write-lands; then
        shim_logs "$1" "{\"swapped\":true,\"method\":\"PUT\",\"path\":$index,\"status\":200}"
      else
        shim_logs "$1" "{\"swapped\":true,\"method\":\"PUT\",\"path\":$index,\"status\":412}"
      fi
      printf '%s' '{"exit_code":2,"published":false}' >"$1"
      ;;
    10)
      if broken refused-event-writes; then
        shim_logs "$1" "{\"method\":\"PUT\",\"path\":\"/openlore-lambda-smoke/index/other\",\"status\":200}"
      fi
      printf '%s' '{"errorType":"EventRefused"}' >"$1"
      ;;
  esac
}

curl_stub() {
  record "curl $*"
  local url="" out="" asks_status=false asks_headers=false uploads=false previous="" word
  for word in "$@"; do
    case "$word" in
      http*) url=$word ;;
      -w) asks_status=true ;;
      -I) asks_headers=true ;;
      --upload-file) uploads=true ;;
    esac
    if [ "$previous" = -o ]; then out=$word; fi
    previous=$word
  done
  if [ "$asks_status" = true ]; then
    printf 200
  elif [ "$uploads" = true ]; then
    object_is smoke-seed
  elif [ "$asks_headers" = true ]; then
    object_headers
  else
    case "$url" in
      */invocations) answer_invoke "$out" ;;
      *"?versions&"*) object_versions ;;
      */last-pass.json) echo "{\"exit_code\":$(value_of last-pass null)}" ;;
    esac
  fi
}

python3_stub() {
  record "python3 $*"
}
