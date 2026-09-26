#!/usr/bin/env bash
# publish-contract round trip (serverless-philosophy-federation, DV-1).
#
# The REAL integration test for the `atproto/` Worker: start it under local
# workerd (`wrangler dev` — never `wrangler deploy`, no Cloudflare account or
# token), then drive the REAL `openlore` CLI through
# `publish init <url>` -> `publish push` -> `publish pull` with the
# 0.0 / 0.5 / 1.0 gold-fixture claims (the f16-representable confidences a
# re-encoding transport corrupts) seeded in a throwaway local store. Fails on
# any CID mismatch or on anything other than `3/3 CIDs verified`.
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

cleanup() {
  if [[ -n "${WRANGLER_PID}" ]]; then
    # wrangler runs in its own process group (set -m): stop it AND workerd.
    kill -TERM -- "-${WRANGLER_PID}" 2>/dev/null || kill -TERM "${WRANGLER_PID}" 2>/dev/null || true
    wait "${WRANGLER_PID}" 2>/dev/null || true
  fi
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

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

# --- 3. The real CLI round trip: init -> push -> pull ------------------------
"${OPENLORE_BIN}" publish init "${INSTANCE_URL}" || fail "publish init ${INSTANCE_URL}"
push_out="$("${OPENLORE_BIN}" publish push)" || fail "publish push: ${push_out}"
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

echo "contract-roundtrip: PASS — ${expected_total}/${expected_total} gold-fixture CIDs (0.0/0.5/1.0) round-tripped identically through the real Worker"
