# CI/CD Pipeline Delta — openlore-federated-read (slice-03)

- **Wave**: DEVOPS
- **Date**: 2026-05-27
- **Architect**: Apex
- **Tool**: GitHub Actions (UNCHANGED from D-D1)
- **Branching**: GitHub Flow (UNCHANGED from D-D7)

This is the slice-03 **delta** to `ci-cd-pipeline.md` (foundation). Read
that file first; this document describes only the additions and the
single-line modifications. No YAML is written here. DELIVER lands the YAML
into the EXISTING `ci.yml` and `nightly.yml` workflow files — no new
workflow file is created.

## 1. Workflow files (no new files)

| File | Triggers (UNCHANGED) | Slice-03 additions |
|---|---|---|
| `.github/workflows/ci.yml` | `pull_request: [main]`, `push: [main]` | Five new acceptance-stage jobs (§3); one new Pact sub-job (§3.6) |
| `.github/workflows/nightly.yml` | `schedule: cron 02:00 UTC daily`, `workflow_dispatch` | Mutation scope MAY widen to include new pure-core modules (§4) |
| `.github/workflows/release.yml` | `push: tags: ['v*']` | Re-runs the new acceptance jobs as part of the existing acceptance re-run; Pact-real-peer once at release (§5) |

## 2. Commit-stage (UNCHANGED)

`fmt`, `lint`, `supply-chain`, `arch-check`, `probe-check`, `test-unit`,
`test-property` all run unchanged. The DESIGN-wave `xtask check-arch` rule
extension (per WD-19: "no JOIN between author_claims and peer_claims that
elides author_did column") runs as part of the existing `arch-check`
stage — same command, expanded rule set. DELIVER lands the rule code in
the `xtask` crate; the CI job invocation is unchanged.

Probe-check (§3.5 of foundation) covers the new peer-pull probe paths
automatically — it's an AST walker over every `impl <Port> for <Adapter>`,
so any new probe method on extended `PdsPort` / `StoragePort` / new
`PeerPort` is in scope by construction. No CI change needed.

## 3. Acceptance-stage additions

All five new jobs run in parallel within the existing acceptance stage,
after the commit-stage gates pass. Each is **blocking on PR** and **gates
release**.

### 3.1 `at-federation-attribution-preserved`
- **Command**: `cargo nextest run --test federation_attribution_preserved`
- **What it does**: composes claims as two distinct DIDs (the user's own + a fixture peer); writes both to the respective stores; runs `graph query --federated`; asserts every rendered row has a non-null `author_did` field; asserts ZERO rows where the author DID cannot be traced back to either `author_claims` or `peer_claims`; asserts the per-row attribution remains stable across query orderings.
- **Maps to**: KPI-FED-1 (attribution fidelity = 100%); KPI-FED-2 (zero merged rows); WD-19 (no JOIN elides author_did)
- **Type**: blocking GUARDRAIL
- **Wall-clock target**: < 20 s

### 3.2 `at-peer-cid-round-trip`
- **Command**: `cargo nextest run --test peer_cid_round_trip`
- **What it does**: wiremock peer publishes a sentinel claim with adversarial Unicode/whitespace/float-boundary fields (mirroring `kpi-4-roundtrip`'s adversarial set); CLI pulls; asserts recomputed CID at pull time matches the CID embedded in the peer record; asserts no field-level normalization occurred during ingest into `peer_claims`.
- **Maps to**: WD-24 (CID recomputation required at pull); inherits slice-01 KPI-4 round-trip extended to peer-sourced claims (per outcome-kpis.md §Mapping to slice-01 KPIs row 1)
- **Type**: blocking
- **Wall-clock target**: < 20 s

### 3.3 `at-peer-tampered-signature-rejected`
- **Command**: `cargo nextest run --test peer_tampered_signature_rejected`
- **What it does**: wiremock adversarial peer fixture publishes three flavors of tampered records — (a) valid claim body, wrong signature; (b) valid signature, body bytes mutated post-signing; (c) valid sig + body, wrong CID in the record; CLI runs `peer pull`; asserts ALL THREE records are REJECTED (per WD-24 reject-per-claim semantics); asserts the pull's other valid records (mixed in) DO proceed; asserts ZERO rows enter `peer_claims` for any of the three tampered CIDs; asserts a structured-log `peer.pull.rejected{reason}` event was emitted per rejected record.
- **Maps to**: KPI-FED-6 (100% — no invalid signatures stored); WD-24
- **Type**: blocking GUARDRAIL (security)
- **Wall-clock target**: < 30 s
- **Fixture**: `tests/fixtures/peer-adversarial/` — wiremock stubs configured by `tests/fixtures/peer-adversarial/setup.rs` (DELIVER writes). The fixture is referenced by CID in the test setup; the `xtask regenerate-peer-fixtures` helper (see §7) regenerates the cached bodies when the Lexicon evolves.

### 3.4 `at-counter-target-cid-round-trip`
- **Command**: `cargo nextest run --test counter_target_cid_round_trip`
- **What it does**: pulls a peer claim; authors a counter-claim against its CID via `openlore claim counter <cid> --reason "..."`; signs and publishes (mock PDS via wiremock); asserts the published counter-claim's `references[]` array contains exactly one entry with `type == Counters` and `cid == <target_cid>` byte-equal; asserts the `reason` field round-trips byte-equal.
- **Maps to**: WD-22 (counter-claim publish reuses slice-01 path; this test asserts the reuse preserves the reference shape); WD-23 (`reason` is forward-compatible — slice-01 readers MUST ignore it; this test asserts the field is present on the published record but does not require slice-01 reader presence to validate)
- **Type**: blocking
- **Wall-clock target**: < 30 s

### 3.5 `at-peer-remove-purge-zero-residue`
- **Command**: `cargo nextest run --test peer_remove_purge_zero_residue`
- **What it does**: subscribes to a fixture peer; pulls N claims; runs `peer remove <did> --purge` (with the interactive confirmation auto-confirmed via `--yes-i-am-tested` test-only flag — flagged for DESIGN as the test-only escape hatch since WD-21 forbids `--yes` in production); asserts `SELECT COUNT(*) FROM peer_claims WHERE author_did = '<did>'` returns 0; asserts subscription row also removed; asserts the user's own counter-claims authored against THAT peer's claims still exist in `author_claims` (per WD-25 — user's published artifacts survive); asserts a subsequent `peer pull` (after re-`peer add`) re-fetches cleanly.
- **Maps to**: KPI-FED-4 (zero residue); WD-25 (soft vs hard semantics; counter-claim survival)
- **Type**: blocking GUARDRAIL
- **Wall-clock target**: < 30 s

### 3.6 `contract-pact-pds-peer` (new sub-job under existing `contract-pact-pds`)
- **Command**: `cargo nextest run --test pact_pds_peer`
- **What it does**: extends the foundation Pact suite (§4.5) with consumer-driven contracts for:
  - `com.atproto.repo.listRecords` — required for `peer pull` to enumerate a peer's `org.openlore.claim` records; pact covers happy path + pagination cursor + empty result + 404 collection.
  - `com.atproto.repo.getRecord` — required ONLY IF DESIGN selects targeted-fetch-by-CID for peer pull (per task spec Phase 4). If DESIGN selects list-only, this Pact is dropped. **Open question to DESIGN**; default: include both Pacts because the cost is low and dropping later is trivial.
  - `com.atproto.identity.resolveHandle` already in foundation Pact; no extension needed (peer DID resolution reuses).
- **Provider**: recorded fixture from `bsky.social` for read paths (captured once by DEVOPS, committed to `tests/contracts/pact/`); wiremock for write paths.
- **Maps to**: WD-24 (pull mechanism); §4 of task spec
- **Type**: blocking
- **Wall-clock target**: < 30 s (mocked); ~2 min for the real-PDS variant (release-tag only)
- **Real-PDS variant**: gated by `PACT_REAL_PDS=1` env var; runs in release workflow only (per D-D12). The new peer-read endpoints are exercised against `bsky.social` once per release — same manual-approval gate as the foundation Pact.

### 3.7 Acceptance-stage summary (delta)

Net additions to foundation §4.6:

| Stage | Wall-clock target | Type | Conditional? |
|---|---|---|---|
| at-federation-attribution-preserved | < 20 s | blocking GUARDRAIL | no |
| at-peer-cid-round-trip | < 20 s | blocking | no |
| at-peer-tampered-signature-rejected | < 30 s | blocking GUARDRAIL (security) | no |
| at-counter-target-cid-round-trip | < 30 s | blocking | no |
| at-peer-remove-purge-zero-residue | < 30 s | blocking GUARDRAIL | no |
| contract-pact-pds-peer (mocked) | < 30 s | blocking | no |
| contract-pact-pds-peer (real bsky) | ~2 min | manual approval at release | release-tag only |

Aggregate added wall-clock: **< 3 min per PR** (jobs parallelize within the
acceptance stage); release-tag overhead **~2 min**. Foundation's target
(< 30 min acceptance) is comfortably preserved.

## 4. Mutation testing (delta)

Per Apex Core Principle 9 + D-D8: nightly-only, scoped to pure-core.

- If DESIGN introduces a new pure-core module (e.g., `peer-claim-verify`
  containing the sig/CID verification logic), it MUST be added to the
  `--package` list of the nightly `cargo mutants` invocation.
- Kill-rate target for the new module: **≥95%** (matches `claim-domain`
  per ADR-006 Earned Trust — sig/CID verification is the load-bearing
  trust primitive of slice-03; pure-core mutation hardness is the price).
- Release-tag mutation re-run inherits the gate from D-D8 (blocking on
  regression).
- DELIVER updates the nightly workflow's `--package` list. No new gate
  semantics; just a wider scope.

Whether a new pure-core module exists is DESIGN's call; if DESIGN keeps
peer-claim verification inside `claim-domain`, this section is a no-op.

## 5. Release workflow (delta)

Per `ci-cd-pipeline.md` (foundation) §7. Slice-03 inserts:

- 5.1 The five new acceptance-stage jobs (§3.1–3.5) are re-run on the tagged ref as part of the existing acceptance re-run. No new step needed; they're already in the workflow.
- 5.2 `contract-pact-pds-peer` with `PACT_REAL_PDS=1` runs against `bsky.social` once at release-tag time, gated by the same manual-approval environment used for the foundation real-PDS Pact (per D-D12). Solo dev clicks Approve once; both old and new Pact suites run against real bsky.
- 5.3 If a new pure-core module exists (§4), the release-tag mutation re-run covers it under the same blocking-on-regression rule.
- 5.4 Substrate matrix: NO new cells; each cell now also exercises the `peer-pull` happy path against the wiremock peer fixture (per `substrate-matrix.md` delta in this dir §1). Same job, expanded body.

Estimated release wall-clock (delta): **+3 to +5 min** (the at-peer-tampered-signature test plus the real-bsky listRecords/getRecord Pact). Foundation estimate was 15–30 min; new estimate 18–35 min. Acceptable.

## 6. Quality-gate enforcement summary (delta rows only)

Insert these rows into the foundation table at §9:

| Gate | Pre-PR (local) | PR | Nightly | Release-tag |
|---|---|---|---|---|
| at-federation-attribution-preserved | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-peer-cid-round-trip | – | ✓ blocking | – | ✓ blocking |
| at-peer-tampered-signature-rejected | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-counter-target-cid-round-trip | – | ✓ blocking | – | ✓ blocking |
| at-peer-remove-purge-zero-residue | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| contract-pact-pds-peer (mocked) | – | ✓ blocking | – | ✓ blocking |
| contract-pact-pds-peer (real bsky) | – | – | – | ✓ manual approval |
| mutation testing (peer-verify if pure-core) | – | – | ✓ advisory | ✓ blocking on regression |

The "Pre-PR (local)" column is intentionally empty for all new gates —
acceptance tests are too slow for pre-push (foundation pre-push runs only
unit + property + arch). The pre-commit and pre-push hook designs from
foundation §5 are unchanged.

## 7. Adversarial-fixture maintenance helper (proposed for DELIVER)

To prevent the tampered-record fixture from drifting away from the live
`org.openlore.claim` Lexicon shape:

- **`cargo xtask regenerate-peer-fixtures`**: reads `lexicons/org/openlore/claim.json`; generates three tampered-record bodies (bad-sig / mutated-body / wrong-CID); writes them to `tests/fixtures/peer-adversarial/{bad_sig,mutated_body,wrong_cid}.json`; updates the wiremock fixture-setup file.
- **CI check**: a separate cheap check verifies the generated bodies are CURRENT (re-runs the regenerator with `--check` flag; fails if the committed bodies differ). Run as part of the existing `arch-check` stage to avoid a new top-level job.
- **DELIVER scope**: if DELIVER's slice-03 scope is tight, this helper can defer to a follow-up — the fixture works without the regenerator; it just risks drift.

## 8. Branch protection rules (UNCHANGED)

Foundation §10 rules carry forward unchanged. The new acceptance jobs are
added to the "required status checks" list at the same level as the
existing acceptance jobs.

## 9. `deny.toml` (UNCHANGED)

Foundation §11 content unchanged. Slice-03 does not introduce new
dependencies whose licenses/sources/bans warrant additions. DELIVER will
amend `deny.toml` if the DESIGN-wave technology choices add a crate not
already covered (e.g., a `pact-verifier` extension for the peer Pact —
already in foundation's Pact tooling chain in practice).

## 10. References

- `platform-design.md` (sibling, this dir) — gate-inventory delta
- `observability.md` (sibling, this dir) — what the new tests' events look like
- `kpi-instrumentation.md` (sibling, this dir) — KPI-FED gate mapping
- Foundation `ci-cd-pipeline.md` — the base to extend
- `docs/feature/openlore-federated-read/feature-delta.md` (WD-14..WD-25)
- `docs/feature/openlore-federated-read/discuss/outcome-kpis.md`
