# shellcheck shell=bash
# lambda.sh -- the `deploy.sh lambda ...` verbs (scale-to-zero-hosting, DEVOPS "Deployment strategy
# and rollback"). deploy.sh sources this for its `lambda` verb only; `remote` never ships it, since
# no Lambda verb touches the shared host. It reuses deploy.sh's die, need, read_build_ref,
# verify_build, image_config, http_status, search_body and post_checks, so the digest and signature
# rules are the same on both paths.
#
#   lambda deploy <git-sha|digest>   blue-green by alias: every gate in order, `live` flips last
#   lambda seed <git-sha|digest>     the deploy's gates and its ECR copy only, then the first
#                                    apply's bootstrap_image_uri
#   lambda republish                 each function's live image again, through the same gates and
#                                    flips, so a config change reaches `live` (U-STZ-2)
#   lambda rollback                  every `live` back to its recorded prev=, `candidate` following
#   lambda stop [--wait]             the kill switch: search concurrency 0, the distribution off
#   lambda start                     search concurrency back to 3, the distribution on
#   lambda pass-off | pass-on        the pass's concurrency 0 / 1, and nothing else
#   lambda probe-off | probe-on      the probe's concurrency 0 / 1, and nothing else
#   lambda trigger                   one real pass now; exits with the pass's exit code
#   lambda probe                     one real probe now; prints its health line
#   lambda reset-index [--trigger]   after the typed word `reset`: a delete marker on the index,
#                                    then with --trigger the pass that rebuilds it
#   lambda restore-index <version-id>  that version copied back once, away from a quarter hour
#   lambda status                    the whole estate on one screen, with the metric check
#   lambda measure                   the R-STZ-1 and R-STZ-2 gate: exit 1 blocks Stage 2
#   lambda kpi [days]                KPIs over the current log group, without gates or test lines
#   lambda parity                    the host and the edge answer alike (trial only)
#   lambda test-alarm <target>       a1|a2|a3|a4|a5|a8|a9|a10|a11|errors|throttles|a11 --metric
#   lambda cutover-watch             the search gap over the cut-over, until interrupted
#
# Each step below does I/O and stops the deploy with `die`. Decisions are in plain predicates
# (a jq filter, an awk program), so a gate reads as what it requires.

LAMBDA_ACCOUNT=091153021562
LAMBDA_SEARCH=openlore-indexer-search
LAMBDA_PASS=openlore-indexer-pass
LAMBDA_PROBE=openlore-indexer-probe
# In flip order: search, then the pass, then the probe. The arrays below follow this order.
LAMBDA_FUNCTIONS=("$LAMBDA_SEARCH" "$LAMBDA_PASS" "$LAMBDA_PROBE")
ECR_REGISTRY=$LAMBDA_ACCOUNT.dkr.ecr.$REGION.amazonaws.com
ECR_REPO=$ECR_REGISTRY/openlore-indexer
INDEX_BUCKET=openlore-indexer-index-$LAMBDA_ACCOUNT
INDEX_KEY=index/index.duckdb
RETAINED_VERSIONS=5
RETAINED_IMAGES=10 # the ECR lifecycle's count of sha-* images
GATE_ATTEMPTS=30   # 30 s apart, so up to 870 s: a pass's 840 s timeout plus 30 s
MAX_EDGE_GAP_MS=30000
SEARCH_CONCURRENCY=3 # fixed, not a saved value: tofu ignores it, so `lambda start` owns it
ONE_SLOT=1           # the reserved concurrency of the pass and the probe
LAST_PASS_KEY=index/last-pass.json
BUDGET=openlore-indexer-lambda
SCHEDULE_GROUP=openlore-indexer
TRIAL_LOG_GROUP=/openlore/prod/indexer-lambda
ALARM_A4=openlore-indexer-search-invocations
ALARM_A5=openlore-indexer-index-size
ALARM_A6=openlore-indexer-lambda-errors
ALARM_A7=openlore-indexer-lambda-throttles
ALARM_A8=openlore-indexer-search-hourly
ALARM_A9=openlore-indexer-edge-requests
ALARM_A10=openlore-indexer-freshness-failed
ALARM_A11=openlore-indexer-search-5xx
LAMBDA_ONLY_ALARMS=("$ALARM_A4" "$ALARM_A5" "$ALARM_A6" "$ALARM_A7" "$ALARM_A8" "$ALARM_A9"
  "$ALARM_A10" "$ALARM_A11")
ALARM_WAIT_S=900 # 15 min for an alarm to arrive, and 15 more for it to clear
OVERSIZED_INDEX_BYTES=104857601 # one past A5's threshold
MEASURE_COLD_CALLS=5
MEASURE_WARM_CALLS=20
COLD_LIMIT_S=3
WARM_LIMIT_S=0.2
PARITY_SEARCHES=3
# What `lambda kpi` counts as a pass: no gate line (D-STZ-44) and no test-fire.
KPI_PASSES='filter event = "indexer.ingest.pass_summary" and not ispresent(no_publish) and pass_id not like /^TEST/'
COLD_STARTS='filter type = "platform.report" and ispresent(record.metrics.initDurationMs)'
PROBE_LINES="filter event = \"indexer.host.health\" and \`source\` = \"lambda-probe\""

# The version a `live` record says it replaced (L-13): a rollback's target, so a prune must read it
# exactly as a rollback does. A jq definition, for the filters that open with it.
RECORDED_PREV='def recorded_prev: .Description // "" | capture("prev=(?<v>[0-9]+)").v;'

COPIED_DIGEST=""      # step 6: set while this run's copy is not yet on every $LATEST
LIVE_VERSIONS=()      # step 7, or a rollback: what each `live` alias holds now
IMAGE_DIGESTS=()      # what each function is set to: a deploy's build, or its own live image
IMAGE_SHAS=()         # the git sha of each of those, for the records
LATEST_REVISIONS=()   # step 8: each $LATEST's revision, to publish without a race
NEW_VERSIONS=()       # step 9, or a rollback: the version each function will flip to
CANDIDATE_BACK_TO=""  # step 10: set while `candidate` is ahead of a `live` that has not flipped
INDEX_VERSION=none    # step 7: the pre-publish index version id
WOULD_PUBLISH=unknown # step 12: printed, not gated
FLIP_TO_FIRST_200_MS=n/a
EDGE_GAP_MS=0
LATEST_IMAGE=""       # measure, a11 --metric: set while $LATEST and `candidate` must be put back
LATEST_ENVIRONMENT="" # ...and what $LATEST's environment goes back to
VARIANT_VERSIONS=()   # ...and the throwaway versions to delete

now_ms() { jq -n 'now * 1000 | floor'; }

index_head() { # the index object's metadata; {} on an empty bucket
  aws s3api head-object --region "$REGION" --bucket "$INDEX_BUCKET" --key "$INDEX_KEY" \
    2>/dev/null || echo '{}'
}

index_version() { # the current index version id; `none` on an empty bucket
  index_head | jq -r '.VersionId // "none"'
}

# What every verb needs. The kill switch runs on this alone, on whatever machine is at hand when
# it is needed, so an image tool must never creep in here.
lambda_account_check() {
  need aws jq
  local account
  account=$(aws sts get-caller-identity --region "$REGION" --output json | jq -r .Account)
  [ "$account" = "$LAMBDA_ACCOUNT" ] ||
    die "refusing: these credentials are account '$account', not $LAMBDA_ACCOUNT"
}

refuse_arguments() { # $1 = verb, $2... = the arguments it was given
  local verb=$1
  shift
  [ "$#" = 0 ] || die "refusing: $verb takes no arguments, got '$*'"
}

refuse_all_but_flag() { # $1 = verb, $2 = its one flag, $3... = the arguments it was given
  local verb=$1 flag=$2
  shift 2
  case "$*" in
    "" | "$flag") ;;
    *) die "refusing: $verb takes only $flag, got '$*'" ;;
  esac
}

# Step 0 of the verbs that handle a build, after the ref check: nothing here may run for a ref
# that is about to be refused.
lambda_preflight() {
  need gh cosign crane curl
  lambda_account_check
}

# Step 5: an older image cannot run the pass, so it must never reach the registry.
refuse_unless_lambda_capable() { # $1 = digest
  image_config "$1" |
    jq -e '.config.WorkingDir == "/"
      and ((.config.Labels // {}) | has("org.openlore.indexer.lambda-root"))' >/dev/null ||
    die "refusing $IMAGE@$1: it has no Lambda root (WorkingDir / and the lambda-root label)"
}

ecr_holds() { # $1 = digest
  aws ecr describe-images --region "$REGION" --repository-name openlore-indexer \
    --image-ids "imageDigest=$1" >/dev/null 2>&1
}

ecr_login() {
  local password
  password=$(aws ecr get-login-password --region "$REGION")
  crane auth login "$ECR_REGISTRY" -u AWS --password-stdin <<<"$password"
}

# The sha-* images one more push would take past the lifecycle's count, because the lifecycle
# counts pushes and knows nothing about aliases (pure). stdin = describe-images JSON -> digests.
evicted_digests() {
  jq -r --argjson keep "$RETAINED_IMAGES" '
    [.imageDetails[]? | select(any(.imageTags[]?; startswith("sha-")))]
    | sort_by(.imagePushedAt) | reverse | .[($keep - 1):][].imageDigest'
}

alias_versions() { # $1 = function, $2 = alias -> the version it holds and the one it records
  aws lambda get-alias --region "$REGION" --function-name "$1" --name "$2" |
    jq -r "$RECORDED_PREV"'.FunctionVersion, recorded_prev'
}

# Lambda resolves a tag to a digest at create time, so ResolvedImageUri is the one to trust.
image_digest() { # stdin = get-function JSON -> the digest; fails if it names none
  jq -er '(.Code.ResolvedImageUri // .Code.ImageUri) | split("@")[1]'
}

function_references() { # $1 = function -> the digests of its aliases, its prev= and its $LATEST
  local versions version answer
  versions=$(alias_versions "$1" live) || return 1
  if [ "$1" = "$LAMBDA_SEARCH" ]; then
    versions+=$'\n'$(alias_versions "$1" candidate) || return 1
  fi
  versions=$(sort -u <<<"$versions")
  for version in $versions; do
    answer=$(aws lambda get-function --region "$REGION" --function-name "$1" \
      --qualifier "$version") || return 1
    image_digest <<<"$answer" || return 1
  done
  answer=$(aws lambda get-function --region "$REGION" --function-name "$1") || return 1
  image_digest <<<"$answer" || return 1
}

# U-STZ-9: what an image must not leave ECR for. Returns non-zero rather than dying, because the
# failed-copy cleanup runs in the EXIT trap and must not stop `cleanup`.
referenced_digests() {
  local function
  for function in "${LAMBDA_FUNCTIONS[@]}"; do
    function_references "$function" || return 1
  done
}

refuse_evicting_a_referenced_image() {
  local images evicted referenced in_use
  images=$(aws ecr describe-images --region "$REGION" --repository-name openlore-indexer)
  evicted=$(evicted_digests <<<"$images")
  [ -n "$evicted" ] || return 0
  referenced=$(referenced_digests) ||
    die "refusing: could not read which images the functions reference"
  in_use=$(grep -Fxf <(echo "$referenced") <<<"$evicted" || true)
  [ -z "$in_use" ] || die "refusing: this copy would push $in_use out of ECR, and it is in use"
}

# Step 6. A tag is immutable, so a second copy would fail: copy only what ECR does not hold.
ecr_copy() { # $1 = digest, $2 = git sha
  local copied
  if ecr_holds "$1"; then
    echo "ECR already holds $1"
    return 0
  fi
  refuse_evicting_a_referenced_image
  ecr_login
  crane copy "$IMAGE@$1" "$ECR_REPO:sha-$2"
  copied=$(crane digest "$ECR_REPO:sha-$2")
  [ "$copied" = "$1" ] || die "refusing: the copy in ECR is '$copied', not $1"
  COPIED_DIGEST=$1
}

# A deploy that stopped before step 8 finished gives its retention slot back, if nothing needs
# the image. When that cannot be read, the copy stays: a spare image costs one slot, and a
# deleted one in use costs new instances of that version.
discard_failed_copy() {
  [ -n "$COPIED_DIGEST" ] || return 0
  local referenced
  if ! referenced=$(referenced_digests); then
    echo "deploy.sh: could not read references; keeping $COPIED_DIGEST in ECR" >&2
    return 0
  fi
  if grep -Fxq "$COPIED_DIGEST" <<<"$referenced"; then
    echo "deploy.sh: keeping $COPIED_DIGEST in ECR: it is referenced" >&2
    return 0
  fi
  aws ecr batch-delete-image --region "$REGION" --repository-name openlore-indexer \
    --image-ids "imageDigest=$COPIED_DIGEST" >/dev/null ||
    echo "deploy.sh: could not delete $COPIED_DIGEST from ECR" >&2
}

live_version() { # $1 = function
  aws lambda get-alias --region "$REGION" --function-name "$1" --name live |
    jq -r .FunctionVersion
}

# Step 7: the versions and the index id every later rollback or restore starts from.
record_rollback_targets() {
  local function version
  for function in "${LAMBDA_FUNCTIONS[@]}"; do
    version=$(live_version "$function") || die "refusing: could not read the live alias of $function"
    LIVE_VERSIONS+=("$version")
  done
  INDEX_VERSION=$(index_version)
}

# Step 8. Nothing invokes $LATEST, so this is invisible to traffic.
update_images() {
  local i function digest config code_sha
  for i in "${!LAMBDA_FUNCTIONS[@]}"; do
    aws lambda update-function-code --region "$REGION" --function-name "${LAMBDA_FUNCTIONS[$i]}" \
      --image-uri "$ECR_REPO@${IMAGE_DIGESTS[$i]}" >/dev/null
  done
  for i in "${!LAMBDA_FUNCTIONS[@]}"; do
    function=${LAMBDA_FUNCTIONS[$i]}
    digest=${IMAGE_DIGESTS[$i]}
    aws lambda wait function-updated-v2 --region "$REGION" --function-name "$function"
    config=$(aws lambda get-function-configuration --region "$REGION" --function-name "$function")
    code_sha=$(jq -r .CodeSha256 <<<"$config")
    [ "$code_sha" = "${digest#sha256:}" ] ||
      die "refusing: $function \$LATEST holds $code_sha, not $digest"
    LATEST_REVISIONS+=("$(jq -r .RevisionId <<<"$config")")
  done
}

# Step 9, with --code-sha256 and --revision-id so a concurrent change cannot slip in.
publish_versions() {
  local i digest version
  for i in "${!LAMBDA_FUNCTIONS[@]}"; do
    digest=${IMAGE_DIGESTS[$i]}
    version=$(aws lambda publish-version --region "$REGION" \
      --function-name "${LAMBDA_FUNCTIONS[$i]}" --code-sha256 "${digest#sha256:}" \
      --revision-id "${LATEST_REVISIONS[$i]}" --description "${IMAGE_SHAS[$i]} $digest" |
      jq -r .Version)
    aws lambda wait published-version-active --region "$REGION" \
      --function-name "${LAMBDA_FUNCTIONS[$i]}" --qualifier "$version"
    NEW_VERSIONS+=("$version")
  done
}

# The functions whose publish answered with the version `live` already holds (pure, on the two
# version arrays). Lambda does that when code and configuration are unchanged.
unchanged_functions() {
  local i
  for i in "${!LAMBDA_FUNCTIONS[@]}"; do
    if [ "${NEW_VERSIONS[$i]}" = "${LIVE_VERSIONS[$i]}" ]; then
      echo "${LAMBDA_FUNCTIONS[$i]}"
    fi
  done
}

# A flip to the version `live` already holds would write a prev= naming that version, and the real
# rollback target would be gone. Any one is enough to refuse: a function that is ahead of the
# others from a half-finished flip reads the same as an unchanged one, and its prev= is what a
# by-hand recovery needs. Nothing has moved yet, so the aliases stay as they are.
refuse_a_publish_that_changed_nothing() {
  local unchanged
  unchanged=$(unchanged_functions)
  [ -z "$unchanged" ] ||
    die "refusing: nothing changed for ${unchanged//$'\n'/, }: each still answers with the" \
      "version live holds, so a flip would lose its rollback target; no alias moved"
}

point_candidate_at() { # $1 = version
  aws lambda update-alias --region "$REGION" --function-name "$LAMBDA_SEARCH" --name candidate \
    --function-version "$1" >/dev/null
}

move_candidate() { # step 10
  CANDIDATE_BACK_TO=${LIVE_VERSIONS[0]}
  point_candidate_at "${NEW_VERSIONS[0]}"
}

# A deploy that stopped before the flip leaves `candidate` on the live version; the new versions
# stay until a later prune. Returns zero whatever happens, like discard_failed_copy: it runs in
# the EXIT trap, and a failure there must not stop `cleanup`.
put_candidate_back() {
  [ -n "$CANDIDATE_BACK_TO" ] || return 0
  point_candidate_at "$CANDIDATE_BACK_TO" ||
    echo "deploy.sh: could not move candidate back to version $CANDIDATE_BACK_TO" >&2
}

search_url() { # $1 = alias
  local url
  url=$(aws lambda get-function-url-config --region "$REGION" --function-name "$LAMBDA_SEARCH" \
    --qualifier "$1" | jq -r .FunctionUrl)
  echo "${url%/}"
}

# Step 11: the host's post-checks against the candidate, then proof it fetched the real index:
# /healthz must name the object's published-at (both empty on an empty bucket). This is also
# what stops a deploy during `lambda stop`: the candidate answers 429.
candidate_checks() {
  local base started health served published
  base=$(search_url candidate)
  started=$(date +%s)
  health=$(curl -fsS -m 30 "$base/healthz") || die "refusing: the candidate's /healthz failed"
  echo "candidate first call: $(($(date +%s) - started)) s (includes the image cache; not gated)"
  (TMPDIR=$LAMBDA_WORK post_checks "$base") || die "refusing: the candidate failed its post-checks"
  served=$(jq -r '.last_successful_pass_at // ""' <<<"$health")
  published=$(index_head | jq -r '.Metadata["published-at"] // ""')
  [ "$served" = "$published" ] ||
    die "refusing: the candidate serves the index of '$served', the bucket holds '$published'"
}

# Synchronous, and never re-invoked by the client: a gate may run for the whole timeout.
# Prints the CLI's answer; $4 receives the function's own reply.
invoke_once() { # $1 = function, $2 = version, $3 = payload, $4 = reply file
  AWS_MAX_ATTEMPTS=1 aws lambda invoke --region "$REGION" --function-name "$1" --qualifier "$2" \
    --cli-read-timeout 0 --cli-binary-format raw-in-base64-out --log-type Tail \
    --payload "$3" "$4"
}

# Pass and probe have one slot each, so a run already in it throttles the invoke: wait it out.
# Says so on stderr, for a caller may be reading the CLI's answer from stdout.
invoke_gate() { # the arguments of invoke_once
  local errors="$LAMBDA_WORK/gate-errors" attempt=1
  until invoke_once "$@" 2>"$errors"; do
    grep -q TooManyRequestsException "$errors" ||
      die "refusing: the invoke of $1 failed: $(cat "$errors")"
    [ "$attempt" -lt "$GATE_ATTEMPTS" ] ||
      die "refusing: $1 was still busy after $GATE_ATTEMPTS attempts"
    echo "deploy.sh: $1 is busy; retrying in 30 s (attempt $attempt of $GATE_ATTEMPTS)" >&2
    attempt=$((attempt + 1))
    sleep 30
  done
}

# Step 12 (D-STZ-44). A version that does not say it ran as no-publish ignored the request and may
# have written, so that is checked first and whatever the exit.
pass_gate() {
  local reply="$LAMBDA_WORK/pass-reply" exit_code
  invoke_gate "$LAMBDA_PASS" "${NEW_VERSIONS[1]}" '{"mode":"no-publish"}' "$reply" >/dev/null
  jq -e '.mode == "no-publish" and .published == false' "$reply" >/dev/null ||
    die "refusing: the pass gate did not run as no-publish; pre-publish index $INDEX_VERSION"
  exit_code=$(jq -r .exit_code "$reply")
  WOULD_PUBLISH=$(jq -r '.would_publish // "unknown"' "$reply")
  echo "pass gate: exit $exit_code, would_publish $WOULD_PUBLISH"
  [ "$exit_code" != 3 ] || [ "${LAMBDA_ACCEPT_EXIT_3:-}" = 1 ] ||
    die "refusing: the pass gate ended in exit 3; LAMBDA_ACCEPT_EXIT_3=1 accepts that exit alone"
  [ "$exit_code" = 0 ] || [ "$exit_code" = 3 ] ||
    die "refusing: the pass gate ended in exit $exit_code"
}

health_lines() { # stdin = the CLI's answer -> the indexer.host.health lines of its log tail
  jq -r '.LogResult // ""' | base64 -d | jq -Rc 'fromjson? | select(.event? == "indexer.host.health")'
}

# One run of the probe. Prints the health line it logged; any other outcome stops the verb.
probe_health() { # $1 = version or alias
  local answer health
  answer=$(invoke_gate "$LAMBDA_PROBE" "$1" '{}' "$LAMBDA_WORK/probe-reply")
  jq -e 'has("FunctionError") | not' <<<"$answer" >/dev/null ||
    die "refusing: the probe ended in a function error"
  health=$(health_lines <<<"$answer")
  [ "$(grep -c . <<<"$health" || true)" = 1 ] ||
    die "refusing: the probe did not log exactly one health line"
  echo "$health"
}

# Step 13. This proves the new probe code and the current index, not the candidate's search:
# the probe calls the public edge, which still reaches the old `live` search.
probe_gate() {
  local health not_live
  health=$(probe_health "${NEW_VERSIONS[2]}")
  not_live=$(jq -r .not_live <<<"$health")
  echo "probe gate: not_live $not_live: $health"
  [ "$not_live" != 1 ] || [ "${LAMBDA_ACCEPT_NOT_LIVE:-}" = 1 ] ||
    die "refusing: the probe gate found the index not live: $health;" \
      "LAMBDA_ACCEPT_NOT_LIVE=1 accepts that check alone"
  [ "$not_live" = 0 ] || [ "$not_live" = 1 ] ||
    die "refusing: the probe gate's health line says not_live '$not_live': $health"
}

move_live() { # $1 = function, $2 = version, $3 = description
  aws lambda update-alias --region "$REGION" --function-name "$1" --name live \
    --function-version "$2" --description "$3" >/dev/null
}

live_record() { # $1 = function -> the description of its `live` alias
  aws lambda get-alias --region "$REGION" --function-name "$1" --name live |
    jq -r '.Description // ""'
}

# Moves `live` and records what it replaced (L-13); $2 is appended for the pass's index id.
flip_live() { # $1 = index into LAMBDA_FUNCTIONS, $2 = extra record
  move_live "${LAMBDA_FUNCTIONS[$1]}" "${NEW_VERSIONS[$1]}" \
    "prev=${LIVE_VERSIONS[$1]} ${IMAGE_SHAS[$1]} ${IMAGE_DIGESTS[$1]} $(date -u +%FT%TZ)${2:+ $2}"
}

distribution_summary() { # the list entry of the distribution serving $INDEX_HOST
  aws cloudfront list-distributions --region "$REGION" |
    jq -c --arg host "$INDEX_HOST" '.DistributionList.Items[]
      | select((.Aliases.Items // []) | index($host))'
}

distribution_field() { # $1 = field -> that field of the distribution serving $INDEX_HOST
  distribution_summary | jq -r --arg field "$1" '.[$field]'
}

search_status() { # $1 = base URL, $2 = request body file -> the status that search got
  http_status -X POST -H 'content-type: application/json' --data-binary "@$2" "$1$SEARCH_PATH"
}

response_headers() { # $1 = URL, $2 = seconds allowed; nothing when nothing answers
  curl -sS -o /dev/null -D - -m "$2" "$1" 2>/dev/null || true
}

served_by_cloudfront() { # stdin = response headers; a CloudFront `via` header marks the edge
  grep -qiE '^via:.*cloudfront'
}

# The public name once the edge serves it (a CloudFront `via` header marks it), else the
# distribution's own domain: during the trial the public name is still the host.
edge_url() {
  local headers
  headers=$(response_headers "https://$INDEX_HOST/healthz" 10)
  if served_by_cloudfront <<<"$headers"; then
    echo "https://$INDEX_HOST"
    return 0
  fi
  local domain
  domain=$(distribution_field DomainName)
  [ -n "$domain" ] || die "refusing: no distribution serves $INDEX_HOST"
  echo "https://$domain"
}

# Once a second: `<ms> ok` when /healthz and the canned search both answer 200, else `<ms> fail`.
start_edge_poller() { # $1 = edge URL
  local base="$1" body="$LAMBDA_WORK/poll-body" health search
  search_body 120 >"$body"
  : >"$EDGE_POLL_LOG"
  (
    while :; do
      health=$(http_status "$base/healthz")
      search=$(search_status "$base" "$body")
      if [ "$health" = 200 ] && [ "$search" = 200 ]; then
        echo "$(now_ms) ok" >>"$EDGE_POLL_LOG"
      else
        echo "$(now_ms) fail" >>"$EDGE_POLL_LOG"
      fi
      sleep 1
    done
  ) >/dev/null 2>&1 &
  POLL_PID=$!
}

stop_edge_poller() {
  if [ -n "$POLL_PID" ]; then kill "$POLL_PID" 2>/dev/null || true; fi
  POLL_PID=""
}

# The longest run in which a sample failed, timed from the last 200 before it to the first 200
# after it (pure). stdin = the poller's log -> ms.
longest_edge_gap() {
  awk 'NR == 1 { last_ok = $1 }
    { if (failing && $1 - last_ok > longest) longest = $1 - last_ok
      if ($2 == "ok") { last_ok = $1; failing = 0 } else failing = 1 }
    END { printf "%d\n", longest }'
}

first_200_after() { # $1 = ms (pure); stdin = the poller's log -> ms from $1 to the first ok sample
  awk -v since="$1" '$2 == "ok" && $1 >= since { printf "%d\n", $1 - since; exit }'
}

# Through the public edge: the same answers a client gets, and no sign of the raw function URL.
edge_checks() { # $1 = edge URL
  local base="$1" body="$LAMBDA_WORK/edge-body" code headers
  code=$(http_status "$base/healthz")
  [ "$code" = 200 ] || die "edge check: GET /healthz answered $code, not 200"
  search_body 120 >"$body"
  code=$(search_status "$base" "$body")
  [ "$code" = 200 ] || die "edge check: a minimal POST search answered $code, not 200"
  code=$(http_status "$base/")
  [ "$code" = 404 ] || die "edge check: GET / answered $code, not 404"
  code=$(http_status -X POST "$base/healthz")
  [ "$code" = 404 ] || die "edge check: POST /healthz answered $code, not 404"
  headers=$(response_headers "$base/healthz" 10)
  if grep -qi '^x-amzn-' <<<"$headers"; then
    die "edge check: the response carries an x-amzn- header"
  fi
  echo "edge checks passed: healthz, search, GET / 404, POST /healthz 404, no x-amzn- header"
}

# Returns non-zero rather than dying, because a failure here has a flip to undo.
edge_holds_up() { # $1 = edge URL, $2 = ms of the flip
  local checked=0
  (edge_checks "$1") || checked=1
  stop_edge_poller
  EDGE_GAP_MS=$(longest_edge_gap <"$EDGE_POLL_LOG")
  FLIP_TO_FIRST_200_MS=$(first_200_after "$2" <"$EDGE_POLL_LOG")
  [ "$checked" = 0 ] || return 1
  [ "$EDGE_GAP_MS" -le "$MAX_EDGE_GAP_MS" ] || {
    echo "deploy.sh: edge check: search was gone for $EDGE_GAP_MS ms, more than $MAX_EDGE_GAP_MS" >&2
    return 1
  }
}

# Search and its candidate go back to the version they replaced; pass and probe never flipped.
# The alias takes back its own record too, or its prev= would name the version it now holds.
# `candidate` is the EXIT trap's again, which keeps this deploy's exit status.
flip_search_back() { # $1 = the description `live` had before the flip
  local version=${LIVE_VERSIONS[0]}
  move_live "$LAMBDA_SEARCH" "$version" "$1" ||
    die "search stayed on version ${NEW_VERSIONS[0]}: could not flip live back to $version"
  CANDIDATE_BACK_TO=$version
  echo "deploy.sh: search is back on version $version; the pass and the probe did not flip" >&2
  exit 1
}

# Step 14: search flips first, watched from 5 s before the flip until the edge checks end.
flip_search() {
  local edge flipped_at record
  edge=$(edge_url)
  record=$(live_record "$LAMBDA_SEARCH")
  start_edge_poller "$edge"
  sleep 5
  flipped_at=$(now_ms)
  flip_live 0
  CANDIDATE_BACK_TO="" # `live` has caught up, so a stop from here is the flip's to undo
  edge_holds_up "$edge" "$flipped_at" || flip_search_back "$record"
}

flip_pass_and_probe() { # step 15
  flip_live 1 "idx=$INDEX_VERSION"
  flip_live 2
}

flip_summary() {
  local i
  for i in "${!LAMBDA_FUNCTIONS[@]}"; do
    echo "${LAMBDA_FUNCTIONS[$i]}: live ${LIVE_VERSIONS[$i]} -> ${NEW_VERSIONS[$i]}"
  done
}

deploy_summary() { # step 16
  flip_summary
  printf 'digest %s\n' "${IMAGE_DIGESTS[@]}" | sort -u
  echo "flip to first 200: ${FLIP_TO_FIRST_200_MS:-n/a} ms; longest gap: $EDGE_GAP_MS ms"
  echo "would_publish: $WOULD_PUBLISH"
  echo "pre-publish index version: $INDEX_VERSION"
  echo "rollback: deploy.sh lambda rollback"
  echo "restore:  deploy.sh lambda restore-index $INDEX_VERSION"
}

# The versions past the newest $RETAINED_VERSIONS that no alias, and no prev= record, names
# (pure). $1 = list-versions-by-function JSON, $2 = list-aliases JSON.
prunable_versions() {
  jq -rn --argjson versions "$1" --argjson aliases "$2" --argjson keep "$RETAINED_VERSIONS" \
    "$RECORDED_PREV"'
    [$aliases.Aliases[]? | .FunctionVersion, recorded_prev] as $referenced
    | [$versions.Versions[]?.Version | select(test("^[0-9]+$")) | tonumber]
    | sort | reverse | .[$keep:][] | tostring
    | select(. as $version | any($referenced[]; . == $version) | not)'
}

prune_function() { # $1 = function
  local versions aliases version doomed
  versions=$(aws lambda list-versions-by-function --region "$REGION" --function-name "$1") ||
    return 1
  aliases=$(aws lambda list-aliases --region "$REGION" --function-name "$1") || return 1
  doomed=$(prunable_versions "$versions" "$aliases") || return 1
  for version in $doomed; do
    aws lambda delete-function --region "$REGION" --function-name "$1" --qualifier "$version" ||
      return 1
  done
}

# The flips are done: a failed prune is worth a line, not a failed deploy.
prune_old_versions() {
  local function
  for function in "${LAMBDA_FUNCTIONS[@]}"; do
    prune_function "$function" || echo "deploy.sh: could not prune $function" >&2
  done
}

# Steps 10-16: everything after every $LATEST holds its image and the versions are published.
# A deploy and a republish share these, so a republish passes the gates a deploy passes.
gate_and_flip() {
  refuse_a_publish_that_changed_nothing
  move_candidate
  candidate_checks
  pass_gate
  probe_gate
  flip_search
  flip_pass_and_probe
  deploy_summary
  prune_old_versions
}

prepare_run() {
  LAMBDA_WORK=$(mktemp -d)
  CLEANUP_PATHS+=("$LAMBDA_WORK")
  EDGE_POLL_LOG=$LAMBDA_WORK/edge-poll
  trap 'discard_failed_copy; put_candidate_back; cleanup' EXIT
}

# Steps 0-6, shared with `seed`: a build reaches ECR by this road alone.
verify_and_copy_build() { # $1 = git sha or digest
  read_build_ref "$1"
  lambda_preflight
  prepare_run
  verify_build
  refuse_unless_lambda_capable "$BUILD_DIGEST"
  ecr_copy "$BUILD_DIGEST" "$BUILD_SHA"
}

lambda_deploy() { # $1 = git sha or digest
  verify_and_copy_build "${1:-}"
  local digest="$BUILD_DIGEST" sha="$BUILD_SHA"
  record_rollback_targets
  local function
  for function in "${LAMBDA_FUNCTIONS[@]}"; do
    IMAGE_DIGESTS+=("$digest")
    IMAGE_SHAS+=("$sha")
  done
  update_images
  COPIED_DIGEST="" # every $LATEST holds it now, so it stays whatever happens next
  publish_versions
  gate_and_flip
  echo "deployed $IMAGE@$digest to Lambda"
}

# The first apply's image, before any function exists. Nothing later can fail, and the first apply
# needs this copy, so the EXIT trap must not discard it.
lambda_seed() { # $1 = git sha or digest
  verify_and_copy_build "${1:-}"
  COPIED_DIGEST=""
  echo "bootstrap_image_uri = \"$ECR_REPO@$BUILD_DIGEST\""
}

# One version's get-function JSON, once it is known to name an image ECR still holds.
readable_version() { # $1 = function, $2 = version, $3 = the image, as a refusal calls it
  local answer digest
  answer=$(aws lambda get-function --region "$REGION" --function-name "$1" --qualifier "$2") ||
    die "refusing: could not read version $2 of $1"
  digest=$(image_digest <<<"$answer") || die "refusing: version $2 of $1 names no image"
  ecr_holds "$digest" || die "refusing: ECR does not hold $digest, $3"
  echo "$answer"
}

# The sha a version was published with, the first word of its description (pure).
# stdin = get-function JSON. A version this script did not publish names none.
version_sha() {
  jq -r '(.Configuration.Description // "" | split(" ")[0]) // "unknown"'
}

# What one function carries: the image of the version step 7 read, not of the alias, which could
# have moved since. Refuses, and never falls back: `$LATEST` may hold an unflipped image, and a
# prev= record names the version before this one. Runs for all three before any function changes.
read_live_image() { # $1 = index into LAMBDA_FUNCTIONS
  local function=${LAMBDA_FUNCTIONS[$1]} answer
  answer=$(readable_version "$function" "${LIVE_VERSIONS[$1]}" "the live image of $function")
  IMAGE_DIGESTS+=("$(image_digest <<<"$answer")")
  IMAGE_SHAS+=("$(version_sha <<<"$answer")")
}

# U-STZ-2: a published version snapshots the function's configuration, so a tofu change reaches
# `live` only through a new version. Each function keeps its own live image, which after a
# partial flip is not the same image everywhere. No CI or cosign gate: it was verified when
# deployed. No copy either, so the EXIT trap has no image to discard.
lambda_republish() {
  refuse_arguments republish "$@"
  lambda_preflight
  prepare_run
  record_rollback_targets
  local i
  for i in "${!LAMBDA_FUNCTIONS[@]}"; do
    read_live_image "$i"
  done
  update_images
  publish_versions
  gate_and_flip
  echo "republished the live images to Lambda"
}

# What one function goes back to. Refuses a missing record or a missing image, so this runs for
# all three before any alias moves. A rollback's targets were verified when first deployed.
read_rollback_target() { # $1 = function
  local versions current target
  versions=$(alias_versions "$1" live) || die "refusing: could not read the live alias of $1"
  current=$(sed -n 1p <<<"$versions")
  target=$(sed -n 2p <<<"$versions")
  [ -n "$target" ] || die "refusing: $1 records no version it replaced"
  readable_version "$1" "$target" "the image of $1 version $target" >/dev/null
  LIVE_VERSIONS+=("$current")
  NEW_VERSIONS+=("$target")
}

# The pre-publish index id the pass recorded; empty if it recorded none. Read before the flip
# rewrites that record.
recorded_index_version() {
  local record
  record=$(live_record "$LAMBDA_PASS") || die "refusing: could not read the live alias of $LAMBDA_PASS"
  jq -Rr 'capture("idx=(?<v>[^ ]+)").v' <<<"$record"
}

# No prev= in the new record: that version is what `live` now holds, so a second rollback is
# refused, not a quiet no-op. $2 keeps the pass's index id, which recovery reads after a rollback.
flip_back() { # $1 = index into LAMBDA_FUNCTIONS, $2 = extra record
  local function=${LAMBDA_FUNCTIONS[$1]}
  move_live "$function" "${NEW_VERSIONS[$1]}" \
    "rolled back from ${LIVE_VERSIONS[$1]} $(date -u +%FT%TZ)${2:+ $2}" ||
    die "$function stayed on version ${LIVE_VERSIONS[$1]}: could not flip live back;" \
      "any function before it in search, pass, probe order is already back"
}

lambda_rollback() {
  refuse_arguments rollback "$@"
  lambda_account_check
  local function index_version
  for function in "${LAMBDA_FUNCTIONS[@]}"; do
    read_rollback_target "$function"
  done
  index_version=$(recorded_index_version)
  flip_back 0
  flip_back 1 "${index_version:+idx=$index_version}"
  flip_back 2
  point_candidate_at "${NEW_VERSIONS[0]}" ||
    die "every live is back, but candidate did not follow to version ${NEW_VERSIONS[0]}"
  flip_summary
  echo "pre-publish index version: ${index_version:-none}"
}

set_concurrency() { # $1 = function, $2 = reserved concurrency
  aws lambda put-function-concurrency --region "$REGION" --function-name "$1" \
    --reserved-concurrent-executions "$2" >/dev/null
  echo "$1: reserved concurrency $2"
}

# Under the If-Match of the config it read, so a concurrent change refuses this one. Returns
# non-zero rather than dying: search has already changed, and the caller says so.
set_edge_enabled() { # $1 = true or false
  local id answer etag config
  id=$(distribution_field Id) || return 1
  [ -n "$id" ] || return 1
  answer=$(aws cloudfront get-distribution-config --region "$REGION" --id "$id") || return 1
  etag=$(jq -r .ETag <<<"$answer") || return 1
  config=$(jq -c --argjson enabled "$1" '.DistributionConfig | .Enabled = $enabled' <<<"$answer") ||
    return 1
  aws cloudfront update-distribution --region "$REGION" --id "$id" --if-match "$etag" \
    --distribution-config "$config" >/dev/null || return 1
  echo "$id: enabled $1, reaching the edge in minutes; watch it with deploy.sh lambda status"
}

# D-STZ-18d. Search first, for it takes seconds and stops the spend. Pass and probe keep running,
# so the index stays fresh, and A3 pages for as long as this lasts.
lambda_stop() { # [--wait]
  refuse_all_but_flag stop --wait "$@"
  lambda_account_check
  set_concurrency "$LAMBDA_SEARCH" 0
  set_edge_enabled false ||
    die "search is stopped, but the edge is not: could not disable the distribution; run stop again"
  if [ "${1:-}" = --wait ]; then
    aws cloudfront wait distribution-deployed --region "$REGION" --id "$(distribution_field Id)"
    echo "the distribution is deployed"
  fi
}

# Restores what stop changed and nothing else, and is safe to run twice.
lambda_start() {
  refuse_arguments start "$@"
  lambda_account_check
  set_concurrency "$LAMBDA_SEARCH" "$SEARCH_CONCURRENCY"
  set_edge_enabled true ||
    die "search is back, but the edge is not: could not enable the distribution; run start again"
}

# One function, and only its concurrency: 0 for maintenance, 1 as tofu sets it, so an apply in
# between turns the function back on. $3.. are the verb's arguments.
lambda_switch() { # $1 = function, $2 = reserved concurrency
  [ "$#" = 2 ] || die "refusing: this switch takes no arguments, got '${*:3}'"
  lambda_account_check
  set_concurrency "$1" "$2"
}

# A real pass on `live`, after any pass already holding the one slot. Exits with the pass's exit
# code, so it ends the verb.
run_pass() {
  local reply="$LAMBDA_WORK/pass-reply" answer exit_code
  answer=$(invoke_gate "$LAMBDA_PASS" live '{}' "$reply")
  jq -e 'has("FunctionError") | not' <<<"$answer" >/dev/null ||
    die "the pass ended in a function error: $(cat "$reply")"
  exit_code=$(jq -r .exit_code "$reply")
  [[ "$exit_code" =~ ^[0-9]+$ ]] || die "the pass replied with no exit code: $(cat "$reply")"
  echo "pass $(jq -r .pass_id "$reply"): exit $exit_code"
  exit "$exit_code"
}

lambda_trigger() {
  refuse_arguments trigger "$@"
  lambda_account_check
  prepare_run
  run_pass
}

# The line says what the probe saw, not_live included; judging it is the operator's.
lambda_probe() {
  refuse_arguments probe "$@"
  lambda_account_check
  prepare_run
  probe_health live
}

# D-STZ-27. Deleting with no version id leaves a delete marker, so every version stays and the
# next pass rebuilds with If-None-Match: *. The empty IFS keeps a stray space in the answer: the
# word must be exact. A closed stdin reads as no word.
lambda_reset_index() { # [--trigger]
  refuse_all_but_flag reset-index --trigger "$@"
  lambda_account_check
  prepare_run
  local word="" version
  echo "This deletes s3://$INDEX_BUCKET/$INDEX_KEY. Type reset to go on:" >&2
  IFS= read -r word || true
  [ "$word" = reset ] || die "refusing: reset-index needs the exact word reset, got '$word'"
  version=$(index_version)
  aws s3api delete-object --region "$REGION" --bucket "$INDEX_BUCKET" --key "$INDEX_KEY" >/dev/null
  echo "index deleted; its version is now noncurrent: $version"
  echo "restore: deploy.sh lambda restore-index $version"
  if [ "${1:-}" = --trigger ]; then
    echo "search serves an empty index within 60 s; this pass rebuilds it"
    run_pass
  fi
  echo "search serves an empty index within 60 s; the next pass rebuilds it, and A10 can fire once"
}

# Within a minute of a quarter hour, when the pass schedule (cron 0/15) has one due or running
# (pure). Base 10, or 08 and 09 are bad octal.
pass_due_or_running() { # $1 = minute of the hour, two digits
  case $((10#$1 % 15)) in
    14 | 0 | 1) return 0 ;;
    *) return 1 ;;
  esac
}

# D-STZ-27. The copy is a new current version with the source's metadata, and the source stays.
# The next pass ingests on top of it, so this is a starting point, not a pin.
lambda_restore_index() { # $1 = version id
  [[ "$#" = 1 && -n "$1" ]] || die "refusing: restore-index takes one version id, got '$*'"
  lambda_account_check
  local version=$1 head minute copied
  head=$(aws s3api head-object --region "$REGION" --bucket "$INDEX_BUCKET" --key "$INDEX_KEY" \
    --version-id "$version") || die "refusing: could not read version $version of the index"
  jq -r --arg version "$version" \
    '"version \($version): \(.ContentLength) bytes, published at \(.Metadata["published-at"] // "unknown")"' \
    <<<"$head"
  minute=$(date -u +%M)
  if pass_due_or_running "$minute"; then
    die "refusing: minute $minute is next to a quarter hour, when a pass is due or running"
  fi
  copied=$(aws s3api copy-object --region "$REGION" --bucket "$INDEX_BUCKET" --key "$INDEX_KEY" \
    --copy-source "$INDEX_BUCKET/$INDEX_KEY?versionId=$version")
  echo "index restored from $version as version $(jq -r .VersionId <<<"$copied")"
  echo "until the next pass rewrites the record, A10 can fire once"
}

# Where `live` logs says which phase this is, and so which group and which A1-A3 are current. The
# alias, not $LATEST: an apply reaches `live` only with the republish after it.
use_current_phase() {
  local group
  group=$(aws lambda get-function-configuration --region "$REGION" --function-name "$LAMBDA_PASS" \
    --qualifier live | jq -r '.LoggingConfig.LogGroup // ""') || group=""
  [ -n "$group" ] || die "refusing: could not read where $LAMBDA_PASS logs"
  LOG_GROUP=$group
  if [ "$group" = "$TRIAL_LOG_GROUP" ]; then
    ALARM_A1=openlore-indexer-lambda-total-outage
    ALARM_A2=openlore-indexer-lambda-pass-failed
    ALARM_A3=openlore-indexer-lambda-not-live
  fi
}

reserved_concurrency() { # $1 = function
  aws lambda get-function-concurrency --region "$REGION" --function-name "$1" |
    jq -r '.ReservedConcurrentExecutions // "unset"'
}

required_concurrency() { # $1 = function (pure)
  if [ "$1" = "$LAMBDA_SEARCH" ]; then echo "$SEARCH_CONCURRENCY"; else echo "$ONE_SLOT"; fi
}

concurrency_report() { # $1 = function, $2 = reserved concurrency (pure)
  local required
  required=$(required_concurrency "$1")
  case "$2/$1" in
    "$required/$1") echo "reserved concurrency $2" ;;
    "0/$LAMBDA_SEARCH") echo "reserved concurrency 0: stopped" ;;
    0/*) echo "reserved concurrency 0: off" ;;
    *) echo "reserved concurrency $2: DIFFERS from the required $required" ;;
  esac
}

# What a published version snapshots, so what differs between $LATEST and `live` (pure).
# stdin = get-function-configuration JSON.
config_snapshot() {
  jq -cS '{Environment, MemorySize, Timeout, LoggingConfig, EphemeralStorage, ImageConfigResponse}'
}

drift_report() { # $1 = function
  local config_latest config_live code_latest code_live
  config_latest=$(aws lambda get-function-configuration --region "$REGION" --function-name "$1")
  config_live=$(aws lambda get-function-configuration --region "$REGION" --function-name "$1" \
    --qualifier live)
  code_latest=$(aws lambda get-function --region "$REGION" --function-name "$1")
  code_live=$(aws lambda get-function --region "$REGION" --function-name "$1" --qualifier live)
  if [ "$(config_snapshot <<<"$config_latest")" != "$(config_snapshot <<<"$config_live")" ]; then
    echo "DRIFT: \$LATEST's configuration differs from live: run republish"
  fi
  local image_latest image_live
  image_latest=$(image_digest <<<"$code_latest")
  image_live=$(image_digest <<<"$code_live")
  if [ "$image_latest" != "$image_live" ]; then
    echo "DRIFT: \$LATEST holds $image_latest, live $image_live: run republish"
  fi
}

status_function() { # $1 = function
  echo "--- $1"
  aws lambda get-alias --region "$REGION" --function-name "$1" --name live |
    jq -r '"live \(.FunctionVersion): \(.Description // "")"'
  if [ "$1" = "$LAMBDA_SEARCH" ]; then
    aws lambda get-alias --region "$REGION" --function-name "$1" --name candidate |
      jq -r '"candidate \(.FunctionVersion): \(.Description // "")"'
  fi
  concurrency_report "$1" "$(reserved_concurrency "$1")"
  drift_report "$1"
}

status_search() { status_function "$LAMBDA_SEARCH"; }
status_pass() { status_function "$LAMBDA_PASS"; }
status_probe() { status_function "$LAMBDA_PROBE"; }

status_schedules() {
  local schedule
  echo "--- schedules"
  for schedule in "$LAMBDA_PASS" "$LAMBDA_PROBE"; do
    aws scheduler get-schedule --region "$REGION" --group-name "$SCHEDULE_GROUP" \
      --name "$schedule" | jq -r --arg name "$schedule" '"\($name): \(.State)"'
  done
}

status_logs() {
  local passes probe
  passes=$(insights_start 172800 'filter event = "indexer.ingest.pass_summary" and pass_id not like /^TEST/ | fields @timestamp, exit_code, cause, pass_id, no_publish | sort @timestamp desc | limit 8')
  probe=$(insights_start 3600 "$PROBE_LINES | fields @message | sort @timestamp desc | limit 1")
  echo "--- last 8 passes in $LOG_GROUP (newest first)"
  insights_wait "$passes" |
    jq -r '"\(."@timestamp")  exit \(.exit_code)  \(.cause // "")  \(.pass_id)\(if .no_publish then "  (no-publish gate)" else "" end)"'
  echo "--- last probe line"
  insights_wait "$probe" | jq -r '."@message"'
}

status_index() {
  echo "--- index object"
  index_head |
    jq -r '"etag \(.ETag // "none"), index_bytes \(.ContentLength // 0), published at \(.Metadata["published-at"] // "none")"'
  echo "--- last-pass.json"
  aws s3 cp --region "$REGION" "s3://$INDEX_BUCKET/$LAST_PASS_KEY" - 2>/dev/null || echo "(none)"
  echo "--- noncurrent index versions"
  aws s3api list-object-versions --region "$REGION" --bucket "$INDEX_BUCKET" --prefix "$INDEX_KEY" |
    jq -r '.Versions[]? | select(.IsLatest | not) | "\(.VersionId)  \(.Size) bytes  \(.LastModified)"'
}

status_alarms() {
  echo "--- alarms"
  aws cloudwatch describe-alarms --region "$REGION" --alarm-names "$ALARM_A1" "$ALARM_A2" \
    "$ALARM_A3" "${LAMBDA_ONLY_ALARMS[@]}" |
    jq -r '.MetricAlarms[] | "\(.AlarmName): \(.StateValue)"'
}

budget_report() {
  aws budgets describe-budget --region "$REGION" --account-id "$LAMBDA_ACCOUNT" \
    --budget-name "$BUDGET" |
    jq -r '"budget actual this month: \(.Budget.CalculatedSpend.ActualSpend.Amount // "unknown") USD"'
}

# One half of the kill switch without the other (pure).
partial_stop() { # $1 = search concurrency, $2 = distribution enabled
  case "$1/$2" in
    0/true) return 0 ;;
    0/*) return 1 ;;
    */false) return 0 ;;
    *) return 1 ;;
  esac
}

status_edge() {
  local summary enabled concurrency
  summary=$(distribution_summary)
  enabled=$(jq -r .Enabled <<<"$summary")
  echo "--- edge"
  jq -r '"distribution \(.Id): \(.Status), \(.DomainName), enabled \(.Enabled)"' <<<"$summary"
  concurrency=$(reserved_concurrency "$LAMBDA_SEARCH")
  if partial_stop "$concurrency" "$enabled"; then
    echo "PARTIAL STOP: search concurrency $concurrency, edge enabled $enabled;" \
      "run stop or start again"
  fi
}

metric_seen() { # $@ = list-metrics filters -> "seen" or "not seen"
  local answer
  answer=$(aws cloudwatch list-metrics --region "$REGION" "$@") || answer='{}'
  if jq -e '(.Metrics // []) | length > 0' <<<"$answer" >/dev/null; then
    echo seen
  else
    echo "not seen"
  fi
}

# A healthy `live` never has a 5xx, so A11's seen-check reads the candidate series, and the alarm's
# own `:live` dimension is checked for shape alone. A list-metrics filter names every dimension,
# or it would also match a series that has more.
status_metrics() {
  local id alarm
  id=$(distribution_field Id)
  echo "--- metric check"
  echo "A8 Invocations FunctionName=$LAMBDA_SEARCH: $(metric_seen --namespace AWS/Lambda \
    --metric-name Invocations --dimensions "Name=FunctionName,Value=$LAMBDA_SEARCH")"
  echo "A9 Requests Region=Global DistributionId=$id: $(metric_seen --namespace AWS/CloudFront \
    --metric-name Requests --dimensions Name=Region,Value=Global "Name=DistributionId,Value=$id")"
  echo "A11 Url5xxCount Resource=$LAMBDA_SEARCH:candidate: $(metric_seen --namespace AWS/Lambda \
    --metric-name Url5xxCount --dimensions "Name=FunctionName,Value=$LAMBDA_SEARCH" \
    "Name=Resource,Value=$LAMBDA_SEARCH:candidate")"
  alarm=$(aws cloudwatch describe-alarms --region "$REGION" --alarm-names "$ALARM_A11")
  if jq -e --arg series "$LAMBDA_SEARCH:live" '.MetricAlarms[0]
    | .MetricName == "Url5xxCount"
      and any(.Dimensions[]?; .Name == "Resource" and .Value == $series)' <<<"$alarm" >/dev/null; then
    echo "A11's alarm watches the same metric on $LAMBDA_SEARCH:live"
  else
    echo "A11's alarm does NOT watch the same metric on $LAMBDA_SEARCH:live"
  fi
}

# Each section runs at once in its own process, so the report stays within the verb's 10 s, and a
# read that fails costs that section alone. They print in the order given.
status_report() { # $@ = one function per section
  local section i=0 pids=()
  for section in "$@"; do
    "$section" >"$LAMBDA_WORK/status-$i" 2>&1 &
    pids+=("$!")
    i=$((i + 1))
  done
  for i in "${!pids[@]}"; do
    local result=0
    wait "${pids[$i]}" || result=$?
    cat "$LAMBDA_WORK/status-$i"
    [ "$result" = 0 ] || echo "(this section stopped early: a read failed)"
  done
}

lambda_status() {
  refuse_arguments status "$@"
  lambda_account_check
  prepare_run
  use_current_phase
  status_report status_search status_pass status_probe status_schedules status_logs status_index \
    status_alarms budget_report status_edge status_metrics
}

# A throwaway version of search on `candidate`, for `measure` and `a11 --metric`, built as a
# republish builds one: from the digest already live, whatever $LATEST holds. EXIT must run
# end_search_variants, which puts back everything this touches. `live` is never moved.
begin_search_variants() {
  local version answer config
  version=$(live_version "$LAMBDA_SEARCH") || die "refusing: could not read the live alias"
  LIVE_VERSIONS+=("$version")
  read_live_image 0
  answer=$(aws lambda get-function --region "$REGION" --function-name "$LAMBDA_SEARCH") ||
    die "refusing: could not read \$LATEST of $LAMBDA_SEARCH"
  config=$(aws lambda get-function-configuration --region "$REGION" \
    --function-name "$LAMBDA_SEARCH") || die "refusing: could not read \$LATEST of $LAMBDA_SEARCH"
  CANDIDATE_BACK_TO=$(alias_versions "$LAMBDA_SEARCH" candidate | sed -n 1p)
  LATEST_ENVIRONMENT=$(jq -c '.Environment // {Variables: {}}' <<<"$config")
  LATEST_IMAGE=$(image_digest <<<"$answer")
  set_search_image "${IMAGE_DIGESTS[0]}"
}

set_search_image() { # $1 = digest
  aws lambda update-function-code --region "$REGION" --function-name "$LAMBDA_SEARCH" \
    --image-uri "$ECR_REPO@$1" >/dev/null
  aws lambda wait function-updated-v2 --region "$REGION" --function-name "$LAMBDA_SEARCH"
}

set_search_environment() { # $1 = the Environment JSON
  aws lambda update-function-configuration --region "$REGION" --function-name "$LAMBDA_SEARCH" \
    --environment "$1" >/dev/null
  aws lambda wait function-updated-v2 --region "$REGION" --function-name "$LAMBDA_SEARCH"
}

# A version is a new one only if its configuration differs, so each caller changes a value.
publish_search_variant() { # $1 = JSON object of the variables to change
  local digest=${IMAGE_DIGESTS[0]} revision version
  set_search_environment "$(jq -c --argjson changes "$1" '.Variables += $changes' \
    <<<"$LATEST_ENVIRONMENT")"
  revision=$(aws lambda get-function-configuration --region "$REGION" \
    --function-name "$LAMBDA_SEARCH" | jq -r .RevisionId)
  version=$(aws lambda publish-version --region "$REGION" --function-name "$LAMBDA_SEARCH" \
    --code-sha256 "${digest#sha256:}" --revision-id "$revision" \
    --description "throwaway: lambda measure or test-alarm" | jq -r .Version)
  aws lambda wait published-version-active --region "$REGION" --function-name "$LAMBDA_SEARCH" \
    --qualifier "$version"
  VARIANT_VERSIONS+=("$version")
  point_candidate_at "$version"
}

# Returns non-zero when something stayed behind, and goes on regardless: it runs in the EXIT trap,
# and each thing left is one the operator must hear about. Safe to run twice.
end_search_variants() {
  [ -n "$LATEST_IMAGE" ] || return 0
  local version left=0
  put_candidate_back
  CANDIDATE_BACK_TO=""
  for version in ${VARIANT_VERSIONS[@]+"${VARIANT_VERSIONS[@]}"}; do
    aws lambda delete-function --region "$REGION" --function-name "$LAMBDA_SEARCH" \
      --qualifier "$version" >/dev/null || {
      echo "deploy.sh: could not delete version $version of $LAMBDA_SEARCH" >&2
      left=1
    }
  done
  VARIANT_VERSIONS=()
  set_search_environment "$LATEST_ENVIRONMENT" || {
    echo "deploy.sh: could not put \$LATEST's environment back: $LATEST_ENVIRONMENT" >&2
    left=1
  }
  set_search_image "$LATEST_IMAGE" || {
    echo "deploy.sh: could not put \$LATEST's image back: $LATEST_IMAGE" >&2
    left=1
  }
  LATEST_IMAGE=""
  return "$left"
}

timed_call() { # $@ = curl args -> `<status> <seconds>`
  curl -sS -o /dev/null -m 30 -w '%{http_code} %{time_total}' "$@" 2>/dev/null || true
}

timed_search() { # $1 = base URL
  timed_call -X POST -H 'content-type: application/json' --data-binary "@$MEASURE_BODY" \
    "$1$SEARCH_PATH"
}

measure_call() { # $1 = path, $2 = kind, $3 = base URL
  echo "$1 $2 $(timed_search "$3")" >>"$MEASURE_LOG"
}

# The first call on a path is what warms it, so it is not a warm call.
measure_warm_calls() { # $1 = path, $2 = base URL
  local call
  timed_search "$2" >/dev/null
  for ((call = 1; call <= MEASURE_WARM_CALLS; call++)); do
    measure_call "$1" warm "$2"
  done
}

# The gate (pure). stdin = samples `<path> <kind> <status> <seconds>`. Prints each group's figures
# and exits 1 unless every group has all its calls, each a 200 under its limit.
lambda_measure_verdict() {
  sort -k1,2 -k4,4n | awk -v cold_limit="$COLD_LIMIT_S" -v warm_limit="$WARM_LIMIT_S" \
    -v cold_calls="$MEASURE_COLD_CALLS" -v warm_calls="$MEASURE_WARM_CALLS" '
    function report(group, calls, limit,    count) {
      count = seen[group] + 0
      printf "measure: %s: %d of %d calls, min %s s, median %s s, max %s s, %d failed the %s s limit\n",
        group, count, calls, times[group, 1], times[group, int((count + 1) / 2)],
        times[group, count], failed[group] + 0, limit
      if (count != calls || failed[group] > 0) verdict = 1
    }
    { group = $1 " " $2
      limit = ($2 == "cold") ? cold_limit : warm_limit
      times[group, ++seen[group]] = $4
      if ($3 != 200 || $4 == "" || $4 + 0 >= limit + 0) failed[group]++ }
    END { report("raw cold", cold_calls, cold_limit)
      report("raw warm", warm_calls, warm_limit)
      report("edge warm", warm_calls, warm_limit)
      exit verdict }'
}

refuse_unless_at_required_concurrency() { # $1 = function
  local reserved required
  reserved=$(reserved_concurrency "$1")
  required=$(required_concurrency "$1")
  [ "$reserved" = "$required" ] ||
    die "refusing: $1 has reserved concurrency $reserved, not $required"
}

# For DELIVER's evidence only: logs lag, so a figure may be missing, and it never moves the gate.
report_init_durations() {
  use_current_phase
  echo "measure: recent init durations (ms):"
  insights_wait "$(insights_start 900 "$COLD_STARTS | fields record.metrics.initDurationMs as init_ms | sort @timestamp desc | limit 10")" |
    jq -r '"  \(.init_ms)"'
}

measure_cold_calls() { # $1 = candidate URL; each call gets a fresh version, so a cold instance
  local call nonce
  for ((call = 1; call <= MEASURE_COLD_CALLS; call++)); do
    nonce=$(jq -cn --arg nonce "$(now_ms)-$call" '{OPENLORE_MEASURE_NONCE: $nonce}')
    publish_search_variant "$nonce"
    measure_call raw cold "$1"
  done
}

# Any way out but a pass is exit 1, die's 2 included, for that is what blocks Stage 2.
measure_stopped() {
  local status=$?
  end_search_variants || status=1
  cleanup
  [ "$status" = 0 ] || exit 1
}

# P-5. Run from the operator's laptop, the client R-STZ-1 and R-STZ-2 mean here. The distribution's
# only origin is the `live` URL, and the cold calls need a fresh version, which only `candidate`
# may hold. So the cold calls take the raw path, and the warm calls take both.
lambda_measure() {
  prepare_run
  trap measure_stopped EXIT
  refuse_arguments measure "$@"
  need curl
  lambda_account_check
  refuse_unless_at_required_concurrency "$LAMBDA_SEARCH"
  refuse_unless_at_required_concurrency "$LAMBDA_PASS"
  MEASURE_BODY=$LAMBDA_WORK/measure-body
  MEASURE_LOG=$LAMBDA_WORK/measure-log
  search_body 120 >"$MEASURE_BODY"
  : >"$MEASURE_LOG"
  local domain raw candidate verdict=0
  domain=$(distribution_field DomainName)
  [ -n "$domain" ] || die "refusing: no distribution serves $INDEX_HOST"
  raw=$(search_url live)
  candidate=$(search_url candidate)
  begin_search_variants
  measure_cold_calls "$candidate"
  end_search_variants
  measure_warm_calls raw "$raw"
  measure_warm_calls edge "https://$domain"
  echo "measure: edge 404 round trip, for information: $(timed_call "https://$domain/")"
  (report_init_durations) || echo "measure: no init durations to show"
  echo "measure: cold calls take the raw path alone: the edge reaches only live"
  lambda_measure_verdict <"$MEASURE_LOG" || verdict=1
  if [ "$verdict" = 0 ]; then
    echo "measure: PASS ${IMAGE_DIGESTS[0]}"
  else
    echo "measure: FAIL ${IMAGE_DIGESTS[0]}"
    exit 1
  fi
}

insights_rows() { # $1 = window seconds, $2 = query, $3 = jq filter for one row
  local rows
  rows=$(insights_wait "$(insights_start "$1" "$2")")
  if [ -n "$rows" ]; then jq -r "$3" <<<"$rows"; else echo "  (none)"; fi
}

transition_count() { # $1 = alarm, $2 = since (Unix s)
  echo "  $1: $(alarm_transitions "$1" "$2" | jq length)"
}

kpi_passes() { # $1 = days, $2 = window seconds, $3 = since, $4 = until (Unix s)
  local q_ok q_fail ok_times fail_times a2
  q_ok=$(insights_start "$2" "$KPI_PASSES and exit_code = 0 | fields @timestamp | limit 10000")
  q_fail=$(insights_start "$2" "$KPI_PASSES and exit_code = 2 | fields @timestamp | limit 10000")
  ok_times=$(insights_wait "$q_ok" | insights_epoch)
  fail_times=$(insights_wait "$q_fail" | insights_epoch | jq -sc .)
  a2=$(alarm_transitions "$ALARM_A2" "$3")
  echo "--- KPIs over the last $1 day(s), $LOG_GROUP"
  freshness_ratio "$3" "$4" <<<"$ok_times"
  unalerted_failures "$fail_times" "$a2"
}

kpi_alarm_transitions() { # $1 = since (Unix s)
  local alarm
  echo "ALARM transitions:"
  for alarm in "$ALARM_A1" "$ALARM_A2" "$ALARM_A3" "${LAMBDA_ONLY_ALARMS[@]}"; do
    transition_count "$alarm" "$1"
  done
}

kpi_platform_figures() { # $1 = window seconds
  echo "cold starts (KPI-STZ-G1):"
  insights_rows "$1" "$COLD_STARTS | stats count() as cold_starts, pct(record.metrics.initDurationMs, 95) as init_p95_ms" \
    '"  \(.cold_starts) cold starts, init p95 \(.init_p95_ms) ms (target < 2000)"'
  echo "warm search (KPI-STZ-G2):"
  insights_rows "$1" "$PROBE_LINES | stats pct(search_ms, 95) as search_p95_ms" \
    '"  probe search_ms p95 \(.search_p95_ms) (target < 200)"'
  echo "max memory (KPI-STZ-G5):"
  insights_rows "$1" 'filter type = "platform.report" | parse @logStream /\/(?<function_name>openlore-indexer-[a-z]+)\[/ | stats max(record.metrics.maxMemoryUsedMB) as max_memory_mb by function_name' \
    '"  \(.function_name): \(.max_memory_mb) MB"'
}

# laptop_kpi's figures, with its own queries: they must skip the gate's lines, and they read the
# current group. The prod root's saved query does neither.
lambda_kpi() { # $1 = days (1-30)
  local days="${1:-7}"
  if [ "$#" -gt 1 ] || ! [[ "$days" =~ ^[0-9]+$ ]] || [ "$days" -lt 1 ] || [ "$days" -gt 30 ]; then
    die "refusing: kpi takes one count of days, 1-30, got '$*'"
  fi
  lambda_account_check
  use_current_phase
  local now window start
  now=$(date +%s)
  window=$((days * 86400))
  start=$((now - window))
  kpi_passes "$days" "$window" "$start" "$now"
  kpi_alarm_transitions "$start"
  kpi_platform_figures "$window"
  budget_report
}

# Sorted, so the order two stores happen to answer in cannot count as a difference.
parity_rows() { # $1 = base URL, $2 = DID -> the sorted (author_did, cid) pairs
  local request
  request=$(jq -cn --arg value "$2" '{dimension: "contributor", value: $value}')
  curl -fsS -m 10 -X POST -H 'content-type: application/json' --data "$request" \
    "$1$SEARCH_PATH" | jq -c '[.results[] | [.author_did, .cid]] | sort'
}

# One DID's search on both, reporting how they compare; non-zero when they differ.
parity_search() { # $1 = edge URL, $2 = DID
  local host_rows edge_rows
  host_rows=$(parity_rows "https://$INDEX_HOST" "$2") ||
    die "refusing: the host did not answer the search for $2"
  edge_rows=$(parity_rows "$1" "$2") ||
    die "refusing: the edge did not answer the search for $2"
  if [ "$host_rows" = "$edge_rows" ]; then
    echo "parity: $2: $(jq length <<<"$host_rows") rows match"
    return 0
  fi
  echo "parity: $2: host $(jq length <<<"$host_rows") rows, edge $(jq length <<<"$edge_rows")," \
    "and they differ"
  return 1
}

# Trial only. Three authors from the DID list are three searches with real rows to compare, which
# the probe's canned search, built to match nothing, is not. A pass publishes at a quarter hour, so
# minute 7 of one leaves the two indexes settled; the rerun 15 min later is minute 7 of the next.
lambda_parity() {
  refuse_arguments parity "$@"
  need curl
  lambda_account_check
  local minute edge dids did differs=0
  minute=$(date -u +%M)
  [ "$((10#$minute % 15))" = 7 ] ||
    die "refusing: minute $minute is not 7 minutes past a quarter hour"
  edge=$(edge_url)
  [ "$edge" != "https://$INDEX_HOST" ] ||
    die "refusing: parity is for the trial; the public name already serves the edge"
  dids=$(aws ssm get-parameter --region "$REGION" --name "$DID_LIST_PARAM" --with-decryption \
    --query Parameter.Value --output text | tr ',' '\n' | head -n "$PARITY_SEARCHES")
  for did in $dids; do
    parity_search "$edge" "$did" || differs=1
  done
  [ "$differs" = 0 ] || {
    echo "parity: FAIL; if a pass was in flight, run it once more 15 min from now" >&2
    exit 1
  }
  echo "parity: PASS"
}

put_test_line() { # $1 = one log line; the metric filters read the whole group, this stream too
  aws logs create-log-stream --region "$REGION" --log-group-name "$LOG_GROUP" \
    --log-stream-name test-fire 2>/dev/null || true
  aws logs put-log-events --region "$REGION" --log-group-name "$LOG_GROUP" \
    --log-stream-name test-fire --log-events "$(jq -cn --argjson t "$(date +%s)000" \
      --arg m "$1" '[{timestamp: $t, message: $m}]')" >/dev/null
  echo "$(date -u +%H:%M:%S) fired $1"
}

fire_and_clear() { # $1 = alarm; exits 1 if either does not arrive within ALARM_WAIT_S
  await_alarm "$1" ALARM "$ALARM_WAIT_S" || exit 1
  await_alarm "$1" OK "$ALARM_WAIT_S" || exit 1
}

# The notification path only. The metric path rests on the metric check in `lambda status`.
notify_alarm() { # $1 = alarm
  aws cloudwatch set-alarm-state --region "$REGION" --alarm-name "$1" --state-value ALARM \
    --state-reason "deploy.sh lambda test-alarm"
  await_alarm "$1" ALARM "$ALARM_WAIT_S" || exit 1
  aws cloudwatch set-alarm-state --region "$REGION" --alarm-name "$1" --state-value OK \
    --state-reason "deploy.sh lambda test-alarm"
  echo "$1: notification path only; the metric check in lambda status covers its metric"
}

# Four synthetic quarters (3, 0, 3, 3) as test_alarm_a1 fires on the host, with the real passes off
# so none of them lands in a quarter under test.
test_alarm_lambda_a1() {
  set_concurrency "$LAMBDA_PASS" 0
  trap 'set_concurrency "$LAMBDA_PASS" "$ONE_SLOT"; cleanup' EXIT
  fire_test_line 3 TEST-a1-1
  next_quarter && fire_test_line 0 TEST-a1-2
  next_quarter && fire_test_line 3 TEST-a1-3
  next_quarter && fire_test_line 3 TEST-a1-4
  local fired=0
  await_alarm "$ALARM_A1" ALARM "$ALARM_WAIT_S" || fired=1
  next_quarter && fire_test_line 0 TEST-a1-5
  set_concurrency "$LAMBDA_PASS" "$ONE_SLOT"
  trap cleanup EXIT
  [ "$fired" = 0 ] || exit 1
  await_alarm "$ALARM_A1" OK "$ALARM_WAIT_S" || exit 1
}

# A synthetic line keeps this off the shared SSM list, which the live host also reads.
test_alarm_lambda_a2() {
  fire_test_line 2 TEST-a2
  fire_and_clear "$ALARM_A2"
}

test_alarm_lambda_a3() { # two periods of not_live, and the real probe lines clear it
  put_test_line '{"event":"indexer.host.health","pass_id":"TEST-a3-1","not_live":1}'
  sleep 300
  put_test_line '{"event":"indexer.host.health","pass_id":"TEST-a3-2","not_live":1}'
  fire_and_clear "$ALARM_A3"
}

test_alarm_a5() { # missing data is ignored, so only a smaller publish clears it
  put_test_line "{\"event\":\"indexer.lambda.published\",\"pass_id\":\"TEST-a5-1\",\"index_bytes\":$OVERSIZED_INDEX_BYTES}"
  await_alarm "$ALARM_A5" ALARM "$ALARM_WAIT_S" || exit 1
  put_test_line '{"event":"indexer.lambda.published","pass_id":"TEST-a5-2","index_bytes":1}'
  await_alarm "$ALARM_A5" OK "$ALARM_WAIT_S" || exit 1
}

test_alarm_a10() {
  local line
  for line in 1 2 3; do
    put_test_line "{\"event\":\"indexer.lambda.freshness_check_failed\",\"pass_id\":\"TEST-a10-$line\"}"
  done
  fire_and_clear "$ALARM_A10"
}

# U-STZ-7: a list is not an object, so the probe's handler refuses it and does nothing else. The
# probe, because it holds no write port, so a slip here cannot touch the single writer.
test_alarm_errors() {
  local answer
  answer=$(invoke_gate "$LAMBDA_PROBE" live '[]' "$LAMBDA_WORK/drill-reply")
  echo "errors drill: the failing invoke landed at $(date -u +%FT%TZ)"
  jq -e 'has("FunctionError")' <<<"$answer" >/dev/null ||
    die "refusing: the probe took the list without a function error, so A6 has nothing to see"
  fire_and_clear "$ALARM_A6"
}

test_alarm_throttles() {
  local raw call
  need curl
  raw=$(search_url live)
  notify_alarm "$ALARM_A7"
  set_concurrency "$LAMBDA_SEARCH" 0
  trap 'set_concurrency "$LAMBDA_SEARCH" "$SEARCH_CONCURRENCY"; cleanup' EXIT
  for ((call = 1; call <= 10; call++)); do
    echo "raw URL answered $(http_status "$raw/healthz")"
  done
  set_concurrency "$LAMBDA_SEARCH" "$SEARCH_CONCURRENCY"
  trap cleanup EXIT
  fire_and_clear "$ALARM_A7"
}

# Stage 0 only. A real 5xx on `candidate`, so Url5xxCount exists to be seen: init refuses a bucket
# that is not there. That also logs health.startup.refused, which is why the actions must be off.
test_alarm_a11_metric() {
  local actions base call
  need curl
  actions=$(aws cloudwatch describe-alarms --region "$REGION" --alarm-names "$ALARM_A11" \
    --query 'MetricAlarms[0].ActionsEnabled' --output text)
  [ "$actions" = False ] ||
    die "refusing: a11 --metric needs alarm actions off (Stage 0); they read '$actions'"
  base=$(search_url candidate)
  trap 'end_search_variants || true; cleanup' EXIT
  begin_search_variants
  publish_search_variant '{"OPENLORE_INDEXER_LAMBDA_BUCKET":"openlore-indexer-no-such-bucket"}'
  for ((call = 1; call <= 5; call++)); do
    echo "the candidate answered $(http_status "$base/healthz")"
  done
  end_search_variants
  trap cleanup EXIT
  echo "Url5xxCount shows under Resource=$LAMBDA_SEARCH:candidate once CloudWatch has it"
}

lambda_test_alarm() { # $@ = target
  case "$*" in
    a1 | a2 | a3 | a4 | a5 | a8 | a9 | a10 | a11 | "a11 --metric" | errors | throttles) ;;
    *) die "refusing: test-alarm takes a1|a2|a3|a4|a5|a8|a9|a10|a11|errors|throttles" \
      "or a11 --metric, got '$*'" ;;
  esac
  lambda_account_check
  prepare_run
  use_current_phase
  case "$*" in
    a1) test_alarm_lambda_a1 ;;
    a2) test_alarm_lambda_a2 ;;
    a3) test_alarm_lambda_a3 ;;
    a4) notify_alarm "$ALARM_A4" ;;
    a5) test_alarm_a5 ;;
    a8) notify_alarm "$ALARM_A8" ;;
    a9) notify_alarm "$ALARM_A9" ;;
    a10) test_alarm_a10 ;;
    a11) notify_alarm "$ALARM_A11" ;;
    "a11 --metric") test_alarm_a11_metric ;;
    errors) test_alarm_errors ;;
    throttles) test_alarm_throttles ;;
  esac
}

# One sample of the public name: `<ms> ok|fail edge|host` (a CloudFront `via` header marks the edge).
watch_sample() { # $1 = base URL, $2 = request body file
  local headers code search answerer=host outcome=fail
  headers=$(response_headers "$1/healthz" 5)
  code=$(awk 'NR == 1 { print $2 }' <<<"$headers")
  search=$(search_status "$1" "$2")
  if served_by_cloudfront <<<"$headers"; then answerer=edge; fi
  if [ "$code" = 200 ] && [ "$search" = 200 ]; then outcome=ok; fi
  echo "$(now_ms) $outcome $answerer"
}

# The last 200 from the host and the first from the edge (pure). stdin = watch_sample lines.
watch_handover() {
  awk '$2 == "ok" && $3 == "host" { host = $1 }
    $2 == "ok" && $3 == "edge" && !edge { edge = $1 }
    END { printf "last host 200 at %s ms, first edge 200 at %s ms\n",
      host ? host : "none", edge ? edge : "none" }'
}

# Stage 2 steps 3-5. Runs until the operator interrupts it, as the runbook's step 5 says.
lambda_cutover_watch() {
  refuse_arguments cutover-watch "$@"
  need curl
  lambda_account_check
  prepare_run
  local body="$LAMBDA_WORK/watch-body" log="$LAMBDA_WORK/watch-log" watching=1 gap
  search_body 120 >"$body"
  : >"$log"
  start_pds_poller
  trap 'watching=0' INT TERM
  echo "watching https://$INDEX_HOST; interrupt to stop and read the summary"
  while [ "$watching" = 1 ]; do
    watch_sample "https://$INDEX_HOST" "$body" >>"$log"
    sleep 1
  done
  trap - INT TERM
  stop_pds_poller
  watch_handover <"$log"
  gap=$(longest_edge_gap <"$log")
  echo "longest gap: $gap ms"
  [ "$gap" -le "$MAX_EDGE_GAP_MS" ] || {
    echo "deploy.sh: search was gone for $gap ms, more than $MAX_EDGE_GAP_MS" >&2
    exit 1
  }
}

lambda_main() {
  local verb="${1:-}"
  shift || true
  case "$verb" in
    deploy) lambda_deploy "$@" ;;
    seed) lambda_seed "$@" ;;
    republish) lambda_republish "$@" ;;
    rollback) lambda_rollback "$@" ;;
    stop) lambda_stop "$@" ;;
    start) lambda_start "$@" ;;
    pass-off) lambda_switch "$LAMBDA_PASS" 0 "$@" ;;
    pass-on) lambda_switch "$LAMBDA_PASS" 1 "$@" ;;
    probe-off) lambda_switch "$LAMBDA_PROBE" 0 "$@" ;;
    probe-on) lambda_switch "$LAMBDA_PROBE" 1 "$@" ;;
    trigger) lambda_trigger "$@" ;;
    probe) lambda_probe "$@" ;;
    reset-index) lambda_reset_index "$@" ;;
    restore-index) lambda_restore_index "$@" ;;
    status) lambda_status "$@" ;;
    measure) lambda_measure "$@" ;;
    kpi) lambda_kpi "$@" ;;
    parity) lambda_parity "$@" ;;
    test-alarm) lambda_test_alarm "$@" ;;
    cutover-watch) lambda_cutover_watch "$@" ;;
    *) die "usage: deploy.sh lambda deploy <git-sha|digest> | seed <git-sha|digest> | republish" \
      "| rollback | stop [--wait] | start | pass-off | pass-on | probe-off | probe-on | trigger" \
      "| probe | reset-index [--trigger] | restore-index <version-id> | status | measure" \
      "| kpi [days] | parity | test-alarm <target> | cutover-watch" ;;
  esac
}
