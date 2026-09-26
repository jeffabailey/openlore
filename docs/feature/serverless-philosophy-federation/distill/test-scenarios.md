# Acceptance Test Design — serverless-philosophy-federation (DISTILL)

- **Wave**: DISTILL
- **Date**: 2026-07-15
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: serverless-philosophy-federation (slices 01-05)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (per ADR-007)
- **Language**: Rust (`[lang-mode] rust`) · Skip marker `#[ignore]` · PBT lib proptest (layer 2 only)
- **Test framework**: Rust std `#[test]` + `assert_cmd` subprocess (repo convention, ADR-009), port-to-port hexagonal — MATCHES the sibling `openlore-federated-read` DISTILL output
- **Inherits**: DISCUSS (D-1..D-9) + DESIGN (DDD-1..8, ADR-062) + DEVOPS (DV-1..8) + SPIKE-00 (OD-SF-1 = opaque transport)
- **Policy mode**: `[policy-mode] inherit` (file present at `docs/architecture/atdd-infrastructure-policy.md`; new publish/instance ports APPENDED) · `[port-mode] inherit` (`tests/common/state_delta.rs` shipped slice-01)

This document is the human-readable map over the executable test skeletons in
`tests/acceptance/{publish_init,publish_roundtrip,publish_push,publish_pull_reconcile,public_card,cross_instance_pull}.rs`.
The `.rs` files are the SSOT for executable scenarios.

---

## 1. Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** across DISCUSS (D-1..D-9) ↔ DESIGN
(DDD-1..8, ADR-062) ↔ DEVOPS (DV-1..8, environments.yaml).

Consistency spot-checks (each PASS):

| Concern | DISCUSS | DESIGN | DEVOPS | Verdict |
|---|---|---|---|---|
| Hosting model | self-hosted serverless, no central authority (D-1/D-3/D-4) | opaque store, single-owner DO (ADR-062 §2) | no central deploy pipeline, per-user self-deploy (DV-2/DV-3) | consistent |
| Round-trip CID integrity | KPI-SF-1 hard gate (D-6) | single-canonicalizer + verbatim store + recompute-on-pull (ADR-062 §3) | CI round-trip + 0.0/0.5/1.0 float guard (DV-1) | consistent |
| Transport | ADR-027 configurable URL, additive (D-5) | opaque content-addressed HTTP blob (DDD-1) | generic opaque HTTP contract (env `deployment_assumptions`) | consistent |
| Card | read-only, signing-incapable, Bluesky-linkable, no consensus row (D-7) | `GET /` read-only from manifest (ADR-062 §1) | reads public (DV-4) | consistent |
| Cross-instance | J-003 transport delta only (D-8) | byte-preserving opaque read + local Rust verify (DDD-5 / §4) | — | consistent |
| Write-auth | (implied by D-7 signing-incapable) | write/read port split (DDD-8) | per-instance bearer token; reads public (DV-4) | **additive, not contradictory** — DV-4 refines D-7 |

DV-4 (writes owner-authed / reads public) ELABORATES D-7's signing-incapable card;
it does not contradict any DISCUSS decision. No CLARIFICATION_NEEDED raised.

---

## 2. Q-SF-D5 resolution (DISTILL-owned open question)

**Question** (DESIGN "Deferred to DISTILL"): how does the CLI detect it is talking
to an opaque *openlore* instance versus an arbitrary URL?

**Resolution (LOCKED by this DISTILL wave)**: the opaque-instance detection marker is
the **`GET /manifest` openlore discriminator envelope**. `openlore publish init <url>`
(and the `adapter-publish-http.probe()` per ADR-062 §6) issues `GET /manifest` and
requires the response to be a well-formed openlore manifest carrying an explicit
`openlore` marker field (instance kind = opaque + contract version). A URL that is
reachable but whose `/manifest` lacks the marker is an arbitrary URL — NOT an openlore
instance — and registration is REFUSED. This piggybacks on the existing "wire then
probe then use" gate (no new route beyond the ADR-062 §1 contract) and gives three
distinct, testable init outcomes: registered (marker present), refused-unreachable,
refused-not-an-openlore-instance (marker absent).

**Bound by**: `publish_init.rs` PI-4
(`publish_init_refuses_a_reachable_url_that_is_not_an_openlore_instance`, `@q-sf-d5`),
with PI-1 (marker present → registered) and PI-2 (unreachable → refused) as the
contrasting outcomes.

**Note on Q-SF-D2** (write-auth, DEVOPS-framed via DV-4, DISTILL owns the detailed
contract): bound by `publish_push.rs` PP-5 (write requires owner token) +
`public_card.rs` PC-6 (reads require no token) — the DV-4 asymmetry made observable.

---

## 3. Scope and shape

Same hexagonal port-to-port discipline as the sibling: every subprocess acceptance
test enters through the real `openlore` CLI driving adapter via `assert_cmd`, exercises
the REAL `claim-domain` (the SOLE canonicalizer — SPIKE-00 / ADR-062 invariant) +
`lexicon` + `adapter-duckdb` local core, and fakes ONLY the NEW external boundary — the
CLI↔Cloudflare-Worker seam — via `openlore_test_support::FakeInstance` (an opaque
content-addressed HTTP double mirroring the ADR-062 §1 contract, the exact
`FakePds`/`FakePeerPds` pattern).

The LIVE `wrangler dev`/workerd round-trip is NOT an acceptance test — it is the DEVOPS
CI contract test `publish-contract.yml` (DV-1, incl. the 0.0/0.5/1.0 float guard). This
suite references it and does not duplicate it.

### Layer placement (Mandate 9)

| Layer | Test file(s) | Real adapters | Test mode |
|---|---|---|---|
| Walking-Skeleton subprocess (layer 5) | `publish_roundtrip.rs` WS-1 (NOT `#[ignore]`) | CLI + DuckDB + FS; FakeInstance double | example-only |
| Subprocess / FS acceptance (layer 3) | all remaining scenarios across the 6 files | CLI + DuckDB + FS; FakeInstance / peer-FakeInstance doubles | example-only (Mandate 11 — sad paths named, never PBT) |

Layer 1 (pure-core unit tests: `compute_cid`, `parse_signed_claim`, publish-domain
decision core) is OUT OF DISTILL SCOPE — DELIVER's inner TDD loop (per ADR-062 pure-core
`publish-domain`). No layer-2 `@property` proptest is warranted here: the CID round-trip
invariant is already property-pinned in the shipped `lexicon_conformance.rs`; this
feature's novel surface is transport + reconcile + card, all example-shaped.

### What is faked, what is real

| Component | Treatment | Why |
|---|---|---|
| `openlore` CLI binary | REAL (subprocess via `assert_cmd`) | the driving adapter — must be exercised |
| `claim-domain` (CID / canonicalize / sign / verify) | REAL | the SOLE canonicalizer; faking = testing-theater and would defeat KPI-SF-1 |
| `lexicon` / `parse_signed_claim` | REAL | the shipped decode path reused on pull (Q-SF-D4) |
| `adapter-duckdb` (local `claims` + `peer_claims` + `<cid>.json`) | REAL (DuckDB under `tempfile::TempDir`) | reconcile + no-silent-overwrite are the point (US-SF-004); catches format drift |
| CLI↔Worker opaque HTTP seam (`PublishPort`/`InstanceReadPort`) | NEW FAKE `FakeInstance` | external + non-deterministic boundary; real workerd round-trip = DV-1 CI contract test |
| peer DID → Cloudflare serviceEndpoint | FAKE (`FakeInstance`-as-peer + fixture resolver) | OD-SF-3; reuses J-003 resolution; real PLC = recorded fixture / DEVOPS contract |
| `adapter-system-clock` | REAL (`std::time`) | degenerate adapter |

---

## 4. Acceptance test inventory (30 scenarios; 1 active + 29 `#[ignore]`)

### `tests/acceptance/publish_roundtrip.rs` — WALKING SKELETON — 4 scenarios

Stories: US-SF-001 + US-SF-002 (J-007 + J-008). WS-1 is **NOT `#[ignore]`** (RED at
handoff via `todo!()`; the first slice DELIVER makes green).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| WS-1 | `publish_round_trips_one_signed_claim_through_my_own_instance_with_identical_cid` | US-SF-001+002 | walking skeleton | `@walking_skeleton @driving_port @real-io @us-sf-001 @us-sf-002 @j-007 @j-008 @kpi-sf-1` |
| RT-2 | `publish_push_rejects_a_claim_whose_cid_does_not_survive_the_boundary` | US-SF-002 | error | `@error @kpi-sf-1` |
| RT-3 | `publish_push_exits_nonzero_when_the_instance_is_unreachable_and_leaves_authoring_untouched` | US-SF-002 | error/boundary | `@error @kpi-sf-5` |
| RT-4 | `publish_round_trips_confidence_zero_half_one_each_cid_identical` | US-SF-002 | **GOLD FIXTURE** | `@gold-fixture @float-guard @regression @kpi-sf-1` |

### `tests/acceptance/publish_init.rs` — 5 scenarios (US-SF-001, J-007)

| # | Test name | Type | Tag(s) |
|---|---|---|---|
| PI-1 | `publish_init_registers_reachable_instance_and_prints_owned_identity` | happy | `@us-sf-001 @driving_port @real-io @j-007 @happy` |
| PI-2 | `publish_init_refuses_to_register_an_unreachable_instance_url` | error | `@error @j-007` |
| PI-3 | `publish_init_never_changes_the_signing_identity_or_local_store` | error | `@error @j-007` |
| PI-4 | `publish_init_refuses_a_reachable_url_that_is_not_an_openlore_instance` | error | `@error @q-sf-d5 @j-007` |
| PI-5 | `publish_status_reports_the_registered_instance_and_leaves_local_store_untouched` | edge | `@edge @j-007` |

### `tests/acceptance/publish_push.rs` — 5 scenarios (US-SF-003, J-008)

| # | Test name | Type | Tag(s) |
|---|---|---|---|
| PP-1 | `publish_push_sends_only_new_claims_additive_and_cid_verified` | happy | `@us-sf-003 @driving_port @real-io @j-008 @happy` |
| PP-2 | `publish_push_is_idempotent_re_push_creates_no_duplicates` | edge | `@edge @j-008` |
| PP-3 | `publish_push_resumes_after_interruption_without_duplicates` | error | `@error @j-008` |
| PP-4 | `publish_push_never_mutates_the_local_store` | guardrail | `@j-008 @guardrail` |
| PP-5 | `publish_push_requires_the_owner_write_token_and_is_refused_without_it` | error | `@error @dv-4 @q-sf-d2 @j-008` |

### `tests/acceptance/publish_pull_reconcile.rs` — 5 scenarios (US-SF-004, J-008)

| # | Test name | Type | Tag(s) |
|---|---|---|---|
| PR-1 | `publish_pull_is_an_additive_reconcile_that_never_overwrites_local_claims` | happy | `@us-sf-004 @driving_port @real-io @j-008 @happy` |
| PR-2 | `publish_pull_rebuilds_local_duckdb_on_a_fresh_machine_with_attribution_intact` | boundary | `@real-io @j-008 @boundary` |
| PR-3 | `publish_pull_surfaces_a_conflict_instead_of_silently_overwriting` | error | `@error @j-008` |
| PR-4 | `publish_pull_recomputes_an_identical_cid_for_every_pushed_claim` | integrity | `@kpi-sf-1 @j-008` |
| PR-5 | `publish_pull_exits_nonzero_when_the_instance_is_unreachable` | error | `@error @kpi-sf-5 @j-008` |

### `tests/acceptance/public_card.rs` — 6 scenarios (US-SF-005, J-007)

| # | Test name | Type | Tag(s) |
|---|---|---|---|
| PC-1 | `public_card_renders_only_pushed_claims_each_attributed_to_its_author_did` | happy | `@us-sf-005 @driving_port @real-io @j-007 @happy` |
| PC-2 | `public_card_on_an_empty_instance_renders_no_claims_published_yet_not_an_error` | edge | `@edge @j-007` |
| PC-3 | `public_card_omits_a_local_claim_that_was_never_pushed` | guardrail | `@j-007 @guardrail` |
| PC-4 | `public_card_offers_no_authoring_or_write_control_and_holds_no_signing_key` | error | `@error @d-7 @j-007` |
| PC-5 | `public_card_shows_two_authors_identical_claim_as_two_attributed_rows_never_one_consensus_row` | anti-merging | `@anti-merging @kpi-sf-3 @j-007` |
| PC-6 | `public_card_and_manifest_reads_require_no_token` | guardrail | `@dv-4 @j-007` |

### `tests/acceptance/cross_instance_pull.rs` — 5 scenarios (US-SF-006, J-003)

| # | Test name | Type | Tag(s) |
|---|---|---|---|
| CI-1 | `peer_pull_fetches_a_peers_claims_from_their_cloudflare_instance_into_peer_claims_attributed` | happy | `@us-sf-006 @driving_port @real-io @j-003 @happy` |
| CI-2 | `peer_pull_rejects_a_tampered_peer_claim_and_stores_the_valid_ones` | error | `@error @j-003` |
| CI-3 | `peer_pull_skips_only_the_unreachable_peer_instance_and_proceeds_with_others` | error | `@error @j-003` |
| CI-4 | `peer_pull_from_an_instance_never_merges_peer_claims_with_my_own` | guardrail | `@anti-merging @j-003 @guardrail` |
| CI-5 | `peer_pull_recomputes_each_peer_claim_cid_from_the_opaque_read_before_store` | integrity | `@kpi-sf-1 @j-003` |

### Totals

30 scenarios: **1 active** (WS-1) + **29 `#[ignore]`**. Error/edge-path scenarios:
14/30 = **46.7%** (>= 40% target). Per-file error ratios: init 4/5, roundtrip 2/4,
push 3/5, pull 2/5, card 2/6, cross-instance 2/5.

---

## 5. US-SF-001..006 coverage (every story has >= 1 bound scenario)

| Story | Job | Bound scenarios |
|---|---|---|
| US-SF-001 deploy + register own instance | J-007 | WS-1, PI-1, PI-2, PI-3, PI-4, PI-5 |
| US-SF-002 round-trip one claim, CID-verified | J-008 | WS-1, RT-2, RT-3, RT-4 |
| US-SF-003 bulk push additive/idempotent/resume | J-008 | PP-1, PP-2, PP-3, PP-4, PP-5 |
| US-SF-004 pull reconcile, no silent overwrite | J-008 | PR-1, PR-2, PR-3, PR-4, PR-5 |
| US-SF-005 public read-only card | J-007 | PC-1, PC-2, PC-3, PC-4, PC-5, PC-6 |
| US-SF-006 cross-instance federated read | J-003 | CI-1, CI-2, CI-3, CI-4, CI-5 |

Zero uncovered stories.

---

## 6. Bound assertions (DESIGN handoff + system constraints)

| Assertion (DESIGN handoff / constraint) | Bound scenario(s) |
|---|---|
| Round-trip CID integrity KPI-SF-1 on the opaque transport (pulled = recomputed Rust CID; mismatch = rejected sync, not silent accept) | WS-1, RT-2, RT-4, PR-4, CI-5 |
| GOLD FIXTURE 0.0 / 0.5 / 1.0 f16 confidence each push→pull CID-identical (guard vs rejected re-encode) | **RT-4** |
| Additive push: no local DuckDB mutation; local stays canonical (D-6) | PP-1, PP-4, WS-1 |
| Idempotent push + interrupted-resume: no duplicates (`PUT /records/:cid` re-PUT no-op) | PP-2, PP-3 |
| Pull reconcile NEVER silently overwrites; surfaces conflicts | PR-1, PR-3 |
| Card: only explicitly-pushed claims; per-author attribution; NO consensus row; read-only + write-incapable | PC-1, PC-3, PC-4, PC-5 |
| Cross-instance pull reuses J-003 verify/attribution/anti-merging; peer_claims separation | CI-1, CI-2, CI-4, CI-5 |
| Offline authoring unaffected (KPI-SF-5): compose/sign + `graph query` never depend on instance reachability | RT-3, PR-5 |
| Write path owner-authed (bearer token, DV-4); reads public | PP-5, PC-6 |
| Q-SF-D5 opaque-instance detection marker (GET /manifest openlore discriminator) | PI-4 |

---

## 7. Driving Adapter coverage (Mandate 1 + RCA P1)

| Verb / surface (DDD-4 / ADR-062) | Scenario coverage |
|---|---|
| `openlore publish init <url>` (NEW) | PI-1 (happy), PI-2 (unreachable), PI-3 (identity unchanged), PI-4 (Q-SF-D5), WS-1 |
| `openlore publish push` (NEW) | WS-1, RT-2, RT-3, RT-4, PP-1..PP-5 |
| `openlore publish pull` (NEW) | WS-1, PR-1..PR-5 |
| `openlore publish status` (NEW) | PI-5 |
| `GET /` public card (NEW, Worker) | PC-1..PC-6 (via FakeInstance; real render = DV-1 CI contract) |
| `openlore peer add/pull` — Cloudflare transport delta (EXTENDED) | CI-1..CI-5 |

Zero uncovered NEW entry points.

---

## 8. Driven adapter coverage (Mandate 6)

| Driven adapter | Real-I/O scenario? | Note |
|---|---|---|
| `adapter-duckdb` (local `claims` + `<cid>.json`) | YES — PP-1/PP-4 (no-mutation), PR-1/PR-2 (reconcile/rebuild), WS-1 | `@real-io` |
| `adapter-duckdb` (`peer_claims`) | YES — CI-1, CI-4 | `@real-io` |
| `claim-domain` CID/verify (pure core, real) | YES — every round-trip/verify scenario | linked, real |
| CLI↔Worker opaque seam (`PublishPort`/`InstanceReadPort`) | FAKE `FakeInstance` — every publish scenario | real workerd round-trip = **DV-1 CI contract test** (`publish-contract.yml`), not DISTILL's |
| peer DID → Cloudflare serviceEndpoint (`IdentityPort::resolve_peer`) | FAKE (`FakeInstance`-as-peer + fixture resolver) — CI-1..CI-5 | real PLC = recorded fixture / DEVOPS |
| `adapter-system-clock` | YES — implicit (status/composed_at) | real |

The FAKE coverage on the two external network boundaries is structural: per DV-1 the
REAL CLI↔Worker contract is verified against live `wrangler dev`/workerd in
`publish-contract.yml` (incl. the 0.0/0.5/1.0 float guard); DISTILL ships the acceptance
shape against the double.

---

## 9. KPI coverage

| KPI | Description | Acceptance coverage |
|---|---|---|
| KPI-SF-1 | 100% round-trip CID integrity (North Star) | WS-1, RT-2, RT-4, PR-4, CI-5 (+ DV-1 CI round-trip) |
| KPI-SF-2 | Dogfood deploy→register→first round-trip < 10 min | Not asserted at this layer — dogfood timing (env `real-deploy`), no telemetry |
| KPI-SF-3 | Card renders 100% pushed claims attributed, 0 consensus rows | PC-1, PC-5 |
| KPI-SF-5 | Local-first: offline authoring never depends on instance | RT-3, PR-5 |

KPI-SF-2 is dogfood-measured (per US-SF-001 Outcome KPIs), not an acceptance assertion.

---

## 10. Three Pillars compliance

| Pillar | How satisfied |
|---|---|
| 1 — Domain language | Scenario titles use `publish`, `push`, `pull`, `round-trip`, `claim`, `CID`, `instance`, `card`, `attribution`, `reconcile`, `peer`. Zero technical jargon (`HTTP`, `endpoint`, `database`, `schema`, `JSON`, `DO`) in ANY test name. (`CID` is a domain term in this codebase — the content address IS the contract.) |
| 2 — Chained narrative | The publish story reads in order: PI-1 (init registers) → WS-1 (init + push + pull round-trip) → PP-1 (bulk push builds on the proven pipe) → PR-1 (pull reconciles what push wrote) → PC-1 (card renders what was pushed). Each `Given` reuses the prior `Given + When` step-methods (`init a registered FakeInstance`, `push local_graph_of(n)`) via shared support helpers — no copy-pasted fixture setup. |
| 3 — App as in production | Every scenario spawns the REAL `openlore` binary via `assert_cmd::cargo_bin`. No hand-rebuilt wiring. Only the external CLI↔Worker seam (+ peer resolver) is a double, per the Architecture of Reference defaults + the Project Infrastructure Policy rows appended this wave. Tier B (state-machine PBT) NOT added — see §11 CM-G. |

---

## 11. Mandate compliance evidence (CM-A..CM-H)

| Mandate | Evidence |
|---|---|
| CM-A (Mandate 1, hexagonal boundary) | All six files invoke `openlore` via subprocess; ZERO direct imports of `claim_domain::*` / `adapter_duckdb::*`. Driving-port-only entry. |
| CM-B (Mandate 2, business language) | Grep of test names: zero `HTTP`/`endpoint`/`database`/`schema`/`JSON`/`worker`. Domain terms only. |
| CM-C (Mandate 3, complete journeys) | Every test traces a user trigger → observable outcome → business value (see §5 traceability). Chained narrative per Pillar 2. |
| CM-D (Mandate 4, pure function extraction) | The publish decision core (`publish-domain` diff/verify decisions per ADR-062) is pure; CLI parameterization is just `tempfile::TempDir` HOME — no environment cross-product. Pure-core unit pinning is DELIVER's layer-1 loop. |
| CM-E (Mandate 8, state-delta + Universe) | **DEFERRED to DELIVER** — same status as the sibling slice-01..05 (`state_delta.rs` shipped slice-01; scenarios use named assertion helpers as the Rust idiomatic mirror). DELIVER migrates the load-bearing scenarios (WS-1, PP-1/PP-4, PR-1/PR-3, CI-1/CI-4) to `assert_state_delta(before, after, universe, expected)`. Universe entries specified per-scenario in the `.rs` `todo!()` bodies and MUST be port-exposed (e.g. `instance.records.cids`, `local.claims.row_count`, `peer_storage.claims.row_count_by_author[did]`, `card.rows[*].author_did`) — NEVER internal struct fields. |
| CM-F (Mandate 9, layered PBT mode) | ZERO proptest at layer 3+; all scenarios example-only. No layer-2 `@property` warranted (the CID invariant is already pinned in shipped `lexicon_conformance.rs`). |
| CM-G (Mandate 10, two-tier acceptance) | **Tier A only.** The publish journey IS >= 3 chained scenarios (init → push → pull → card), but the input space is NOT domain-rich (it is "a set of already-signed claims keyed by CID" — a fixed content-addressed set, not emails/free-text/payloads). Per Mandate 10 "skip Tier B when the input space is not domain-rich", Tier A example tests cover the contract surface; the load-bearing invariant (round-trip CID integrity) is a single-canonicalizer STRUCTURAL property (ADR-062), not a state-space to explore. Revisit if multi-instance reconcile conflict-resolution policy grows a real state machine. |
| CM-H (Mandate 11, sad paths example-based) | Every sad path is a named example (`*_rejects_*`, `*_refuses_*`, `*_exits_nonzero_*`, `*_surfaces_a_conflict_*`): RT-2, RT-3, PI-2, PI-3, PI-4, PP-3, PP-5, PR-3, PR-5, PC-4, CI-2, CI-3. ZERO PBT machinery at layer 3+. |

---

## 12. Pre-requisites for compilation (DELIVER wiring expectations)

The skeletons use `use openlore_test_support::...` for the NEW `FakeInstance` double and
new support helpers. Intentional consequence (mirrors the sibling DISTILL→DELIVER
boundary):

1. `cargo build --tests` fails until DELIVER's first slice-01 step scaffolds:
   - `crates/test-support/src/fake_instance.rs` — `FakeInstance` opaque HTTP double +
     postures (`fresh`, `with_records`, `unreachable`, `not_an_openlore_instance`,
     `with_cid_mismatch`, `requiring_write_token`) + `endpoint_url` / `card_html` /
     `manifest` / `stored_cids`.
   - `tests/acceptance/support/mod.rs` extensions: `run_openlore_publish(env, args,
     &FakeInstance)` (wires `OPENLORE_PUBLISH_ENDPOINT`), gold fixtures
     (`gold_claims_confidence_0_half_1`, `local_graph_of(n)`), assertion helpers
     (`assert_instance_stores_cid`, `assert_local_claims_unchanged`,
     `assert_card_attributes_to`, `assert_no_consensus_row`).
   - `crates/ports`: `PublishPort` (write) + `InstanceReadPort` (read-only) + boundary
     ADTs + probe-refusal reasons (`publish.cid_roundtrip_failed`,
     `publish.instance_unreachable`).
   - `crates/adapter-publish-http` + `crates/publish-domain` (or fold, Q-SF-D6).
   - `crates/cli`: the `openlore publish {init,push,pull,status}` verb handlers +
     the peer-pull Cloudflare transport delta.
   - Workspace `Cargo.toml`: `[[test]]` entries for the six new files.

2. Once those land, the tests compile to "all `#[test]` bodies `todo!()` → panic" → RED
   per Mandate 7. WS-1 (not `#[ignore]`) is the first RED the crafter turns green;
   DELIVER unskips the remaining 29 one at a time.

3. **Rust scaffold marker** per Mandate 7 + sibling precedent: every file carries a
   `// SCAFFOLD: true` module marker; every body is `todo!("DELIVER (...)")`. Detection:
   `grep -r "SCAFFOLD: true" tests/`.

4. **Pre-DELIVER fail-for-right-reason gate**: DEFERRED (same logic as the sibling
   slice-01..05) — the test-support `FakeInstance` + ports + publish crate land in
   DELIVER's first step; only then do the tests compile and reach the `todo!()` panic
   that classifies as RED.

---

## 13. Definition of Done (DISTILL → DELIVER)

- [x] All 30 scenarios written as RED-ready Rust `todo!()` skeletons; WS-1 not `#[ignore]`.
- [x] Every US-SF-001..006 story has >= 1 bound scenario (§5).
- [x] Every NEW/EXTENDED CLI verb + the card surface covered by a subprocess scenario (§7).
- [x] Every driven adapter mapped (real, or fake with explicit DV-1 CI-contract justification) (§8).
- [x] Three Pillars verified (§10).
- [x] Wave-decision reconciliation passed (0 contradictions, §1).
- [x] Q-SF-D5 resolved + bound (§2); Q-SF-D2 write-auth bound (PP-5/PC-6).
- [x] GOLD FIXTURE 0.0/0.5/1.0 present (RT-4).
- [x] Error/edge path ratio 46.7% (>= 40%).
- [x] Project Infrastructure Policy present; new publish/instance ports appended.
- [ ] **Pre-DELIVER fail-for-right-reason gate**: DEFERRED until DELIVER scaffolds
      `FakeInstance` + ports + publish crate (§12).

Handoff-ready: **YES**, conditional on DELIVER's first slice-01 step landing the
`FakeInstance` double + `PublishPort`/`InstanceReadPort` + the publish verb handlers +
the six `[[test]]` Cargo entries before running the suite the first time.

---

## 14. Open items for DELIVER

1. Materialize `crates/test-support/src/fake_instance.rs` (opaque HTTP double + 6 postures).
2. Extend `tests/acceptance/support/mod.rs` with `run_openlore_publish` + fixtures +
   assertion helpers (§12).
3. Land `crates/ports` `PublishPort`/`InstanceReadPort` + ADTs + probe reasons; decide
   Q-SF-D6 (`publish-domain` crate vs fold into `cli`).
4. Implement `openlore publish {init,push,pull,status}` handlers + peer-pull Cloudflare
   transport delta; reuse `parse_signed_claim` (Q-SF-D4 hoist-vs-dup).
5. Migrate the load-bearing scenarios (WS-1, PP-1/PP-4, PR-1/PR-3, CI-1/CI-4) to
   `assert_state_delta` with port-exposed universe entries (CM-E).
6. Build the `atproto/` Worker (TS) + wire the DV-1 `publish-contract.yml` CI round-trip
   (live `wrangler dev`, incl. the 0.0/0.5/1.0 float guard) — coordination point, not a
   Rust-crafter deliverable.
7. `xtask check-arch`: `publish_write_capability_isolated` + `atproto/` no-IPLD/CBOR
   dependency guard (ADR-062 §6 Earned Trust).

---

## 15. References

- DISCUSS: `../feature-delta.md` (D-1..D-9, US-SF-001..006, slices)
- DESIGN: `../design/wave-decisions.md` (DDD-1..8) + `docs/adrs/ADR-062-serverless-opaque-federation-transport.md`
- DEVOPS: `../devops/wave-decisions.md` (DV-1..8) + `../environments.yaml`
- SPIKE-00: `../spike/findings.md` (opaque transport, OD-SF-1)
- Sibling pattern: `docs/feature/openlore-federated-read/distill/acceptance-tests.md` + `tests/acceptance/peer_*.rs`
- Policy: `docs/architecture/atdd-infrastructure-policy.md` (publish/instance rows appended this wave)
- Executable SSOT: `tests/acceptance/{publish_init,publish_roundtrip,publish_push,publish_pull_reconcile,public_card,cross_instance_pull}.rs`
