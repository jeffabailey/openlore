# Wave Decisions — DISTILL — openlore-github-scraper (slice-02)

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-github-scraper (slice-02)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (per ADR-007)
- **Inherits**: DISCUSS WD-46..WD-58 + DESIGN WD-59..WD-68 + ADR-017..019;
  slice-01 DISTILL DD-1..DD-13 + slice-03 DISTILL DD-FED-1..DD-FED-14 also
  apply where the slice-02 surface is symmetric

This file records DISTILL-wave decisions (DD-SCR-N prefix to keep the
namespace distinct from slice-01 DD-N and slice-03 DD-FED-N). Decisions
that point at a test artifact (a file under `tests/acceptance/` or
`crates/test-support/src/`) are binding for DELIVER unless re-opened.

---

## Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** between DISCUSS WD-46..WD-58
(+ OD-SCR-1..4 accepted at default) and DESIGN WD-59..WD-68. Checked each
DISCUSS lock against DESIGN:

| DISCUSS lock | DESIGN | Verdict |
|---|---|---|
| WD-50 verb = sugar `scrape github [--sign]` | WD-60 LOCKED sugar verb (ADR-017) | consistent — DESIGN confirms the default |
| WD-54 optional PAT (env OR config — DESIGN's call) | WD-63 LOCKED env-var `GITHUB_TOKEN` only | NARROWS the explicitly-deferred choice; not a contradiction |
| WD-58 provenance (display OR signed — DESIGN's call) | WD-62 LOCKED display-only (ADR-018) | RESOLVES the explicitly-deferred choice; not a contradiction |
| WD-56 pure/effect split | WD-59 (two crates) + WD-65 (check-arch) | consistent |
| WD-57 two new crates | WD-59 LOCKED count 8->10 | consistent |
| WD-49 human-gate / WD-55 nothing persisted unsigned | WD-66 single bridge `CandidatePrefill`; I-SCR-1 | consistent — DESIGN strengthens with architecture-layer enforcement |
| WD-51 public-data-only | WD-61 new `GithubPort` + probe step 2; I-SCR-2 | consistent |
| WD-52 confidence 0.25 numeric, no inflate | (inherited WD-10) + I-SCR-3 | consistent |
| WD-53 mapping SSOT | WD-67 embedded from jobs.yaml + `mapping_matches_ssot`; I-SCR-5 | consistent — DESIGN picks embed-at-build mechanism |

**No `# DISTILL: confirm` flags exist.** The DISCUSS feature-delta flagged
`gherkin-scenarios-expanded.md` (anxiety/habit scenarios with potential
`# DISTILL: confirm` markers) as "to be produced alongside DISTILL handoff";
that file was NOT materialized, and DESIGN's WD-60/62/63 + Q-DELIVER-5
already resolve every open behavior it would have flagged (verb shape,
provenance storage, PAT surface, batch-skip gesture). DISTILL therefore has
zero unresolved ambiguity at scenario-write time. The anxiety force
(surveillance fear, assertion fear, over-confidence fear) is covered by the
sad-path + human-gate scenarios directly (SG-4/5/6, SC-3/5, SS-2/3/4,
SA-4/5).

DEVOPS missing (`docs/feature/openlore-github-scraper/devops/` is an empty
directory) → applied Graceful Degradation matrix WARN + default environment
matrix (clean | with-pre-commit | with-stale-config); no slice-02 acceptance
scenario depends on a per-environment fixture cross-product (the test seam is
subprocess + `FakeGithub` + `FakePds`/`FakeIdentity` + tempfile HOME).

---

## Locked decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| DD-SCR-1 | Slice-02 acceptance tests inherit the slice-01/03 framework: Rust std `#[test]` with snake_case function names encoding the scenario (`scrape_github_*`, `scrape_candidates_*`, `scrape_sign_*`, `scrape_auth_*`, `scraper_domain_*`). No new test-framework dependency; no `.feature` files. | Symmetric with slice-01 DD-1 + slice-03 DD-FED-1; preserves `cargo test --test <file>` ergonomics; one place for the test inventory per file. | LOCKED |
| DD-SCR-2 | The GitHub test double is a NEW crate-internal module `openlore_test_support::fake_github::FakeGithub`, DISTINCT from `FakePds` AND `FakePeerPds`. GitHub is HONESTLY a wholly different actor (public REST/GraphQL; no ATProto method shape, auth model, rate-limit semantic, or failure surface in common — WD-61 / ADR-019). | Folding GitHub harvest into `FakePds`/`FakePeerPds` would conflate two unrelated trust boundaries (slice-03 peer reads genuinely WERE ATProto XRPC, so `PdsPort` extension was right there; GitHub shares nothing). A separate type keeps the boundary honest and gives the double its own posture vocabulary (`for_public_repo`, `for_private_target`, `rate_limited_anon`, …). | LOCKED |
| DD-SCR-3 | Adversarial / degradation postures are constructor-style on `FakeGithub` (`for_not_found`, `for_private_target`, `offline`, `rate_limited_anon`, `with_rejected_token`, `with_no_matching_signals`, `with_multi_signal_single_predicate`) rather than mutating methods on an already-constructed fake. State is fixed at construction; the SUT sees a deterministic posture for the scenario lifetime. | Construction-time posture pinning prevents "did the test arm the rate-limit before or after resolve?" race-condition test-bugs. Same pattern as slice-03 DD-FED-3 (`FakePeerPds::with_tampered_signature`) and slice-01's `FakePds::for_did`. | LOCKED |
| DD-SCR-4 | The DISTILL test placement is the FLAT layout under `tests/acceptance/` matching slice-01/03. Files: `scrape_github.rs`, `scrape_candidates.rs`, `scrape_sign.rs`, `scrape_auth.rs`, `scraper_domain.rs`. NOT nested under `tests/acceptance/openlore_github_scraper/`. | Preserves `cargo test --test <file>` ergonomics; five new files clearly labeled by domain. Rust integration-test discovery does not recurse subdirectories; a nested layout needs a `main.rs` aggregator that defeats per-file filtering OR duplicates the shared `support/`. Same rationale as slice-03 DD-FED-4/DD-FED-14. Total top-level test files: 5 (slice-01) + 5 (slice-03) + 5 (slice-02) = 15 — within the ~20 revisit threshold. | LOCKED |
| DD-SCR-5 | The shared `tests/acceptance/support/mod.rs` is EXTENDED, not duplicated. New scrape runners (`run_openlore_scrape`, `run_openlore_scrape_with_stdin`, `run_openlore_scrape_with_token`) + assertion helpers (`assert_no_claim_persisted`, `assert_candidate_names_signal`, `assert_candidate_confidence`, `assert_candidate_confidence_unchanged`, `assert_scraper_reuses_slice01_publish_path`, `assert_only_public_endpoints_called`, `assert_token_value_absent`) land in `support/mod.rs` as `todo!()` signatures; DELIVER materializes the bodies. | One source of truth for `TestEnv`, subprocess plumbing, and assertion helpers across all acceptance tests. Symmetric with slice-03 DD-FED-5. The sign-path scenarios (SS-*) reuse the slice-01 `assert_compose_preview_contains_not_as_truth` + `assert_no_pds_call_was_made` + `assert_pds_contains_record_at` helpers UNCHANGED. | LOCKED |
| DD-SCR-6 | Layered placement (Mandate 9): the `scrape_*` subprocess files are layer 3/5 — example-only (Mandate 11), each example a real `openlore` invocation. The `scraper_domain.rs` file is layer 2 (pure-core direct invocation) — PBT full per Mandate 9, with `@property` tags on the auditability (SD-1), no-inflation (SD-2), and determinism (SD-3) invariants. ZERO proptest at layer 3+. | Layered test discipline: the auditability + no-inflation + determinism invariants are pure-data properties best expressed generatively at the cheap layer-2 boundary; the verb-orchestration behavior (banner, harvest, render, sign, publish) is example-pinned at layer 3 because each example is a real subprocess with real I/O. Symmetric with slice-03 DD-FED-12. | LOCKED |
| DD-SCR-7 | Pure-core unit tests for `scraper_domain::derive_candidates` + `load_mapping` (exhaustive per-`SignalKind` arms, malformed-mapping errors, boundary parsing) are DELIVER's responsibility (inner TDD loop), NOT DISTILL's. The slice-02 layer-2 `scraper_domain.rs` exercises the LOAD-BEARING invariants at the in-memory acceptance layer (SD-1..SD-6) but the exhaustive coverage lives in `crates/scraper-domain/src/`'s `#[cfg(test)] mod tests` block. | Symmetric with slice-01 + slice-03 DD-FED-7. Pure functions live in DELIVER's inner TDD loop; the outer-loop acceptance suite asserts the contract via the auditability/no-inflation/determinism properties + mapping-SSOT conformance, not via exhaustive unit coverage. `scraper-domain` is the mutation-test target of the slice (component-boundaries §`crates/scraper-domain`). | LOCKED |
| DD-SCR-8 | Layer-2 scraper-domain scenarios (SD-1..SD-6) live in their own file `scraper_domain.rs` rather than being appended to slice-01's `lexicon_conformance.rs` or slice-03's `lexicon_counter_claim.rs`. Slice-02's pure-core concerns are "candidate derivation + signal->predicate mapping SSOT", a focused surface distinct from the lexicon files. | Keeps the inherited lexicon files pristine (locked + committed). The `@property` proptest harness for `derive_candidates` is the most distinctive shape in this file and warrants its own test-binary boundary (faster `cargo test --test scraper_domain`). Symmetric with slice-03 DD-FED-8. | LOCKED |
| DD-SCR-9 | Tier B (state-machine PBT) is NOT added for slice-02 per Mandate 10 evaluation. The scrape->propose->sign journey is 3-4 chained scenarios (qualifying on chain length) BUT the input space is bounded: a fixed 5-entry signal->predicate mapping + a small set of harvested signals + a candidate index list. The auditability + no-inflation invariants ARE cross-rule properties, but SD-1/SD-2/SD-3 assert them generatively at layer 2 (`@property`), which is the cheaper instrument than a full `RuleBasedStateMachine` over a small fixed state space. Re-evaluate at slice-04 (scoring-graph) where cross-repo triangulation + confidence weighting genuinely expand the input space. | Symmetric with slice-01 DD-4 + slice-03 DD-FED-9. Tier B costs the in-memory composition root + the state-machine model + the InMemoryComposition wiring; the cost-benefit only swings positive when the state space is too large for examples + layer-2 properties to cover. Slice-02's pure derivation is a stateless function (no journey state machine to model); the `@property` layer-2 tests are the right instrument. | LOCKED — revisit at slice-04 |
| DD-SCR-10 | State-delta + Universe assertions (Mandate 8) at layer 3 (subprocess acceptance) are written via named assertion-helper functions in `support/mod.rs` (e.g. `assert_no_claim_persisted`, `assert_candidate_confidence_unchanged`), NOT via `assert_state_delta(before, after, universe, expected)` directly. The Rust `state_delta` port at `tests/common/state_delta.rs` was bootstrapped by slice-01; slice-02 INHERITS it (no re-bootstrap). DELIVER MUST migrate the load-bearing human-gate scenario (SG-1 `assert_no_claim_persisted`) to explicit `assert_state_delta` form once the helper bodies are real — modeled on slice-03's `assert_purge_state_delta`. | Two-stage bootstrap symmetric with slice-01 DD-3 + slice-03 DD-FED-10: DISTILL declares the contract via named helper signatures; DELIVER materializes the universe wiring as each scenario goes green. The universe entries MUST be port-exposed names (`author_claims.row_count`, `pds.records.len`, `claims_dir.artifact_count`, `claims/<cid>.json::confidence`, `github.seen_paths`) — NEVER internal struct fields per Mandate 8. | LOCKED |
| DD-SCR-11 | The Project Infrastructure Policy file at `docs/architecture/atdd-infrastructure-policy.md` is STILL NOT written by this DISTILL wave. The orchestrator brief limits writes to `docs/feature/openlore-github-scraper/distill/` + `tests/acceptance/` + `crates/test-support/src/`. The slice-02 additions to the inherited inline policy are documented in `acceptance-tests.md §11`; slice-01 + slice-03 + slice-02 policy entries should all land at the project-local file on a future wave whose orchestrator scope permits. | Continues slice-01 DD-11 + slice-03 DD-FED-11 deferral; cross-wave write-surface convention unchanged. | LOCKED |
| DD-SCR-12 | `FakeGithub` enforces public-data-only + human-gate + no-token-leak STRUCTURALLY, not by convention: (a) no constructor exposes a private surface, so `scraper_only_reads_public_data` is enforced by the double's shape + a `seen_paths` allowlist assertion; (b) the double holds NO storage/identity/pds reference, so it CANNOT sign or publish (mirrors production `adapter-github`); (c) the token value lives only in the fake's auth state, observable via `saw_token(token) -> bool` but never echoed in `Debug` or output, so `assert_token_value_absent` + `saw_token` together prove auth happened WITHOUT leaking the value. | Structural enforcement is stronger than assertion-only: a test double that has no private surface cannot accidentally serve private data; a double with no signing key cannot accidentally sign. Mirrors slice-03 DD-FED-2's "peer PDS exposes no write endpoint at all (any write 405s)" structural guarantee. | LOCKED |
| DD-SCR-13 | Pre-DELIVER fail-for-right-reason gate (slice-02) runs in DELIVER's first slice-02 step (step-07-01), AFTER (a) the `GithubPort` trait + `TargetKind` + `GithubError` + slice-02 `ProbeRefusalReason` variants are scaffolded in `crates/ports/`, AND (b) the `scraper-domain` crate's `Signal` + `CandidateClaim` + `SignalPredicateMapping` ADTs + `derive_candidates` + `load_mapping` + `proptest_strategies` surface are scaffolded, AND (c) `FakeGithub` + `fixtures_github` bodies are materialized in `crates/test-support/src/`, AND (d) the `cli` verb dispatch wires `scrape github [--sign]` (even with `todo!()` bodies). At that point every slice-02 acceptance test MUST classify as RED (panic at `todo!()`), not BROKEN (import error, missing trait method, missing fixture). | Same logic as slice-01 DD-2 + slice-03 DD-FED-13: the source tree changes shape under DELIVER's hand before the suite can compile; the gate runs at the first moment the suite compiles. The gate is still HARD — any scenario in BROKEN state at that moment blocks the start of the outside-in TDD loop. | LOCKED |
| DD-SCR-14 | The new `[[test]]` targets (`scrape_github`, `scrape_candidates`, `scrape_sign`, `scrape_auth`, `scraper_domain`) are NOT registered in `crates/cli/Cargo.toml` by THIS DISTILL wave. DELIVER's per-file first step registers each target (one `[[test]]` block per file), mirroring the slice-03 precedent where the orchestrator brief reserved Cargo.toml registration for DELIVER. | Slice-03 DD precedent: the orchestrator commits; DISTILL writes the scaffold `.rs` + test-support but leaves the `[[test]]` registration to DELIVER's per-file bootstrap so that registration + first-unskip happen atomically per file. Keeps the DISTILL diff to docs + scaffolds + test-support only. | LOCKED |

---

## Inheritance from slice-01 + slice-03 DISTILL (still binding)

| Slice-01/03 DD | Status in slice-02 |
|---|---|
| DD-1 / DD-FED-1 (Rust `#[test]` framework, no `.feature`) | Inherited verbatim (see DD-SCR-1) |
| DD-2 / DD-FED-13 (fail-for-right-reason gate deferred until DELIVER scaffolds the new surface) | Inherited and re-scoped (see DD-SCR-13) |
| DD-3 / DD-FED-10 (state-delta + Universe lazy bootstrap) | Inherited; slice-01 already bootstrapped the Rust port at `tests/common/state_delta.rs` — slice-02 just consumes it. See DD-SCR-10 |
| DD-4 / DD-FED-9 (Tier B not added) | Re-evaluated, same conclusion for slice-02 (see DD-SCR-9) |
| DD-5 (subprocess invocation = driving-adapter coverage) | Inherited verbatim |
| DD-6 / DD-FED-2 (test doubles in `test-support`) | EXTENDED — slice-02 adds `FakeGithub` as a NEW external-system double (DD-SCR-2) |
| DD-7 / DD-FED-4 (test directory = `tests/acceptance/` flat) | Inherited verbatim (DD-SCR-4) |
| DD-8 (error-path ratio; infra-failure deferred to adapter tests) | Slice-02 error-path ratio is healthy (see acceptance-tests.md §4); per-OS infra-failure (disk full) continues deferral to DELIVER adapter tests |
| DD-9 / DD-FED-9 (DISTILL flag resolutions) | Slice-02 has NO `# DISTILL: confirm` flags — all open behaviors resolved by DESIGN WD-60/62/63 + Q-DELIVER-5 (see Reconciliation result) |
| DD-10 (Rust polyglot matrix entry) | Inherited verbatim |
| DD-11 / DD-FED-11 (Project Infrastructure Policy file deferral) | Continued (see DD-SCR-11) |
| DD-12 / DD-FED-8 / DD-FED-12 (pure-core file is its own file, layer-2 + proptest) | Symmetric — slice-02 has its own `scraper_domain.rs` with 3 `@property` tests (see DD-SCR-8 + DD-SCR-6) |
| DD-13 / DD-FED-13 (WS scenario count) | Slice-02 HAS a new walking skeleton (SG-1 scrape->propose + SS-1 sign half) — slice-02 IS the walking-skeleton feature for the scraper slice (WD-46); it does not reuse slice-01's WS like slice-03 did |

---

## Open questions handed to DELIVER (slice-02)

These are deliberately deferred to the DELIVER wave (consistent with the
DESIGN Q-DELIVER-1..7 deferrals):

1. **State-delta universe naming**: which port-exposed names go into the
   universe for SG-1's `assert_no_claim_persisted` migration to
   `assert_state_delta` (DD-SCR-10). The proposed universe is
   `{author_claims.row_count, pds.records.len, claims_dir.artifact_count}`
   all `set_to("0")` — DELIVER fills in the explicit `universe = {...}` set
   modeled on slice-03's `assert_purge_state_delta`.

2. **`FakeGithub::serve_http` runtime model**: same `tokio::spawn +
   AbortOnDrop` background-thread shutdown pattern as `FakePds` /
   `FakePeerPds`? Recommended: yes — proven on macOS APFS; identical RAII
   semantics. The base-URL injection seam is `OPENLORE_GITHUB_API_BASE`
   (mirrors the slice-03 `OPENLORE_PEER_PDS_ENDPOINT_<did>` seam).

3. **GitHub API base + token env seams**: DESIGN WD-63 fixes `GITHUB_TOKEN`
   as the PAT seam. DISTILL adds `OPENLORE_GITHUB_API_BASE` as the test-only
   base-URL seam so `adapter-github` resolves against `FakeGithub` instead of
   `api.github.com`. DELIVER wires both in `cli::Wiring`.

4. **`derive_candidates` proptest strategy**: SD-1/SD-2/SD-3 reference
   `scraper_domain::proptest_strategies::arb_signal_set()`. DELIVER defines
   the generator (domain-realistic: signals whose kinds span the 5 mapping
   entries + signals whose kinds match NO entry, so the negative arm is
   non-vacuous per Hebert ch.6 negative testing). Reuse the slice-01/03
   `proptest.toml` seed-pin convention.

5. **Skip gesture for batch sign (SS-8)**: DESIGN Q-DELIVER-5 leaves the
   exact gesture (Ctrl-C-per-candidate vs explicit "skip" input) to DELIVER.
   SS-8 asserts the BEHAVIOR ("skip one without aborting the rest; summary
   reports signed/skipped"), not the keystroke. DELIVER picks the gesture and
   wires `run_openlore_scrape_with_stdin` to drive it.

6. **REST vs GraphQL per signal (Q-DELIVER-2)** + **`harvest_user` page cap
   (Q-DELIVER-4)** + **YAML parser pin (Q-DELIVER-1)**: all DESIGN-deferred
   crafter calls; the acceptance tests assert OBSERVABLE behavior (signal
   count, candidate render, bounded aggregate), not the transport or the
   exact cap, so any DELIVER choice that satisfies the asserted lines is
   valid.

7. **SG-3 user-target aggregate shape**: the `fixture_torvalds_user_aggregate_signals`
   fixture supplies a bounded aggregate; DELIVER decides the exact signal
   count + page cap (Q-DELIVER-4) as long as SG-3's "resolves as user +
   bounded aggregate count reported" contract holds.

---

## Out of scope for this DISTILL (explicit deferrals)

- **Config-file PAT support** — RESOLVED by WD-63 env-var-only; deferred to
  a later slice. No slice-02 scenario asserts config-file token loading.
- **Deep cross-repo triangulation for user targets** — RESOLVED by WD-64
  bounded-aggregate; deferred to slice-04 (scoring-graph). SG-3 asserts only
  the bounded aggregate.
- **`derived-from` as a signed-payload field** — RESOLVED by WD-62 / ADR-018
  display-only. SS-3 explicitly asserts the provenance is NOT in the signed
  payload and the CID is unchanged. No Lexicon change ships.
- **ML-based philosophy inference** — out of scope indefinitely (WD-53); the
  mapping is small + auditable. No scenario asserts inference.
- **Non-GitHub sources / scheduled scraping / daemon / auto-publish** —
  intentional non-goals per story-map "What is NOT in scope". No scenario.
- **GitHub public-endpoint allowlist Pact contract test (real-HTTP variant)**
  — DEVOPS's deliverable per outcome-kpis.md DEVOPS handoff + DESIGN §Annotation
  for platform-architect. SG-5 asserts the allowlist via `FakeGithub::seen_paths`
  in-process; the real-HTTP Pact contract is DEVOPS's, not DISTILL's.
- **KPI-SCR-1 latency assertion (`scrape.to_sign.duration_seconds`)** —
  collected via tracing telemetry in production (outcome-kpis.md measurement
  plan), NOT asserted at the acceptance boundary. SS-1 proves the path exists.
- **KPI-SCR-5 edit-rate** — author-side telemetry (candidate-vs-signed diff),
  a DEVOPS/PO measurement, not an acceptance assertion. SS-2 proves the
  zero-edit-sign byte-for-byte contract that the telemetry measures against.
- **Per-OS infrastructure-failure scenarios (disk full, fsync lies)** —
  continues slice-01 DD-8 deferral to DELIVER's adapter-level integration
  tests.
- **Bootstrapping `docs/architecture/atdd-infrastructure-policy.md`** —
  continues slice-01 DD-11 + slice-03 DD-FED-11 + DD-SCR-11 deferral.

---

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (`@nw-functional-software-crafter`) | `acceptance-tests.md`; `traceability.md`; the 5 slice-02 test skeletons in `tests/acceptance/`; this file; the open-questions list above; DESIGN's `component-boundaries.md` for the new `GithubPort` trait + `scraper-domain` ADTs; DESIGN's `data-models.md` for the candidate/signal shapes + scrape verb output format | First step (step-07-01) bootstraps: (a) `crates/ports/src/lib.rs` extended with `GithubPort` + `TargetKind` + `GithubError` + slice-02 `ProbeRefusalReason` variants; (b) `crates/scraper-domain/` crate (PURE) with `Signal` + `CandidateClaim` + `SignalPredicateMapping` + `derive_candidates` + `load_mapping` + `proptest_strategies` stubs + the embedded jobs.yaml mapping snapshot + `mapping_matches_ssot` build-time test; (c) `crates/adapter-github/` crate (EFFECT) with `AdapterGithub` impl + `probe()` + `GITHUB_TOKEN` + `OPENLORE_GITHUB_API_BASE` seams; (d) `crates/cli/` verb dispatch for `scrape github [--sign]` + `CandidateRenderer` + `CandidatePrefill` + `SelectionParser` (bodies `todo!()`); (e) `crates/test-support/src/{fake_github,fixtures_github}.rs` bodies; (f) `crates/cli/Cargo.toml` 5 new `[[test]]` targets (DD-SCR-14). After step-07-01 all slice-02 acceptance tests classify as RED — DD-SCR-13 fail-for-right-reason gate runs. Then one-at-a-time scenario implementation per outside-in TDD. |

---

## Changelog

- 2026-05-28 — Quinn — initial DISTILL-wave decisions for slice-02. All
  decisions DD-SCR-1..DD-SCR-14 LOCKED. Reconciliation against DISCUSS
  WD-46..WD-58 + DESIGN WD-59..WD-68 passed with 0 contradictions. No
  `# DISTILL: confirm` flags to resolve (all open behaviors resolved by
  DESIGN WD-60/62/63 + Q-DELIVER-5).
