#!/usr/bin/env bash
# lambda-smoke (DEVOPS, D-STZ-24): the release image the way Lambda runs it, under the
# Runtime Interface Emulator, with moto standing in for S3 and SSM behind the race
# shim. The Lambda root's only end-to-end run. Blocking in CI's indexer-image job.
#
# Every invoke gets a fresh container, so each is a cold start, write probe included.
# The assertions run in order: each one's bucket is the one before it left.
#
# Needs: docker, python3, curl, jq.
#   LOCAL_TAG    the image
#   RIE_BIN      the aws-lambda-rie for this platform; mounted, never in the image
#   SEED_INDEX   the one-claim index.duckdb of crates/adapter-index-store/examples
#   MOTO_IMAGE   moto by digest, the rio-object-store pin
set -euo pipefail

: "${LOCAL_TAG:?the image to smoke}"
: "${RIE_BIN:?the aws-lambda-rie binary}"
: "${SEED_INDEX:?the seed index.duckdb}"
: "${MOTO_IMAGE:?moto by digest}"

root=$(cd "$(dirname "$0")/../../.." && pwd)
smoke_dir="$root/deploy/indexer/smoke"

REGION=us-east-1
BUCKET=openlore-lambda-smoke
INDEX_KEY=index/index.duckdb
LAST_PASS_KEY=index/last-pass.json
WRITE_PROBE_KEY=index/.write-probe
DID_PARAMETER=/openlore/smoke/indexer/repo-dids
UNRESOLVABLE_DID=did:web:smoke.invalid
SEARCH=/xrpc/org.openlore.appview.searchClaims
SEED_PASS_ID=smoke-seed
SHIM_PASS_ID=race-shim
INVOKE_LIMIT_SECS=120

MOTO_PORT=5055
SHIM_PORT=5056
FUNCTION_PORT=9000
MOTO=http://127.0.0.1:$MOTO_PORT
SHIM=http://127.0.0.1:$SHIM_PORT
FUNCTION=http://127.0.0.1:$FUNCTION_PORT
INVOCATIONS=2015-03-31/functions/function/invocations
THROUGH_THE_SHIM=http://host.docker.internal:$SHIM_PORT

function_container=openlore-lambda-smoke
moto_container=openlore-lambda-smoke-moto
work=$(mktemp -d)
calls="$work/calls.jsonl"
function_log="$work/function.log"
answer="$work/answer.json"
shim_pid=""

clean_up() {
  docker rm -f "$function_container" "$moto_container" >/dev/null 2>&1 || true
  if [ -n "$shim_pid" ]; then
    kill "$shim_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap clean_up EXIT

assertion() {
  echo "lambda-smoke: assertion $*"
}

fail() {
  echo "::error::lambda-smoke: $*"
  echo "--- the function's log"
  tail -n 50 "$function_log" 2>/dev/null || true
  echo "--- the shim's calls"
  tail -n 30 "$calls" 2>/dev/null || true
  echo "--- the last answer"
  cat "$answer" 2>/dev/null || true
  exit 1
}

wait_for() { # $1 = a name, $2 = a URL that answers something when it is up
  for _ in $(seq 1 60); do
    if [ "$(curl -s -o /dev/null -m 2 -w '%{http_code}' "$2")" != 000 ]; then
      return 0
    fi
    sleep 1
  done
  fail "$1 did not answer at $2"
}

# ---- moto and the shim

# Moto checks no signature, but it refuses an object call with no Authorization, and
# reads the service from the credential scope.
signed_for() { # $1 = the service
  echo "Authorization: AWS4-HMAC-SHA256 Credential=test/20260101/$REGION/$1/aws4_request, SignedHeaders=host, Signature=0"
}

moto_s3() {
  curl -fsS -H "$(signed_for s3)" "$@"
}

start_the_store() {
  docker run -d --name "$moto_container" -p "127.0.0.1:$MOTO_PORT:5000" "$MOTO_IMAGE" >/dev/null
  wait_for moto "$MOTO/"
  python3 "$smoke_dir/race_shim.py" --port "$SHIM_PORT" --upstream "127.0.0.1:$MOTO_PORT" \
    --log "$calls" &
  shim_pid=$!
  wait_for "the race shim" "$SHIM/"
  : >"$calls"
  moto_s3 -o /dev/null -X PUT "$MOTO/$BUCKET"
  moto_s3 -o /dev/null -X PUT "$MOTO/$BUCKET?versioning" -H 'Content-Type: application/xml' --data \
    '<VersioningConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Status>Enabled</Status></VersioningConfiguration>'
}

set_the_did_list() {
  curl -fsS -o /dev/null "$MOTO/" \
    -H 'X-Amz-Target: AmazonSSM.PutParameter' \
    -H 'Content-Type: application/x-amz-json-1.1' \
    -H "$(signed_for ssm)" \
    --data "$(jq -cn --arg name "$DID_PARAMETER" --arg value "$1" \
      '{Name: $name, Value: $value, Type: "String", Overwrite: true}')"
}

# Straight to moto, not through the shim: the seeded author is on no list, so a pass
# starts from F1 = (1 claim, 1 author, its CID hash) and the purge gives F0 = (0, 0, 0).
seed_the_bucket() {
  moto_s3 -o /dev/null --upload-file "$SEED_INDEX" \
    -H 'x-amz-meta-published-at: 2026-01-01T00:00:00Z' \
    -H "x-amz-meta-pass-id: $SEED_PASS_ID" \
    "$MOTO/$BUCKET/$INDEX_KEY"
  set_the_did_list "$UNRESOLVABLE_DID"
}

arm_the_shim() { # $1 = the path a PUT of it is raced
  curl -fsS -o /dev/null -X POST --data "$1" "$SHIM/__race_shim/arm"
}

object_versions() {
  moto_s3 "$MOTO/$BUCKET?versions&prefix=index/"
}

# A header of the index object's current version, as moto holds it.
current_header() { # $1 = the header
  moto_s3 -I "$MOTO/$BUCKET/$INDEX_KEY" | tr -d '\r' | grep -i "^$1: " | cut -d' ' -f2-
}

last_pass_exit() {
  moto_s3 "$MOTO/$BUCKET/$LAST_PASS_KEY" | jq -e '.exit_code'
}

no_index_published() {
  object_versions >"$work/versions.xml"
  ! grep -q "<Key>$INDEX_KEY</Key>" "$work/versions.xml"
}

shim_calls_since() { # $1 = how many lines were there, $2 = a jq filter
  tail -n "+$(($1 + 1))" "$calls" | jq -c "$2"
}

# ---- the function

# What lambda-env.json leaves to lambda.tf, and what Lambda itself sets. The keys are
# moto's, not an AWS credential, and go in at run time only.
environment_of() { # $1 = the mode
  jq -r 'to_entries[] | "\(.key)=\(.value)"' \
    "$root/deploy/tofu/environments/prod-indexer/lambda-env.json"
  printf '%s\n' \
    "AWS_REGION=$REGION" \
    "AWS_ACCESS_KEY_ID=test" \
    "AWS_SECRET_ACCESS_KEY=test" \
    "OPENLORE_INDEXER_LAMBDA_BUCKET=$BUCKET" \
    "OPENLORE_INDEXER_LAMBDA_INDEX_KEY=$INDEX_KEY" \
    "OPENLORE_INDEXER_LAMBDA_LAST_PASS_KEY=$LAST_PASS_KEY"
  case $1 in
  pass) echo "OPENLORE_INDEXER_LAMBDA_DID_PARAMETER=$DID_PARAMETER" ;;
  probe) echo "OPENLORE_INDEXER_LAMBDA_PROBE_BASE_URL=https://smoke.invalid" ;;
  esac
}

start_function() { # $1 = the mode, $2 = the store's endpoint, the rest = more docker run
  local mode=$1 endpoint=$2
  shift 2
  environment_of "$mode" >"$work/function.env"
  : >"$function_log"
  docker run -d --name "$function_container" \
    --user 993:990 --read-only --tmpfs /tmp:size=512m --memory 512m --memory-swap 512m \
    --add-host host.docker.internal:host-gateway \
    -p "127.0.0.1:$FUNCTION_PORT:8080" \
    -v "$RIE_BIN:/aws-lambda-rie:ro" --entrypoint /aws-lambda-rie \
    --env-file "$work/function.env" -e "AWS_ENDPOINT_URL=$endpoint" "$@" \
    "$LOCAL_TAG" /usr/local/bin/openlore-indexer-lambda "$mode" >/dev/null
  wait_for "the emulator" "$FUNCTION/"
}

invoke() { # $1 = the event, or @file
  curl -sS -m "$INVOKE_LIMIT_SECS" -o "$answer" -X POST \
    -H 'content-type: application/json' --data-binary "$1" "$FUNCTION/$INVOCATIONS"
}

stop_function() {
  docker logs "$function_container" >"$function_log" 2>&1
  docker rm -f "$function_container" >/dev/null
}

run_once() { # $1 = the mode, $2 = the exact event
  start_function "$1" "$THROUGH_THE_SHIM"
  invoke "$2"
  stop_function
}

# The emulator inits on the first invoke, and an init that exits gives that invoke no
# answer at all, so the init error is read from the emulator's own line. Runtime.ExitError
# is the part of it that holds still: its "Init failed" line reorders its fields.
refused_start() { # $1 = the mode, $2 = the store's endpoint, the rest = more docker run
  local mode=$1 endpoint=$2
  shift 2
  start_function "$mode" "$endpoint" "$@"
  invoke '{}' || true
  stop_function
  grep -q 'Runtime.ExitError' "$function_log" || fail "$mode did not end in an init error"
  [ "$(count_of health.startup.refused)" -ge 1 ] || fail "$mode wrote no health.startup.refused"
}

events_named() { # $1 = the event
  jq -Rc --arg name "$1" 'fromjson? | select(type == "object" and .event == $name)' "$function_log"
}

count_of() { # $1 = the event
  events_named "$1" | grep -c . || true
}

line_of() { # $1 = the event: which line of the log holds it
  grep -n "\"event\":\"$1\"" "$function_log" | head -n 1 | cut -d: -f1
}

one_summary_with() { # $1 = a jq condition on the one pass_summary
  [ "$(count_of indexer.ingest.pass_summary)" = 1 ] || fail "wanted exactly one pass_summary"
  events_named indexer.ingest.pass_summary | jq -e "$1" >/dev/null \
    || fail "the pass_summary is not $1"
}

answer_is() { # $1 = a jq condition on the invoke's answer
  jq -e "$1" "$answer" >/dev/null || fail "the answer is not $1"
}

# ---- search, as a Function URL hands it over

function_url_event() { # $1 = the method, $2 = the path, $3 = a file holding the body
  jq -n --arg method "$1" --arg path "$2" --rawfile body "$3" '{
    version: "2.0",
    routeKey: "$default",
    rawPath: $path,
    rawQueryString: "",
    headers: {host: "smoke.lambda-url.us-east-1.on.aws", "content-type": "application/json"},
    requestContext: {
      accountId: "anonymous",
      apiId: "smoke",
      domainName: "smoke.lambda-url.us-east-1.on.aws",
      domainPrefix: "smoke",
      http: {method: $method, path: $path, protocol: "HTTP/1.1", sourceIp: "203.0.113.1", userAgent: "lambda-smoke"},
      requestId: "lambda-smoke",
      routeKey: "$default",
      stage: "$default",
      time: "01/Jan/2026:00:00:00 +0000",
      timeEpoch: 0
    },
    body: $body,
    isBase64Encoded: false
  }'
}

ask_search() { # $1 = the method, $2 = the path, $3 = a file holding the body
  function_url_event "$1" "$2" "$3" >"$work/event.json"
  invoke "@$work/event.json"
}

padded_search() { # $1 = total bytes: a valid JSON search padded with spaces
  local search='{"dimension":"subject","value":"x"}'
  printf '%s' "$search"
  head -c "$(($1 - ${#search}))" /dev/zero | tr '\0' ' '
}

status_is() { # $1 = the status
  answer_is ".statusCode == $1"
}

# ---- the assertions

start_the_store
set_the_did_list "$UNRESOLVABLE_DID"

assertion "1: the image's working directory is /"
working_dir=$(docker inspect --format '{{.Config.WorkingDir}}' "$LOCAL_TAG")
[ "$working_dir" = / ] || fail "WorkingDir is '$working_dir', wanted /"

assertion "2: search on an empty bucket"
: >"$work/no-body"
printf '%s' '{"dimension":"subject","value":"https://smoke.invalid/x"}' >"$work/search.json"
padded_search 8192 >"$work/8k.json"
padded_search 9216 >"$work/9k.json"
start_function search "$THROUGH_THE_SHIM"
ask_search GET /healthz "$work/no-body"
status_is 200
answer_is '.body | fromjson | has("last_successful_pass_at") and .last_successful_pass_at == null'
ask_search POST "$SEARCH" "$work/search.json"
status_is 200
answer_is '.body | fromjson | .results == []'
ask_search GET /xrpc/x "$work/no-body"
status_is 404
ask_search POST "$SEARCH" "$work/8k.json"
answer_is '.statusCode != 413'
ask_search POST "$SEARCH" "$work/9k.json"
status_is 413
stop_function

assertion "3: search with the store unreachable at init"
refused_start search "http://host.docker.internal:9"

assertion "4: a pass with one unresolvable DID, on an empty bucket"
run_once pass '{}'
answer_is 'has("errorType") | not'
answer_is '.exit_code == 3 and .published == false'
one_summary_with '.exit_code == 3 and (.pass_id != null)'
[ "$(last_pass_exit)" = 3 ] || fail "last-pass.json does not hold the summary's exit"
no_index_published || fail "a pass that changed nothing published an index"

assertion "5: the probe against an unreachable edge"
run_once probe '{}'
answer_is 'has("errorType") | not'
[ "$(count_of indexer.host.health)" = 1 ] || fail "wanted exactly one indexer.host.health"
events_named indexer.host.health | jq -e '.not_live == 1' >/dev/null || fail "the probe said live"

assertion "6: the release image refuses the loopback seam"
for mode in pass search; do
  refused_start "$mode" "$THROUGH_THE_SHIM" -e OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1
  grep -q OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP "$function_log" || fail "$mode refused for another reason"
done

seed_the_bucket
seed_etag=$(current_header etag)
seed_version=$(current_header x-amz-version-id)

assertion "7: no-publish: F1 to F0 is a change, and nothing moves"
versions_before=$(object_versions)
run_once pass '{"mode":"no-publish"}'
answer_is '.mode == "no-publish" and .published == false and .would_publish == true'
one_summary_with '.no_publish == true'
[ "$(object_versions)" = "$versions_before" ] || fail "a no-publish run moved an object version"

assertion "8: a pass publishes with If-Match"
called=$(wc -l <"$calls")
run_once pass '{}'
answer_is '.exit_code == 3 and .published == true'
one_summary_with '.exit_code == 3'
publishes=$(shim_calls_since "$called" \
  "select(.method == \"PUT\" and .path == \"/$BUCKET/$INDEX_KEY\") | .if_match")
[ "$publishes" = "$(jq -cn --arg etag "$seed_etag" '$etag')" ] \
  || fail "the publish's If-Match was $publishes, wanted the seed's $seed_etag"
[ "$(current_header x-amz-version-id)" != "$seed_version" ] || fail "no new version"
[ -n "$(current_header x-amz-meta-published-at)" ] || fail "the new version has no published-at"
pass_id=$(current_header x-amz-meta-pass-id)
[ -n "$pass_id" ] && [ "$pass_id" != "$SEED_PASS_ID" ] || fail "the new version's pass-id is '$pass_id'"
[ "$(line_of indexer.lambda.published)" -lt "$(line_of indexer.ingest.pass_summary)" ] \
  || fail "indexer.lambda.published does not come before the pass_summary"

seed_the_bucket

assertion "9: a stale ETag is a conflict, not a lost update"
called=$(wc -l <"$calls")
arm_the_shim "/$BUCKET/$INDEX_KEY"
run_once pass '{}'
answer_is '.exit_code == 2 and .published == false'
one_summary_with '.exit_code == 2 and .cause == "publish_conflict"'
[ "$(line_of indexer.lambda.publish_conflict)" -lt "$(line_of indexer.ingest.pass_summary)" ] \
  || fail "indexer.lambda.publish_conflict does not come before the pass_summary"
[ "$(last_pass_exit)" = 2 ] || fail "last-pass.json does not say exit 2"
raced=$(shim_calls_since "$called" 'select(.swapped) | "\(.path) \(.status)"')
[ "$raced" = "\"/$BUCKET/$INDEX_KEY 412\"" ] || fail "the raced PUT was $raced"
[ "$(current_header x-amz-meta-pass-id)" = "$SHIM_PASS_ID" ] || fail "the shim's object is not current"

assertion "10: a misspelled key is refused and touches nothing"
called=$(wc -l <"$calls")
versions_before=$(object_versions)
run_once pass '{"mdoe":"no-publish"}'
answer_is 'has("errorType")'
[ "$(count_of indexer.ingest.pass_summary)" = 0 ] || fail "a refused event wrote a pass_summary"
touched=$(shim_calls_since "$called" \
  "select(.path == \"/$BUCKET/$INDEX_KEY\" or .path == \"/$BUCKET/$LAST_PASS_KEY\")")
[ -z "$touched" ] || fail "a refused event reached the index or the record: $touched"
others=$(shim_calls_since "$called" \
  "select(.method == \"PUT\" and (.path != \"/$BUCKET/$WRITE_PROBE_KEY\" or .status != 412))")
[ -z "$others" ] || fail "a refused event wrote: $others"
[ "$(object_versions)" = "$versions_before" ] || fail "a refused event moved an object version"

echo "lambda-smoke: passed"
