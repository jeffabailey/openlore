# shellcheck shell=bash
# lambda.sh -- the `deploy.sh lambda ...` verbs (scale-to-zero-hosting, DEVOPS "Deployment strategy
# and rollback"). deploy.sh sources this for its `lambda` verb only; `remote` never ships it, since
# no Lambda verb touches the shared host. It reuses deploy.sh's die, need, read_build_ref,
# verify_build, image_config, http_status, search_body and post_checks, so the digest and signature
# rules are the same on both paths.
#
#   lambda deploy <git-sha|digest>   blue-green by alias: every gate in order, `live` flips last
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
MAX_EDGE_GAP_MS=30000

COPIED_DIGEST=""      # step 6: set while this run's copy is not yet on every $LATEST
LIVE_VERSIONS=()      # step 7: what each `live` alias holds now
LATEST_REVISIONS=()   # step 8: each $LATEST's revision, to publish without a race
NEW_VERSIONS=()       # step 9: the version each function will flip to
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
  local function
  for function in "${LAMBDA_FUNCTIONS[@]}"; do
    LIVE_VERSIONS+=("$(live_version "$function")")
  done
  INDEX_VERSION=$(index_head | jq -r '.VersionId // "none"')
}

# Step 8. Nothing invokes $LATEST, so this is invisible to traffic.
update_images() { # $1 = digest
  local function config code_sha
  for function in "${LAMBDA_FUNCTIONS[@]}"; do
    aws lambda update-function-code --region "$REGION" --function-name "$function" \
      --image-uri "$ECR_REPO@$1" >/dev/null
  done
  for function in "${LAMBDA_FUNCTIONS[@]}"; do
    aws lambda wait function-updated-v2 --region "$REGION" --function-name "$function"
    config=$(aws lambda get-function-configuration --region "$REGION" --function-name "$function")
    code_sha=$(jq -r .CodeSha256 <<<"$config")
    [ "$code_sha" = "${1#sha256:}" ] ||
      die "refusing: $function \$LATEST holds $code_sha, not $1"
    LATEST_REVISIONS+=("$(jq -r .RevisionId <<<"$config")")
  done
}

# Step 9, with --code-sha256 and --revision-id so a concurrent change cannot slip in.
publish_versions() { # $1 = digest, $2 = git sha
  local i version
  for i in "${!LAMBDA_FUNCTIONS[@]}"; do
    version=$(aws lambda publish-version --region "$REGION" \
      --function-name "${LAMBDA_FUNCTIONS[$i]}" --code-sha256 "${1#sha256:}" \
      --revision-id "${LATEST_REVISIONS[$i]}" --description "$2 $1" | jq -r .Version)
    aws lambda wait published-version-active --region "$REGION" \
      --function-name "${LAMBDA_FUNCTIONS[$i]}" --qualifier "$version"
    NEW_VERSIONS+=("$version")
  done
}

move_candidate() { # step 10
  aws lambda update-alias --region "$REGION" --function-name "$LAMBDA_SEARCH" --name candidate \
    --function-version "${NEW_VERSIONS[0]}" >/dev/null
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
  post_checks "$base"
  served=$(jq -r '.last_successful_pass_at // ""' <<<"$health")
  published=$(index_head | jq -r '.Metadata["published-at"] // ""')
  [ "$served" = "$published" ] ||
    die "refusing: the candidate serves the index of '$served', the bucket holds '$published'"
}

# Synchronous, and never re-invoked by the client: a gate may run for the whole timeout.
# Prints the CLI's answer; $4 receives the function's own reply. A later step adds the throttle
# wait here (`sleep 30`), shared by both gates.
invoke_gate() { # $1 = function, $2 = version, $3 = payload, $4 = reply file
  AWS_MAX_ATTEMPTS=1 aws lambda invoke --region "$REGION" --function-name "$1" --qualifier "$2" \
    --cli-read-timeout 0 --cli-binary-format raw-in-base64-out --log-type Tail \
    --payload "$3" "$4"
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
  [ "$exit_code" = 0 ] || die "refusing: the pass gate ended in exit $exit_code"
}

health_lines() { # stdin = the CLI's answer -> the indexer.host.health lines of its log tail
  jq -r '.LogResult // ""' | base64 -d | jq -Rc 'fromjson? | select(.event? == "indexer.host.health")'
}

# Step 13. This proves the new probe code and the current index, not the candidate's search:
# the probe calls the public edge, which still reaches the old `live` search.
probe_gate() {
  local reply="$LAMBDA_WORK/probe-reply" answer health
  answer=$(invoke_gate "$LAMBDA_PROBE" "${NEW_VERSIONS[2]}" '{}' "$reply")
  jq -e 'has("FunctionError") | not' <<<"$answer" >/dev/null ||
    die "refusing: the probe gate ended in a function error"
  health=$(health_lines <<<"$answer")
  [ "$(grep -c . <<<"$health" || true)" = 1 ] ||
    die "refusing: the probe gate did not log exactly one health line"
  jq -e '.not_live == 0' <<<"$health" >/dev/null ||
    die "refusing: the probe gate found the index not live: $health"
  echo "probe gate: live"
}

# Moves `live` and records what it replaced (L-13); $2 is appended for the pass's index id.
flip_live() { # $1 = index into LAMBDA_FUNCTIONS, $2 = digest, $3 = git sha, $4 = extra record
  local description
  description="prev=${LIVE_VERSIONS[$1]} $3 $2 $(date -u +%FT%TZ)${4:+ $4}"
  aws lambda update-alias --region "$REGION" --function-name "${LAMBDA_FUNCTIONS[$1]}" --name live \
    --function-version "${NEW_VERSIONS[$1]}" --description "$description" >/dev/null
}

distribution_domain() {
  aws cloudfront list-distributions --region "$REGION" |
    jq -r --arg host "$INDEX_HOST" '.DistributionList.Items[]
      | select((.Aliases.Items // []) | index($host)) | .DomainName'
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
  domain=$(distribution_domain)
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

# Step 14: search flips first, watched from 5 s before the flip until the edge checks end.
flip_search() { # $1 = digest, $2 = git sha
  local edge flipped_at
  edge=$(edge_url)
  start_edge_poller "$edge"
  sleep 5
  flipped_at=$(now_ms)
  flip_live 0 "$1" "$2"
  edge_checks "$edge"
  stop_edge_poller
  EDGE_GAP_MS=$(longest_edge_gap <"$EDGE_POLL_LOG")
  FLIP_TO_FIRST_200_MS=$(first_200_after "$flipped_at" <"$EDGE_POLL_LOG")
  [ "$EDGE_GAP_MS" -le "$MAX_EDGE_GAP_MS" ] ||
    die "edge check: search was gone for $EDGE_GAP_MS ms, more than $MAX_EDGE_GAP_MS"
}

flip_pass_and_probe() { # step 15; $1 = digest, $2 = git sha
  flip_live 1 "$1" "$2" "idx=$INDEX_VERSION"
  flip_live 2 "$1" "$2"
}

deploy_summary() { # step 16; $1 = digest
  local i
  for i in "${!LAMBDA_FUNCTIONS[@]}"; do
    echo "${LAMBDA_FUNCTIONS[$i]}: live ${LIVE_VERSIONS[$i]} -> ${NEW_VERSIONS[$i]}"
  done
  echo "digest $1"
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

lambda_deploy() { # $1 = git sha or digest
  read_build_ref "${1:-}"
  lambda_preflight
  LAMBDA_WORK=$(mktemp -d)
  CLEANUP_PATHS+=("$LAMBDA_WORK")
  EDGE_POLL_LOG=$LAMBDA_WORK/edge-poll
  trap 'discard_failed_copy; cleanup' EXIT
  verify_build
  local digest="$BUILD_DIGEST" sha="$BUILD_SHA"
  refuse_unless_lambda_capable "$digest"
  ecr_copy "$digest" "$sha"
  record_rollback_targets
  update_images "$digest"
  COPIED_DIGEST="" # every $LATEST holds it now, so it stays whatever happens next
  publish_versions "$digest" "$sha"
  move_candidate
  candidate_checks
  pass_gate
  probe_gate
  flip_search "$digest" "$sha"
  flip_pass_and_probe "$digest" "$sha"
  deploy_summary "$digest"
  prune_old_versions
  echo "deployed $IMAGE@$digest to Lambda"
}

lambda_main() {
  local verb="${1:-}"
  shift || true
  case "$verb" in
    deploy) lambda_deploy "$@" ;;
    *) die "usage: deploy.sh lambda deploy <git-sha|digest>" ;;
  esac
}
