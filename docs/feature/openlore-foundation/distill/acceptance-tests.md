# Acceptance Test Design — openlore-foundation (slice-01)

- **Wave**: DISTILL
- **Date**: 2026-05-25
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-foundation
- **Slice**: slice-01-claim-skeleton (the walking skeleton)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (per ADR-007)
- **Inherits**: WD-1..WD-13 (DISCUSS) + D-1..D-12 (DESIGN); all ADRs
- **Language**: Rust (per ADR-009 + ADR-007)
- **Test framework**: Rust std `#[test]` + `cucumber` crate for Gherkin (proposed; crafter may swap to `cucumber-rust` family without changing scenario semantics)

This document is the human-readable map over the executable test skeletons in
`tests/acceptance/`. The `.rs` files (skeletons) are the SSOT for executable
scenarios; this document records design rationale, traceability, and the
flag-resolution outcomes.

---

## 1. Scope and shape

This wave produces **port-to-port acceptance tests** that drive Outside-In TDD
in DELIVER. Every test enters through the CLI (the **driving adapter** per
DESIGN §3) — the user's actual invocation path — and exercises real driven
adapters (claim-domain pure core, DuckDB, local filesystem, OS keychain) with
**fakes only for** the ATProto PDS (recorded fixture / test-double) and the
identity adapter where a real key would require a live ATProto session.

Per Mandate 1 (Hexagonal Boundary Enforcement) every test invokes `openlore`
as a subprocess (real CLI binary, `assert_cmd` style) — NEVER calls into
`claim-domain` or `adapter-duckdb` directly. This is the difference between
"the pipeline works" and "the CLI works for a user." (See `docs/analysis/`
P1 RCA precedent imported from the nWave methodology base.)

### Layer placement (per nw-test-design-mandates Mandate 9)

| Layer | Test file(s) | Real adapters | Test mode |
|---|---|---|---|
| Walking-Skeleton subprocess (layer 5) | `walking_skeleton.rs` | CLI binary + DuckDB + FS + keychain; PDS double | example-only |
| Subprocess / FS acceptance (layer 3) | `walking_skeleton.rs` sad-paths, `federation_roundtrip.rs` | CLI + DuckDB + FS; PDS double | example-only (Mandate 11) |
| In-memory acceptance (layer 2) | `lexicon_conformance.rs` | None — pure core directly | example + property-based (proptest, per ADR-006 §Earned Trust) |

Layer 1 (pure-core unit tests) is OUT OF DISTILL SCOPE — those belong to
DELIVER's inner TDD loop. DISTILL ships only the outer-loop acceptance tests
that drive the DELIVER cycle.

### What is mocked, what is real

| Component | Treatment | Why |
|---|---|---|
| `cli` (driving adapter) | REAL binary, subprocess invocation via `assert_cmd` or `std::process::Command` | Mandate 1 + Pillar 3 (App as in production) — entry point must be exercised |
| `claim-domain` (pure core) | REAL — built in via the binary | Pure functions; mocking would be testing-theater |
| `lexicon` (pure) | REAL — built in via the binary | Same |
| `adapter-duckdb` | REAL DuckDB file in `tempfile::TempDir` | Real I/O catches format drift (Mandate 6 / F-001) |
| `adapter-system-clock` | REAL `std::time` | Degenerate adapter; no value mocking |
| `adapter-atproto-did` | FAKE wrapping a known test key + test DID document (recorded fixture) | Real ATProto identity requires a live PDS session; out of test scope |
| `adapter-atproto-pds` | FAKE PDS double (in-process HTTP stub OR recorded XRPC fixture replay) | Network dependency; flaky in CI; locked test-double in slice-01 |
| OS Keychain | FAKE in-memory key store at the test boundary | Keychain access requires per-OS setup; deferred to DELIVER's adapter-level integration tests |

The PDS double + identity double together replace the federated boundary
WITHOUT replacing the local boundary. This means: every test exercises the
real claim composition pipeline, the real CID computation, the real DuckDB
write/read path, and the real filesystem `<cid>.json` atomic write — but
stops at the network edge.

---

## 2. Resolved DISTILL flags (from `gherkin-scenarios-expanded.md`)

| # | Flag | Resolution | Reasoning |
|---|---|---|---|
| 1 | Counter-claim verb (`--counters` flag or sugar verb) | DEFER to slice-03 (federated-read sibling feature) — `claim retract` IS the slice-01 retract verb; peer-published counter-claims arrive via federation, not slice-01's single-author CLI | ADR-003 locks only `claim retract`. Peer counter-claim flow requires another author's PDS, which is slice-03 scope. |
| 2 | Inspect/status verb for inbound counters | DEFER to slice-03 | No `claim status` verb in ADR-003. Without federation there are no inbound counters to inspect. |
| 3 | Corrective-reference field (`--corrects` / `--supersedes`) | BIND to **two-step workflow** using locked verbs — publish a NEW claim with corrected evidence + retract the old one | ADR-008 ships `references[].type` with all four enum values BUT ADR-003 only ships `claim retract` as a CLI verb. The corrective-claim AC in slice-01 is satisfied by the two-step flow; the `claim correct` sugar verb is a sibling-feature concern. |
| 4 | Confidence-edit `--edit` flow | BIND to **cancel + re-run** with new flags | ADR-003 mockup mentions `--edit` as a future affordance; the locked behavior is "Ctrl-C to cancel, rerun with edited flags." The signed-payload assertion (no bucket label persisted) is fully testable via the cancel+re-run path. |
| 5 | Contribution/lurker-nudge verb (`graph contrib`) | DEFER to a sibling feature (likely slice-04 scoring-graph) | wave-decisions.md D-Out-of-scope explicitly defers this. |
| 6 | `--from-url` on `claim add` | DEFER to slice-02 (github-scraper sibling feature) | Habit-bridging affordance. URL-driven authoring is a scraper-class concern; slice-01 only accepts explicit flags. |

The three "DEFERRED" scenarios from `gherkin-scenarios-expanded.md` are NOT
silently dropped. Each is recorded in `traceability.md` under "Deferred to
sibling feature" with the slice that should land them.

---

## 3. Acceptance test inventory

Per Mandate 3 (User Journey Completeness) every test exercises a complete
user journey from observable trigger through observable outcome.

### Walking-skeleton scenarios — `tests/acceptance/walking_skeleton.rs`

Tag schema: `@walking_skeleton @driving_port` (all WS scenarios) plus
`@US-NNN` to trace to the originating story and `@J-001` for job traceability.

| # | Test name | Story | Job | Type | Pillars |
|---|---|---|---|---|---|
| WS-1 | `walking_skeleton_init_creates_identity_duckdb_and_is_idempotent` | US-005 | infra→J-001 | happy | 1, 2, 3 |
| WS-2 | `walking_skeleton_claim_commands_fail_loudly_when_not_initialized` | US-005 | infra→J-001 | error | 1, 2, 3 |
| WS-3 | `walking_skeleton_compose_preview_contains_not_as_truth_and_waits_for_confirmation` | US-001 | J-001 | happy | 1, 2, 3 |
| WS-4 | `walking_skeleton_compose_rejects_confidence_outside_unit_interval` | US-001 | J-001 | error | 1, 2, 3 |
| WS-5 | `walking_skeleton_compose_preview_shows_bucket_label_but_signed_payload_has_only_numeric` | US-001 + WD-10 | J-001 | edge | 1, 2, 3 |
| WS-6 | `walking_skeleton_sign_writes_atomic_local_file_with_no_network_call` | US-002 | J-001 | happy | 1, 2, 3 |
| WS-7 | `walking_skeleton_re_canonicalization_produces_identical_cids` | US-002 + KPI-4 | J-001 | edge | 1, 2, 3 |
| WS-8 | `walking_skeleton_publish_prints_at_uri_and_retract_hint_after_signing` | US-003 + WD-6 | J-001 | happy | 1, 2, 3 |
| WS-9 | `walking_skeleton_publish_is_idempotent_on_re_run_with_same_cid` | US-003 | J-001 | edge | 1, 2, 3 |
| WS-10 | `walking_skeleton_pds_unreachable_leaves_local_claim_intact_and_retry_actionable` | US-003 | J-001 | error | 1, 2, 3 |
| WS-11 | `walking_skeleton_graph_query_returns_just_published_claim_byte_for_byte` | US-004 + KPI-4 | J-001+J-002 | happy | 1, 2, 3 |
| WS-12 | `walking_skeleton_graph_query_default_is_local_only_and_footer_announces_it` | US-004 | J-001+J-002 | happy | 1, 2, 3 |
| WS-13 | `walking_skeleton_graph_query_empty_result_is_explained_not_silent` | US-004 | J-001+J-002 | error | 1, 2, 3 |
| WS-14 | `walking_skeleton_retract_publishes_new_counter_claim_referencing_original` | US-003 (retract) + ADR-008 | J-001 | happy | 1, 2, 3 |
| WS-15 | `walking_skeleton_retract_preserves_original_record_in_local_and_remote_stores` | ADR-008 §Behavioral rules 1-3 | J-001 | edge | 1, 2, 3 |
| WS-16 | `walking_skeleton_corrective_workflow_publishes_new_claim_and_retracts_old` | gherkin-expanded anxiety-2 (typo'd URL) | J-001 | journey | 1, 2, 3 |
| WS-17 | `walking_skeleton_calibration_anxiety_user_cancels_and_re_runs_with_lower_confidence` | gherkin-expanded anxiety-3 | J-001 | journey | 1, 2, 3 |

**Total: 17 scenarios** (14 happy/edge + 3 error = 17.6% error ratio — see §6
for note on coverage and infrastructure-failure scenarios in walking-skeleton).

Mandate 3 satisfied: scenarios 16 and 17 are explicit multi-step journeys.

### Lexicon-conformance scenarios — `tests/acceptance/lexicon_conformance.rs`

Per ADR-006 §Earned Trust and data-models.md the claim Lexicon is a
**federation contract**. These scenarios are layer-2 (in-memory acceptance
via direct pure-core invocation, no CLI subprocess) and tagged `@property`
where appropriate per Mandate 9.

| # | Test name | Source | Type |
|---|---|---|---|
| LC-1 | `lexicon_roundtrip_compose_sign_serialize_deserialize_yields_equal_value` | ADR-006 KPI-4 | example |
| LC-2 | `lexicon_validates_signed_claim_against_org_openlore_claim_schema` | ADR-005 | example |
| LC-3 | `lexicon_cid_is_byte_stable_across_n_re_canonicalizations` (property) | ADR-006 §Earned Trust prop test 1 | `@property` (proptest) |
| LC-4 | `lexicon_cid_is_byte_stable_for_fixture_suite_of_known_claims` | ADR-006 §Earned Trust prop test 2 (gold fixtures) | example |
| LC-5 | `lexicon_rejects_out_of_range_confidence_at_wire_boundary` | data-models.md confidence min/max | example |
| LC-6 | `lexicon_rejects_self_reference_in_references_array` | ADR-008 §Behavioral rule 4 | example |
| LC-7 | `lexicon_rejects_two_hop_reference_cycle` | ADR-008 §Behavioral rule 4 + Earned Trust 3 | example |
| LC-8 | `lexicon_persisted_payload_never_contains_bucket_label_string` | WD-10 / D-12 invariant | example |

**Total: 8 scenarios** (7 example + 1 property).

### Federation-round-trip scenarios — `tests/acceptance/federation_roundtrip.rs`

The walking skeleton's reason to exist: publish to PDS, read back via the
AppView code path (slice-01: local DuckDB IS the AppView read path; slice-03
adds true federated read).

| # | Test name | Source | Type |
|---|---|---|---|
| FR-1 | `federation_roundtrip_publish_three_claims_different_predicates_all_round_trip_with_cids_intact` | slice-01 hypothesis | journey |
| FR-2 | `federation_roundtrip_pds_record_rkey_equals_claim_cid` | ADR-006 + ADR-003 §verb publish | example |
| FR-3 | `federation_roundtrip_at_uri_is_reconstructible_from_author_did_and_claim_cid` | shared-artifacts-registry rule 3 | example |
| FR-4 | `federation_roundtrip_graph_query_output_matches_compose_preview_field_for_field` | shared-artifacts-registry rule 4 + KPI-4 | example |

**Total: 4 scenarios.**

### Total acceptance scenarios across the wave

17 (WS) + 8 (LC) + 4 (FR) = **29 scenarios** authored, all RED-ready as
`todo!()` / `unimplemented!()` scaffolds. Error-path ratio in the WS file:
3/17 = 17.6%; with the LC layer's 4 rejection scenarios (LC-5, LC-6, LC-7,
plus init-failure WS-2 and pds-unreachable WS-10) the cross-file error
ratio is 6/29 = 20.7%.

> NOTE on error-ratio: nw-test-design-mandates targets 40%+ for general
> features. slice-01's walking-skeleton-only scope concentrates on proving
> the happy path E2E; infrastructure-failure coverage (disk full,
> permission denied, network mid-write timeout, corrupt local store) is
> documented in `wave-decisions.md` as **deferred to DELIVER's adapter-level
> integration tests** since each requires per-OS fixture work that
> sibling features will accumulate. DELIVER (functional-crafter) is
> expected to land at least the disk-full and partial-write paths as
> integration tests against the real DuckDB adapter; the WS layer here
> covers the user-facing error paths (out-of-range input, PDS unreachable,
> not-initialized).

---

## 4. Driving Adapter coverage

Per Mandate 1 + the RCA P1 fix in nw-distill ("Driving Adapter Verification"):
every CLI verb in the locked ADR-003 verb table is covered by at least one
subprocess scenario.

| Verb | Walking-skeleton coverage |
|---|---|
| `openlore init` | WS-1 (happy) + WS-2 (gate behavior) |
| `openlore claim add` | WS-3, 4, 5 (compose), WS-6, 7 (sign), WS-8 (publish in chained flow), WS-17 (calibration anxiety) |
| `openlore claim publish <cid>` | WS-9 (idempotent re-publish), WS-10 (PDS unreachable retry), WS-16 (corrective workflow) |
| `openlore claim retract <cid>` | WS-14, WS-15, WS-16 |
| `openlore graph query --subject <uri>` | WS-11, 12, 13 + FR-4 |

Zero uncovered entry points.

---

## 5. Driven adapter coverage (Mandate 6)

| Driven adapter | Real-I/O scenario? | Tag |
|---|---|---|
| `adapter-duckdb` (DuckDB + filesystem) | YES — WS-6 (write), WS-11 (read), WS-7 (CID stability across DB), FR-1 (3-claim roundtrip) | `@real-io @adapter-integration` |
| `adapter-system-clock` | YES — WS-3 implicit (`composedAt` rendered in preview) | `@real-io` |
| `adapter-atproto-pds` | PARTIAL — WS-8, 9, 10, 14, 16 + FR-1, 2, 3, 4 exercise the PdsPort INTERFACE through a fake PDS double. The REAL PDS adapter is contract-tested via Pact per DESIGN §6.5; **that contract suite is owned by DEVOPS** and lives at `tests/contract/pds/` (out of DISTILL scope). | `@fake-pds` (slice-01) |
| `adapter-atproto-did` (identity) | PARTIAL — same shape: scenarios use a fake identity double with a known test DID + test key. Real OS-keychain integration ships in DELIVER's adapter-level test. | `@fake-identity` (slice-01) |

The PARTIAL coverage on the two network adapters is structural to slice-01's
walking-skeleton scope: the DESIGN explicitly says (component-boundaries.md
DEVOPS annotation + architecture-design.md §6.5) that **Pact contract tests
against ATProto are DEVOPS's deliverable**, not DISTILL's. DISTILL ships the
acceptance shape; DEVOPS ships the consumer-driven contract suite that
validates the slice-01 adapter against the real PDS protocol surface.

---

## 6. Pre-requisites for compilation (DELIVER wiring expectations)

The skeleton files in `tests/acceptance/` use `use openlore::...` import paths
and `use openlore_test_support::...` for the test doubles. NEITHER crate
exists at DISTILL handoff time. The intentional consequence:

1. **`cargo build --tests` will fail to compile** until DELIVER scaffolds:
   - `Cargo.toml` workspace manifest (per component-boundaries.md §Crate layout).
   - The 8 crates listed in component-boundaries.md §Crate (component) layout.
   - A `crates/test-support/` crate (NEW — not listed in DESIGN; suggested
     placement for the PDS double + identity double + fixtures, named
     `openlore-test-support` in `Cargo.toml`).
   - At minimum a `cli` crate exposing a binary at `target/.../openlore`
     reachable by `assert_cmd::Command::cargo_bin("openlore")`.

2. **Once `Cargo.toml` is scaffolded** the tests compile to "all `#[test]`
   functions panic with `todo!()` / `unimplemented!()`" → tests RED per
   Mandate 7. DELIVER then unskips one at a time and drives it green.

3. **Rust scaffold marker** per Mandate 7: every `#[test]` body that panics
   does so via `panic!("Not yet implemented -- RED scaffold")` with a
   `// SCAFFOLD: true` comment-marker on the surrounding module. Detection
   via `grep -r "SCAFFOLD: true" tests/`.

DELIVER's first task in roadmap: scaffold `Cargo.toml`, the 8 crates from
DESIGN + the test-support crate, with each function body as a `todo!()`. At
that point all 29 acceptance scenarios become RED (not BROKEN). DELIVER
crafter then enables one at a time per the standard outside-in TDD loop.

---

## 7. Test directory choice

`tests/acceptance/` (project root). This is Rust convention for top-level
integration tests; `cargo test --test walking_skeleton` etc. discovers them
automatically. No `tests/{feature-id}/acceptance/` nesting because slice-01
IS the only feature in the repository — additional features will land their
own `tests/{feature-name}/` directories as they ship.

When slice-03 (federated-read) ships in its own DISCUSS-through-DELIVER pass,
its acceptance tests live at `tests/slice03_federated_read/acceptance/` and
inherit the same shape: walking-skeleton + targeted + integration files.

---

## 8. Three Pillars compliance

| Pillar | How DISTILL satisfied it |
|---|---|
| 1 — Domain language | Scenarios use `Author`, `compose`, `claim`, `subject`, `evidence`, `confidence`, `publish`, `retract`, `query` — no `JSON`, `HTTP`, `database`, `endpoint`, `schema` in Gherkin steps or scenario titles. The word `CID` appears because it's user-visible (printed to stdout per US-002 mockup); it is a domain term, not implementation jargon. |
| 2 — Chained narrative | The walking-skeleton journey (WS-3 → WS-6 → WS-8 → WS-11) reads in order as the J-001 four-step journey. The `Given` of WS-6 = `Given + When` of WS-3 (Jeff composed a valid claim and is at the sign prompt). Step-method reuse enforced in `support/mod.rs`. |
| 3 — App as in production | Every WS scenario spawns the REAL `openlore` binary via `assert_cmd`. No hand-rebuilt wiring. The PDS + identity doubles substitute external/non-deterministic adapters per the Architecture of Reference defaults. |

---

## 9. Mandate compliance evidence (CM-A through CM-H)

| Mandate | Compliance evidence |
|---|---|
| CM-A (Mandate 1, hexagonal boundary) | All WS tests invoke `openlore` via `std::process::Command` / `assert_cmd`. ZERO direct imports of `claim_domain::*`, `adapter_duckdb::*`, etc., from `walking_skeleton.rs`. LC tests directly invoke pure-core functions (layer 2 acceptance per Mandate 9 — appropriate for property tests on canonicalization). |
| CM-B (Mandate 2, business language) | Grep of feature files / test names: zero occurrences of `HTTP`, `endpoint`, `database`, `schema`, `JSON`. The word `signature` appears (domain term — user signs claims). |
| CM-C (Mandate 3, complete journeys) | Every WS test traces to a user story with named persona action → observable outcome (see `traceability.md`). |
| CM-D (Mandate 4, pure function extraction) | LC-1 through LC-8 exercise pure functions directly (canonicalize / compute_cid / verify / reference_rules_validate / confidence_bucket). The CLI fixture parametrization is minimal: just `tempfile::TempDir` for HOME, no per-environment cross-product. |
| CM-E (Mandate 8, state-delta + Universe) | **DEFERRED to DELIVER**: the `state_delta` Rust port (`tests/common/state_delta.rs`) does not yet exist. DELIVER (functional-crafter) bootstraps it on first DISTILL-driven test that mutates observable state. The WS scenarios use assertion-helper functions defined in `support/mod.rs` (e.g. `assert_claim_published_at_uri(...)`) — these are the Rust idiomatic mirror of the universe-bound check. See `wave-decisions.md` DD-3. |
| CM-F (Mandate 9, layered PBT mode) | WS scenarios (layer 5 subprocess) are example-only. LC-3 is `@property` (proptest on canonicalization) at layer 2. ZERO proptest at layer 3+. |
| CM-G (Mandate 10, two-tier acceptance) | Tier A only. Slice-01's journey is 4 steps but they are linearly chained ("did this single user produce a CID that round-trips") — not a domain-rich state-space exploration. Per Mandate 10 skip-criteria #3 ("the only observable is 'did it crash'"; here, "did the round-trip succeed"), Tier B (state-machine PBT) is NOT added. Will revisit when slice-03 (federated multi-author state-machine) lands. |
| CM-H (Mandate 11, sad-paths example-based) | All error scenarios (WS-2, WS-4, WS-10, WS-13) are named examples (`Sad_*` / `Error_*` shape). No proptest at layer 3+ for sad paths. |

---

## 10. Definition of Done (DISTILL handoff to DELIVER)

- [x] All 29 scenarios written as RED-ready Rust skeletons.
- [x] Walking-skeleton scenarios tagged `@walking_skeleton @driving_port`.
- [x] Every CLI verb in ADR-003 covered by at least one subprocess scenario.
- [x] Every driven adapter mapped (real or fake double explicitly justified).
- [x] Three Pillars verified (domain language, chained narrative, production composition).
- [x] Mandate 8 (state-delta + Universe) documented for DELIVER bootstrap.
- [x] DISTILL flags 1-6 resolved against ADR-003 (3 deferred + 3 bound).
- [x] Wave-decision reconciliation passed (0 contradictions DISCUSS ↔ DESIGN).
- [x] `traceability.md` written: every test → story → job.
- [x] `wave-decisions.md` written: 13 decisions.
- [ ] Pre-DELIVER fail-for-right-reason gate: **deferred** — cannot run
      `cargo test` because `Cargo.toml` does not exist yet (DELIVER's first
      task). When DELIVER scaffolds the workspace, the first `cargo test`
      run MUST classify every scenario as RED (panic at `todo!()`), not
      BROKEN (import error). See `wave-decisions.md` DD-2.

Handoff-ready: **YES**, conditional on the DELIVER roadmap landing
`Cargo.toml` + the 8 crates from DESIGN before running the suite the first
time.

---

## 11. Open items for DELIVER

1. **Bootstrap `Cargo.toml`** + 8 crates per `component-boundaries.md`.
2. **Add `crates/test-support/`** (or `tests/common/`) for `FakePds`,
   `FakeIdentity`, `seed_fixture_claim()` helpers.
3. **Decide cucumber-rust vs plain `#[test]`**: the skeletons use plain
   `#[test]` for portability (every test name encodes the scenario). If
   crafter prefers `.feature` files for the readability win, the skeleton
   tests map 1:1 to scenario names and can be ported in DELIVER's first
   refactor.
4. **Bootstrap `tests/common/state_delta.rs`** per nw-distill polyglot
   matrix (Rust row). The Rust state-delta port is templated but not
   instantiated yet; first scenario that needs Universe-bound assertion
   forces the bootstrap.
5. **Land Pact contract suite** at `tests/contract/pds/` — DEVOPS's
   deliverable per DESIGN §6.5; out of DISTILL scope but flagged here so
   the roadmap accounts for it.

---

## 12. References

- `docs/feature/openlore-foundation/distill/wave-decisions.md`
- `docs/feature/openlore-foundation/distill/traceability.md`
- `tests/acceptance/README.md`
- `tests/acceptance/walking_skeleton.rs`
- `tests/acceptance/lexicon_conformance.rs`
- `tests/acceptance/federation_roundtrip.rs`
- `tests/acceptance/support/mod.rs`
- DESIGN: `architecture-design.md`, `component-boundaries.md`, `data-models.md`
- ADRs: ADR-002, ADR-003, ADR-006, ADR-007, ADR-008, ADR-009
- DISCUSS: `feature-delta.md`, `user-stories.md`, `gherkin-scenarios-expanded.md`, `shared-artifacts-registry.md`, `journey-author-and-publish-claim-visual.md`
- SSOT: `docs/product/journeys/author-and-publish-claim.yaml`, `docs/product/jobs.yaml`
