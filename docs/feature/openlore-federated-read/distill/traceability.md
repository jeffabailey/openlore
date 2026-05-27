# Traceability Matrix — openlore-federated-read (slice-03)

- **Wave**: DISTILL
- **Date**: 2026-05-27
- **Acceptance Designer**: Quinn

Every slice-03 acceptance scenario maps to (a) a user story, (b) a
Job-To-Be-Done / sub-job from `docs/product/jobs.yaml`, (c) the
originating wave-decision lock (DISCUSS WD-N OR DESIGN WD-N), (d) the
DESIGN ADR(s) constraining the observable contract, (e) the journey-YAML
step where applicable, (f) the integration-validation gate from
shared-artifacts-registry.md, and (g) the KPI link.

---

## 1. Coverage matrix — `peer_subscribe.rs` (8 scenarios)

| Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|
| `peer_subscribe_add_resolves_did_and_persists_subscription` | US-FED-001 | J-003 | WD-29 (IdentityPort extension), ADR-013 §verb peer add | ADR-013, ADR-014 | subscribe-and-read step 1 (subscribe) | n/a | KPI-FED-5 (enabling) |
| `peer_subscribe_add_is_idempotent_on_re_subscribe` | US-FED-001 | J-003 | WD-29, ADR-013 §verb peer add (idempotency row) | ADR-013 | step 1 (failure_modes — Example 2) | n/a | n/a (UX) |
| `peer_subscribe_add_rejects_unresolvable_did_and_writes_no_subscription` | US-FED-001 | J-003 | ADR-013 §exit-code table (peer add exit-1) | ADR-013 | step 1 (failure_modes — Example 3) | n/a | n/a |
| `peer_subscribe_add_rejects_self_did_subscription` | US-FED-001 | J-003 | US-FED-001 AC #5; ADR-013 exit-code table | ADR-013 | step 1 (failure_modes — UAT scenario #4) | n/a | n/a |
| `peer_subscribe_remove_soft_keeps_cached_peer_claims` | US-FED-005 | J-003c (revocability without residue) | WD-25 (soft-remove retains cache) | ADR-013, ADR-014 | (peer remove journey — soft branch) | n/a | KPI-FED-4 (partial) |
| `peer_subscribe_remove_purge_with_confirmation_deletes_peer_claims_and_preserves_user_counters` | US-FED-005 | J-003c | WD-21 (interactive confirmation), WD-25 (counter-claims survive), I-FED-4 (atomic transaction) | ADR-013, ADR-014 | (peer remove journey — purge branch) | **Gate 4: `peer_remove_purge_separation`** | **KPI-FED-4 (release-blocking)** |
| `peer_subscribe_remove_purge_declined_leaves_state_unchanged` | US-FED-005 | J-003c | WD-21, US-FED-005 AC #4 | ADR-013 | (peer remove journey — Example 3) | n/a | n/a |
| `peer_subscribe_remove_purge_refuses_no_tty_mode` | US-FED-005 | J-003c | **WD-36** (--no-tty REFUSES --purge) | ADR-013 §exit-code table | (peer remove journey — scripting safety) | n/a | KPI-FED-4 (defense-in-depth) |

---

## 2. Coverage matrix — `peer_pull.rs` (8 scenarios)

| Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|
| `peer_pull_fetches_verifies_and_stores_peer_claims_attributed_per_record` | US-FED-002 | J-003 + J-003a (anti-merging) | WD-24 (sig + CID at pull), WD-27 (PeerStoragePort), WD-30 (anti-merging type+structural+behavioral), ADR-013 output convention "None merged with your own claims." | ADR-013, ADR-014, ADR-016 | subscribe-and-read step 2 (pull) | n/a (gate 1 lives at query layer) | KPI-FED-1 + KPI-FED-2 |
| `peer_pull_is_idempotent_skipping_already_stored_claims_by_cid` | US-FED-002 | J-003 | US-FED-002 AC #6, ADR-013 §peer pull idempotency | ADR-013, ADR-016 | step 2 (failure_modes — UAT scenario #4) | n/a | n/a (UX) |
| `peer_pull_rejects_tampered_signature_per_record_and_stores_honest_records` | US-FED-002 | J-003a | WD-24, WD-37 (fault isolation), ADR-013 exit-code (peer pull exit-1) | ADR-013, ADR-016 | step 2 (failure_modes — Example 2) | **Gate 5: `peer_tampered_signature_rejected`** | **KPI-FED-6 (release-blocking)** |
| `peer_pull_rejects_cid_mismatch_per_record_and_stores_honest_records` | US-FED-002 | J-003a | WD-24, WD-37 | ADR-006 (CID determinism), ADR-013, ADR-016 | step 2 (failure_modes — UAT scenario #3) | **Gate 2: `peer_cid_round_trip`** | KPI-FED-1 + KPI-FED-2 |
| `peer_pull_rejects_self_attribution_at_write_time` | US-FED-002 | J-003a | **WD-40** (SelfAttribution at write), I-FED-2 (peer_claims.author_did NEVER NULL) | ADR-014 | step 2 (failure_modes — defense vs key compromise) | n/a (write-time guard backs gates 1-2) | KPI-FED-1 (defense-in-depth) |
| `peer_pull_rejects_cross_attribution_to_third_party_did_at_write_time` | US-FED-002 | J-003a | **WD-41** (CrossAttribution at write — RESOLVES `# DISTILL: confirm` anxiety scenario 1.2) | ADR-014 | step 2 (failure_modes — adversarial peer) | n/a (write-time guard backs gates 1-2) | KPI-FED-1 (defense-in-depth) |
| `peer_pull_skips_unreachable_peer_and_proceeds_with_others` | US-FED-002 | J-003 | WD-37 (sequential pull, fault-isolated), ADR-013 exit-code | ADR-013, ADR-016 | step 2 (failure_modes — Example 3) | n/a | n/a (UX) |
| `peer_pull_with_zero_subscriptions_prints_no_peers_subscribed_and_exits_zero` | US-FED-002 | J-003 | WD-18 (pull-on-demand only), ADR-013 §Earned Trust #4 | ADR-013, ADR-016 | step 2 (failure_modes — empty subscription list) | n/a | n/a (UX) |

---

## 3. Coverage matrix — `counter_claim.rs` (6 scenarios)

| Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|
| `counter_claim_compose_signs_and_publishes_via_slice_01_pipeline_with_required_framing` | US-FED-004 | J-003b (counter-claim as first-class disagreement) + J-001 (publish flow) | WD-17 (sugar verb), WD-22 (reuse VerbClaimPublish), WD-23 (reason field optional in Lexicon; required at CLI), I-FED-5 (no parallel publish path), ADR-013 §inheritance from ADR-003 (two-prompt + literals) | ADR-003, ADR-013, ADR-015 | author-counter-claim steps 1-2-3 (compose + sign + publish) | **Gate 3: `counter_target_cid_round_trip`** | **KPI-FED-3 (north star)** + KPI-FED-1 + KPI-FED-2 |
| `counter_claim_rejects_missing_reason_pre_compose` | US-FED-004 | J-003b | **WD-20** (--reason REQUIRED 1..=1000), WD-34 (validate in pure core pre-compose) | ADR-013, ADR-015 | author-counter-claim step 1 (failure_modes — Example 3) | n/a | n/a (defense-in-depth) |
| `counter_claim_rejects_reason_exceeding_one_thousand_chars` | US-FED-004 | J-003b | WD-20 (1..=1000 upper bound), ADR-015 §Lexicon maxLength | ADR-013, ADR-015 | author-counter-claim step 1 (boundary) | n/a | n/a (defense-in-depth) |
| `counter_claim_rejects_self_counter_with_retract_hint` | US-FED-004 | J-003b | **WD-34** (validate_counter_claim in pure core; ClaimLookup spans claims + peer_claims) | ADR-013, ADR-015 | author-counter-claim step 1 (failure_modes — Example 2) | n/a | n/a (UX — guides user to correct verb) |
| `counter_claim_first_invocation_renders_one_time_framing_block_then_omits_on_subsequent_invocations` | US-FED-004 | J-003b (habit affordance) | **WD-43** (once-per-user, NOT first-3-times — RESOLVES `# DISTILL: confirm` habit scenario 2 framing-block trigger), WD-39 (OrientationState mechanism) | ADR-013, ADR-016 | author-counter-claim step 1 (habit-bridging) | n/a | KPI-FED-3 (habit affordance) |
| `counter_claim_publish_does_not_auto_notify_target_peer_pds` | US-FED-004 | J-003b | **WD-44** (no auto-notification — RESOLVES `# DISTILL: confirm` anxiety scenario 4) | ADR-013, ADR-016 | author-counter-claim step 3 (out-of-scope notifications) | n/a | n/a (anxiety mitigation observable) |

---

## 4. Coverage matrix — `federated_query.rs` (8 scenarios)

| Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|
| `federated_query_returns_author_and_peer_claims_grouped_by_author_did` | US-FED-003 | J-003a (anti-merging — load-bearing) | WD-30 (anti-merging three-layer), I-FED-1 (cross-table no-elide-author), ADR-013 footer convention | ADR-013, ADR-014 | subscribe-and-read step 3 (federated query) | **Gate 1: `federation_attribution_preserved`** | **KPI-FED-1 + KPI-FED-2 (release-blocking)** |
| `federated_query_renders_identical_content_from_different_authors_as_two_separate_rows` | US-FED-003 | J-003a | KPI-FED-2 zero-merge guardrail | ADR-014 | step 3 (UAT scenario #2; Example 3) | (drives Gate 1) | KPI-FED-2 (release-blocking) |
| `federated_query_default_without_flag_is_byte_identical_to_slice_01_behavior` | US-FED-003 | J-003 | WD-13 (federation = slice-03), US-FED-003 AC #2 (--federated default OFF) | ADR-013 | step 3 (regression vs slice-01) | n/a | n/a (regression) |
| `federated_query_with_zero_peers_subscribed_degrades_with_hint` | US-FED-003 | J-003 | US-FED-003 AC #7 (graceful degrade with hint) | ADR-013 | step 3 (failure_modes — Example 2) | n/a | n/a (UX) |
| `federated_query_annotates_counter_relationships_bidirectionally` | US-FED-003 + US-FED-004 | J-003a + J-003b | US-FED-003 AC #8 (bidirectional counters/countered-by), shared-artifacts §gate 3 | ADR-013, ADR-014 | author-counter-claim step 4 (peer observes counter via own pull); subscribe-and-read step 3 (counter annotation in federated query) | n/a (drives Gate 3 indirectly) | KPI-FED-3 |
| `federated_query_first_invocation_emits_orientation_then_omits_on_subsequent_invocations` | US-FED-003 | J-003 (habit) | **WD-39** (once-per-user — RESOLVES `# DISTILL: confirm` habit scenario 1 first-federated-query trigger), OD-FED-2 (once-per-user via identity.toml) | ADR-013, ADR-016 | step 3 (habit-bridging) | n/a | KPI-FED-3 (habit affordance — first impression) |
| `federated_query_renders_inline_counter_template_per_peer_row_by_default` | US-FED-003 | J-003b (habit) | **WD-42** (template shown by default — RESOLVES `# DISTILL: confirm` habit scenario 2 inline-template trigger) | ADR-013 | step 3 (habit-bridging) | n/a | KPI-FED-3 (habit affordance — friction reduction) |
| `federated_query_no_merged_rows_across_multi_author_multi_record_fixture` | US-FED-003 | J-003a | KPI-FED-2 release gate | ADR-014 (I-FED-1 behavioral layer) | step 3 (the multi-author release-gate test) | **Gate 1: `federation_attribution_preserved` (multi-author shape)** | **KPI-FED-1 + KPI-FED-2 (release-blocking)** |

---

## 5. Coverage matrix — `lexicon_counter_claim.rs` (5 scenarios; layer 2)

| Test name | Story | Job | Wave-decision lock | ADR(s) | KPI link |
|---|---|---|---|---|---|
| `lexicon_counter_claim_slice_01_era_claim_loads_without_reason_field` | US-FED-006 | infra (→J-003) | I-FED-6 (reason field optional at Lexicon level — forward-compat) | ADR-005 (Lexicon stability), ADR-015 | n/a (forward-compat guardrail) |
| `lexicon_counter_claim_reason_none_preserves_cid_stability_with_slice_01` | US-FED-006 | infra (→J-003) | I-FED-7 (CID stability across upgrade), WD-32 (top-level optional field) | ADR-006 (CID determinism), ADR-015 | KPI-FED-1 (cross-slice attribution fidelity) |
| `lexicon_counter_claim_normalize_reason_is_idempotent_property` (`@property`) | US-FED-004 (supports) | J-003b | **WD-35** (NFC normalization), data-models.md property 2 | ADR-006, ADR-015 | n/a (correctness invariant) |
| `lexicon_counter_claim_normalize_reason_unifies_canonically_equivalent_strings_property` (`@property`) | US-FED-004 (supports) | J-003b | WD-35, data-models.md property 3 | ADR-006, ADR-015 | n/a (correctness invariant — copy-paste workflows) |
| `lexicon_counter_claim_rejects_reason_length_outside_one_to_one_thousand` | US-FED-004 (supports) | J-003b | WD-20 (1..=1000 enforced at Lexicon layer for defense-in-depth) | ADR-015 (§Lexicon minLength/maxLength) | n/a (defense-in-depth) |

---

## 6. Story coverage (every story has ≥ 1 acceptance test)

| Story | Title | Test count | Test names |
|---|---|---|---|
| US-FED-001 | Subscribe to a peer's claim stream | 4 | PS-1, PS-2, PS-3, PS-4 |
| US-FED-002 | Pull peer claims with sig + CID verification | 8 | PP-1..PP-8 (incl. WD-40 self-attr + WD-41 cross-attr + KPI-FED-6 tampered sig) |
| US-FED-003 | Read federated graph with per-author attribution | 8 | FQ-1..FQ-8 |
| US-FED-004 | Author and publish a counter-claim | 7 | CC-1..CC-6 + LCC-3/LCC-4/LCC-5 (supporting layer-2 properties + boundary) — the cross-file annotation pair FQ-5 also touches US-FED-004 |
| US-FED-005 | Remove a peer subscription with optional purge | 4 | PS-5, PS-6, PS-7, PS-8 |
| US-FED-006 | Bootstrap peer storage + Lexicon `reason` field (`@infrastructure`) | 2 | LCC-1, LCC-2 (the Lexicon forward-compat + CID stability scenarios); the schema migration itself is implicitly tested by every subprocess scenario that initializes a fresh `TestEnv` (which runs `openlore init` and therefore migration v3) |
| (Cross-cutting: J-003a anti-merging) | n/a | 7 | PP-1, PP-3, PP-4, PP-5, PP-6, FQ-1, FQ-2, FQ-8 (load-bearing) |
| (Cross-cutting: J-003c revocability) | n/a | 4 | PS-5, PS-6, PS-7, PS-8 |
| (Cross-cutting: WD-39/42/43 habit-bridging) | n/a | 3 | FQ-6, FQ-7, CC-5 |

Every story has at least 2 scenarios; the most-tested story is US-FED-002
(8 scenarios) because peer pull is the load-bearing federation beat AND
the adversarial-fixture surface (KPI-FED-6 + WD-40 + WD-41).

---

## 7. Job coverage

| Job / sub-job | In slice-03? | Test count | Scenarios |
|---|---|---|---|
| J-003 Read another developer's federated claims with weighting | YES — primary (walking-skeleton job for this feature per WD-16) | 35 (all slice-03 scenarios) | every PS / PP / CC / FQ / LCC scenario |
| J-003a Attribute every peer claim without merging | YES — LOAD-BEARING | 8 | PP-1, PP-3, PP-4, PP-5, PP-6, FQ-1, FQ-2, FQ-8 |
| J-003b Counter-claim authoring as first-class disagreement | YES | 7 | CC-1..CC-6 + FQ-5 (cross-file annotation) |
| J-003c Subscription is revocable without residue | YES | 4 | PS-5, PS-6, PS-7, PS-8 |
| J-001 Author a signed philosophical claim | PARTIAL — inherited (CC-1 reuses VerbClaimPublish via WD-22 / I-FED-5) | 1 explicit | CC-1 |
| J-002 Explore the philosophy graph | PARTIAL — extended (US-FED-003 extends graph query) | 8 | FQ-1..FQ-8 |

---

## 8. Wave-decision coverage (DISCUSS WD-14..WD-25 + DESIGN WD-26..WD-45)

Every locked DISCUSS WD-N + DESIGN WD-N + OD-FED-N decision that touches
user-observable behavior maps to at least one acceptance test:

### DISCUSS

| Wave decision | Coverage |
|---|---|
| WD-14 (slice-03 = SIBLING feature; sibling-feature pattern) | This entire DISTILL wave's directory structure |
| WD-15 (P-002 primary, P-001 secondary in federation-reader hat) | All scenarios reference P-002 (Maria/Rachel/Aanya/Tobias) per the user-stories fixtures |
| WD-16 (J-003 = walking-skeleton job for slice-03) | All scenarios trace to J-003 or sub-jobs (§7) |
| WD-17 (counter-claim verb = sugar verb `claim counter`) | CC-1 asserts the verb shape; ADR-013 ties down the contract |
| WD-18 (pull-on-demand only; no auto-pull, no push, no daemon) | PP-8 (zero subscriptions = no-op exit 0); ADR-013 §Earned Trust #4 |
| WD-19 (single DuckDB file with two new tables; xtask check-arch enforces) | Implicit in every scenario that runs migration v3 at TestEnv init; assertion lives at xtask layer (out of DISTILL scope) |
| WD-20 (--reason REQUIRED, 1..=1000 chars) | CC-2 (missing), CC-3 (boundary), LCC-5 (Lexicon-layer defense-in-depth) |
| WD-21 (`--purge` REQUIRES interactive confirmation) | PS-6 (confirm), PS-7 (decline), PS-8 (--no-tty refusal via WD-36) |
| WD-22 (counter-claim reuses VerbClaimPublish; no parallel path) | CC-1 (asserts the chained sign + publish via slice-01 pipeline); I-FED-5 enforced at code-review + cli probe layer |
| WD-23 (reason field OPTIONAL in Lexicon — forward-compat) | LCC-1 (slice-01-era loads without field), LCC-2 (CID stability) |
| WD-24 (per-claim sig verify + CID recompute at pull time) | PP-1 (happy path; both checks pass), PP-3 (sig fail), PP-4 (CID fail) |
| WD-25 (soft-remove retains cache; hard-purge deletes peer cache; user counters survive) | PS-5 (soft), PS-6 (hard + counters survive) |
| OD-FED-1 (default = sugar verb `claim counter`) | CC-1 binds this default (no override observed) |
| OD-FED-2 (default = once-per-user via identity.toml) | WD-39 + FQ-6 + CC-5 bind this default |
| OD-FED-3 (default = defer `peer audit` to slice-04) | NO scenario asserts `peer audit` (correct deferral) |

### DESIGN

| Wave decision | Coverage |
|---|---|
| WD-26 (no new crates; extend in place) | DISTILL constraint — slice-03 extends `crates/test-support/` (NOT a new crate) |
| WD-27 (NEW `PeerStoragePort`; shares DuckDB pool) | All PP / PS scenarios exercise this port via the CLI verbs |
| WD-28 (PdsPort EXTENDED with peer-read methods) | All PP scenarios; FakePeerPds is the test seam |
| WD-29 (IdentityPort EXTENDED with `resolve_peer`) | PS-1 + PS-3 + PS-4 |
| WD-30 (anti-merging type+structural+behavioral three-layer) | Behavioral layer asserted by FQ-1 + FQ-2 + FQ-8 (drives Gate 1) |
| WD-31 (peer_claims partition tree `peer_claims/<peer_did>/<cid>.json`) | PS-6 (hard-purge observes directory removal) |
| WD-32 (`reason` is TOP-LEVEL OPTIONAL property) | LCC-1 + LCC-2 + LCC-5 |
| WD-33 (counter-claim reuses VerbClaimPublish — function call) | CC-1 |
| WD-34 (self-counter rejection in PURE CORE pre-compose; ClaimLookup spans both stores) | CC-4 |
| WD-35 (--reason NFC-normalized in `claim_domain::normalize_reason`) | LCC-3 (idempotency), LCC-4 (NFC-unification) |
| WD-36 (`--purge` REFUSES --no-tty mode; refusal branch) | PS-8 (the directing-error refusal) |
| WD-37 (peer pull sequential; fault-isolated per-peer per-record) | PP-3 (per-record), PP-7 (per-peer) |
| WD-38 (no query-time re-verification in slice-03 — RESOLVES `# confirm` anxiety 2) | INTENTIONALLY UNTESTED — slice-04 surface |
| WD-39 (once-per-user orientation via identity.toml) | FQ-6 (first-federated-query); CC-5 (first-counter-claim); pull-orientation drives cli probe #7 of ADR-013 §Earned Trust |
| WD-40 (SelfAttribution rejection at write — defense vs key compromise) | PP-5 |
| WD-41 (CrossAttribution rejection at write — RESOLVES `# confirm` anxiety 1.2) | PP-6 |
| WD-42 (inline counter-claim template by default — RESOLVES `# confirm` habit 2 template) | FQ-7 |
| WD-43 (first-counter-claim framing block once-per-user — RESOLVES `# confirm` habit 2 framing) | CC-5 |
| WD-44 (no auto-notification — RESOLVES `# confirm` anxiety 4) | CC-6 |
| WD-45 (ADR-013..016 accepted; no further iterations needed) | INTENTIONALLY UNTESTED — process decision |

---

## 9. Integration gate coverage (shared-artifacts-registry.md)

| Gate | Where asserted | Test name(s) | Mandatory for KPI |
|---|---|---|---|
| 1. `federation_attribution_preserved` (anti-merging) | layer 3 subprocess | FQ-1, FQ-2, FQ-8 | KPI-FED-1 + KPI-FED-2 (release-blocking) |
| 2. `peer_cid_round_trip` | layer 3 subprocess + layer 2 (LCC-2 covers the slice-01-era CID-stability sibling) | PP-4 (CID mismatch rejection) | KPI-FED-1 + KPI-FED-2 |
| 3. `counter_target_cid_round_trip` | layer 3 subprocess | CC-1 (compose preview + signed payload + subsequent query annotation via FQ-5 chained) | KPI-FED-3 |
| 4. `peer_remove_purge_separation` | layer 3 subprocess | PS-6 | KPI-FED-4 (release-blocking) |
| 5. `peer_tampered_signature_rejected` (KPI-FED-6 adversarial fixture) | layer 3 subprocess | PP-3 | KPI-FED-6 (release-blocking) |

All five gates have at least one acceptance test. The DEVOPS fixture
extension (wiremock-based adversarial peer for KPI-FED-6 real-HTTP variant)
extends gate 5 but is out of DISTILL scope per DD-FED-6.

---

## 10. KPI coverage

| KPI | Description | Acceptance coverage | Type |
|---|---|---|---|
| KPI-FED-1 | Attribution fidelity 100% | FQ-1 (load-bearing), FQ-2, FQ-8 + LCC-2 (cross-slice stability) | Leading (Outcome) |
| KPI-FED-2 | Zero merged rows guardrail | FQ-2, FQ-8 | Leading (Guardrail) |
| KPI-FED-3 | Counter-claim publication rate ≥30% in 30 days | CC-1 (verb works), CC-5 + FQ-6 + FQ-7 (habit affordances) | Leading (Outcome — north star) |
| KPI-FED-4 | Zero purge residue | PS-6 (load-bearing) + PS-8 (defense vs accidental scripting) | Leading (Outcome / Guardrail) |
| KPI-FED-5 | Subscribe → pull → query e2e under 90s | PP-1 + FQ-1 (path exists; latency measurement is DEVOPS perf-suite, NOT acceptance — per outcome-kpis.md measurement plan) | Leading (Outcome) |
| KPI-FED-6 | Zero invalid signatures stored | PP-3 (load-bearing — release-blocking) | Leading (Guardrail / Security) |

---

## 11. Resolved DISTILL flag → scenario mapping (consolidated)

DESIGN WD-38..WD-44 RESOLVED all seven `# DISTILL: confirm` flags from
`docs/feature/openlore-federated-read/discuss/gherkin-scenarios-expanded.md`.
This DISTILL wave inherits the resolutions and binds the scenarios:

| `# DISTILL: confirm` flag (gherkin-scenarios-expanded.md) | DESIGN resolution | Scenario binding |
|---|---|---|
| Anxiety 1.2 — cross-attributed peer record store/reject | **WD-41 — REJECT with `CrossAttribution`** | PP-6 |
| Anxiety 2 — query-time re-verification | **WD-38 — DEFER to slice-04** | NO slice-03 scenario (intentional) |
| Anxiety 3 — `peer audit <did>` verb | **OD-FED-3 — DEFER to slice-04** | NO slice-03 scenario (intentional) |
| Anxiety 4 — no auto-notification of retraction | **WD-44 — NO notification mechanism** | CC-6 |
| Habit 1 — first-pull orientation trigger | **WD-39 — once-per-user via identity.toml** | FQ-6 (first-federated-query is the symmetric variant); pull-orientation covered by cli probe #7 of ADR-013 §Earned Trust + the OrientationState wiring asserted indirectly in PP-1 |
| Habit 2 — inline counter-claim template visibility | **WD-42 — shown by default; no `--verbose` gate** | FQ-7 |
| Habit 2 — first-counter-claim framing block trigger | **WD-43 — exactly once per user (not first-3-times)** | CC-5 |

---

## 12. Cross-feature inheritance from openlore-foundation

Slice-03 INHERITS without modification:

| Inherited from slice-01 | Status in slice-03 |
|---|---|
| WS-1..WS-17 walking-skeleton scenarios | UNCHANGED — slice-01 is the umbrella walking skeleton |
| LC-1..LC-8 lexicon-conformance scenarios | UNCHANGED |
| FR-1..FR-4 federation-roundtrip scenarios | UNCHANGED |
| `tests/acceptance/support/mod.rs` | EXTENDED with peer-claim helpers (DD-FED-5) |
| `crates/test-support/src/lib.rs + fake_pds.rs + identity.rs + fixtures.rs` | EXTENDED with `fake_peer_pds.rs + fixtures_peer.rs` |
| Slice-01 DD-1..DD-13 | All still binding (see wave-decisions.md §"Inheritance from slice-01 DISTILL") |
| Slice-01 ADR-001..ADR-012 | All still binding |

---

## 13. Changelog

- 2026-05-27 — Quinn — initial traceability matrix for slice-03. All 35
  scenarios mapped to story / job / sub-job / wave-decision / ADR /
  integration gate / KPI. Zero un-traced scenarios. The 7 `# DISTILL:
  confirm` flags from gherkin-scenarios-expanded.md are mapped to their
  WD-38..WD-44 resolutions and the corresponding scenario bindings.
