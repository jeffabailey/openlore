# Traceability Matrix — openlore-foundation (slice-01)

- **Wave**: DISTILL
- **Date**: 2026-05-25
- **Acceptance Designer**: Quinn

Every acceptance scenario maps to (a) a user story, (b) a Job-To-Be-Done from
`docs/product/jobs.yaml`, (c) the originating wave-decision lock, (d) the
DESIGN ADR(s) that constrain its observable contract, and (e) the journey-YAML
step where applicable.

---

## 1. Coverage matrix — Walking-Skeleton scenarios

File: `tests/acceptance/walking_skeleton.rs`

| Test name | Story | Job | Wave-decision lock | ADR(s) | Journey step | KPI link |
|---|---|---|---|---|---|---|
| `walking_skeleton_init_creates_identity_duckdb_and_is_idempotent` | US-005 | infra (→J-001) | WD-9 (carpaccio), WD-12 (DID) | ADR-002, ADR-001, ADR-009 | n/a (bootstrap) | n/a (enabling) |
| `walking_skeleton_claim_commands_fail_loudly_when_not_initialized` | US-005 | infra (→J-001) | WD-9 | ADR-009 (probe gauntlet) | n/a | n/a (guardrail) |
| `walking_skeleton_compose_preview_contains_not_as_truth_and_waits_for_confirmation` | US-001 | J-001 | WD-6 ("not as truth" + retract hint) | ADR-003 §two-prompt | step 1 (Compose) | KPI-1, KPI-3 |
| `walking_skeleton_compose_rejects_confidence_outside_unit_interval` | US-001 | J-001 | WD-10 (numeric [0,1]) | ADR-003, data-models.md confidence min/max | step 1 (Compose, failure_modes) | KPI-1 |
| `walking_skeleton_compose_preview_shows_bucket_label_but_signed_payload_has_only_numeric` | US-001 | J-001 | WD-10, D-12 | ADR-003, data-models.md | step 1 | KPI-4 |
| `walking_skeleton_sign_writes_atomic_local_file_with_no_network_call` | US-002 | J-001 | WD-3 (walking skeleton scope), D-1 (hexagonal) | ADR-006, ADR-009 | step 2 (Sign + persist) | KPI-5 (local-first invariant) |
| `walking_skeleton_re_canonicalization_produces_identical_cids` | US-002 | J-001 | risk #2 (CID determinism) | ADR-006 §Earned Trust | step 2 | KPI-4 |
| `walking_skeleton_publish_prints_at_uri_and_retract_hint_after_signing` | US-003 | J-001 | WD-6 (retract hint at publish), WD-11 (counter-claim) | ADR-003, ADR-008 | step 3 (Publish) | KPI-3, KPI-6 |
| `walking_skeleton_publish_is_idempotent_on_re_run_with_same_cid` | US-003 | J-001 | (idempotency = ADR-003 §verb publish) | ADR-003 §verb publish + ADR-006 (CID as rkey) | step 3 (failure_modes) | n/a (UX safety) |
| `walking_skeleton_pds_unreachable_leaves_local_claim_intact_and_retry_actionable` | US-003 | J-001 | WD-11 (no hard-delete) ensures local survives | ADR-009 §failure modes | step 3 (failure_modes) | KPI-5 |
| `walking_skeleton_graph_query_returns_just_published_claim_byte_for_byte` | US-004 | J-001 + J-002 | WD-4 (P-001 only in slice-01; J-002 partial) | data-models.md §Validation rules row 4 | step 4 (Read back) | KPI-4 |
| `walking_skeleton_graph_query_default_is_local_only_and_footer_announces_it` | US-004 | J-001 + J-002 | WD-13 (federation = slice-03) | ADR-003 §verb graph query | step 4 | n/a (UX) |
| `walking_skeleton_graph_query_empty_result_is_explained_not_silent` | US-004 | J-001 + J-002 | (US-004 AC) | n/a (UX) | step 4 (failure_modes) | n/a |
| `walking_skeleton_retract_publishes_new_counter_claim_referencing_original` | US-003 (retract extension) | J-001 | WD-11, D-8 | ADR-003 §verb retract, ADR-008 §Behavioral rules | post step 4 (retract is a sibling beat) | KPI-3 (anxiety mitigation observable) |
| `walking_skeleton_retract_preserves_original_record_in_local_and_remote_stores` | (ADR-008 §Behavioral 1-3) | J-001 | WD-11 (no hard-delete) | ADR-008 §Behavioral 1, 2, 3 | post step 4 | n/a (guardrail) |
| `walking_skeleton_corrective_workflow_publishes_new_claim_and_retracts_old` | gherkin-expanded anxiety-2 | J-001 | DD-9 (two-step corrective workflow) | ADR-008 (references[] mechanism) | journey-extended | KPI-3 |
| `walking_skeleton_calibration_anxiety_user_cancels_and_re_runs_with_lower_confidence` | gherkin-expanded anxiety-3 | J-001 | WD-10 (bucket display only), DD-9 (cancel + re-run) | ADR-003 (Ctrl-C cancel; re-run with new flags) | step 1 (extended) | KPI-3 |

---

## 2. Coverage matrix — Lexicon-conformance scenarios

File: `tests/acceptance/lexicon_conformance.rs`

| Test name | Story | Job | Wave-decision lock | ADR(s) | KPI link |
|---|---|---|---|---|---|
| `lexicon_roundtrip_compose_sign_serialize_deserialize_yields_equal_value` | US-002 + US-004 | J-001 | risk #2 (CID determinism) | ADR-006 §Earned Trust prop 1 | KPI-4 |
| `lexicon_validates_signed_claim_against_org_openlore_claim_schema` | US-005 | infra | D-6 (lexicon namespace) | ADR-005 | n/a (federation guardrail) |
| `lexicon_cid_is_byte_stable_across_n_re_canonicalizations` (`@property`) | US-002 | J-001 | risk #2 | ADR-006 §Earned Trust prop 1 (proptest) | KPI-4 |
| `lexicon_cid_is_byte_stable_for_fixture_suite_of_known_claims` | US-002 | J-001 | risk #2 | ADR-006 §Earned Trust prop 2 (gold fixtures) | KPI-4 |
| `lexicon_rejects_out_of_range_confidence_at_wire_boundary` | US-001 | J-001 | WD-10 | data-models.md confidence min/max | n/a (defense-in-depth on top of CLI pre-sign) |
| `lexicon_rejects_self_reference_in_references_array` | (ADR-008 Behavioral 4) | J-001 | WD-11 | ADR-008 §Behavioral 4, §Earned Trust 2 | n/a (guardrail) |
| `lexicon_rejects_two_hop_reference_cycle` | (ADR-008 Behavioral 4) | J-001 | WD-11 | ADR-008 §Earned Trust 3 | n/a (guardrail) |
| `lexicon_persisted_payload_never_contains_bucket_label_string` | US-001 + US-002 | J-001 | WD-10, D-12 | data-models.md §Confidence buckets are NOT persisted | n/a (CI-failable invariant) |

---

## 3. Coverage matrix — Federation-round-trip scenarios

File: `tests/acceptance/federation_roundtrip.rs`

| Test name | Story | Job | Wave-decision lock | ADR(s) | KPI link |
|---|---|---|---|---|---|
| `federation_roundtrip_publish_three_claims_different_predicates_all_round_trip_with_cids_intact` | US-003 + US-004 | J-001 + J-002 (partial) | WD-3 (slice-01 = WS for OpenLore umbrella) | ADR-006, ADR-008 | KPI-4 (round-trip across 3 records) |
| `federation_roundtrip_pds_record_rkey_equals_claim_cid` | US-003 | J-001 | WD-11 (idempotency depends on this) | ADR-003 §verb publish, ADR-006 | n/a (federation contract) |
| `federation_roundtrip_at_uri_is_reconstructible_from_author_did_and_claim_cid` | US-003 + US-004 | J-001 + J-002 | shared-artifacts-registry rule 3 | ADR-002 (DID), ADR-006 (CID), data-models.md | KPI-4 |
| `federation_roundtrip_graph_query_output_matches_compose_preview_field_for_field` | US-001 + US-004 | J-001 + J-002 | shared-artifacts-registry rule 4 | data-models.md §Validation rules row 4 | KPI-4 (the load-bearing one) |

---

## 4. Story coverage (every story has ≥ 1 acceptance test)

| Story | Title | Test count | Test names |
|---|---|---|---|
| US-001 | Author a single signed claim from the CLI | 4 | WS-3, WS-4, WS-5, WS-17 (calibration anxiety) |
| US-002 | Sign and persist a claim locally before any publication | 3 | WS-6, WS-7, LC-1 (round-trip) |
| US-003 | Publish a signed claim to the author's PDS | 6 | WS-8, WS-9, WS-10, WS-14, WS-15, WS-16 |
| US-004 | Read back local claims by subject | 4 | WS-11, WS-12, WS-13, FR-4 |
| US-005 | Bootstrap claim Lexicon, identity wiring, and DuckDB schema | 3 | WS-1, WS-2, LC-2 |
| (Cross-cutting: ADR-008 retraction) | n/a | 4 | WS-14, WS-15, LC-6, LC-7 |
| (Cross-cutting: WD-10 confidence) | n/a | 3 | WS-5, LC-5, LC-8 |
| (Cross-cutting: ADR-006 CID stability) | n/a | 4 | WS-7, LC-3, LC-4, FR-2 |
| (Cross-cutting: federation round-trip) | n/a | 4 | FR-1, FR-2, FR-3, FR-4 |

Every story has at least 3 scenarios; the most-tested story is US-003 (6 scenarios) because publish is the load-bearing federation beat AND the retract surface AND the corrective workflow.

---

## 5. Job coverage

| Job | In slice-01? | Test count | Scenarios |
|---|---|---|---|
| J-001 Author a signed philosophical claim | YES — primary, walking-skeleton | 27 (everything except 2 in J-002 and LC-2 in infra) | nearly all WS + all LC + all FR |
| J-002 Explore the philosophy graph | PARTIAL — slice-01 touches it via US-004 | 4 | WS-11, WS-12, WS-13, FR-4 |
| J-003 Read another developer's federated claims | NO — slice-03 | 0 | (deferred) |
| J-004 Evaluate a contributor's body of work | NO — slice-04/05 | 0 | (deferred) |
| infrastructure (US-005) | YES — supports J-001 | 3 | WS-1, WS-2, LC-2 |

---

## 6. Wave-decision coverage

Every locked DISCUSS WD-N and DESIGN D-N decision that touches user-observable
behavior maps to at least one acceptance test:

| Wave decision | Coverage |
|---|---|
| WD-3 (walking-skeleton slice-01) | the ENTIRE walking_skeleton.rs file |
| WD-6 ("not as truth" + retract hint) | WS-3 (explicit literal-text assertion), WS-8 (retract hint at publish) |
| WD-9 (carpaccio split locked) | WS-1 (init bootstrap demonstrates the slice's standalone shape) |
| WD-10 (numeric confidence + display buckets, never persisted) | WS-4 (rejection), WS-5 (bucket display), LC-5 (Lexicon rejection), LC-8 (no bucket in persisted payload) |
| WD-11 (retraction = counter-claim, no hard-delete) | WS-14, WS-15, LC-6, LC-7 |
| WD-12 (existing DID + per-app derived key) | WS-1 (init resolves DID + derives key), FR-3 (at_uri reconstructible from author_did) |
| WD-13 (federation = slice-03) | WS-12 (footer announces local-only + slice-03 hint) |
| D-2 (DuckDB) | WS-6, WS-7, WS-11, FR-1 (all exercise real DuckDB I/O) |
| D-4 (two-prompt CLI) | WS-3, WS-6, WS-8 (three scenarios assert two-prompt observable beats in sequence) |
| D-6 (`org.openlore.*` namespace) | LC-2 (schema validation) |
| D-7 (CIDv1 dag-cbor sha2-256 base32-lower) | WS-7, LC-3 (`@property`), LC-4 (gold fixtures), FR-2 |
| D-8 (retraction via references[]) | WS-14, WS-15, LC-6, LC-7 |
| D-9 (wire-then-probe-then-use) | WS-1 (init runs probe gauntlet), WS-2 (probe refusal exits 2) |
| D-12 (confidence buckets never persisted) | LC-8 |

---

## 7. Deferred scenarios (recorded for sibling features)

These are the `# DISTILL: confirm command name` scenarios from
`gherkin-scenarios-expanded.md` that were resolved as DEFERRED in DD-9. They
are recorded here so the sibling-feature DISTILL waves can pick them up.

| Anxiety/Habit scenario | Reason deferred | Sibling feature |
|---|---|---|
| Anxiety 1.1 — peer publishes counter-claim with damning evidence | requires federation (another author's PDS) | slice-03 federated-read |
| Anxiety 1.2 — user notified of inbound counter-claim (`claim status`) | requires federation + new CLI verb | slice-03 |
| Habit 2.1 — lurker-nudge contribution view (`graph contrib --me`) | requires read-count telemetry + new CLI verb | slice-04 scoring-graph |
| Habit 2.2 — contribution view stays silent for active publishers | same as habit 2.1 | slice-04 |
| Habit 1.1 — `--from-url` pre-fills evidence from browser tab URL | habit-bridging affordance; URL-driven authoring is scraper-class | slice-02 github-scraper |

The COVERED anxiety scenarios:
- Anxiety 1.3 (soft-retract preserves public history) → WS-15
- Anxiety 2 (correct typo'd evidence via two-step workflow) → WS-16
- Anxiety 3 (calibration: cancel + re-run lower confidence) → WS-17

---

## 8. KPI coverage

From `outcome-kpis.md` (read indirectly via the DISCUSS feature-delta):

| KPI | Description | Acceptance coverage |
|---|---|---|
| KPI-1 | <2 minute end-to-end | implicit in WS-3 + WS-6 + WS-8 (timing assertion deferred to DEVOPS perf suite) |
| KPI-2 | (acceptance-only KPI per DISCUSS) | WS-3 (compose preview presence) |
| KPI-3 | "claims-not-truth" landing | WS-3 (literal text), WS-8 (retract hint), WS-14 (retract works), WS-16, WS-17 |
| KPI-4 | round-trip identity (zero silent normalization) | WS-7, WS-11, LC-1, LC-3, LC-4, FR-1, FR-3, FR-4 |
| KPI-5 | local-first invariant (offline up to publish) | WS-6 ("no network call"), WS-10 (PDS unreachable preserves local) |
| KPI-6 | publication willingness at day-30 | n/a (longitudinal KPI, not test-observable) |

---

## 9. Changelog

- 2026-05-25 — Quinn — initial traceability matrix. All 29 DISTILL
  scenarios mapped to story / job / wave-decision / ADR. Zero
  un-traced scenarios.
