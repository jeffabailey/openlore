# Wave Decisions — DISTILL — openlore-federated-read (slice-03)

- **Wave**: DISTILL
- **Date**: 2026-05-27
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-federated-read (slice-03)
- **Inherits**: DISCUSS WD-14..WD-25 + DESIGN WD-26..WD-45 + all 16 ADRs
  (ADR-001..ADR-016); slice-01 DISTILL DD-1..DD-13 also apply where the
  slice-03 surface is symmetric

This file records DISTILL-wave decisions (DD-FED-N prefix to keep the
namespace distinct from slice-01 DD-N). Decisions that point at a test
artifact (a file under `tests/acceptance/` or `crates/test-support/src/`)
are binding for DELIVER unless re-opened.

---

## Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** between DISCUSS
WD-14..WD-25 + OD-FED-1/2/3 (accepted at default) and DESIGN
WD-26..WD-45. DESIGN's WD-38..WD-44 RESOLVE the seven
`# DISTILL: confirm` flags from
`docs/feature/openlore-federated-read/discuss/gherkin-scenarios-expanded.md`;
DISTILL inherits those resolutions verbatim (see acceptance-tests.md §2
for the consolidated table).

DEVOPS missing → applied Graceful Degradation matrix WARN + default
environment matrix; no acceptance scenario in slice-03 depends on a
per-environment fixture cross-product.

---

## Locked decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| DD-FED-1 | Slice-03 acceptance tests inherit the slice-01 framework: Rust std `#[test]` with snake_case function names encoding the scenario (`peer_subscribe_*`, `peer_pull_*`, `counter_claim_*`, `federated_query_*`, `lexicon_counter_claim_*`). No new test-framework dependency; no `.feature` files. | Symmetric with slice-01 DD-1; preserves `cargo test --test <file>` ergonomics; one place for the test inventory per file. | LOCKED |
| DD-FED-2 | Peer-PDS test double is a NEW crate-internal module `openlore_test_support::fake_peer_pds::FakePeerPds`, DISTINCT from the existing `FakePds`. Peer PDSes are HONESTLY a different actor (read-only XRPC; no createRecord; adversarial postures preconfigured at construction). | Folding peer-read into `FakePds` would (a) expose write-paths on a surface that has none in production, (b) make the adversarial constructors (`with_tampered_signature` etc.) confusingly applicable to the user's-own-PDS double. Separate types = separate contracts. | LOCKED |
| DD-FED-3 | Adversarial-peer fixtures are constructor-style on `FakePeerPds` (e.g. `FakePeerPds::with_tampered_signature(peer_did, honest_records)`) rather than mutating methods on an already-constructed fake. State is fixed at construction; the SUT sees a deterministic posture for the lifetime of the scenario. | Construction-time posture pinning prevents "did the test set up the adversarial mode before or after the subscribe step?" race-condition test-bugs. Same pattern as slice-01's `FakePds::for_did` constructor-time DID pinning. | LOCKED |
| DD-FED-4 | The DISTILL test placement is the FLAT layout under `tests/acceptance/` matching the slice-01 pattern (option A from task brief). Files: `peer_subscribe.rs`, `peer_pull.rs`, `counter_claim.rs`, `federated_query.rs`, `lexicon_counter_claim.rs`. NOT nested under `tests/acceptance/openlore_federated_read/`. | Preserves `cargo test --test <file>` ergonomics from slice-01; four new files are clearly labeled by their domain. A nested directory would require either a `tests/acceptance/openlore_federated_read/main.rs` aggregator OR splitting the shared `support/` into per-feature copies — both anti-patterns. The slice-01 test files (`walking_skeleton.rs` etc.) remain at the top level for backward compatibility with the slice-01 distill artifacts. | LOCKED |
| DD-FED-5 | The shared `tests/acceptance/support/mod.rs` is EXTENDED, not duplicated. Slice-03's `peer_*.rs` + `counter_claim.rs` + `federated_query.rs` `mod support; use support::*;` at the top, same as slice-01's `walking_skeleton.rs`. New assertion helpers (`assert_peer_claims_attributed_to(did, count)`, `assert_no_merged_rows_in_federated_output`, `assert_peer_claims_dir_removed_for(did)`, `assert_orientation_emitted_exactly_once(...)`) land in `support/mod.rs` as DELIVER materializes them. | One source of truth for `TestEnv`, subprocess helpers, and assertion helpers across all acceptance tests. Per-feature support directories would (a) duplicate the subprocess plumbing, (b) make cross-file chained narratives (CC-1 → FQ-5) cumbersome. | LOCKED |
| DD-FED-6 | Pact contract tests for the peer-read paths (`com.atproto.repo.listRecords`, `com.atproto.repo.getRecord`, `com.atproto.identity.resolveDid`) and the adversarial-CI fixture for KPI-FED-6 are DEVOPS's deliverables, NOT DISTILL's. They extend the slice-01 Pact suite. Slice-03 acceptance tests use the in-process `FakePeerPds` double. | Per DESIGN §6.4 + outcome-kpis.md DEVOPS handoff + slice-01 DD-6 precedent. Acceptance tests prove the slice-03 contract through `FakePeerPds`; DEVOPS Pact prove the real adapter against the real ATProto protocol surface. Two surfaces; two deliverables. | LOCKED |
| DD-FED-7 | Pure-core unit tests for `claim_domain::normalize_reason` + `claim_domain::validate_counter_claim` are DELIVER's responsibility (inner TDD loop), NOT DISTILL's. The slice-03 layer-2 `lexicon_counter_claim.rs` exercises them at the in-memory acceptance layer (LCC-3/4/5) but the exhaustive property suite + boundary-condition coverage lives in `crates/claim-domain/src/`'s `#[cfg(test)] mod tests` block. | Symmetric with slice-01 (which kept pure-core unit tests in DELIVER's scope, exposing only the layer-2 acceptance properties to DISTILL). Pure functions live in DELIVER's inner TDD loop; the outer-loop acceptance suite asserts the contract via lexicon validation + CID stability, not via exhaustive unit coverage. | LOCKED |
| DD-FED-8 | Layer-2 lexicon scenarios (LCC-1..LCC-5) live in their own file `lexicon_counter_claim.rs` rather than being appended to slice-01's `lexicon_conformance.rs`. Slice-01 lexicon is "the slice-01 Lexicon shape"; slice-03 lexicon additions are "the `reason` field + NFC normalization + slice-01→slice-03 CID stability". | Keeps the slice-01 file pristine (it's already locked + committed + green); slice-03's lexicon concerns are a focused surface that deserves its own role-prefixed file per DD-1 / DD-FED-1. The `@property` proptest harness for normalize_reason is the most distinctive shape in this file and warrants the separate test-binary boundary (faster `cargo test --test lexicon_counter_claim`). | LOCKED |
| DD-FED-9 | Tier B (state-machine PBT) is NOT added for slice-03 per Mandate 10 evaluation. The two journeys (subscribe-and-read; counter-claim) are 3 and 5 chained scenarios respectively — qualifying on chain length — BUT the input space is bounded (peer DID list + record set + reason text from generated UTF-8). The J-003a anti-merging invariant IS the kind of cross-rule property Tier B catches, but FQ-2 + FQ-8 explicitly assert it at the example layer with multi-author multi-record fixtures. Re-evaluate at slice-04 (multi-peer reputation weighting) where the state space genuinely expands. | Symmetric with slice-01 DD-4 reasoning. Tier B costs the in-memory composition root + the state-machine model + the InMemoryComposition wiring; the cost-benefit only swings positive when the state space is too large for examples to cover. Slice-03's adversarial-peer surface is bounded (4 named postures); FQ-8 explicitly tests the multi-author multi-record release gate. | LOCKED — revisit at slice-04 |
| DD-FED-10 | State-delta + Universe assertions (Mandate 8) at layer 3 (subprocess acceptance) are written via named assertion helper functions in `support/mod.rs` (e.g. `assert_peer_claims_attributed_to(did, expected_count)`), NOT via `assert_state_delta(before, after, universe, expected)` directly. The Rust `state_delta` port at `tests/common/state_delta.rs` was bootstrapped by slice-01; slice-03 INHERITS the port. DELIVER MUST migrate the load-bearing scenarios (PS-6 purge, PP-1 happy pull, PP-3 tampered, PP-5 self-attr, PP-6 cross-attr, FQ-2 zero-merge, FQ-8 release-gate) to explicit `assert_state_delta` form once the assertion helpers' bodies are real. | Two-stage bootstrap symmetric with slice-01 DD-3: DISTILL declares the contract via named helper signatures; DELIVER materializes the universe wiring as each scenario goes green. The universe entries MUST be port-exposed names (`peer_storage.claims.row_count_by_author[did]`, `cli.graph_query.distinct_authors_in_output`, `filesystem.peer_claims_dir.exists[did]`) — NEVER internal struct fields per Mandate 8. Helper names already encode the universe shape; the migration is mechanical. | LOCKED |
| DD-FED-11 | The Project Infrastructure Policy file at `docs/architecture/atdd-infrastructure-policy.md` is STILL NOT written by this DISTILL wave. The orchestrator brief limits writes to `docs/feature/openlore-federated-read/distill/` + `tests/acceptance/` + `crates/test-support/src/`. The slice-03 additions to the inherited inline policy are documented in `acceptance-tests.md §11`; both slice-01 + slice-03 policy entries should land at the project-local file on a future wave whose orchestrator scope permits. | Continues slice-01 DD-11 deferral; cross-wave write-surface convention unchanged. | LOCKED |
| DD-FED-12 | Lexicon `reason` field + claim-domain `normalize_reason` + slice-01→slice-03 CID stability tests live at layer 2 (`lexicon_counter_claim.rs`) with `@property` tags on LCC-3/4 per Mandate 9 layer-2 PBT-full. The CLI-surface counter-claim verb tests (CC-1..CC-6) live at layer 3 (subprocess) and are example-only per Mandate 11. | Layered test discipline: the wire-format + normalization invariants are pure-data properties best expressed at layer 2 with generators; the verb-orchestration behavior (sign + publish + framing + orientation) is example-pinned at layer 3 because each example is a real subprocess invocation with real I/O. | LOCKED |
| DD-FED-13 | Pre-DELIVER fail-for-right-reason gate (slice-03) runs in DELIVER's first slice-03 step (step-06-01), AFTER (a) the `PeerStoragePort` + extended `PdsPort` + extended `IdentityPort` trait surfaces are scaffolded in `crates/ports/`, AND (b) `FakePeerPds` + `fixtures_peer` bodies are materialized in `crates/test-support/src/`, AND (c) the `cli` verb dispatch wires through the new verbs (even with `todo!()` bodies). At that point every slice-03 acceptance test MUST classify as RED (panic at `todo!()`), not BROKEN (import error, missing trait method, missing fixture). | Same logic as slice-01 DD-2: the source tree changes shape under DELIVER's hand before the suite can compile; the gate runs at the first moment the suite compiles. The gate is still HARD — any scenario in BROKEN state at that moment blocks the start of the outside-in TDD loop. | LOCKED |
| DD-FED-14 | Nested-directory placement (option B from task brief — `tests/acceptance/openlore_federated_read/{peer_subscribe,peer_pull,counter_claim,federated_query}.rs` with per-feature `support/`) was CONSIDERED and REJECTED in favor of the flat layout (DD-FED-4). Rationale: (a) Rust's integration-test discovery does NOT recurse into subdirectories by default — each `tests/<file>.rs` is a separate test binary; nested paths require a `mod.rs` aggregator file that defeats the per-file `cargo test --test` filtering. (b) Per-feature `support/` would duplicate ~700 lines of subprocess + TestEnv plumbing. (c) Cross-file chained narratives (CC-1 in `counter_claim.rs` → FQ-5 in `federated_query.rs`) work transparently with shared support; nested support directories complicate the import paths. | If a future feature ships dozens of slice-specific test files such that the flat root directory becomes cluttered, revisit at that time — slice-03's 5 new files keep the count manageable (5 slice-01 + 5 slice-03 = 10 test files at the top level). | LOCKED — revisit if file count exceeds ~20 |

---

## Inheritance from slice-01 DISTILL (still binding)

| Slice-01 DD | Status in slice-03 |
|---|---|
| DD-1 (Rust `#[test]` framework, no `.feature`) | Inherited verbatim (see DD-FED-1) |
| DD-2 (fail-for-right-reason gate deferred until `Cargo.toml` exists) | Inherited and re-scoped (see DD-FED-13) |
| DD-3 (state-delta + Universe lazy bootstrap) | Inherited; slice-01 already bootstrapped the Rust port at `tests/common/state_delta.rs` — slice-03 just consumes it. See DD-FED-10 |
| DD-4 (Tier B not added) | Re-evaluated, same conclusion for slice-03 (see DD-FED-9) |
| DD-5 (subprocess invocation = driving-adapter coverage) | Inherited verbatim |
| DD-6 (PDS + identity fake doubles in `test-support`) | EXTENDED — slice-03 adds `FakePeerPds` as a NEW peer-side double (DD-FED-2) |
| DD-7 (test directory = `tests/acceptance/` flat) | Inherited verbatim (DD-FED-4) |
| DD-8 (error-path ratio < 40%; infra-failure deferred to adapter tests) | Slice-03 brings the aggregate up (31.4% across slice-03 files alone; PP file is 62.5%); same deferral logic for infra-failure scenarios at the WS layer |
| DD-9 (DISTILL flag resolutions) | Slice-03 DISTILL flags RESOLVED by DESIGN WD-38..WD-44 (see acceptance-tests.md §2) |
| DD-10 (Rust polyglot matrix entry) | Inherited verbatim |
| DD-11 (Project Infrastructure Policy file deferral) | Continued (see DD-FED-11) |
| DD-12 (lexicon conformance is its own file, layer-2 + proptest) | Symmetric — slice-03 has its own lexicon file `lexicon_counter_claim.rs` with 2 `@property` tests (see DD-FED-8 + DD-FED-12) |
| DD-13 (WS scenario count = 17) | Slice-03 has NO new walking skeleton — slice-01's WS is the umbrella walking skeleton; slice-03 reuses the established e2e path and adds 35 targeted scenarios |

---

## Open questions handed to DELIVER (slice-03)

These are deliberately deferred to the DELIVER wave:

1. **State-delta universe naming**: which port-exposed names go into the
   universe for PS-6 hard-purge (which observes state-delta across the
   `peer_subscriptions` row, the `peer_claims` row count by author, the
   `author_claims` row count — which MUST be unchanged — and the
   filesystem `peer_claims/<did>/` directory). DD-FED-10 names the
   helpers; DELIVER fills in the explicit `universe = {...}` set when
   migrating PS-6 to `assert_state_delta`.

2. **`FakePeerPds::serve_http` runtime model**: same `ManuallyDrop +
   background-thread shutdown` pattern as the slice-01 `FakePds` (per
   `tests/acceptance/support/mod.rs` lines 234-247 + `crates/test-support/
   src/fake_pds.rs` Drop impl)? Recommended: yes — proven on macOS APFS;
   identical RAII semantics.

3. **Pinning RFC3339 timestamps for `subscribed_at` / `fetched_at` /
   `composed_at` across slice-03 scenarios**: slice-01 introduced
   `OPENLORE_TEST_NOW` as the test-only clock-pin env var. Slice-03
   scenarios involving counter-claim authoring (CC-1, CC-5) MUST pin the
   clock so the counter-claim CID is reproducible across runs. DELIVER
   reuses the slice-01 helper `run_openlore_claim_add_with_pinned_now`
   for the counter-claim verb (extending the binary names if necessary).

4. **Fixture peer DID encoding for filesystem path**: per ADR-014 +
   Q-DELIVER-2 of DESIGN's wave-decisions, the DID-to-fs-path encoding
   replaces `:` with `_`. The `peer_claims/<peer_did>/<cid>.json` layout
   under `tempfile::TempDir` MUST honor this encoding consistently in
   both production code and the test-support assertion helper
   `assert_peer_claims_dir_removed_for(did)`. DELIVER picks the helper
   shape.

5. **Proptest seed pinning for layer-2 lexicon properties**: LCC-3 +
   LCC-4 use proptest per slice-01 precedent (`proptest.toml` in repo
   root). DELIVER extends the existing seed pin to cover the NFC
   normalization properties — same shape as LC-3.

---

## Out of scope for this DISTILL (explicit deferrals)

- **Query-time re-verification of cached peer claim signatures** —
  RESOLVED by WD-38; deferred to slice-04 `peer verify --all` verb. No
  slice-03 scenario asserts query-time re-verification.

- **`peer audit <did>` verb** — RESOLVED by OD-FED-3; deferred to
  slice-04. No slice-03 scenario asserts the audit-verb output.

- **`--yes` flag on `peer remove --purge`** — RESOLVED by WD-21 + WD-36;
  deferred to slice-04 if scripting need surfaces. Slice-03 asserts
  `--no-tty --purge` REFUSES to run (PS-8).

- **Auto-pull on subscribe / push notifications / background daemon** —
  RESOLVED by WD-18 + ADR-016 LOCKED REJECTED. Slice-03 asserts
  pull-on-demand only (PP-8 + ADR-013 §Earned Trust #4 mapping).

- **Peer-pull rate limits + per-peer `--peer <did>` filter + `--since
  <ts>` filter on `peer pull`** — deferred to slice-04 per ADR-016.

- **Auto-notification of retraction or counter-claim to target peer** —
  RESOLVED by WD-44 LOCKED REJECTED. CC-6 explicitly asserts NO network
  call is made to the peer's PDS at counter-claim publish time.

- **Per-OS infrastructure-failure scenarios (disk full, fsync lies, etc.)**
  — continues slice-01 DD-8 deferral to DELIVER's adapter-level
  integration tests.

- **Pact contract tests for peer-read paths + adversarial CI fixture for
  KPI-FED-6 real-HTTP variant** — continues DD-FED-6; DEVOPS's
  deliverable per DESIGN §6.4 + outcome-kpis.md.

- **Bootstrapping `docs/architecture/atdd-infrastructure-policy.md`** —
  continues slice-01 DD-11 + DD-FED-11 deferral.

---

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (`@nw-functional-software-crafter`) | `acceptance-tests.md`; `traceability.md`; the 5 slice-03 test skeletons in `tests/acceptance/`; this file; the open-questions list above; DESIGN's `component-boundaries.md` for the new `PeerStoragePort` trait surface; DESIGN's `data-models.md` for the schema v3 migration + on-disk partition tree | First step (step-06-01) bootstraps: (a) `crates/ports/src/lib.rs` extended with `PeerStoragePort` + `PeerInfo` + extended `PdsPort` + extended `IdentityPort` traits; (b) `crates/adapter-duckdb/` schema v3 + `DuckDbPeerStorageAdapter` stubs; (c) `crates/adapter-atproto-{did,pds}/` extended adapter stubs; (d) `crates/cli/` verb dispatch for the 4 new verbs + `--federated` flag (bodies `todo!()`); (e) `crates/test-support/src/{fake_peer_pds,fixtures_peer}.rs` HTTP server + adversarial fixture bodies. After step-06-01 all 35 slice-03 acceptance tests classify as RED — DD-FED-13 fail-for-right-reason gate runs. Then one-at-a-time scenario implementation per outside-in TDD. |

---

## Changelog

- 2026-05-27 — Quinn — initial DISTILL-wave decisions for slice-03. All
  decisions DD-FED-1..DD-FED-14 LOCKED. Reconciliation against
  DISCUSS WD-14..WD-25 + DESIGN WD-26..WD-45 passed with 0
  contradictions. DESIGN WD-38..WD-44 resolutions of the 7
  `# DISTILL: confirm` flags inherited verbatim.
