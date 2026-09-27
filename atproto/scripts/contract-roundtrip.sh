#!/usr/bin/env bash
# publish-contract round trip (serverless-philosophy-federation, DV-1).
#
# The REAL integration test for the `atproto/` Worker: start it under local
# workerd (`wrangler dev` — never `wrangler deploy`, no Cloudflare account or
# token), then drive the REAL `openlore` CLI through
# `publish init <url>` -> `publish push` -> `publish pull` with the
# 0.0 / 0.5 / 1.0 gold-fixture claims (the f16-representable confidences a
# re-encoding transport corrupts) seeded in a throwaway local store. Fails on
# any CID mismatch or on anything other than `3/3 CIDs verified`. Then
# asserts idempotency (step 02-02): a re-push reports `pushed: 0` and raw
# re-PUTs (incl. concurrent ones) of a committed CID never grow the manifest.
#
# Write auth (step 02-03, DV-4 / Q-SF-D2): the script GENERATES a throwaway
# fixture owner token into the git-ignored `atproto/.dev.vars`
# (OPENLORE_WRITE_TOKEN — the Worker secret under `wrangler dev`) and hands
# the same value to the CLI via OPENLORE_PUBLISH_TOKEN. It first asserts that
# a token-less (and a wrong-token) push is refused as an `unauthorized write`
# with nothing stored, and that reads stay public. An existing developer
# `.dev.vars` is backed up and restored on exit. No real Cloudflare credential
# is ever involved.
#
# Usage: atproto/scripts/contract-roundtrip.sh [path/to/openlore]
#   OPENLORE_BIN   the openlore binary (default: target/release/openlore)
#   CONTRACT_PORT  local port for wrangler dev (default: 8787)
set -euo pipefail

ATPROTO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "${ATPROTO_DIR}/.." && pwd)"
OPENLORE_BIN="${1:-${OPENLORE_BIN:-${REPO_ROOT}/target/release/openlore}}"
PORT="${CONTRACT_PORT:-8787}"
INSTANCE_URL="http://127.0.0.1:${PORT}"
GOLD_CONFIDENCES=(0 0.5 1)

fail() {
  echo "contract-roundtrip: FAIL — $*" >&2
  exit 1
}

[[ -x "${OPENLORE_BIN}" ]] || fail "openlore binary not found at ${OPENLORE_BIN}"

WORK_DIR="$(mktemp -d)"
WRANGLER_LOG="${WORK_DIR}/wrangler.log"
WRANGLER_PID=""
DEV_VARS="${ATPROTO_DIR}/.dev.vars"
DEV_VARS_BACKUP="${WORK_DIR}/dev.vars.backup"
AUTH_HEADER_FILE="${WORK_DIR}/owner-auth.header"

cleanup() {
  if [[ -n "${WRANGLER_PID}" ]]; then
    # wrangler runs in its own process group (set -m): stop it AND workerd.
    kill -TERM -- "-${WRANGLER_PID}" 2>/dev/null || kill -TERM "${WRANGLER_PID}" 2>/dev/null || true
    wait "${WRANGLER_PID}" 2>/dev/null || true
  fi
  # Restore the developer's own .dev.vars (if any); never leave the fixture.
  if [[ -f "${DEV_VARS_BACKUP}" ]]; then
    mv -f "${DEV_VARS_BACKUP}" "${DEV_VARS}"
  else
    rm -f "${DEV_VARS}"
  fi
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

# --- 0. A throwaway FIXTURE owner write token (never a real credential) -----
FIXTURE_WRITE_TOKEN="contract-fixture-$(openssl rand -hex 24 2>/dev/null || od -An -N24 -tx1 /dev/urandom | tr -d ' \n')"
[[ -f "${DEV_VARS}" ]] && cp -p "${DEV_VARS}" "${DEV_VARS_BACKUP}"
( umask 077; printf 'OPENLORE_WRITE_TOKEN=%s\n' "${FIXTURE_WRITE_TOKEN}" >"${DEV_VARS}" )
( umask 077; printf 'authorization: Bearer %s\n' "${FIXTURE_WRITE_TOKEN}" >"${AUTH_HEADER_FILE}" )

# --- 1. The Worker under LOCAL workerd, with fresh Durable Object state -------
set -m
(
  cd "${ATPROTO_DIR}"
  exec npx wrangler dev --ip 127.0.0.1 --port "${PORT}" \
    --persist-to "${WORK_DIR}/state" --show-interactive-dev-session=false
) >"${WRANGLER_LOG}" 2>&1 &
WRANGLER_PID=$!
set +m

echo "contract-roundtrip: waiting for wrangler dev on ${INSTANCE_URL} ..."
ready=""
for _ in $(seq 1 120); do
  if curl -fsS "${INSTANCE_URL}/manifest" >/dev/null 2>&1; then
    ready="yes"
    break
  fi
  if ! kill -0 "${WRANGLER_PID}" 2>/dev/null; then
    cat "${WRANGLER_LOG}" >&2
    fail "wrangler dev exited before becoming ready"
  fi
  sleep 1
done
if [[ -z "${ready}" ]]; then
  cat "${WRANGLER_LOG}" >&2
  fail "wrangler dev not ready after 120s"
fi
echo "contract-roundtrip: instance ready"

# --- 2. A throwaway local store holding the three gold-fixture claims ---------
export OPENLORE_HOME="${WORK_DIR}/home"
export OPENLORE_DID="did:plc:contract-roundtrip"
export OPENLORE_KEY_SEED_HEX="0000000000000000000000000000000000000000000000000000000000000000"
export OPENLORE_PUBLISH_ENDPOINT="${INSTANCE_URL}"
mkdir -p "${OPENLORE_HOME}"

"${OPENLORE_BIN}" init --handle contract.test --app-password contract-unused \
  || fail "openlore init"

seeded_cids=()
for confidence in "${GOLD_CONFIDENCES[@]}"; do
  # Enter = sign locally; N = do not publish to a PDS (local-only claim).
  added="$(printf '\nN\n' | "${OPENLORE_BIN}" claim add \
    --subject github:rust-lang/rust \
    --predicate embodiesPhilosophy \
    --object org.openlore.philosophy.memory-safety \
    --evidence https://www.rust-lang.org/ \
    --confidence "${confidence}")" || fail "claim add --confidence ${confidence}"
  cid="$(printf '%s\n' "${added}" | sed -n 's/.*Computing claim CID \([^ ]*\).*/\1/p' | head -n 1)"
  [[ -n "${cid}" ]] || fail "no CID printed for confidence ${confidence}: ${added}"
  echo "contract-roundtrip: seeded confidence ${confidence} -> ${cid}"
  seeded_cids+=("${cid}")
done

# --- 2b. Write auth (step 02-03, DV-4): writes need the owner token ----------
# Reads are public: the card and the manifest answer without any token.
curl -fsS -o /dev/null "${INSTANCE_URL}/" || fail "GET / must not require a token"
curl -fsS -o /dev/null "${INSTANCE_URL}/manifest" || fail "GET /manifest must not require a token"
[[ "$(curl -sS -o /dev/null -w '%{http_code}' "${INSTANCE_URL}/records/bafynotstored")" == "404" ]] \
  || fail "GET /records/:cid must not require a token (expected a public 404)"

# A raw token-less PUT is refused with 401 and stores nothing.
raw_status="$(curl -sS -o /dev/null -w '%{http_code}' -X PUT --data-binary "x" \
  "${INSTANCE_URL}/records/bafytokenless")"
[[ "${raw_status}" == "401" ]] || fail "a token-less raw PUT returned ${raw_status}, expected 401"

# A token-less CLI push, and a wrong-token one, are refused as an
# `unauthorized write`; the manifest stays empty.
refused_push() {
  local label="$1" out
  shift
  if out="$("$@" 2>&1)"; then
    fail "a ${label} push was accepted: ${out}"
  fi
  grep -q "unauthorized write" <<<"${out}" || fail "a ${label} push did not report 'unauthorized write': ${out}"
  grep -qF "${FIXTURE_WRITE_TOKEN}" <<<"${out}" && fail "the CLI echoed the owner token"
  echo "contract-roundtrip: ${label} push refused as unauthorized write"
}
refused_push "token-less" env -u OPENLORE_PUBLISH_TOKEN "${OPENLORE_BIN}" publish push
refused_push "wrong-token" env OPENLORE_PUBLISH_TOKEN=not-the-owner-token "${OPENLORE_BIN}" publish push
grep -q '"cid"' <<<"$(curl -fsS "${INSTANCE_URL}/manifest")" \
  && fail "a refused push stored a record"

# From here on the CLI is the owner: the same fixture value the Worker holds.
export OPENLORE_PUBLISH_TOKEN="${FIXTURE_WRITE_TOKEN}"

# --- 3. The real CLI round trip: init -> push -> pull ------------------------
"${OPENLORE_BIN}" publish init "${INSTANCE_URL}" || fail "publish init ${INSTANCE_URL}"
push_out="$("${OPENLORE_BIN}" publish push 2>&1)" || fail "publish push: ${push_out}"
grep -qF "${FIXTURE_WRITE_TOKEN}" <<<"${push_out}" && fail "the CLI echoed the owner token"
echo "${push_out}"
pull_out="$("${OPENLORE_BIN}" publish pull)" || fail "publish pull (a CID did not verify): ${pull_out}"
echo "${pull_out}"

# --- 4. Every CID recomputed in Rust byte-matches its key --------------------
expected_total="${#seeded_cids[@]}"
grep -qx "${expected_total}/${expected_total} CIDs verified" <<<"${pull_out}" \
  || fail "pull did not report ${expected_total}/${expected_total} CIDs verified"
grep -q "MISMATCH" <<<"${pull_out}" && fail "pull reported a CID mismatch"

manifest="$(curl -fsS "${INSTANCE_URL}/manifest")"
for cid in "${seeded_cids[@]}"; do
  grep -qx "  ${cid} verified" <<<"${pull_out}" || fail "${cid} was not verified on pull"
  grep -q "\"cid\":\"${cid}\"" <<<"${manifest}" || fail "${cid} missing from GET /manifest"
done
manifest_count="$(grep -o '"cid":"[^"]*"' <<<"${manifest}" | wc -l | tr -d ' ')"
[[ "${manifest_count}" == "${expected_total}" ]] \
  || fail "manifest lists ${manifest_count} records, expected ${expected_total}"

# --- 5. Idempotency under real workerd (step 02-02, ADR-062 §1) -------------
# 5a. Re-push of an already-complete set pushes nothing and adds no entry.
repush_out="$("${OPENLORE_BIN}" publish push)" || fail "re-push: ${repush_out}"
echo "${repush_out}"
grep -qx "pushed: 0, skipped: ${expected_total}, verified: 0/0" <<<"${repush_out}" \
  || fail "re-push did not report 'pushed: 0, skipped: ${expected_total}'"

# 5b. Raw re-PUTs of a committed CID — with the commit header, without it, and
# a burst of concurrent identical committing PUTs — are no-op successes: the
# stored bytes stay the first write and the manifest gains no entry.
target_cid="${seeded_cids[0]}"
original_bytes="$(curl -fsS "${INSTANCE_URL}/records/${target_cid}" | shasum -a 256)"
reput_entry="{\"cid\":\"${target_cid}\",\"note\":\"re-put\"}"
reput() {
  curl -fsS -o /dev/null -X PUT -H "@${AUTH_HEADER_FILE}" --data-binary "not-the-original-bytes" "$@" \
    "${INSTANCE_URL}/records/${target_cid}"
}
reput -H "x-openlore-manifest-entry: ${reput_entry}" || fail "committing re-PUT was not a success"
reput || fail "bare re-PUT was not a success"
burst_pids=()
for _ in 1 2 3 4 5 6 7 8; do
  reput -H "x-openlore-manifest-entry: ${reput_entry}" &
  burst_pids+=("$!")
done
for pid in "${burst_pids[@]}"; do
  wait "${pid}" || fail "a concurrent committing re-PUT was not a success"
done

[[ "$(curl -fsS "${INSTANCE_URL}/records/${target_cid}" | shasum -a 256)" == "${original_bytes}" ]] \
  || fail "a re-PUT overwrote the stored bytes of ${target_cid}"
manifest_after="$(curl -fsS "${INSTANCE_URL}/manifest")"
manifest_cids="$(grep -o '"cid":"[^"]*"' <<<"${manifest_after}")"
after_count="$(wc -l <<<"${manifest_cids}" | tr -d ' ')"
[[ "${after_count}" == "${expected_total}" ]] \
  || fail "manifest grew to ${after_count} entries after re-push/re-PUT, expected ${expected_total}"
duplicates="$(sort <<<"${manifest_cids}" | uniq -d)"
[[ -z "${duplicates}" ]] || fail "manifest lists a CID more than once: ${duplicates}"
echo "contract-roundtrip: re-push pushed 0; re-PUTs (incl. 8 concurrent) left the manifest at ${expected_total} distinct CIDs"

echo "contract-roundtrip: PASS — ${expected_total}/${expected_total} gold-fixture CIDs (0.0/0.5/1.0) round-tripped identically through the real Worker; re-push and re-PUT are idempotent; token-less writes refused, reads public"
