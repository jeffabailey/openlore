# shellcheck shell=bash
# lambda.sh -- the `deploy.sh lambda ...` verbs (scale-to-zero-hosting, DEVOPS "Deployment strategy
# and rollback"). deploy.sh sources this for its `lambda` verb only; `remote` never ships it, since
# no Lambda verb touches the shared host. It reuses deploy.sh's die, need, read_build_ref,
# verify_build, image_config, http_status, search_body and post_checks, so the digest and signature
# rules are the same on both paths.
#
#   lambda deploy <git-sha|digest>   blue-green by alias: every gate in order, `live` flips last
#   lambda republish                 each function's live image again, through the same gates and
#                                    flips, so a config change reaches `live` (U-STZ-2)
#   lambda rollback                  every `live` back to its recorded prev=, `candidate` following
#   lambda stop [--wait]             the kill switch: search concurrency 0, the distribution off
#   lambda start                     search concurrency back to 3, the distribution on
#   lambda pass-off | pass-on        the pass's concurrency 0 / 1, and nothing else
#   lambda probe-off | probe-on      the probe's concurrency 0 / 1, and nothing else
#   lambda reset-index               after the typed word `reset`: a delete marker on the index
#   lambda restore-index <version-id>  that version copied back once, away from a quarter hour
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

now_ms() { jq -n 'now * 1000 | floor'; }

index_head() { # the index object's metadata; {} on an empty bucket
  aws s3api head-object --region "$REGION" --bucket "$INDEX_BUCKET" --key "$INDEX_KEY" \
    2>/dev/null || echo '{}'
}

# Step 0, after the ref check: nothing here may run for a ref that is about to be refused.
lambda_preflight() {
  need aws gh cosign crane jq curl
  local account
  account=$(aws sts get-caller-identity --region "$REGION" --output json | jq -r .Account)
  [ "$account" = "$LAMBDA_ACCOUNT" ] ||
    die "refusing: these credentials are account '$account', not $LAMBDA_ACCOUNT"
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
    jq -r '.FunctionVersion, (.Description // "" | capture("prev=(?<v>[0-9]+)").v)'
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
  INDEX_VERSION=$(index_head | jq -r '.VersionId // "none"')
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

candidate_url() {
  local url
  url=$(aws lambda get-function-url-config --region "$REGION" --function-name "$LAMBDA_SEARCH" \
    --qualifier candidate | jq -r .FunctionUrl)
  echo "${url%/}"
}

# Step 11: the host's post-checks against the candidate, then proof it fetched the real index:
# /healthz must name the object's published-at (both empty on an empty bucket). This is also
# what stops a deploy during `lambda stop`: the candidate answers 429.
candidate_checks() {
  local base started health served published
  base=$(candidate_url)
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
invoke_gate() { # the arguments of invoke_once
  local errors="$LAMBDA_WORK/gate-errors" attempt=1
  until invoke_once "$@" 2>"$errors"; do
    grep -q TooManyRequestsException "$errors" ||
      die "refusing: the gate's invoke of $1 failed: $(cat "$errors")"
    [ "$attempt" -lt "$GATE_ATTEMPTS" ] ||
      die "refusing: $1 was still busy after $GATE_ATTEMPTS attempts"
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

# Step 13. This proves the new probe code and the current index, not the candidate's search:
# the probe calls the public edge, which still reaches the old `live` search.
probe_gate() {
  local reply="$LAMBDA_WORK/probe-reply" answer health not_live
  answer=$(invoke_gate "$LAMBDA_PROBE" "${NEW_VERSIONS[2]}" '{}' "$reply")
  jq -e 'has("FunctionError") | not' <<<"$answer" >/dev/null ||
    die "refusing: the probe gate ended in a function error"
  health=$(health_lines <<<"$answer")
  [ "$(grep -c . <<<"$health" || true)" = 1 ] ||
    die "refusing: the probe gate did not log exactly one health line"
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

distribution_field() { # $1 = field -> that field of the distribution serving $INDEX_HOST
  aws cloudfront list-distributions --region "$REGION" |
    jq -r --arg host "$INDEX_HOST" --arg field "$1" '.DistributionList.Items[]
      | select((.Aliases.Items // []) | index($host)) | .[$field]'
}

# The public name once the edge serves it (a CloudFront `via` header marks it), else the
# distribution's own domain: during the trial the public name is still the host.
edge_url() {
  local headers
  headers=$(curl -sS -o /dev/null -D - -m 10 "https://$INDEX_HOST/healthz" 2>/dev/null || true)
  if grep -qiE '^via:.*cloudfront' <<<"$headers"; then
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
      search=$(http_status -X POST -H 'content-type: application/json' --data-binary "@$body" \
        "$base$SEARCH_PATH")
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
  code=$(http_status -X POST -H 'content-type: application/json' --data-binary "@$body" \
    "$base$SEARCH_PATH")
  [ "$code" = 200 ] || die "edge check: a minimal POST search answered $code, not 200"
  code=$(http_status "$base/")
  [ "$code" = 404 ] || die "edge check: GET / answered $code, not 404"
  code=$(http_status -X POST "$base/healthz")
  [ "$code" = 404 ] || die "edge check: POST /healthz answered $code, not 404"
  headers=$(curl -sS -o /dev/null -D - -m 10 "$base/healthz" 2>/dev/null || true)
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
  jq -rn --argjson versions "$1" --argjson aliases "$2" --argjson keep "$RETAINED_VERSIONS" '
    [$aliases.Aliases[]? | .FunctionVersion, (.Description // "" | capture("prev=(?<v>[0-9]+)").v)]
      as $referenced
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

lambda_deploy() { # $1 = git sha or digest
  read_build_ref "${1:-}"
  lambda_preflight
  prepare_run
  verify_build
  local digest="$BUILD_DIGEST" sha="$BUILD_SHA"
  refuse_unless_lambda_capable "$digest"
  ecr_copy "$digest" "$sha"
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

# The sha a version was published with, the first word of its description (pure).
# stdin = get-function JSON. A version this script did not publish names none.
version_sha() {
  jq -r '(.Configuration.Description // "" | split(" ")[0]) // "unknown"'
}

# What one function carries: the image of the version step 7 read, not of the alias, which could
# have moved since. Refuses, and never falls back: `$LATEST` may hold an unflipped image, and a
# prev= record names the version before this one. Runs for all three before any function changes.
read_live_image() { # $1 = index into LAMBDA_FUNCTIONS
  local function=${LAMBDA_FUNCTIONS[$1]} version=${LIVE_VERSIONS[$1]} answer digest
  answer=$(aws lambda get-function --region "$REGION" --function-name "$function" \
    --qualifier "$version") || die "refusing: could not read version $version of $function"
  digest=$(image_digest <<<"$answer") || die "refusing: version $version of $function names no image"
  ecr_holds "$digest" || die "refusing: ECR does not hold $digest, the live image of $function"
  IMAGE_DIGESTS+=("$digest")
  IMAGE_SHAS+=("$(version_sha <<<"$answer")")
}

# U-STZ-2: a published version snapshots the function's configuration, so a tofu change reaches
# `live` only through a new version. Each function keeps its own live image, which after a
# partial flip is not the same image everywhere. No CI or cosign gate: it was verified when
# deployed. No copy either, so the EXIT trap has no image to discard.
lambda_republish() {
  [ "$#" = 0 ] || die "refusing: republish takes no arguments, got '$*'"
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
  local versions current target answer digest
  versions=$(alias_versions "$1" live) || die "refusing: could not read the live alias of $1"
  current=$(sed -n 1p <<<"$versions")
  target=$(sed -n 2p <<<"$versions")
  [ -n "$target" ] || die "refusing: $1 records no version it replaced"
  answer=$(aws lambda get-function --region "$REGION" --function-name "$1" \
    --qualifier "$target") || die "refusing: could not read version $target of $1"
  digest=$(image_digest <<<"$answer") || die "refusing: version $target of $1 names no image"
  ecr_holds "$digest" || die "refusing: ECR does not hold $digest, the image of $1 version $target"
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
  [ "$#" = 0 ] || die "refusing: rollback takes no arguments, got '$*'"
  lambda_preflight
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
  case "$*" in
    "" | --wait) ;;
    *) die "refusing: stop takes only --wait, got '$*'" ;;
  esac
  lambda_preflight
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
  [ "$#" = 0 ] || die "refusing: start takes no arguments, got '$*'"
  lambda_preflight
  set_concurrency "$LAMBDA_SEARCH" "$SEARCH_CONCURRENCY"
  set_edge_enabled true ||
    die "search is back, but the edge is not: could not enable the distribution; run start again"
}

# One function, and only its concurrency: 0 for maintenance, 1 as tofu sets it, so an apply in
# between turns the function back on. $3.. are the verb's arguments.
lambda_switch() { # $1 = function, $2 = reserved concurrency
  [ "$#" = 2 ] || die "refusing: this switch takes no arguments, got '${*:3}'"
  lambda_preflight
  set_concurrency "$1" "$2"
}

# D-STZ-27. Deleting with no version id leaves a delete marker, so every version stays and the
# next pass rebuilds with If-None-Match: *. The empty IFS keeps a stray space in the answer: the
# word must be exact. A closed stdin reads as no word.
lambda_reset_index() {
  [ "$#" = 0 ] || die "refusing: reset-index takes no arguments, got '$*'"
  lambda_preflight
  local word="" version
  echo "This deletes s3://$INDEX_BUCKET/$INDEX_KEY. Type reset to go on:" >&2
  IFS= read -r word || true
  [ "$word" = reset ] || die "refusing: reset-index needs the exact word reset, got '$word'"
  version=$(index_head | jq -r '.VersionId // "none"')
  aws s3api delete-object --region "$REGION" --bucket "$INDEX_BUCKET" --key "$INDEX_KEY" >/dev/null
  echo "index deleted; its version is now noncurrent: $version"
  echo "search serves an empty index within 60 s; the next pass rebuilds it, and A10 can fire once"
  echo "restore: deploy.sh lambda restore-index $version"
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
  lambda_preflight
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

lambda_main() {
  local verb="${1:-}"
  shift || true
  case "$verb" in
    deploy) lambda_deploy "$@" ;;
    republish) lambda_republish "$@" ;;
    rollback) lambda_rollback "$@" ;;
    stop) lambda_stop "$@" ;;
    start) lambda_start "$@" ;;
    pass-off) lambda_switch "$LAMBDA_PASS" 0 "$@" ;;
    pass-on) lambda_switch "$LAMBDA_PASS" 1 "$@" ;;
    probe-off) lambda_switch "$LAMBDA_PROBE" 0 "$@" ;;
    probe-on) lambda_switch "$LAMBDA_PROBE" 1 "$@" ;;
    reset-index) lambda_reset_index "$@" ;;
    restore-index) lambda_restore_index "$@" ;;
    *) die "usage: deploy.sh lambda deploy <git-sha|digest> | republish | rollback | stop [--wait]" \
      "| start | pass-off | pass-on | probe-off | probe-on | reset-index" \
      "| restore-index <version-id>" ;;
  esac
}
