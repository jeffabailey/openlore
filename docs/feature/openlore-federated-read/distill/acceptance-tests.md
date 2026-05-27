# Acceptance Test Design — openlore-federated-read (slice-03)

- **Wave**: DISTILL
- **Date**: 2026-05-27
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-federated-read
- **Slice**: slice-03-federated-read (sibling feature; sibling-feature pattern per WD-9)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (per ADR-007)
- **Inherits**: slice-01 DISTILL artifacts (`docs/feature/openlore-foundation/distill/`) + slice-03 DISCUSS WD-14..WD-25 + slice-03 DESIGN WD-26..WD-45 + ADR-013..ADR-016
- **Language**: Rust (per ADR-009)
- **Test framework**: same as slice-01 — Rust std `#[test]` (per DD-1)

This document is the human-readable map over the executable test skeletons in
`tests/acceptance/peer_*.rs + counter_claim.rs + federated_query.rs +
lexicon_counter_claim.rs`. The `.rs` files are the SSOT for executable
scenarios.

---

## 1. Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** between DISCUSS WD-14..WD-25
(+ OD-FED-1/2/3 accepted at default) and DESIGN WD-26..WD-45. DESIGN
WD-38..WD-44 RESOLVE the seven `# DISTILL: confirm` flags in
`gherkin-scenarios-expanded.md` — see §2 below for the consolidated
resolution table; this DISTILL wave inherits the resolutions as binding.

DEVOPS-wave artifacts are absent (`docs/feature/openlore-federated-read/devops/`
does not exist as of this writing). Per `nw-distill` Graceful Degradation
matrix: WARN + apply default environment matrix (clean | with-pre-commit |
with-stale-config); DO NOT block. Slice-03's acceptance scenarios do not
depend on a per-environment fixture cross-product (the test seam is the
subprocess + `FakePds` + `FakePeerPds` + tempfile HOME pattern inherited
from slice-01).

---

## 2. Resolved DISTILL flags (DESIGN WD-38..WD-44)

DESIGN already RESOLVED all 7 `# DISTILL: confirm` flags from
`docs/feature/openlore-federated-read/discuss/gherkin-scenarios-expanded.md`.
DISTILL inherits the resolutions verbatim and binds the corresponding
acceptance scenarios.

| # | Original DISCUSS flag | DESIGN resolution | Bound scenario(s) |
|---|---|---|---|
| 1 | Anxiety 1.2 — cross-attributed peer record: store vs reject | **WD-41 LOCKED** — reject at pull-time with `PeerStorageError::CrossAttribution` | `peer_pull_rejects_cross_attribution_to_third_party_did_at_write_time` (PP-6) |
| 2 | Anxiety 2 — query-time re-verification of cached peer claim sigs | **WD-38 LOCKED** — slice-03 does NOT re-verify at query time; deferred to slice-04 `peer verify --all` verb | DEFERRED — no slice-03 scenario |
| 3 | Anxiety 3 — `peer audit <did>` verb | **OD-FED-3 LOCKED DISCUSS-side** — defer to slice-04 (derivable from `peer list --include-purged` + `graph query --include-counters`, both deferred) | DEFERRED — no slice-03 scenario |
| 4 | Anxiety 4 — no auto-notification of retraction to peer | **WD-44 LOCKED** — slice-03 ships NO notification mechanism in either direction | `counter_claim_publish_does_not_auto_notify_target_peer_pds` (CC-6) |
| 5 | Habit 1 — first-pull orientation trigger | **WD-39 LOCKED** — once-per-user via `~/.config/openlore/identity.toml [federation]` | `federated_query_first_invocation_emits_orientation_then_omits_on_subsequent_invocations` (FQ-6); first-pull orientation itself drives a probe in `cli` per ADR-013 §Earned Trust #7 — covered by `peer_pull_*` scenarios indirectly via OrientationState |
| 6 | Habit 2 — inline counter-claim template visibility | **WD-42 LOCKED** — shown by default; NO `--verbose` gate | `federated_query_renders_inline_counter_template_per_peer_row_by_default` (FQ-7) |
| 7 | Habit 2 — first-counter-claim framing block trigger | **WD-43 LOCKED** — exactly once per user (NOT first-3-times) | `counter_claim_first_invocation_renders_one_time_framing_block_then_omits_on_subsequent_invocations` (CC-5) |

All 7 flags resolved. Zero open ambiguity at scenario-write time.

---

## 3. Scope and shape

Same hexagonal port-to-port discipline as slice-01: every subprocess
acceptance test enters through the CLI driving adapter via `assert_cmd`,
exercises the real `claim-domain` + `lexicon` + `adapter-duckdb` pure
core / local-effect stack, and fakes ONLY the external/non-deterministic
boundaries (PDS via `FakePds`, peer PDS via new `FakePeerPds`, identity
via `FakeIdentity`).

### Layer placement (per nw-test-design-mandates Mandate 9)

| Layer | Test file(s) | Real adapters | Test mode |
|---|---|---|---|
| Walking-Skeleton subprocess (layer 5) | (slice-03 has NO new walking skeleton — slice-01's `walking_skeleton.rs` already shipped the e2e CLI wiring; slice-03 reuses it) | n/a | n/a |
| Subprocess / FS acceptance (layer 3) | `peer_subscribe.rs` (8), `peer_pull.rs` (8), `counter_claim.rs` (6), `federated_query.rs` (8) | CLI binary + DuckDB + FS; PDS + PeerPDS doubles | example-only (Mandate 11) |
| In-memory acceptance (layer 2) | `lexicon_counter_claim.rs` (5; 2 `@property`) | None — pure core directly | example + `@property` proptest (per Mandate 9 layer 2 PBT full) |

Layer 1 (pure-core unit tests for `normalize_reason`,
`validate_counter_claim`, peer-pull verify pipeline) is OUT OF DISTILL
SCOPE — those belong to DELIVER's inner TDD loop (per DD-FED-7 below).

### What is mocked, what is real (slice-03 additions to slice-01 table)

| Component | Treatment | Why |
|---|---|---|
| Peer ATProto PDS (XRPC peer-read paths) | NEW FAKE: `openlore_test_support::FakePeerPds` | Distinct from `FakePds` because peer PDS is a different actor (read-only; no createRecord on peer surface). Adversarial postures (tampered sig, CID mismatch, self-attribution per WD-40, cross-attribution per WD-41) are preconfigured constructors |
| PLC directory (peer DID resolution) | FAKE via `FakePeerPds::serve_http`'s identity.resolveDid handler | Real PLC requires network; recorded fixture replay acceptable. The peer-PDS fake's HTTP server hosts the identity endpoint on the same base URL — one server per peer keeps wiring simple |
| `peer_subscriptions` + `peer_claims` tables | REAL DuckDB (migration v3) | Mandate 6 — every driven adapter has at least one real-I/O scenario |
| `peer_claims/<did>/<cid>.json` directory tree | REAL filesystem under `tempfile::TempDir` | Same; the partition-by-DID layout is part of the hard-purge observable surface (WD-31) |
| Counter-claim `reason` field on the wire | REAL lexicon + claim-domain serde | NFC normalization runs in pure core; CID stability assertion needs real CBOR |

### Test file rationale + placement (DD-FED-14)

Per task brief option (A): flat layout under `tests/acceptance/` matching
the slice-01 pattern. Five new files alongside the existing three
(walking_skeleton.rs / lexicon_conformance.rs / federation_roundtrip.rs):

```
tests/acceptance/
  walking_skeleton.rs            # slice-01, unchanged
  lexicon_conformance.rs         # slice-01, unchanged
  federation_roundtrip.rs        # slice-01, unchanged
  peer_subscribe.rs              # slice-03 NEW (8 scenarios; US-FED-001 + US-FED-005)
  peer_pull.rs                   # slice-03 NEW (8 scenarios; US-FED-002 incl. WD-40/41)
  counter_claim.rs               # slice-03 NEW (6 scenarios; US-FED-004 + WD-43/44)
  federated_query.rs             # slice-03 NEW (8 scenarios; US-FED-003 + WD-39/42)
  lexicon_counter_claim.rs       # slice-03 NEW (5 scenarios; 2 @property — layer 2)
  support/
    mod.rs                       # EXTENDED — adds peer-claim helpers + adversarial-fixture assertion helpers
  README.md                      # slice-01, will be updated to mention slice-03 file roles
```

Rationale: preserves the `cargo test --test <file>` ergonomics from
slice-01; the four new slice-03 files are clearly labeled by domain
(`peer_subscribe`, `peer_pull`, `counter_claim`, `federated_query`,
`lexicon_counter_claim`). The shared `support/mod.rs` + the
`openlore-test-support` crate are EXTENDED, not duplicated. See DD-FED-14
below for the alternative (nested `tests/acceptance/openlore_federated_read/`)
considered and rejected.

---

## 4. Acceptance test inventory

Per Mandate 3 (User Journey Completeness) every test exercises a complete
user journey from observable trigger through observable outcome.

### `tests/acceptance/peer_subscribe.rs` — 8 scenarios

Stories: US-FED-001 (subscribe), US-FED-005 (remove with optional purge).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| PS-1 | `peer_subscribe_add_resolves_did_and_persists_subscription` | US-FED-001 | happy | `@us-fed-001 @real-io @driving_port @j-003` |
| PS-2 | `peer_subscribe_add_is_idempotent_on_re_subscribe` | US-FED-001 | edge | `@edge` |
| PS-3 | `peer_subscribe_add_rejects_unresolvable_did_and_writes_no_subscription` | US-FED-001 | error | `@error` |
| PS-4 | `peer_subscribe_add_rejects_self_did_subscription` | US-FED-001 | error | `@error` |
| PS-5 | `peer_subscribe_remove_soft_keeps_cached_peer_claims` | US-FED-005 | happy | `@j-003c` |
| PS-6 | `peer_subscribe_remove_purge_with_confirmation_deletes_peer_claims_and_preserves_user_counters` | US-FED-005 | happy | `@j-003c` (drives integration gate 4) |
| PS-7 | `peer_subscribe_remove_purge_declined_leaves_state_unchanged` | US-FED-005 | edge | `@edge` |
| PS-8 | `peer_subscribe_remove_purge_refuses_no_tty_mode` | US-FED-005 + WD-36 | error | `@error` |

Error-path ratio: 3/8 = 37.5%.

### `tests/acceptance/peer_pull.rs` — 8 scenarios

Story: US-FED-002 (pull peer claims with verification).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| PP-1 | `peer_pull_fetches_verifies_and_stores_peer_claims_attributed_per_record` | US-FED-002 | happy | `@us-fed-002 @j-003a @happy` |
| PP-2 | `peer_pull_is_idempotent_skipping_already_stored_claims_by_cid` | US-FED-002 | edge | `@edge` |
| PP-3 | `peer_pull_rejects_tampered_signature_per_record_and_stores_honest_records` | US-FED-002 + KPI-FED-6 | error | `@error @kpi-fed-6` |
| PP-4 | `peer_pull_rejects_cid_mismatch_per_record_and_stores_honest_records` | US-FED-002 | error | `@error` (drives integration gate 2) |
| PP-5 | `peer_pull_rejects_self_attribution_at_write_time` | US-FED-002 + WD-40 | error | `@error @wd-40` |
| PP-6 | `peer_pull_rejects_cross_attribution_to_third_party_did_at_write_time` | US-FED-002 + WD-41 | error | `@error @wd-41` |
| PP-7 | `peer_pull_skips_unreachable_peer_and_proceeds_with_others` | US-FED-002 + WD-37 | error | `@error` |
| PP-8 | `peer_pull_with_zero_subscriptions_prints_no_peers_subscribed_and_exits_zero` | US-FED-002 | edge | `@edge` |

Error-path ratio: 5/8 = 62.5%. (Reflects the adversarial-peer surface; sad
paths are the bulk of slice-03's release-gate work per KPI-FED-6 + WD-40 +
WD-41.)

### `tests/acceptance/counter_claim.rs` — 6 scenarios

Story: US-FED-004 (author + publish counter-claim).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| CC-1 | `counter_claim_compose_signs_and_publishes_via_slice_01_pipeline_with_required_framing` | US-FED-004 | happy | `@j-003b @kpi-fed-3 @happy` (drives integration gate 3) |
| CC-2 | `counter_claim_rejects_missing_reason_pre_compose` | US-FED-004 + WD-20 | error | `@error @wd-20` |
| CC-3 | `counter_claim_rejects_reason_exceeding_one_thousand_chars` | US-FED-004 + WD-20 | error | `@error @wd-20` |
| CC-4 | `counter_claim_rejects_self_counter_with_retract_hint` | US-FED-004 + WD-34 | error | `@error @wd-34` |
| CC-5 | `counter_claim_first_invocation_renders_one_time_framing_block_then_omits_on_subsequent_invocations` | US-FED-004 + WD-43 | habit | `@habit @wd-43` |
| CC-6 | `counter_claim_publish_does_not_auto_notify_target_peer_pds` | US-FED-004 + WD-44 | edge | `@wd-44` |

Error-path ratio: 3/6 = 50%.

### `tests/acceptance/federated_query.rs` — 8 scenarios

Story: US-FED-003 (federated graph query with per-author attribution).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| FQ-1 | `federated_query_returns_author_and_peer_claims_grouped_by_author_did` | US-FED-003 | happy | `@j-003a @kpi-fed-1 @kpi-fed-2 @happy` (drives integration gate 1) |
| FQ-2 | `federated_query_renders_identical_content_from_different_authors_as_two_separate_rows` | US-FED-003 + KPI-FED-2 | guardrail | `@anti-merging @kpi-fed-2` |
| FQ-3 | `federated_query_default_without_flag_is_byte_identical_to_slice_01_behavior` | US-FED-003 | regression | `@regression @default-off` |
| FQ-4 | `federated_query_with_zero_peers_subscribed_degrades_with_hint` | US-FED-003 | edge | `@edge` |
| FQ-5 | `federated_query_annotates_counter_relationships_bidirectionally` | US-FED-003 + US-FED-004 | happy (chained narrative — Pillar 2) | `@j-003a @j-003b @happy` |
| FQ-6 | `federated_query_first_invocation_emits_orientation_then_omits_on_subsequent_invocations` | US-FED-003 + WD-39 | habit | `@habit @wd-39` |
| FQ-7 | `federated_query_renders_inline_counter_template_per_peer_row_by_default` | US-FED-003 + WD-42 | habit | `@habit @wd-42` |
| FQ-8 | `federated_query_no_merged_rows_across_multi_author_multi_record_fixture` | US-FED-003 + KPI-FED-2 | release-gate | `@kpi-fed-1 @kpi-fed-2 @release-gate` |

Error-path ratio: 0/8 — federated query is a read surface; sad paths
(storage corruption, schema mismatch) belong to the adapter integration
suite per DD-FED-6 + slice-01 DD-8 precedent.

### `tests/acceptance/lexicon_counter_claim.rs` — 5 scenarios (2 `@property`)

Story: US-FED-006 (infra — Lexicon `reason` field forward-compat); also
covers WD-35 NFC normalization properties.

| # | Test name | Source | Type | Tag(s) |
|---|---|---|---|---|
| LCC-1 | `lexicon_counter_claim_slice_01_era_claim_loads_without_reason_field` | US-FED-006 + ADR-015 | forward-compat | `@forward-compat @adr-015` |
| LCC-2 | `lexicon_counter_claim_reason_none_preserves_cid_stability_with_slice_01` | I-FED-7 + data-models | cid-stability | `@cid-stability @adr-015` |
| LCC-3 | `lexicon_counter_claim_normalize_reason_is_idempotent_property` | WD-35 + data-models property 2 | `@property` | `@property @wd-35` |
| LCC-4 | `lexicon_counter_claim_normalize_reason_unifies_canonically_equivalent_strings_property` | WD-35 + data-models property 3 | `@property` | `@property @wd-35` |
| LCC-5 | `lexicon_counter_claim_rejects_reason_length_outside_one_to_one_thousand` | ADR-015 minLength/maxLength | boundary | `@error @adr-015` |

### Total slice-03 scenarios across the wave

8 (PS) + 8 (PP) + 6 (CC) + 8 (FQ) + 5 (LCC) = **35 scenarios** authored,
all RED-ready as `todo!()` scaffolds. Cross-file error-path ratio:
11/35 = 31.4%. Below the 40% nw-test-design-mandates target overall but
above the slice-01 baseline (20.7%); the read-only `federated_query.rs`
brings the aggregate down. Per-file ratios are healthy where the surface
admits sad paths: `peer_pull.rs` 62.5% (the adversarial surface),
`counter_claim.rs` 50% (validation-heavy), `peer_subscribe.rs` 37.5%.

(Slice-01 shipped 29 scenarios for a 17-scenario walking-skeleton + 8
lexicon + 4 federation-roundtrip surface; slice-03's 35 is consistent —
the broader CLI surface and the adversarial-peer fixtures expand the
count.)

---

## 5. Driving Adapter coverage (Mandate 1 + RCA P1)

Every NEW or EXTENDED CLI verb in ADR-013 covered by at least one
subprocess scenario:

| Verb / flag | Scenario coverage |
|---|---|
| `openlore peer add <did>` (NEW) | PS-1 (happy), PS-2 (idempotent), PS-3 (error — DID), PS-4 (error — self) |
| `openlore peer pull` (NEW) | PP-1 through PP-8 (8 scenarios incl. WD-40, WD-41, KPI-FED-6) |
| `openlore peer remove <did> [--purge]` (NEW) | PS-5 (soft), PS-6 (purge happy), PS-7 (purge declined), PS-8 (--no-tty refusal per WD-36) |
| `openlore claim counter <cid> --reason "..."` (NEW) | CC-1 (happy), CC-2/3/4 (errors), CC-5 (first-invocation orientation), CC-6 (no-auto-notify) |
| `openlore graph query --federated` (EXTENDED) | FQ-1 through FQ-8; FQ-3 specifically asserts default-off regression |
| `openlore init` (EXTENDED — migration v3) | NOT covered by a slice-03-specific scenario; the migration-on-existing-DB observable is the existence of `peer_subscriptions` + `peer_claims` tables, which every other slice-03 scenario depends on (via `TestEnv::initialized`). Per US-FED-006 UAT scenario #2 the migration is integration-tested at adapter level (DELIVER's adapter integration suite per DD-FED-6). |

Zero uncovered NEW entry points.

---

## 6. Driven adapter coverage (Mandate 6)

| Driven adapter | Real-I/O scenario? | Tag |
|---|---|---|
| `adapter-duckdb` (StoragePort extension: `query_federated_by_subject`) | YES — FQ-1, FQ-2, FQ-5, FQ-8 exercise it via `graph query --federated` | `@real-io` |
| `adapter-duckdb` (NEW PeerStoragePort: write_peer_claim + soft_remove + hard_purge + list_active_subscriptions) | YES — PP-1, PP-2, PP-3, PP-4, PP-5, PP-6, PP-7, PS-1, PS-2, PS-5, PS-6, PS-7, FQ-5 | `@real-io` |
| `adapter-atproto-did` (IdentityPort extension: `resolve_peer`) | PARTIAL — PS-1, PS-3, PS-4 exercise the surface but via the `FakePeerPds::serve_http` PLC handler (recorded fixture replay), NOT a real PLC HTTP call. Real PLC integration is contract-tested via the Pact suite (DEVOPS's deliverable per DESIGN §6.4) | `@fake-peer-pds` |
| `adapter-atproto-pds` (PdsPort extension: `list_peer_records` + `get_peer_record`) | PARTIAL — every PP-* scenario hits the surface via `FakePeerPds`. Real peer-PDS contract test = DEVOPS Pact suite (per DESIGN §6.4 + outcome-kpis.md DEVOPS handoff) | `@fake-peer-pds` |
| `adapter-system-clock` | YES — every scenario implicitly (subscribed_at, fetched_at, composed_at timestamps) | `@real-io` |
| Filesystem `peer_claims/<did>/<cid>.json` directory tree | YES — PP-1 (write), PS-6 (hard-purge directory removal observation) | `@real-io` |

The PARTIAL coverage on the two network adapters (peer DID resolution +
peer PDS read) is structural to slice-03's acceptance scope: per DESIGN
§6.4 + outcome-kpis.md DEVOPS handoff, **Pact contract tests against the
real ATProto peer-read paths are DEVOPS's deliverable**, not DISTILL's.
DISTILL ships the acceptance shape; DEVOPS extends the slice-01 Pact
suite with consumer-driven contracts for `com.atproto.repo.listRecords`,
`com.atproto.repo.getRecord`, and `com.atproto.identity.resolveDid`.

---

## 7. Integration gates coverage (shared-artifacts-registry.md)

Per DISCUSS shared-artifacts-registry.md §"Integration validation gates":

| Gate | Where asserted | Mandatory for KPI |
|---|---|---|
| 1. `federation_attribution_preserved` (anti-merging) | FQ-1 + FQ-2 + FQ-8 | KPI-FED-1 + KPI-FED-2 (release-blocking) |
| 2. `peer_cid_round_trip` | PP-4 (CID mismatch rejection) | KPI-FED-1 + KPI-FED-2 |
| 3. `counter_target_cid_round_trip` | CC-1 (compose preview + signed payload + subsequent query annotation) | KPI-FED-3 |
| 4. `peer_remove_purge_separation` | PS-6 (purge happy path with confirmation) | KPI-FED-4 (release-blocking) |
| 5. `peer_tampered_signature_rejected` (KPI-FED-6 adversarial fixture) | PP-3 | KPI-FED-6 (release-blocking) |

All five gates have at least one acceptance test. The adversarial fixture
for KPI-FED-6 is preconfigured via `FakePeerPds::with_tampered_signature`
+ `fixture_adversarial_peer_tampered_signature` — DEVOPS's CI fixture
work (per outcome-kpis.md DEVOPS handoff) extends this to a wiremock-based
real-HTTP variant.

---

## 8. KPI coverage

| KPI | Description | Acceptance coverage |
|---|---|---|
| KPI-FED-1 | Attribution fidelity 100% | FQ-1 (the load-bearing assertion), FQ-2 (zero-merge gate), FQ-8 (multi-author release-gate) |
| KPI-FED-2 | Zero merged rows guardrail | FQ-2 (explicit zero-merge assertion), FQ-8 (release gate) |
| KPI-FED-3 | Counter-claim publication rate (north star) | CC-1 (verb works), CC-5 (habit affordance via framing block), FQ-7 (habit affordance via inline template per WD-42) |
| KPI-FED-4 | Zero purge residue | PS-6 (load-bearing — assertion that peer_claims rows + filesystem directory are gone AND user counters preserved) |
| KPI-FED-5 | E2E latency ≤90s | Implicit in PP-1 + FQ-1 (timing assertion deferred to DEVOPS perf suite per outcome-kpis.md measurement plan) |
| KPI-FED-6 | Zero invalid signatures stored | PP-3 (the adversarial-signature rejection — release-blocking) |

KPI-FED-5 is the only KPI without a hard assertion at this layer — by
design, per outcome-kpis.md: latency is collected via tracing telemetry
in production, not asserted at the acceptance-test boundary.

---

## 9. Three Pillars compliance

| Pillar | How DISTILL satisfied it |
|---|---|
| 1 — Domain language | Scenario titles use `peer`, `subscribe`, `pull`, `counter`, `purge`, `attribution`, `federated`, `author DID`, `signature`, `CID`, `reason`. Zero technical jargon: NO `JSON`, `HTTP`, `database`, `endpoint`, `schema`, `SQL`, `XRPC`. (The word `XRPC` appears in test-support comments because that IS the protocol name peer adapters speak; it does NOT appear in any scenario title or step-method name.) |
| 2 — Chained narrative | Multi-scenario journeys read in order: PS-5 (soft remove) → PS-6 (purge happy) → PS-7 (purge declined) reuses the same Given (Maria has subscribed + pulled). PP-1 (pull happy) → PP-3 (tampered) → PP-7 (unreachable) share the "Given Maria has subscribed to peer X" preamble via TestEnv. CC-1 (compose+sign+publish) → FQ-5 (subsequent query annotates the counter-claim) is the cross-file chained pair. |
| 3 — App as in production | Every scenario except `lexicon_counter_claim.rs` (layer 2, pure-core direct) spawns the REAL `openlore` binary via `assert_cmd::Command::cargo_bin`. No hand-rebuilt wiring. PeerPDS + PDS + Identity doubles substitute external/non-deterministic adapters per the Architecture of Reference defaults; the Project Infrastructure Policy entries are extended below in §11. |

---

## 10. Mandate compliance evidence (CM-A through CM-H)

Same shape as slice-01 acceptance-tests.md §9:

| Mandate | Compliance evidence |
|---|---|
| CM-A (Mandate 1, hexagonal boundary) | All slice-03 acceptance tests invoke `openlore` via subprocess; ZERO direct imports of `claim_domain::*`, `adapter_duckdb::*`, etc., from `peer_*.rs / counter_claim.rs / federated_query.rs`. The `lexicon_counter_claim.rs` layer-2 tests directly invoke pure-core `claim_domain::normalize_reason` + `lexicon::validate` — appropriate at layer 2 per Mandate 9 |
| CM-B (Mandate 2, business language) | Grep of test names: zero `HTTP`, `endpoint`, `database`, `schema`, `JSON`, `SQL`, `XRPC`. Domain terms only (`subscribe`, `pull`, `counter`, `purge`, `attribution`, `federated`, `peer`, `reason`, `confirmation`) |
| CM-C (Mandate 3, complete journeys) | Every test traces to a user story → see traceability.md. The chained narratives (PS-5/6/7 purge sequence; CC-1 → FQ-5 publish-then-query annotation cross-file pair) satisfy Pillar 2 |
| CM-D (Mandate 4, pure function extraction) | `normalize_reason` + `validate_counter_claim` exercised DIRECTLY in `lexicon_counter_claim.rs` LCC-3/LCC-4/LCC-5 — pure functions, no fixtures, no adapters. CLI parameterization is just `tempfile::TempDir` for HOME (no environment cross-product) |
| CM-E (Mandate 8, state-delta + Universe) | **DEFERRED to DELIVER** — same status as slice-01 DD-3. The Rust `state_delta` port at `tests/common/state_delta.rs` was bootstrapped by slice-01 (see test-support Cargo.toml `[[test]] name = "state_delta_bootstrap"`). Slice-03 scenarios use named assertion helpers in `support/mod.rs` (e.g. `assert_peer_claims_attributed_to(did, count)`, `assert_no_merged_rows_in_federated_output`) as the Rust idiomatic mirror; DELIVER migrates the load-bearing scenarios (PS-6, PP-1, PP-3, PP-5, PP-6, FQ-2, FQ-8) to `assert_state_delta(before, after, universe, expected)` form. Universe entries MUST be port-exposed (e.g. `peer_storage.claims.row_count_by_author[did]`, `cli.graph_query.distinct_authors_in_output`) per Mandate 8 — NEVER internal struct fields |
| CM-F (Mandate 9, layered PBT mode) | LCC-3 + LCC-4 are `@property` at layer 2 (proptest); ALL subprocess scenarios at layer 3+ are example-only. ZERO proptest at layer 3+ |
| CM-G (Mandate 10, two-tier acceptance) | Tier A only. Per Mandate 10 add-if-both-conditions criteria: slice-03 journeys are 2-3 chained scenarios with small fixed input spaces (subscribe + pull + remove is 3 scenarios but the input space is "peer DID list + record set"); the input space is not domain-rich (vs. emails / payloads / free-text). Tier B would model `peer_subscriptions × peer_claims` as a state machine; the J-003a anti-merging invariant IS the kind of cross-rule invariant Tier B catches. **Recommendation**: revisit at slice-04 (multi-peer reputation weighting) where the state space genuinely expands; for slice-03 the example tests cover the contract surface and KPI-FED-2 is asserted at the explicit FQ-8 release gate. See DD-FED-9 below |
| CM-H (Mandate 11, sad-paths example-based) | Every sad path is a named `Sad_*` / `*_rejects_*` scenario: PP-3, PP-4, PP-5, PP-6, PP-7, PS-3, PS-4, PS-8, CC-2, CC-3, CC-4. ZERO proptest at layer 3+ for sad paths |

---

## 11. Project Infrastructure Policy — slice-03 additions

Slice-01 DD-11 deferred writing
`docs/architecture/atdd-infrastructure-policy.md` until the orchestrator
scope permits. Slice-03's orchestrator brief also limits writes to the
slice-03 directories + `tests/acceptance/` + `crates/test-support/src/`.
The policy file is STILL not created in this DISTILL wave. The new
entries that SHOULD land when the surface opens:

```markdown
# Slice-03 additions to ATDD Infrastructure Policy

## Driving (extends slice-01)
| Port | Mechanism | Note |
|---|---|---|
| CLI (`openlore peer {add,pull,remove}` + `claim counter` + `graph query --federated`) | subprocess from `tempfile::TempDir` via `assert_cmd` | inherits slice-01 cli mechanism; new verbs reuse the same env-var seam (`OPENLORE_PEER_PDS_ENDPOINT_<did>` planned) |

## Driven internal (real) — extends slice-01
| Port | Mechanism | Note |
|---|---|---|
| PeerStoragePort (DuckDB peer_subscriptions + peer_claims + peer_claim_references + peer_claim_evidence + filesystem peer_claims/<did>/<cid>.json) | real DuckDB file + real filesystem under `tempfile::TempDir`; migration v3 runs at TestEnv init | hard-purge directory removal observable via tempdir inspection |
| StoragePort (extended with query_federated_by_subject) | real DuckDB file (UNION ALL pattern per data-models.md §Cross-store query examples) | cross-store UNION ALL; xtask check-arch enforces no-elide-author SQL rule |

## Driven external / non-deterministic (fake) — extends slice-01
| Port | Fake | Note |
|---|---|---|
| PdsPort (peer-read methods: list_peer_records + get_peer_record) | `openlore_test_support::FakePeerPds` (read-only HTTP XRPC stub; preconfigured adversarial postures) | real peer-PDS contract test = DEVOPS Pact suite extension per DESIGN §6.4 |
| IdentityPort (resolve_peer) | `FakePeerPds::serve_http` includes a `com.atproto.identity.resolveDid` handler returning fixture peer DID document | real PLC integration = DEVOPS Pact suite + recorded fixture replay |
```

---

## 12. Pre-requisites for compilation (DELIVER wiring expectations)

The slice-03 skeletons use `use openlore::...` paths via the existing
slice-01 binary AND `use openlore_test_support::...` for the new doubles
(`FakePeerPds`, `FakePeerRecord`, `fixture_other_developer_three_claims`,
`fixture_adversarial_peer_*`). The intentional consequence:

1. **`cargo build --tests` will fail on `openlore-test-support` until
   DELIVER's slice-03 step-06-01 scaffold lands** the bodies for:
   - `FakePeerPds::for_peer` + `with_*` adversarial constructors +
     `serve_http`
   - `fixture_other_developer_three_claims` + `fixture_adversarial_peer_*`
   - The new ports types referenced by step-defs:
     `ports::PeerStoragePort` + `ports::PeerInfo` + the extended
     `PdsPort` + extended `IdentityPort` methods per
     component-boundaries.md §`crates/ports`.

2. **Once `crates/test-support/src/fake_peer_pds.rs` + `fixtures_peer.rs`
   are materialized AND `ports` is extended**, the slice-03 tests compile
   to "all `#[test]` functions panic with `todo!()`" → tests RED per
   Mandate 7. DELIVER then unskips one at a time.

3. **Rust scaffold marker** per Mandate 7 + slice-01 precedent: every
   `#[test]` body that panics does so via `todo!("DELIVER (slice-03):
   ...")` with a `// SCAFFOLD: true` comment-marker on the surrounding
   module. Detection via `grep -r "SCAFFOLD: true" tests/`. The new
   modules `fake_peer_pds.rs` + `fixtures_peer.rs` carry the same marker.

4. **Pre-DELIVER fail-for-right-reason gate (slice-03)** is deferred per
   the same logic as slice-01 DD-2: the test-support extensions land in
   DELIVER's first slice-03 step (step-06-01); only after that step do
   the tests compile and reach the `todo!()` panic that classifies as
   RED. See DD-FED-13 below.

DELIVER's first slice-03 task (proposed): bootstrap the four new tables
in `adapter-duckdb` (`peer_subscriptions`, `peer_claims`,
`peer_claim_references`, `peer_claim_evidence`) per ADR-014 + the new
`PeerStoragePort` trait stubs in `ports` per component-boundaries.md, +
the `FakePeerPds` HTTP server + adversarial fixture bodies in
test-support. At that point the slice-03 acceptance suite classifies as
RED and the standard outside-in TDD loop resumes.

---

## 13. Definition of Done (DISTILL handoff to DELIVER)

- [x] All 35 slice-03 scenarios written as RED-ready Rust skeletons.
- [x] Every NEW or EXTENDED CLI verb in ADR-013 covered by at least one
      subprocess scenario.
- [x] Every NEW or EXTENDED driven adapter mapped (real or fake double
      explicitly justified) — see §6.
- [x] Three Pillars verified (domain language, chained narrative,
      production composition).
- [x] All 7 `# DISTILL: confirm` flags from gherkin-scenarios-expanded.md
      resolved per DESIGN WD-38..WD-44 — see §2.
- [x] Wave-decision reconciliation passed (0 contradictions
      DISCUSS ↔ DESIGN).
- [x] `traceability.md` written: every test → story → job →
      ADR / wave-decision.
- [x] `wave-decisions.md` written: DD-FED-1..DD-FED-14.
- [x] Integration gates 1-5 from shared-artifacts-registry.md covered.
- [x] All 6 KPIs mapped to acceptance scenarios (or to telemetry — KPI-FED-5).
- [ ] **Pre-DELIVER fail-for-right-reason gate**: DEFERRED until DELIVER
      scaffolds the slice-03 test-support extensions (`FakePeerPds` +
      `fixtures_peer` bodies + extended ports trait surface).
      See §12 + DD-FED-13.

Handoff-ready: **YES**, conditional on DELIVER's slice-03 step-06-01
landing the four new ports + the test-support extensions + the four new
DuckDB tables before running the suite the first time.

---

## 14. Open items for DELIVER

1. **Bootstrap slice-03 PortStorage** per component-boundaries.md
   §`crates/ports` + `crates/adapter-duckdb`: new `PeerStoragePort` trait
   + extended `StoragePort::query_federated_by_subject` + extended
   `PdsPort::list_peer_records + get_peer_record` + extended
   `IdentityPort::resolve_peer`. All `probe()` methods per ADR-009.

2. **Materialize `FakePeerPds` HTTP server + adversarial fixtures** in
   `crates/test-support/src/fake_peer_pds.rs` (scaffolded by THIS
   DISTILL wave; bodies are `todo!()`). See per-fixture comments for
   posture details.

3. **Migrate DuckDB schema to v3** per data-models.md §"DuckDB schema —
   slice-03 additions (migration v3)". Idempotent forward-only.

4. **Implement `claim-domain::normalize_reason` + `validate_counter_claim`**
   per component-boundaries.md §`crates/claim-domain` extensions; pin
   them via slice-01 LC-3-style proptest in `claim-domain`'s unit tests
   (layer 1 — out of DISTILL scope, DELIVER's call).

5. **Extend `OrientationState`** in `cli` per data-models.md §"OrientationState
   — identity.toml extensions". Three keys; three message gates.

6. **Implement `cli` verb handlers**: `VerbPeerAdd`, `VerbPeerPull`,
   `VerbPeerRemove`, `VerbClaimCounter`, `VerbGraphQuery` extension for
   `--federated`. All reuse slice-01 internals where possible
   (`VerbClaimPublish` for counter; existing `VerbGraphQuery` for the
   read path).

7. **xtask check-arch slice-03 rule**: `no_cross_table_join_elides_author`
   per data-models.md §"Cross-store query examples" FORBIDDEN-pattern
   section.

8. **Tier B (state-machine PBT)** revisit decision per CM-G §10 — slice-04
   is the right surface for adding Tier B once multi-peer reputation
   weighting expands the input space.

9. **DEVOPS Pact suite extension** for peer-read paths + adversarial CI
   fixture for KPI-FED-6 — per outcome-kpis.md DEVOPS handoff; not a
   DELIVER deliverable but a coordination point.

---

## 15. References

- `docs/feature/openlore-federated-read/distill/wave-decisions.md`
- `docs/feature/openlore-federated-read/distill/traceability.md`
- `tests/acceptance/peer_subscribe.rs`
- `tests/acceptance/peer_pull.rs`
- `tests/acceptance/counter_claim.rs`
- `tests/acceptance/federated_query.rs`
- `tests/acceptance/lexicon_counter_claim.rs`
- `crates/test-support/src/fake_peer_pds.rs`
- `crates/test-support/src/fixtures_peer.rs`
- DISCUSS: `docs/feature/openlore-federated-read/feature-delta.md` +
  `discuss/{user-stories,gherkin-scenarios-expanded,shared-artifacts-registry,outcome-kpis}.md` +
  `discuss/journey-{subscribe-and-read-federated,author-counter-claim}-visual.md`
- DESIGN: `docs/feature/openlore-federated-read/design/{architecture-design,component-boundaries,data-models,wave-decisions}.md`
- ADRs: ADR-013..ADR-016
- SSOT: `docs/product/journeys/{subscribe-and-read-federated,author-counter-claim}.yaml` +
  `docs/product/jobs.yaml` (J-003 + sub-jobs) +
  `docs/product/personas/researcher-tech-lead.yaml` (federation-reader hat)
- Inherited from slice-01:
  - `docs/feature/openlore-foundation/distill/{acceptance-tests,wave-decisions,traceability}.md`
  - `tests/acceptance/{walking_skeleton,lexicon_conformance,federation_roundtrip}.rs`
  - `tests/acceptance/support/mod.rs`
  - `crates/test-support/src/{lib,fake_pds,identity,fixtures}.rs`
