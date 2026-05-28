# Traceability Matrix — openlore-github-scraper (slice-02)

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn

Every slice-02 acceptance scenario maps to (a) a user story, (b) a
Job-To-Be-Done / sub-job from `docs/product/jobs.yaml` (J-004 + J-004a/b/c),
(c) the originating wave-decision lock (DISCUSS WD-N OR DESIGN WD-N), (d) the
DESIGN ADR(s) constraining the observable contract, (e) the journey-YAML step
where applicable, (f) the integration-validation gate from
shared-artifacts-registry.md, and (g) the KPI link.

---

## 1. Coverage matrix — `scrape_github.rs` (9 scenarios)

| Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|
| `scrape_github_harvests_public_repo_proposes_candidates_and_persists_nothing` | US-SCR-001 + US-SCR-002 | J-004 + J-004a + J-004b | WD-50/60 (verb), WD-51 (public-only), WD-55 (nothing persisted) | ADR-017, ADR-019 | steps 1+2 (scrape + propose) | **Gate 1: `scraper_never_persists_unsigned`** | **KPI-SCR-2 (release-blocking)** |
| `scrape_github_prints_public_data_banner_before_any_harvest` | US-SCR-001 | J-004a | WD-51 (public-data-only banner) | ADR-017, ADR-019 | step 1 (banner before harvest) | n/a (reassurance) | KPI-SCR-4 (trust affordance) |
| `scrape_github_resolves_user_target_and_harvests_bounded_aggregate` | US-SCR-001 | J-004a | **WD-64** (bounded aggregate; triangulation -> slice-04) | ADR-019 | step 1 (Example 2 — user target) | n/a | KPI-SCR-1 (real-target reach) |
| `scrape_github_rejects_nonexistent_target_with_zero_candidates` | US-SCR-001 | J-004a | ADR-019 §GithubError::NotFound | ADR-017, ADR-019 | step 1 (failure_modes — 404) | n/a | n/a (UX) |
| `scrape_github_refuses_private_target_and_calls_no_private_endpoint` | US-SCR-001 | J-004a | **WD-51** (public-data-only), I-SCR-2 | ADR-019 | step 1 (failure_modes — private) | **Gate 3: `scraper_only_reads_public_data`** | **KPI-SCR-4 (release-blocking)** |
| `scrape_github_offline_exits_with_requires_network_and_no_partial_list` | US-SCR-001 | J-004a | ADR-019 §GithubError::Network | ADR-017, ADR-019 | step 1 (failure_modes — offline) | n/a | KPI-SCR-5 (local-first scoped exception — harvest needs network) |
| `scrape_github_with_no_matching_signals_proposes_nothing_and_exits_zero` | US-SCR-002 | J-004b | WD-53 (mapping), US-SCR-002 AC 6 | ADR-018 | step 2 (failure_modes — no candidates) | n/a | n/a (UX) |
| `scrape_github_without_sign_makes_zero_pds_writes` | US-SCR-001 + US-SCR-002 | J-004 + J-004c | **WD-49** (human-gate), WD-55 | ADR-017 | step 4 (gherkin — no --sign signs nothing) | **Gate 1: `scraper_never_persists_unsigned`** | **KPI-SCR-2 (release-blocking)** |
| `scrape_github_is_a_pure_read_persisting_nothing_across_repeated_runs` | US-SCR-001 | J-004 + J-004c | WD-55 (nothing persisted unsigned) | ADR-017 | step 1 (idempotent read) | (drives Gate 1) | KPI-SCR-2 |

---

## 2. Coverage matrix — `scrape_candidates.rs` (5 scenarios)

| Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|
| `scrape_candidates_each_names_its_exact_source_signal` | US-SCR-002 | J-004b (auditability — load-bearing) | **WD-53** (mapping), I-SCR-4 (source-signals non-empty) | ADR-018 | step 2 (each candidate names its signal) | **Gate 2: `candidate_names_source_signal`** | **KPI-SCR-3 (auditability)** |
| `scrape_candidates_footer_states_nothing_is_signed_until_user_signs` | US-SCR-002 | J-004b | WD-49 (human-gate footer), US-SCR-002 AC 7 | ADR-017 | step 2 (footer) | n/a | KPI-SCR-2 (human-gate affordance) |
| `scrape_candidates_all_default_to_speculative_quarter_confidence` | US-SCR-002 | J-004b | **WD-52** (0.25, never above 0.3), WD-10 (numeric-only), I-SCR-3 | ADR-018 | step 2 (confidence 0.25 speculative) | **Gate 4: `candidate_confidence_no_autoinflate` (proposal half)** | **KPI-SCR-2 (release-blocking)** |
| `scrape_candidates_collapse_multiple_signals_for_one_predicate_into_one` | US-SCR-002 | J-004b | US-SCR-002 Ex 4, I-SCR-4 | ADR-018 | step 2 (Example 4 — collapse) | n/a (drives Gate 2 shape) | KPI-SCR-3 |
| `scrape_candidates_disagreed_candidate_is_auditable_and_persists_nothing_when_unsigned` | US-SCR-002 | J-004b + J-004c | WD-53 (auditable mapping), WD-55 (nothing persisted) | ADR-017, ADR-018 | step 2 (Example 3 — reject the mapping) | n/a (drives Gates 1+2) | KPI-SCR-3 + KPI-SCR-2 |

---

## 3. Coverage matrix — `scrape_sign.rs` (9 scenarios)

| Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|
| `scrape_sign_one_candidate_signs_and_publishes_via_slice_01_pipeline` | US-SCR-003 | J-004c (human-always-signs) + J-001 (compose-sign-publish) | **WD-66** (single bridge, reuse VerbClaimAdd/Publish), I-SCR-6, I-7 ("not as truth"), I-8 (retract hint) | ADR-003, ADR-017, ADR-018 | steps 3+4 (edit + sign + publish) | **Gate 5: `scraper_reuses_slice01_publish_path`** | **KPI-SCR-1 (north star)** |
| `scrape_sign_accepting_all_defaults_signs_proposal_byte_for_byte_no_inflation` | US-SCR-003 | J-004c | **WD-52** (no auto-inflate), I-SCR-3 | ADR-018 | step 4 (Example 2 — accept defaults) | **Gate 4: `candidate_confidence_no_autoinflate` (sign half)** | **KPI-SCR-2 (release-blocking)** + KPI-SCR-5 (zero-edit baseline) |
| `scrape_sign_provenance_is_display_only_and_does_not_alter_signed_cid` | US-SCR-003 | J-004c | **WD-62** (provenance display-only), WD-58, I-SCR-7 (CID stability) | ADR-005, ADR-018 | step 4 (derived-from line) | n/a (preserves ADR-005/006 CID) | KPI-SCR-4 inherited (KPI-4 slice-01 round-trip) |
| `scrape_sign_out_of_range_index_is_rejected_before_compose` | US-SCR-003 | J-004c | US-SCR-003 AC 7 (reject before compose) | ADR-017 | step 3 (failure_modes — out of range) | n/a | n/a (UX) |
| `scrape_sign_out_of_range_confidence_reprompts_without_writing` | US-SCR-003 | J-004c | WD-52 ([0.0,1.0] constraint), WD-10 | ADR-017, ADR-018 | step 3 (failure_modes — bad confidence) | n/a | n/a (UX) |
| `scrape_sign_declining_publish_retains_local_claim_with_publish_hint` | US-SCR-003 | J-004c + J-001 | WD-66 (slice-01 two-prompt contract), US-SCR-003 AC 8 | ADR-003, ADR-017 | step 4 (Example 5 — decline publish) | (drives Gate 5 — local sign path) | KPI-SCR-1 (sign succeeds offline) |
| `scrape_sign_batch_walks_each_candidate_through_individual_compose_and_sign` | US-SCR-005 | J-004c (batch on the human-gated flow) | **WD-49** (human-gate, no batch-sign), US-SCR-005 AC 2-4 | ADR-017 | step 3 (--sign N,N,...) | (drives Gate 5 per candidate) | KPI-SCR-1 (amortized) + KPI-SCR-2 |
| `scrape_sign_batch_skip_one_candidate_does_not_abort_the_rest` | US-SCR-005 | J-004c | **Q-DELIVER-5** (skip behavior, gesture is DELIVER's), US-SCR-005 AC 5 | ADR-017 | step 3 (Example 2 — skip mid-batch) | n/a | n/a (UX) |
| `scrape_sign_batch_invalid_selection_list_is_rejected_before_compose` | US-SCR-005 | J-004c | US-SCR-005 AC 6 (reject before compose) | ADR-017 | step 3 (Example 3 — invalid list) | n/a | n/a (UX) |

---

## 4. Coverage matrix — `scrape_auth.rs` (5 scenarios)

| Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|
| `scrape_auth_authenticated_harvest_reports_budget_and_never_leaks_token` | US-SCR-004 | J-004a (real-target reach) | **WD-54/63** (optional PAT env-only; token never logged) | ADR-019 | step 1 (auth line — Example 1) | n/a | KPI-SCR-1 (cost holds for real targets) |
| `scrape_auth_unauthenticated_small_target_succeeds_within_anonymous_budget` | US-SCR-004 | J-004a | WD-54 (unauth default), US-SCR-004 AC 3 | ADR-019 | step 1 (Example 2 — anon small) | n/a | KPI-SCR-1 |
| `scrape_auth_anonymous_rate_limit_exhausted_suggests_token_no_partial_list` | US-SCR-004 | J-004a | WD-54 (remediation), US-SCR-004 AC 4 | ADR-019 §GithubError::RateLimited | step 1 (failure_modes — rate limit) | n/a | KPI-SCR-1 (remediation reach) |
| `scrape_auth_rejected_token_exits_with_401_without_echoing_value` | US-SCR-004 | J-004a | **WD-63** (token never echoed), US-SCR-004 AC 5 | ADR-019 §GithubError::TokenRejected | step 1 (failure_modes — 401) | n/a | n/a (UX / security) |
| `scrape_auth_token_never_reaches_signed_claim_or_output_on_authenticated_sign` | US-SCR-004 | J-004a + J-004c | **WD-63** (token = effect-shell credential only), WD-56 (pure core never sees it) | ADR-007, ADR-019 | step 4 (signed claim has no token) | n/a (defense-in-depth) | n/a (security) |

---

## 5. Coverage matrix — `scraper_domain.rs` (6 scenarios; layer 2)

| Test name | Story | Job | Wave-decision lock | ADR(s) | KPI link |
|---|---|---|---|---|---|
| `scraper_domain_every_candidate_names_at_least_one_source_signal_property` (`@property`) | US-SCR-002 | J-004b | I-SCR-4 (source-signals non-empty), WD-53 | ADR-018 | KPI-SCR-3 (auditability invariant) |
| `scraper_domain_every_candidate_confidence_is_the_quarter_default_property` (`@property`) | US-SCR-002 | J-004b | **WD-52** (0.25, never above 0.3), I-SCR-3, WD-10 | ADR-018 | KPI-SCR-2 (no-inflation invariant) |
| `scraper_domain_derive_candidates_is_deterministic_property` (`@property`) | US-SCR-002 | J-004b | WD-53 (deterministic mapping), component-boundaries §scraper-domain property 1 | ADR-007, ADR-018 | n/a (correctness invariant) |
| `scraper_domain_multiple_signals_for_one_predicate_collapse_into_one_candidate` | US-SCR-002 | J-004b | US-SCR-002 Ex 4, I-SCR-4 | ADR-018 | KPI-SCR-3 |
| `scraper_domain_zero_matching_signals_derive_an_empty_candidate_list` | US-SCR-002 | J-004b | US-SCR-002 Ex 2 | ADR-018 | n/a (edge — not an error) |
| `scraper_domain_embedded_mapping_matches_jobs_yaml_ssot` | US-SCR-006 | J-004b (infra → J-004) | **WD-53** (SSOT), **WD-67** (embedded + mapping_matches_ssot), I-SCR-5 | ADR-018 | n/a (drift guardrail) |

---

## 6. Story coverage (every story has ≥ 1 acceptance test)

| Story | Title | Test count | Test names |
|---|---|---|---|
| US-SCR-001 | Harvest a public GitHub target's signals | 8 | SG-1, SG-2, SG-3, SG-4, SG-5, SG-6, SG-8, SG-9 (+ SA-1..4 touch the harvest path) |
| US-SCR-002 | Derive auditable candidate claims from signals | 12 | SG-1, SG-7 + SC-1..5 + SD-1..6 (layer-2 properties + SSOT) |
| US-SCR-003 | Review, edit, and sign a candidate via slice-01 | 6 | SS-1, SS-2, SS-3, SS-4, SS-5, SS-6 |
| US-SCR-004 | Use an optional PAT for higher rate limits | 5 | SA-1, SA-2, SA-3, SA-4, SA-5 |
| US-SCR-005 | Select and sign several candidates in one pass | 3 | SS-7, SS-8, SS-9 |
| US-SCR-006 | Bootstrap GithubPort + adapter-github + scraper-domain (`@infrastructure`) | 1 explicit + implicit | SD-6 (mapping SSOT / `mapping_matches_ssot`); the new crates + `GithubPort` + probe are implicitly exercised by every subprocess scenario that wires `adapter-github` + `scraper-domain` through the CLI. The `xtask check-arch`/`check-probes` infra UAT scenarios (US-SCR-006 gherkin) are DELIVER's xtask-layer tests (out of DISTILL scope per DD-SCR-7, symmetric with slice-03 US-FED-006 migration treatment) |
| (Cross-cutting: J-004c human-gate) | n/a | 7 | SG-1, SG-8, SG-9, SC-3, SC-5, SS-2, SS-7 (load-bearing) |
| (Cross-cutting: public-data-only) | n/a | 2 | SG-5 (load-bearing release-gate), SG-2 (banner) |

Every story has at least 1 scenario; the most-tested story is US-SCR-002
(12 scenarios) because candidate derivation is the auditability load-bearing
beat AND it spans both the layer-3 rendering (SC-*) and the layer-2 pure
properties (SD-*).

---

## 7. Job coverage

| Job / sub-job | In slice-02? | Test count | Scenarios |
|---|---|---|---|
| J-004 Evaluate a contributor's body of work through a philosophy lens | YES — primary (walking-skeleton job for this feature per WD-48) | 34 (all slice-02 scenarios) | every SG / SC / SS / SA / SD scenario |
| J-004a Harvest a contributor's/repo's public GitHub signals | YES — LOAD-BEARING | 13 | SG-1..6, SG-8, SG-9 + SA-1..5 |
| J-004b Derive editable candidate claims from signals | YES — LOAD-BEARING | 13 | SG-1, SG-7 + SC-1..5 + SD-1..6 |
| J-004c The human always signs — the scraper never asserts | YES — LOAD-BEARING | 9 | SG-8, SG-9, SC-5 + SS-1..9 + SA-5 |
| J-001 Author a signed philosophical claim | PARTIAL — inherited (SS-1/SS-6 reuse VerbClaimAdd/VerbClaimPublish via WD-66 / I-SCR-6) | 2 explicit | SS-1, SS-6 |
| J-002 Explore the philosophy graph | PARTIAL — a signed-from-scraper claim becomes queryable via slice-01 graph query (story-map demo gate) | 0 explicit (out of slice-02 scope; the read-back is slice-01's surface) | n/a |

---

## 8. Wave-decision coverage (DISCUSS WD-46..WD-58 + DESIGN WD-59..WD-68)

Every locked DISCUSS WD-N + DESIGN WD-N + OD-SCR-N decision that touches
user-observable behavior maps to at least one acceptance test:

### DISCUSS

| Wave decision | Coverage |
|---|---|
| WD-46 (slice-02 = SIBLING feature; walking-skeleton feature) | This entire DISTILL wave's directory + SG-1/SS-1 are the walking skeleton |
| WD-47 (P-002 primary, P-001 secondary in contributor-evaluator hat) | All scenarios reference P-002 (Maria/Tobias/Aanya) per the user-stories fixtures |
| WD-48 (J-004 = walking-skeleton job for slice-02) | All scenarios trace to J-004 or sub-jobs (§7) |
| WD-49 (human-gate: scraper proposes, human signs) | SG-1, SG-8, SG-9 (nothing persisted), SC-5, SS-7 (each individually signed, no batch-sign affordance) |
| WD-50 (sugar verb `scrape github [--sign]`) | SG-1 (verb shape), SS-1 (--sign continuation) |
| WD-51 (public-data-only; private/non-existent refused) | SG-4 (404), SG-5 (private — load-bearing), SG-2 (banner) |
| WD-52 (confidence 0.25, never above 0.3; only human raises) | SC-3, SS-2, SD-2 (`@property`) |
| WD-53 (small auditable signal->predicate mapping SSOT) | SC-1, SC-4, SD-1, SD-4, SD-6 |
| WD-54 (optional PAT; works unauth for small targets; token never leaks) | SA-1..5 |
| WD-55 (nothing persisted unsigned; zero rows/PDS without --sign) | SG-1, SG-8, SG-9, SC-5 |
| WD-56 (pure/effect split — scraper-domain pure, adapter-github effect) | SD-1..6 (pure core direct); SA-5 (token never reaches pure core); the @infrastructure xtask checks are DELIVER's |
| WD-57 (two new production crates) | DISTILL constraint — slice-02 adds `FakeGithub` + `fixtures_github` to test-support; the two production crates are DELIVER's bootstrap |
| WD-58 (provenance informational, never alters confidence/federation; storage = DESIGN's call) | SS-3 (display-only, CID unchanged) |
| OD-SCR-1 (default = sugar verb) | SG-1/SS-1 bind this default |
| OD-SCR-2 (default = env-var PAT only) | SA-1..5 bind this default |
| OD-SCR-3 (default = display-only provenance) | SS-3 binds this default |
| OD-SCR-4 (default = bounded aggregate for user targets) | SG-3 binds this default |

### DESIGN

| Wave decision | Coverage |
|---|---|
| WD-59 (two-crate additive extension; count 8->10) | DISTILL constraint — the two production crates are referenced by step-defs; DELIVER bootstraps them |
| WD-60 (sugar verb LOCKED, ADR-017) | SG-1, SS-1 |
| WD-61 (GithubPort is a NEW port) | All SG/SC/SA scenarios exercise it via `FakeGithub`; DD-SCR-2 makes the double a separate type |
| WD-62 (provenance display-only LOCKED, ADR-018) | SS-3 (load-bearing — provenance NOT in signed payload; CID unchanged) |
| WD-63 (PAT env-var only LOCKED, ADR-019) | SA-1, SA-4, SA-5 |
| WD-64 (bounded aggregate for user targets) | SG-3 |
| WD-65 (scraper-domain in check-arch pure set; YAML parser whitelisted) | DELIVER's xtask work; SD-6 asserts the mapping-SSOT contract the whitelist supports |
| WD-66 (single bridge CandidatePrefill -> VerbClaimAdd/Publish; no parallel path) | SS-1 (load-bearing `scraper_reuses_slice01_publish_path`), SS-6, SS-7 |
| WD-67 (mapping embedded from jobs.yaml SSOT; `mapping_matches_ssot`) | SD-6 |
| WD-68 (ADR-017/018/019 accepted) | INTENTIONALLY UNTESTED — process decision |
| Q-DELIVER-5 (batch-skip gesture deferred) | SS-8 (asserts behavior, not keystroke) |

---

## 9. Integration gate coverage (shared-artifacts-registry.md)

| Gate | Where asserted | Test name(s) | Mandatory for KPI |
|---|---|---|---|
| 1. `scraper_never_persists_unsigned` | layer 3 subprocess | SG-1 (load-bearing), SG-8, SG-9 | **KPI-SCR-2 (release-blocking)** |
| 2. `candidate_names_source_signal` | layer 3 subprocess + layer 2 (SD-1 `@property`) | SC-1 (load-bearing), SC-5, SD-1 | **KPI-SCR-3** |
| 3. `scraper_only_reads_public_data` | layer 3 subprocess | SG-5 (load-bearing; `seen_paths` allowlist) | **KPI-SCR-4 (release-blocking)** |
| 4. `candidate_confidence_no_autoinflate` | layer 3 subprocess (both halves) + layer 2 (SD-2 `@property`) | SC-3 (proposal), SS-2 (sign — load-bearing), SD-2 | **KPI-SCR-2 (release-blocking)** |
| 5. `scraper_reuses_slice01_publish_path` | layer 3 subprocess | SS-1 (load-bearing), SS-7 | preserves ADR-003 invariant |

All five gates have at least one acceptance test. Gates 1, 3, and 4 are the
KPI release-gates (KPI-SCR-2 + KPI-SCR-4 are unshippable guardrails per
outcome-kpis.md §Disprovers). The DEVOPS public-endpoint allowlist Pact
contract extends gate 3 but is out of DISTILL scope per DD-SCR-11.

---

## 10. KPI coverage

| KPI | Description | Acceptance coverage | Type |
|---|---|---|---|
| KPI-SCR-1 | Cost-to-first-signed-claim under 2 min (north star) | SS-1 (path exists), SS-7 (amortized); latency = telemetry (`scrape.to_sign.duration_seconds`), NOT asserted at acceptance — per outcome-kpis.md measurement plan | Leading (Outcome — north star) |
| KPI-SCR-2 | Human-gate: zero unsigned persistence / auto-publish | SG-1, SG-8, SG-9 (`scraper_never_persists_unsigned`); SC-3, SS-2, SD-2 (`candidate_confidence_no_autoinflate`) | Leading (Guardrail — release-blocking) |
| KPI-SCR-3 | Auditability: every candidate names its source signal | SC-1 (load-bearing), SC-5, SD-1 (`@property`) | Leading (Outcome) |
| KPI-SCR-4 | Public-data-only: zero private endpoint calls | SG-5 (load-bearing release-gate; `seen_paths` allowlist) | Leading (Guardrail / Trust — release-blocking) |
| KPI-SCR-5 | Edit rate >=50% of signed-from-scraper claims | SS-2 (the zero-edit byte-for-byte baseline the edit-rate telemetry measures against); edit rate itself = author-side telemetry, NOT an acceptance assertion | Leading (Outcome) |

---

## 11. Resolved open-behavior → scenario mapping (consolidated)

Slice-02 has NO `# DISTILL: confirm` flags (the
`gherkin-scenarios-expanded.md` that would have carried them was never
materialized; DESIGN resolved every open behavior up-front). The open
behaviors and their bindings:

| Open behavior (would-have-been a `# confirm` flag) | DESIGN resolution | Scenario binding |
|---|---|---|
| Verb shape (sugar vs flag) | **WD-60 — sugar verb (ADR-017)** | SG-1, SS-1 |
| Provenance storage (signed vs display) | **WD-62 — display-only (ADR-018)** | SS-3 |
| PAT surface (env vs env+config) | **WD-63 — env-var only (ADR-019)** | SA-1, SA-4, SA-5 |
| Batch-skip gesture | **Q-DELIVER-5 — behavior asserted, keystroke DELIVER's** | SS-8 |
| User-target depth | **WD-64 — bounded aggregate** | SG-3 |

---

## 12. Cross-feature inheritance from openlore-foundation + openlore-federated-read

Slice-02 INHERITS without modification:

| Inherited | Status in slice-02 |
|---|---|
| WS-1..WS-17 (slice-01) + slice-03 acceptance scenarios | UNCHANGED |
| `tests/acceptance/support/mod.rs` | EXTENDED with scrape runners + slice-02 assertion helpers (DD-SCR-5) |
| `crates/test-support/src/{lib,fake_pds,fake_peer_pds,identity,fixtures,fixtures_peer}.rs` | EXTENDED with `fake_github.rs` + `fixtures_github.rs` |
| Slice-01 DD-1..DD-13 + slice-03 DD-FED-1..DD-FED-14 | All still binding (see wave-decisions.md §"Inheritance") |
| Slice-01 ADR-001..012 + slice-03 ADR-013..016 | All still binding; slice-02 adds ADR-017..019 |
| Literal "not as truth" (I-7) + retract hint (I-8) | Inherited UNCHANGED in the sign path (SS-1 asserts both) |
| Single publish path (VerbClaimPublish; ADR-003 + WD-22) | Reused by SS-1/SS-6/SS-7 (`scraper_reuses_slice01_publish_path`); no parallel path |
| KPI-4 (slice-01 zero-silent-normalization / round-trip identity) | Inherited UNCHANGED — a signed-from-scraper claim is byte-identical in shape to a hand-authored one (SS-3 CID stability) |

---

## 13. Changelog

- 2026-05-28 — Quinn — initial traceability matrix for slice-02. All 34
  scenarios mapped to story / job / sub-job / wave-decision / ADR /
  integration gate / KPI. Zero un-traced scenarios. No `# DISTILL: confirm`
  flags (all open behaviors resolved by DESIGN WD-60/62/63 + WD-64 +
  Q-DELIVER-5).
