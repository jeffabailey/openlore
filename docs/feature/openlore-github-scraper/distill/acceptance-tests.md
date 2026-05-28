# Acceptance Test Design — openlore-github-scraper (slice-02)

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-github-scraper
- **Slice**: slice-02-github-scraper (sibling feature; sibling-feature pattern per WD-46)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (per ADR-007)
- **Inherits**: slice-01 DISTILL (`docs/feature/openlore-foundation/distill/`) + slice-03 DISTILL (`docs/feature/openlore-federated-read/distill/`) + slice-02 DISCUSS WD-46..WD-58 + slice-02 DESIGN WD-59..WD-68 + ADR-017..019
- **Language**: Rust (per ADR-009)
- **Test framework**: same as slice-01/03 — Rust std `#[test]` (per DD-SCR-1)

This document is the human-readable map over the executable test skeletons
in `tests/acceptance/scrape_github.rs + scrape_candidates.rs + scrape_sign.rs
+ scrape_auth.rs + scraper_domain.rs`. The `.rs` files are the SSOT for
executable scenarios.

---

## 1. Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** between DISCUSS WD-46..WD-58
(+ OD-SCR-1..4 accepted at default) and DESIGN WD-59..WD-68. See
`wave-decisions.md §Wave-Decision Reconciliation result` for the full
DISCUSS-lock × DESIGN check table. DESIGN's WD-60 (sugar verb), WD-62
(provenance display-only), WD-63 (PAT env-only), and Q-DELIVER-5 (batch-skip
gesture) RESOLVE every open behavior the DISCUSS-flagged
`gherkin-scenarios-expanded.md` would have carried as `# DISTILL: confirm` —
and that file was never materialized, so DISTILL has **zero unresolved
ambiguity** at scenario-write time.

DEVOPS-wave artifacts are absent (`docs/feature/openlore-github-scraper/devops/`
is an empty directory). Per `nw-distill` Graceful Degradation matrix: WARN +
apply default environment matrix (clean | with-pre-commit | with-stale-config);
DO NOT block. Slice-02's acceptance scenarios do not depend on a
per-environment fixture cross-product (the test seam is subprocess +
`FakeGithub` + slice-01 `FakePds`/`FakeIdentity` + tempfile HOME).

---

## 2. Resolved open behaviors (no `# DISTILL: confirm` flags)

Unlike slice-03 (which inherited 7 `# DISTILL: confirm` flags from its
`gherkin-scenarios-expanded.md`), slice-02 has NONE — that expansion file
was flagged "to be produced" in the DISCUSS feature-delta but was not
materialized, and DESIGN resolved every open behavior up-front:

| Open behavior | DESIGN resolution | Bound scenario(s) |
|---|---|---|
| Verb shape (sugar `scrape github` vs `claim add --from-github`) | **WD-60 / ADR-017** sugar verb `scrape github <target> [--sign N[,N...]]` | SG-1, SS-1 (verb shape) |
| `derived-from` provenance storage (signed-payload field vs display-only) | **WD-62 / ADR-018** display-only; NO Lexicon change; CID unchanged | SS-3 (display-only + CID stability) |
| PAT surface (env-var vs env+config) | **WD-63 / ADR-019** `GITHUB_TOKEN` env-var only | SA-1, SA-4, SA-5 |
| Batch-skip gesture (Ctrl-C vs explicit "skip") | **Q-DELIVER-5** behavior asserted, keystroke is DELIVER's | SS-8 (skip behavior) |

The anxiety force the DISCUSS ask-intelligent menu fired on (surveillance
fear, assertion fear, over-confidence fear) is covered by the human-gate +
public-data-only + no-inflation scenarios directly: surveillance-fear ->
SG-5 (private refused, public-only allowlist), assertion-fear -> SG-1/SG-8
(nothing persisted unsigned) + SS-2 (no auto-inflation), over-confidence-fear
-> SC-3 + SD-2 (0.25 default, never above 0.3).

---

## 3. Scope and shape

Same hexagonal port-to-port discipline as slice-01/03: every subprocess
acceptance test enters through the CLI driving adapter via `assert_cmd`,
exercises the real `claim-domain` + `lexicon` + `adapter-duckdb` + the new
`scraper-domain` (PURE) stack, and fakes ONLY the external/non-deterministic
boundaries (GitHub via the new `FakeGithub`; the user's own PDS via the
slice-01 `FakePds`; identity via `FakeIdentity`).

### Layer placement (per nw-test-design-mandates Mandate 9 + DD-SCR-6)

| Layer | Test file(s) | Real adapters | Test mode |
|---|---|---|---|
| Walking-Skeleton subprocess (layer 5) | `scrape_github.rs` SG-1 (scrape->propose half) + `scrape_sign.rs` SS-1 (sign half) | CLI binary + DuckDB + FS; GitHub + PDS + Identity doubles | example-only (Mandate 11) |
| Subprocess / FS acceptance (layer 3) | `scrape_github.rs` (rest), `scrape_candidates.rs` (5), `scrape_sign.rs` (rest), `scrape_auth.rs` (5) | CLI binary + DuckDB + FS; GitHub + PDS + Identity doubles | example-only (Mandate 11) |
| In-memory acceptance (layer 2) | `scraper_domain.rs` (6; 3 `@property`) | None — pure core directly | example + `@property` proptest (Mandate 9 layer-2 PBT full) |

Layer 1 (exhaustive pure-core unit tests for `derive_candidates` per-arm,
`load_mapping` malformed-entry errors, boundary parsing) is OUT OF DISTILL
SCOPE — DELIVER's inner TDD loop (DD-SCR-7).

### What is mocked, what is real (slice-02 additions to slice-01/03 table)

| Component | Treatment | Why |
|---|---|---|
| GitHub public REST/GraphQL API | NEW FAKE: `openlore_test_support::FakeGithub` | GitHub is a wholly different external system from ATProto (WD-61). Public-data-only + human-gate are STRUCTURAL: the double has no private surface and holds no storage/identity/pds reference (DD-SCR-12). Postures (`for_public_repo`, `for_private_target`, `rate_limited_anon`, `with_rejected_token`, …) are constructor-pinned (DD-SCR-3). |
| GitHub PAT auth (`GITHUB_TOKEN`) | FAKE via `FakeGithub::authenticated` + `with_rejected_token` postures; token injected via `GITHUB_TOKEN` env (WD-63) | Token-never-leaks asserted both ways: `FakeGithub::saw_token(t)` confirms the production code SENT it; `assert_token_value_absent` confirms it is ABSENT from output (DD-SCR-12). |
| `scraper-domain` pure derivation | REAL pure core (layer-2 direct in `scraper_domain.rs`) | Mandate 6 — the load-bearing auditability + no-inflation + determinism invariants run against the real derivation, generatively (`@property`). |
| The user's OWN `claims` table + `claims/<cid>.json` tree | REAL DuckDB + REAL filesystem under `tempfile::TempDir` | The human-gate-at-storage proof (`scraper_never_persists_unsigned`) needs the real store to assert ZERO rows / ZERO files after a no-`--sign` scrape. |
| Sign-from-scraper publish | REAL slice-01 `VerbClaimAdd`/`VerbClaimPublish` via the CLI; `FakePds` captures the published record | `scraper_reuses_slice01_publish_path` (I-SCR-6) asserts exactly ONE record via the slice-01 path — no parallel path. |
| `derived-from` provenance | display-only; asserted ABSENT from the signed payload | WD-62 / ADR-018; SS-3 proves CID stability vs a hand-authored claim. |

### Test file rationale + placement (DD-SCR-4)

Flat layout under `tests/acceptance/` matching slice-01/03. Five new files:

```
tests/acceptance/
  walking_skeleton.rs            # slice-01, unchanged
  lexicon_conformance.rs         # slice-01, unchanged
  federation_roundtrip.rs        # slice-01, unchanged
  peer_subscribe.rs              # slice-03, unchanged
  peer_pull.rs                   # slice-03, unchanged
  counter_claim.rs               # slice-03, unchanged
  federated_query.rs             # slice-03, unchanged
  lexicon_counter_claim.rs       # slice-03, unchanged
  scrape_github.rs               # slice-02 NEW (9 scenarios; US-SCR-001 harvest + US-SCR-002 happy + WS)
  scrape_candidates.rs           # slice-02 NEW (5 scenarios; US-SCR-002 candidate rendering + auditability)
  scrape_sign.rs                 # slice-02 NEW (9 scenarios; US-SCR-003 single sign + US-SCR-005 batch)
  scrape_auth.rs                 # slice-02 NEW (5 scenarios; US-SCR-004 optional PAT)
  scraper_domain.rs              # slice-02 NEW (6 scenarios; 3 @property — layer 2)
  support/
    mod.rs                       # EXTENDED — adds scrape runners + slice-02 assertion helpers
```

Rationale: preserves `cargo test --test <file>` ergonomics; five new files
clearly labeled by domain. The shared `support/mod.rs` + the
`openlore-test-support` crate are EXTENDED, not duplicated (DD-SCR-5). See
DD-SCR-4 + DD-SCR-14 for the nested-layout rejection + Cargo.toml
registration deferral.

---

## 4. Acceptance test inventory

Per Mandate 3 (User Journey Completeness) every test exercises a complete
user journey from observable trigger through observable outcome.

### `tests/acceptance/scrape_github.rs` — 9 scenarios

Stories: US-SCR-001 (harvest), US-SCR-002 (derive — happy render).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| SG-1 | `scrape_github_harvests_public_repo_proposes_candidates_and_persists_nothing` | US-SCR-001 + US-SCR-002 | WS / happy | `@walking_skeleton @driving_port @driving_adapter @real-io @kpi-scr-2 @happy` (drives gate `scraper_never_persists_unsigned`) |
| SG-2 | `scrape_github_prints_public_data_banner_before_any_harvest` | US-SCR-001 | happy | `@happy` |
| SG-3 | `scrape_github_resolves_user_target_and_harvests_bounded_aggregate` | US-SCR-001 | edge | `@wd-64 @edge` |
| SG-4 | `scrape_github_rejects_nonexistent_target_with_zero_candidates` | US-SCR-001 | error | `@error` |
| SG-5 | `scrape_github_refuses_private_target_and_calls_no_private_endpoint` | US-SCR-001 | error | `@kpi-scr-4 @error @release-gate` (drives gate `scraper_only_reads_public_data`) |
| SG-6 | `scrape_github_offline_exits_with_requires_network_and_no_partial_list` | US-SCR-001 | error | `@error` |
| SG-7 | `scrape_github_with_no_matching_signals_proposes_nothing_and_exits_zero` | US-SCR-002 | edge | `@edge` |
| SG-8 | `scrape_github_without_sign_makes_zero_pds_writes` | US-SCR-001 + US-SCR-002 | edge | `@kpi-scr-2 @edge` (reinforces `scraper_never_persists_unsigned`) |
| SG-9 | `scrape_github_is_a_pure_read_persisting_nothing_across_repeated_runs` | US-SCR-001 | edge | `@kpi-scr-2 @edge` |

Error-path ratio: 3/9 = 33% (SG-4/5/6); plus 3 edge (SG-3/7/8/9). The
read-half of the journey is success-shaped; the sad-path bulk lives in
`scrape_sign.rs` + `scrape_auth.rs`.

### `tests/acceptance/scrape_candidates.rs` — 5 scenarios

Story: US-SCR-002 (derive auditable candidate claims).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| SC-1 | `scrape_candidates_each_names_its_exact_source_signal` | US-SCR-002 | happy | `@kpi-scr-3 @happy @release-gate` (drives gate `candidate_names_source_signal`) |
| SC-2 | `scrape_candidates_footer_states_nothing_is_signed_until_user_signs` | US-SCR-002 | happy | `@happy` |
| SC-3 | `scrape_candidates_all_default_to_speculative_quarter_confidence` | US-SCR-002 | happy | `@wd-52 @kpi-scr-2 @happy` (drives `candidate_confidence_no_autoinflate`, proposal half) |
| SC-4 | `scrape_candidates_collapse_multiple_signals_for_one_predicate_into_one` | US-SCR-002 | edge | `@i-scr-4 @edge` |
| SC-5 | `scrape_candidates_disagreed_candidate_is_auditable_and_persists_nothing_when_unsigned` | US-SCR-002 | edge | `@kpi-scr-3 @edge` |

### `tests/acceptance/scrape_sign.rs` — 9 scenarios

Stories: US-SCR-003 (single sign), US-SCR-005 (batch).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| SS-1 | `scrape_sign_one_candidate_signs_and_publishes_via_slice_01_pipeline` | US-SCR-003 | WS / happy | `@walking_skeleton @driving_port @driving_adapter @real-io @kpi-scr-1 @i-scr-6 @happy @release-gate` (drives gate `scraper_reuses_slice01_publish_path`) |
| SS-2 | `scrape_sign_accepting_all_defaults_signs_proposal_byte_for_byte_no_inflation` | US-SCR-003 | edge | `@wd-52 @kpi-scr-2 @release-gate @edge` (drives `candidate_confidence_no_autoinflate`, sign half) |
| SS-3 | `scrape_sign_provenance_is_display_only_and_does_not_alter_signed_cid` | US-SCR-003 | happy | `@wd-62 @i-scr-7 @cid-stability @happy` |
| SS-4 | `scrape_sign_out_of_range_index_is_rejected_before_compose` | US-SCR-003 | error | `@error` |
| SS-5 | `scrape_sign_out_of_range_confidence_reprompts_without_writing` | US-SCR-003 | error | `@error` |
| SS-6 | `scrape_sign_declining_publish_retains_local_claim_with_publish_hint` | US-SCR-003 | edge | `@i-scr-6 @edge` |
| SS-7 | `scrape_sign_batch_walks_each_candidate_through_individual_compose_and_sign` | US-SCR-005 | happy | `@kpi-scr-1 @kpi-scr-2 @happy` |
| SS-8 | `scrape_sign_batch_skip_one_candidate_does_not_abort_the_rest` | US-SCR-005 | edge | `@edge` |
| SS-9 | `scrape_sign_batch_invalid_selection_list_is_rejected_before_compose` | US-SCR-005 | error | `@error` |

Error-path ratio: 4/9 = 44% (SS-4/5/9 + the sad branch of SS-8).

### `tests/acceptance/scrape_auth.rs` — 5 scenarios

Story: US-SCR-004 (optional PAT for higher rate limits).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| SA-1 | `scrape_auth_authenticated_harvest_reports_budget_and_never_leaks_token` | US-SCR-004 | happy | `@wd-63 @happy` |
| SA-2 | `scrape_auth_unauthenticated_small_target_succeeds_within_anonymous_budget` | US-SCR-004 | edge | `@wd-63 @edge` |
| SA-3 | `scrape_auth_anonymous_rate_limit_exhausted_suggests_token_no_partial_list` | US-SCR-004 | error | `@error` |
| SA-4 | `scrape_auth_rejected_token_exits_with_401_without_echoing_value` | US-SCR-004 | error | `@wd-63 @error` |
| SA-5 | `scrape_auth_token_never_reaches_signed_claim_or_output_on_authenticated_sign` | US-SCR-004 | edge | `@wd-63 @edge` |

Error-path ratio: 2/5 = 40%.

### `tests/acceptance/scraper_domain.rs` — 6 scenarios (3 `@property`; layer 2)

Stories: US-SCR-002 (pure derivation), US-SCR-006 (mapping SSOT).

| # | Test name | Source | Type | Tag(s) |
|---|---|---|---|---|
| SD-1 | `scraper_domain_every_candidate_names_at_least_one_source_signal_property` | US-SCR-002 + I-SCR-4 | `@property` | `@property @i-scr-4 @kpi-scr-3` |
| SD-2 | `scraper_domain_every_candidate_confidence_is_the_quarter_default_property` | US-SCR-002 + WD-52 | `@property` | `@property @wd-52 @i-scr-3 @kpi-scr-2` |
| SD-3 | `scraper_domain_derive_candidates_is_deterministic_property` | US-SCR-002 | `@property` | `@property` |
| SD-4 | `scraper_domain_multiple_signals_for_one_predicate_collapse_into_one_candidate` | US-SCR-002 Ex 4 + I-SCR-4 | example | `@i-scr-4` |
| SD-5 | `scraper_domain_zero_matching_signals_derive_an_empty_candidate_list` | US-SCR-002 Ex 2 | example / edge | `@edge` |
| SD-6 | `scraper_domain_embedded_mapping_matches_jobs_yaml_ssot` | US-SCR-006 + WD-53 + WD-67 | example | `@wd-53 @wd-67 @i-scr-5` |

### Total slice-02 scenarios across the wave

9 (SG) + 5 (SC) + 9 (SS) + 5 (SA) + 6 (SD) = **34 scenarios** authored, all
RED-ready as `todo!()` scaffolds. Cross-file error-path ratio: 9/34 = 26.5%
across all files; but the sad-path-admitting surfaces are healthy
(`scrape_sign.rs` 44%, `scrape_auth.rs` 40%, `scrape_github.rs` 33% +
3 edges). The read-only candidate-rendering (`scrape_candidates.rs`) +
pure-derivation (`scraper_domain.rs`) files are success/property-shaped by
nature (the sad paths for derivation are SD-5 empty-list + the layer-3 SG-4/5/6
target-refusal scenarios). This mirrors the slice-03 aggregate (31.4%); the
human-gate + public-data-only guardrails are over-tested where it matters
(SG-5 release-gate, SG-8/9 persist-nothing, SS-2 no-inflation release-gate).

---

## 5. Driving Adapter coverage (Mandate 1 + RCA P1)

Every NEW or EXTENDED CLI verb/flag in ADR-017 covered by at least one
subprocess scenario:

| Verb / flag | Scenario coverage |
|---|---|
| `openlore scrape github <target>` (NEW) | SG-1 (WS happy), SG-2 (banner), SG-3 (user target), SG-4 (404), SG-5 (private), SG-6 (offline), SG-7 (no-match), SG-8/9 (persist-nothing) |
| `openlore scrape github <target> --sign <N>` (NEW) | SS-1 (WS sign happy), SS-2 (no-inflate), SS-3 (provenance display-only), SS-4 (out-of-range), SS-5 (bad confidence), SS-6 (decline publish) |
| `openlore scrape github <target> --sign <N,N,...>` (NEW — batch) | SS-7 (batch happy), SS-8 (skip mid-batch), SS-9 (invalid list) |
| `GITHUB_TOKEN` env-var (NEW — WD-63) | SA-1 (authenticated), SA-3 (anon rate limit), SA-4 (rejected token), SA-5 (token never leaks) |

Zero uncovered NEW entry points. The candidate-list render (US-SCR-002) is
reached automatically after every `scrape github` invocation (no separate
command), covered by SG-1/SG-7 + all SC-* + SD-* (the pure derivation
behind it).

---

## 6. Driven adapter coverage (Mandate 6)

| Driven adapter | Real-I/O scenario? | Tag |
|---|---|---|
| `adapter-github` (NEW `GithubPort`: resolve_target + harvest_repo + harvest_user) | PARTIAL — every SG-*/SC-*/SA-* scenario hits the surface via `FakeGithub` (in-process HTTP). Real public-GitHub contract test = DEVOPS Pact suite (public-endpoint allowlist, KPI-SCR-4) per outcome-kpis.md DEVOPS handoff | `@fake-github` |
| `scraper-domain` (NEW PURE: derive_candidates + load_mapping) | YES — SD-1..SD-6 exercise the real pure core directly (layer 2) | `@real-io` (pure-core direct) |
| `adapter-duckdb` (StoragePort: write_signed_claim + the user's own `claims` table) | YES — SG-1/SG-8/SG-9 assert ZERO rows (persist-nothing); SS-1/SS-2/SS-6 assert the signed claim lands in the real store | `@real-io` |
| `adapter-atproto-pds` (PdsPort publish, reused slice-01) | YES — SS-1/SS-7 assert publish via `FakePds`; SG-8/SS-6 assert ZERO publish | `@real-io` (via FakePds capture) |
| `adapter-atproto-did` (IdentityPort sign, reused slice-01) | YES — SS-1/SS-2/SS-7 sign via the real slice-01 path; `FakeIdentity` provides the keypair | `@real-io` |
| Filesystem `claims/<cid>.json` tree | YES — SG-1/SG-8/SG-9 assert zero artifacts; SS-1/SS-3/SS-6 assert the signed artifact + SS-3 asserts NO provenance key | `@real-io` |

The PARTIAL coverage on `adapter-github` is structural to slice-02's
acceptance scope: per DESIGN §Annotation-for-platform-architect +
outcome-kpis.md DEVOPS handoff, **the public-endpoint allowlist Pact contract
test against the real GitHub API is DEVOPS's deliverable**, not DISTILL's.
DISTILL ships the acceptance shape + the in-process `seen_paths` allowlist
assertion (SG-5); DEVOPS extends the slice-01 Pact suite with consumer-driven
contracts for the GitHub read paths + the no-private-endpoint assertion +
the rate-limit/rejected-token fixtures.

---

## 7. Integration gates coverage (shared-artifacts-registry.md)

Per DISCUSS shared-artifacts-registry.md §"Integration gates (handed to DISTILL)":

| Gate | Where asserted | Test name(s) | Mandatory for KPI |
|---|---|---|---|
| 1. `scraper_never_persists_unsigned` | layer 3 subprocess | SG-1 (load-bearing), SG-8 (PDS half), SG-9 (repeated-run) | **KPI-SCR-2 (release-blocking guardrail)** |
| 2. `candidate_names_source_signal` | layer 3 subprocess + layer 2 (SD-1 `@property`) | SC-1 (load-bearing), SC-5 + SD-1 | **KPI-SCR-3 (auditability)** |
| 3. `scraper_only_reads_public_data` | layer 3 subprocess | SG-5 (load-bearing; `seen_paths` allowlist) | **KPI-SCR-4 (release-blocking guardrail)** |
| 4. `candidate_confidence_no_autoinflate` | layer 3 subprocess (both halves) + layer 2 (SD-2 `@property`) | SC-3 (proposal half), SS-2 (sign half, load-bearing), SD-2 | **KPI-SCR-2 (release-blocking guardrail)** |
| 5. `scraper_reuses_slice01_publish_path` | layer 3 subprocess | SS-1 (load-bearing), SS-7 (batch) | preserves ADR-003 invariant |

All five gates have at least one acceptance test. Gates 1, 3, and 4 are the
KPI release-gates (KPI-SCR-2 + KPI-SCR-4 are the unshippable guardrails per
outcome-kpis.md §Disprovers). The DEVOPS public-endpoint Pact contract
extends gate 3 but is out of DISTILL scope per DD-SCR-11 + DESIGN handoff.

---

## 8. KPI coverage

| KPI | Description | Acceptance coverage | Type |
|---|---|---|---|
| KPI-SCR-1 | Cost-to-first-signed-claim under 2 min (north star) | SS-1 (the value-capture path exists), SS-7 (amortized batch); latency measured via `scrape.to_sign.duration_seconds` telemetry, NOT asserted at the acceptance boundary (outcome-kpis.md measurement plan) | Leading (Outcome — north star) |
| KPI-SCR-2 | Human-gate: zero unsigned persistence / auto-publish (guardrail) | SG-1 + SG-8 + SG-9 (`scraper_never_persists_unsigned`); SC-3 + SS-2 + SD-2 (`candidate_confidence_no_autoinflate`) | Leading (Guardrail — release-blocking) |
| KPI-SCR-3 | Auditability: every candidate names its source signal | SC-1 (load-bearing), SC-5, SD-1 (`@property`) | Leading (Outcome) |
| KPI-SCR-4 | Public-data-only: zero private endpoint calls (guardrail) | SG-5 (load-bearing release-gate; `seen_paths` allowlist) | Leading (Guardrail / Trust — release-blocking) |
| KPI-SCR-5 | Edit rate >=50% of signed-from-scraper claims | SS-2 (proves the zero-edit-sign byte-for-byte contract the edit-rate telemetry measures against); edit rate itself is author-side telemetry, NOT an acceptance assertion | Leading (Outcome) |

KPI-SCR-1 (latency) and KPI-SCR-5 (edit rate) are the only KPIs without a
hard assertion at this layer — by design, per outcome-kpis.md: both are
collected via author-side tracing telemetry in production, not asserted at
the acceptance-test boundary. SS-1 + SS-2 establish the contracts those
telemetry signals measure against.

---

## 9. Three Pillars compliance

| Pillar | How DISTILL satisfied it |
|---|---|
| 1 — Domain language | Scenario titles use `scrape`, `harvest`, `signal`, `candidate`, `sign`, `publish`, `confidence`, `speculative`, `public data`, `source signal`, `provenance`, `target`, `rate budget`, `token`. Zero technical jargon: NO `JSON`, `HTTP`, `database`, `endpoint`, `schema`, `SQL`, `REST`, `GraphQL` in any scenario title or step-method name. (The word `endpoint` appears in `scrape_github.rs` SG-5's comment + the `seen_paths` allowlist helper because the no-surveillance contract IS about which network endpoints are called — but the user-facing scenario asserts "no private data is read", and the helper name `assert_only_public_endpoints_called` is the auditability vocabulary, not implementation jargon.) |
| 2 — Chained narrative | Multi-scenario journeys read in order: SG-1 (scrape->propose, nothing persisted) → SC-1/SC-3 (the candidate list it rendered, audited + conservative) → SS-1 (one candidate carried into sign) → SS-7 (several signed in one pass). The "Given Maria has harvested rust-lang/cargo" preamble is shared via the `FakeGithub::for_public_repo(..., fixture_cargo_five_signals())` setup. SS-2 (accept-all-defaults) reuses SS-1's Given+When (a candidate list + a `--sign` invocation). The cross-file chain SG-1 → SC-* → SS-1 is the walking-skeleton journey. |
| 3 — App as in production | Every scenario except `scraper_domain.rs` (layer 2, pure-core direct) spawns the REAL `openlore` binary via `assert_cmd::Command::cargo_bin`. No hand-rebuilt wiring. The GitHub + PDS + Identity doubles substitute external/non-deterministic adapters per the Architecture of Reference defaults; the Project Infrastructure Policy entries are listed in §11. The sign path reuses the REAL slice-01 `VerbClaimAdd`/`VerbClaimPublish` internals (SS-1's `scraper_reuses_slice01_publish_path` proves it). |

---

## 10. Mandate compliance evidence (CM-A through CM-H)

| Mandate | Compliance evidence |
|---|---|
| CM-A (Mandate 1, hexagonal boundary) | All slice-02 subprocess acceptance tests invoke `openlore` via subprocess; ZERO direct imports of `scraper_domain::*`, `adapter_github::*`, `adapter_duckdb::*` from `scrape_*.rs`. The `scraper_domain.rs` layer-2 tests directly invoke pure-core `scraper_domain::derive_candidates` + `load_mapping` — appropriate at layer 2 per Mandate 9 (the function signature IS the driving port). |
| CM-B (Mandate 2, business language) | Grep of test names: zero `HTTP`, `endpoint`, `database`, `schema`, `JSON`, `SQL`, `REST`, `GraphQL`. Domain terms only (`scrape`, `harvest`, `signal`, `candidate`, `sign`, `confidence`, `speculative`, `public data`, `provenance`, `token`, `rate budget`). |
| CM-C (Mandate 3, complete journeys) | Every test traces to a user story → see traceability.md. The chained narrative (SG-1 → SC-* → SS-1 → SS-7) satisfies Pillar 2. |
| CM-D (Mandate 4, pure function extraction) | `derive_candidates` + `load_mapping` exercised DIRECTLY in `scraper_domain.rs` SD-1..SD-6 — pure functions, no fixtures, no adapters. The token is held only in the effect shell (`adapter-github`); the pure `scraper-domain` never sees it (SA-5). CLI parameterization is just `tempfile::TempDir` for HOME + the `OPENLORE_GITHUB_API_BASE`/`GITHUB_TOKEN` seams (no environment cross-product). |
| CM-E (Mandate 8, state-delta + Universe) | **DEFERRED to DELIVER** — same status as slice-01 DD-3 + slice-03 DD-FED-10. The Rust `state_delta` port at `tests/common/state_delta.rs` was bootstrapped by slice-01; slice-02 INHERITS it. Slice-02 scenarios use named assertion helpers in `support/mod.rs` (`assert_no_claim_persisted`, `assert_candidate_confidence_unchanged`, `assert_scraper_reuses_slice01_publish_path`) as the Rust idiomatic mirror; DELIVER migrates the load-bearing SG-1 human-gate scenario to `assert_state_delta(before, after, universe, expected)` form (universe `{author_claims.row_count, pds.records.len, claims_dir.artifact_count}` all `set_to("0")`, modeled on slice-03's `assert_purge_state_delta`). Universe entries MUST be port-exposed per Mandate 8 — NEVER internal struct fields. |
| CM-F (Mandate 9, layered PBT mode) | SD-1 + SD-2 + SD-3 are `@property` at layer 2 (proptest); ALL subprocess scenarios at layer 3+ are example-only. ZERO proptest at layer 3+. |
| CM-G (Mandate 10, two-tier acceptance) | Tier A only. Per Mandate 10 add-if-both-conditions: the scrape->propose->sign journey is 3-4 chained scenarios (qualifying on chain length) BUT the input space is bounded (a fixed 5-entry mapping + a small signal set + a candidate index list). `derive_candidates` is a STATELESS function (no journey state machine to model — Mandate 10 "the only observable is did-it-crash / no state mutation to model" leans toward skip). The cross-rule auditability + no-inflation invariants ARE the kind Tier B catches, but SD-1/SD-2/SD-3 assert them generatively at layer 2 (cheaper than a `RuleBasedStateMachine`). **Recommendation**: revisit at slice-04 (scoring-graph) where cross-repo triangulation + confidence weighting expand the state space. See DD-SCR-9. |
| CM-H (Mandate 11, sad-paths example-based) | Every layer-3 sad path is a named `*_rejects_*` / `*_refuses_*` / `*_exits_*` scenario: SG-4, SG-5, SG-6, SS-4, SS-5, SS-9, SA-3, SA-4. ZERO proptest at layer 3+ for sad paths. The only generative tests are the SD-1/2/3 layer-2 invariants (positive-arm properties, with the negative arm — no-match -> empty — pinned as the SD-5 example + SD-1's non-vacuity precondition per Hebert ch.6). |

---

## 11. Project Infrastructure Policy — slice-02 additions

Slice-01 DD-11 + slice-03 DD-FED-11 deferred writing
`docs/architecture/atdd-infrastructure-policy.md` until the orchestrator
scope permits. Slice-02's orchestrator brief also limits writes to the
slice-02 directories + `tests/acceptance/` + `crates/test-support/src/`. The
policy file is STILL not created in this DISTILL wave (DD-SCR-11). The new
entries that SHOULD land when the surface opens:

```markdown
# Slice-02 additions to ATDD Infrastructure Policy

## Driving (extends slice-01/03)
| Port | Mechanism | Note |
|---|---|---|
| CLI (`openlore scrape github <target> [--sign N[,N...]]`) | subprocess from `tempfile::TempDir` via `assert_cmd` | inherits the slice-01 cli mechanism; new verb reuses the env-var seams (`OPENLORE_GITHUB_API_BASE` for the GitHub base, `GITHUB_TOKEN` for the PAT) |

## Driven internal (real) — extends slice-01/03
| Port | Mechanism | Note |
|---|---|---|
| `scraper-domain` (PURE: derive_candidates + load_mapping) | real pure core, direct invocation at layer 2 | no I/O; proptest `@property` for the load-bearing invariants |
| StoragePort (the user's own `claims` table + `claims/<cid>.json`) | real DuckDB file + real filesystem under `tempfile::TempDir` | the human-gate-at-storage proof needs the real store to assert ZERO rows/files |

## Driven external / non-deterministic (fake) — extends slice-01/03
| Port | Fake | Note |
|---|---|---|
| `GithubPort` (resolve_target + harvest_repo + harvest_user + probe) | `openlore_test_support::FakeGithub` (read-only public-API HTTP stub; constructor-pinned postures; no private surface; holds no storage/identity/pds ref) | real public-GitHub contract test = DEVOPS Pact suite (public-endpoint allowlist, rate-limit + rejected-token fixtures) per DESIGN §platform-architect annotation |
```

---

## 12. Pre-requisites for compilation (DELIVER wiring expectations)

The slice-02 skeletons use `use openlore::...` via the existing slice-01
binary AND `use openlore_test_support::...` for the new doubles
(`FakeGithub`, `FakeSignal`, `fixture_cargo_five_signals`, …). The
intentional consequence:

1. **`cargo build --tests` will fail until DELIVER's slice-02 step-07-01
   scaffold lands** the bodies for:
   - `FakeGithub::for_public_repo` + `for_public_user` + `for_not_found` +
     `for_private_target` + `offline` + `rate_limited_anon` +
     `with_rejected_token` + `with_no_matching_signals` +
     `with_multi_signal_single_predicate` + `authenticated` + `serve_http` +
     `saw_token` + `seen_paths` + `auth_mode`.
   - `fixture_cargo_five_signals` + `fixture_torvalds_user_aggregate_signals`
     + `fixture_three_docs_signals_one_predicate`.
   - The new ports types referenced by step-defs: `ports::GithubPort` +
     `ports::TargetKind` + `ports::GithubError` + the slice-02
     `ProbeRefusalReason` variants per component-boundaries §`crates/ports`.
   - The `scraper-domain` crate: `Signal` + `CandidateClaim` +
     `SignalPredicateMapping` + `derive_candidates` + `load_mapping` +
     `proptest_strategies::{arb_signal_set}` per component-boundaries
     §`crates/scraper-domain`.
   - The `support/mod.rs` scrape runners + assertion helpers (DD-SCR-5).

2. **Once the test-support bodies + the two new crates + the extended `ports`
   are materialized AND the `cli` wires the `scrape github [--sign]` verb**,
   the slice-02 tests compile to "all `#[test]` functions panic with
   `todo!()`" → tests RED per Mandate 7. DELIVER then unskips one at a time.

3. **Rust scaffold marker** per Mandate 7 + slice-01/03 precedent: every
   `#[test]` body that panics does so via `todo!("DELIVER (slice-02): ...")`
   with a `// SCAFFOLD: true` comment-marker on the surrounding module. The
   new modules `fake_github.rs` + `fixtures_github.rs` carry the same marker.
   Detection: `grep -rn "SCAFFOLD: true" tests/acceptance/scrape*.rs
   tests/acceptance/scraper_domain.rs crates/test-support/src/fake_github.rs
   crates/test-support/src/fixtures_github.rs`.

4. **`[[test]]` registration (DD-SCR-14)**: the 5 new `[[test]]` targets are
   NOT registered in `crates/cli/Cargo.toml` by this DISTILL wave. DELIVER's
   per-file first step registers each (mirrors slice-03 precedent).

5. **Pre-DELIVER fail-for-right-reason gate (slice-02)** is deferred per the
   same logic as slice-01 DD-2 + slice-03 DD-FED-13: the test-support
   extensions + the two new crates + the extended `ports` + the `cli` verb
   wiring land in DELIVER's first slice-02 step (step-07-01); only after that
   step do the tests compile and reach the `todo!()` panic that classifies as
   RED. See DD-SCR-13.

DELIVER's first slice-02 task (proposed): bootstrap the `GithubPort` trait +
`TargetKind` + `GithubError` in `ports`; the PURE `scraper-domain` crate
(ADTs + `derive_candidates` + `load_mapping` + embedded jobs.yaml mapping +
`mapping_matches_ssot` build-time test); the EFFECT `adapter-github` crate
(`AdapterGithub` + `probe()` + `GITHUB_TOKEN` + `OPENLORE_GITHUB_API_BASE`
seams); the `cli` `scrape github [--sign]` dispatch + `CandidateRenderer` +
`CandidatePrefill` + `SelectionParser`; the `FakeGithub` + `fixtures_github`
bodies; `xtask check-arch`/`check-probes` extension for the two new crates;
and the 5 `[[test]]` Cargo.toml targets. At that point the slice-02
acceptance suite classifies as RED and the standard outside-in TDD loop
resumes.

---

## 13. Definition of Done (DISTILL handoff to DELIVER)

- [x] All 34 slice-02 scenarios written as RED-ready Rust skeletons.
- [x] Every NEW CLI verb/flag in ADR-017 covered by at least one subprocess
      scenario (§5).
- [x] Every NEW driven adapter mapped (real or fake double explicitly
      justified) — see §6.
- [x] Three Pillars verified (domain language, chained narrative, production
      composition) — §9.
- [x] No `# DISTILL: confirm` flags to resolve (all open behaviors resolved
      by DESIGN WD-60/62/63 + Q-DELIVER-5) — §2.
- [x] Wave-decision reconciliation passed (0 contradictions DISCUSS ↔ DESIGN)
      — §1.
- [x] `traceability.md` written: every test → story → job → ADR →
      wave-decision → integration gate → KPI.
- [x] `wave-decisions.md` written: DD-SCR-1..DD-SCR-14.
- [x] All 5 integration gates from shared-artifacts-registry.md covered (§7).
- [x] All 5 KPIs mapped to acceptance scenarios (or to telemetry —
      KPI-SCR-1 latency + KPI-SCR-5 edit-rate) — §8.
- [x] Walking skeleton present + user-centric: SG-1 (scrape->propose) + SS-1
      (sign half), `@walking_skeleton @driving_port`, demo-able to a
      stakeholder ("scrape a repo, see auditable proposals, sign one").
- [x] Error-path coverage healthy on sad-path-admitting surfaces
      (`scrape_sign.rs` 44%, `scrape_auth.rs` 40%).
- [ ] **Pre-DELIVER fail-for-right-reason gate**: DEFERRED until DELIVER
      scaffolds the slice-02 test-support extensions + the two new crates +
      the extended `ports` + the `cli` verb wiring. See §12 + DD-SCR-13.

Handoff-ready: **YES**, conditional on DELIVER's slice-02 step-07-01 landing
the new ports + the `scraper-domain` + `adapter-github` crates + the
test-support extensions + the `cli` verb wiring before running the suite the
first time.

---

## 14. Open items for DELIVER

1. **Bootstrap `GithubPort`** per component-boundaries.md §`crates/ports`:
   new trait + `TargetKind` + `GithubError` + slice-02 `ProbeRefusalReason`
   variants. All `probe()` per ADR-009/ADR-019 (public reachability + private
   refusal + auth-mode + rate-limit-header + no-token-leak — the five probe
   responsibilities in §`crates/adapter-github`).

2. **Add the PURE `scraper-domain` crate** per component-boundaries.md
   §`crates/scraper-domain`: `Signal` + `CandidateClaim` +
   `SignalPredicateMapping` + `derive_candidates` + `load_mapping` +
   `proptest_strategies::arb_signal_set`. Embed the jobs.yaml mapping
   snapshot (`include_str!` + pure parse) + the `mapping_matches_ssot`
   build-time test (WD-67). `xtask check-arch` whitelist for the pure YAML
   parser (WD-65). Pin the exhaustive per-arm unit coverage in the crate's
   own `#[cfg(test)] mod tests` (layer 1 — DELIVER's call, DD-SCR-7).

3. **Add the EFFECT `adapter-github` crate** per component-boundaries.md
   §`crates/adapter-github`: `AdapterGithub` impl over the workspace
   `reqwest`; `GITHUB_TOKEN` (WD-63) + `OPENLORE_GITHUB_API_BASE` (test
   seam) reads; rate-limit detection + remediation messaging;
   public-data-only refusal; `probe()` within 250ms (I-5). Holds NO
   storage/identity/pds reference (human-gate at the architecture layer,
   I-SCR-1). `cargo deny check` for any new dep (I-11; prefer reusing the
   workspace HTTP client).

4. **Materialize `FakeGithub` + `fixtures_github`** in
   `crates/test-support/src/` (scaffolded by THIS DISTILL wave; bodies are
   `todo!()`). Same `tokio::spawn + AbortOnDrop` runtime as `FakePds`/`FakePeerPds`.

5. **Extend `cli`**: `scrape github <target> [--sign N[,N...]]` dispatch +
   `CandidateRenderer` + `CandidatePrefill` (the ONLY bridge from
   `CandidateClaim` to a signed claim, WD-66/I-SCR-6) + `SelectionParser`
   (duplicate/out-of-range rejection pre-compose). Reuse `VerbClaimAdd` +
   `VerbClaimPublish` UNCHANGED. The display-only `derived-from` line (WD-62)
   in the compose preview + publish output, NEVER in the signed payload.

6. **Extend `xtask check-arch`/`check-probes`** for the two new crates
   (WD-65): `scraper-domain` PURE, `adapter-github` effect wired only by
   `cli` + ships a `probe()`.

7. **Migrate SG-1 to explicit `assert_state_delta`** (DD-SCR-10) once the
   `assert_no_claim_persisted` body is real — universe
   `{author_claims.row_count, pds.records.len, claims_dir.artifact_count}`
   all `set_to("0")`, modeled on slice-03's `assert_purge_state_delta`.

8. **Tier B (state-machine PBT) revisit** per CM-G §10 — slice-04
   (scoring-graph) is the right surface once cross-repo triangulation +
   confidence weighting expand the input space.

9. **DEVOPS Pact suite extension** for the GitHub public-endpoint allowlist
   (KPI-SCR-4 release-gate) + rate-limit/rejected-token fixtures — per
   outcome-kpis.md DEVOPS handoff + DESIGN §platform-architect annotation;
   not a DELIVER deliverable but a coordination point.

10. **Register the 5 `[[test]]` targets** in `crates/cli/Cargo.toml`
    (DD-SCR-14) — DELIVER's per-file first step, mirroring slice-03.

---

## 15. References

- `docs/feature/openlore-github-scraper/distill/wave-decisions.md`
- `docs/feature/openlore-github-scraper/distill/traceability.md`
- `tests/acceptance/scrape_github.rs`
- `tests/acceptance/scrape_candidates.rs`
- `tests/acceptance/scrape_sign.rs`
- `tests/acceptance/scrape_auth.rs`
- `tests/acceptance/scraper_domain.rs`
- `crates/test-support/src/fake_github.rs`
- `crates/test-support/src/fixtures_github.rs`
- DISCUSS: `docs/feature/openlore-github-scraper/feature-delta.md` +
  `discuss/{user-stories,story-map,outcome-kpis,shared-artifacts-registry}.md` +
  `discuss/journey-scrape-propose-sign-visual.md`
- DESIGN: `docs/feature/openlore-github-scraper/design/{architecture-design,component-boundaries,data-models,wave-decisions}.md`
- ADRs: ADR-017..019 (this slice); ADR-003/005/007/009 (inherited, amended/extended)
- SSOT: `docs/product/journeys/scrape-propose-sign.yaml` +
  `docs/product/jobs.yaml` (J-004 + sub-jobs J-004a/b/c + signal_predicate_mapping) +
  `docs/product/personas/researcher-tech-lead.yaml` (contributor-evaluator hat)
- Inherited from slice-01/03:
  - `docs/feature/openlore-foundation/distill/{acceptance-tests,wave-decisions,traceability}.md`
  - `docs/feature/openlore-federated-read/distill/{acceptance-tests,wave-decisions,traceability}.md`
  - `tests/acceptance/support/mod.rs`
  - `crates/test-support/src/{lib,fake_pds,fake_peer_pds,identity,fixtures,fixtures_peer}.rs`
