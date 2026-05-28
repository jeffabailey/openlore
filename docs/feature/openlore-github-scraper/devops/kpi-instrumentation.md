# KPI-SCR Instrumentation — openlore-github-scraper (slice-02)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex
- **Source-of-truth KPIs**: `docs/feature/openlore-github-scraper/discuss/outcome-kpis.md`
- **Foundation cross-link**: `docs/feature/openlore-foundation/devops/kpi-instrumentation.md` (KPI-1..6 — still in force, unchanged)
- **Slice-03 cross-link**: `docs/feature/openlore-federated-read/devops/kpi-instrumentation.md` (KPI-FED-1..6 — still in force, unchanged)

This document traces each of the 5 outcome KPI-SCR targets to **what** measures
it, **where** the data lives, **how** the developer-as-operator reads it, and a
**feasibility tag** (GREEN/YELLOW/RED) for slice-02. It mirrors the slice-03
GREEN-per-user / YELLOW-cohort policy (D-D17 → D-D26).

## 1. Summary table

| KPI | Type | Instrumentation | Read mechanism | Feasibility |
|---|---|---|---|---|
| KPI-SCR-1 (scrape→sign < 2 min) | Leading (Outcome / North Star) | post-hoc join `scrape.started` → first `claim.signed.from_scraper` per `scrape_id` (histogram `scrape_to_sign_seconds`, bucketed by target-kind) + 30-day think-aloud | per-user: `openlore stats --scraper` / `scripts/kpi-scr-1.jq`. Cohort percentile: future telemetry endpoint | **GREEN per-user / YELLOW cohort** |
| KPI-SCR-2 (human-gate: zero unsigned persistence / auto-publish) | Leading (Guardrail) | ATs `scraper_never_persists_unsigned` + `candidate_confidence_no_autoinflate` + runtime counters `scraper_unsigned_residue_total`, `scraper_confidence_autoinflate_total` | CI status (release-blocking) + `openlore stats --scraper` | **GREEN** |
| KPI-SCR-3 (auditability: every candidate names its source signal) | Leading (Outcome) | AT `candidate_names_source_signal` + runtime counter `scraper_candidate_missing_source_total` + `scrape.candidates.derived.source_signal_coverage` | CI status + `openlore stats --scraper` + 5-user day-30 think-aloud | **GREEN** |
| KPI-SCR-4 (public-data-only: zero private endpoint calls) | Leading (Guardrail / Trust) | AT `scraper_only_reads_public_data` + `contract-pact-github` **public-endpoint allowlist assertion** + probe step 2 (`private_refusal` fixture) | CI status (release-blocking) + `openlore stats --scraper` refusal breakdown | **GREEN** |
| KPI-SCR-5 (edit-rate ≥50% of signed-from-scraper claims) | Leading (Outcome) | `tracing` event `claim.signed.from_scraper{fields_edited, edit_count}` + ratio metric `scraper_edit_rate` (30-day window) | per-user: `openlore stats --scraper` / `scripts/kpi-scr-5.jq`. Cohort rate: future telemetry endpoint OR PO day-30 outreach | **GREEN per-user / YELLOW cohort** |

**No KPI-SCR is RED.** Every one has a designed capture mechanism that ships in
slice-02. The two guardrails (KPI-SCR-2, KPI-SCR-4) are fully GREEN and
release-blocking. The two outcome KPIs with a cohort dimension (KPI-SCR-1
percentile, KPI-SCR-5 cohort rate) are GREEN per-user / YELLOW cohort — the same
deferred-telemetry-endpoint constraint that slice-03 KPI-FED-3/5 and foundation
KPI-3/6 carry (D-D26).

## 2. KPI-SCR-1 — scrape→sign < 2 minutes (NORTH STAR)

### What
A contributor-evaluator produces an evidence-backed SIGNED claim about a target,
by reviewing scraper candidates, in under 2 minutes from `scrape github` to the
signed claim — INCLUDING predicate-vocabulary discovery (which the candidate
list removes). Comparison baseline: the slice-01 hand-authoring time ("< 2 min
once the predicate vocabulary is known").

### Where the data lives
- **Per-user runtime signal**: JSON log (foundation §3.3). The duration is
  computed POST-HOC (D-D27, mirrors slice-03 D-D16) by joining, per `scrape_id`:
  - start: `scrape.started` timestamp (the `scrape github` invocation start);
  - end: the FIRST `claim.signed.from_scraper` event with the same `scrape_id`.
  Histogram `scrape_to_sign_seconds`, bucketed by `target_kind` (small repo /
  large repo / user) per `outcome-kpis.md` Handoff item 2.
- **Per-user qualitative signal**: the 30-day think-aloud — delivered via the
  SAME one-shot Likert prompt mechanism as slice-03 D-D18 / foundation KPI-3
  (after the first `claim.signed.from_scraper`, if the status file is absent).
  Stored at `$XDG_DATA_HOME/openlore/surveys/post-scrape-sign.response.json`.

### How the developer reads it
- **Per-user (verb)**: `openlore stats --scraper` shows scrape→sign p50/p95 per target-kind.
- **Per-user (fallback)**: `scripts/kpi-scr-1.jq` joins the events offline.
- **Cohort**: NOT in slice-02. Luna (PO) collects the slice-01 hand-authoring baseline from the dogfood cohort (if available) and runs the day-30 think-aloud directly (the slice-01/03 model for KPI-3/6/FED-3).

### Aggregation across cohort
- Per-user: GREEN.
- Cohort percentile (P50/P95 across all users): requires the future telemetry endpoint. The histogram event is shaped to be aggregable (bucketed target-kind + duration); rollup is straightforward once an endpoint exists.

### Feasibility: GREEN per-user / YELLOW cohort
Per-user capture is solid and ships in slice-02 (the `scrape_id` correlation +
the reused sign/publish boundary). Cohort percentiles are deferred (future
endpoint) — same constraint as slice-03 KPI-FED-5.

### Disprover wiring
The kill-criterion "KPI-SCR-1 > 4 minutes (2x budget)" (outcome-kpis.md
disprovers §3) is surfaced as an INFORMATIONAL alert (observability §10): P95 > 4
min escalates to PO; it does NOT auto-block release (there is no central metric
store to alert against; the operator notices on `openlore stats --scraper`).

## 3. KPI-SCR-2 — Human-gate: zero unsigned persistence / auto-publish (GUARDRAIL)

### What
Running `scrape github` WITHOUT `--sign` signs NOTHING, persists NOTHING as a
claim, publishes NOTHING. No candidate is ever stamped above 0.25 confidence by
the tool (only the human raises it). This is the single most load-bearing
guardrail of the slice — violating it collapses the trust model (unshippable).

### Where the data lives
- **CI signal (PRIMARY)**: two AT results —
  - `scraper_never_persists_unsigned` (`ci-cd-pipeline.md` delta §3.2): asserts zero `author_claims` rows, zero PDS writes, zero files after a `scrape` without `--sign`.
  - `candidate_confidence_no_autoinflate` (§3.2): asserts every candidate is numeric 0.25; none above 0.3.
- **Runtime signals**: counters `scraper_unsigned_residue_total` (from `scrape.completed.signed_unsigned_residue`; target = 0 forever) and `scraper_confidence_autoinflate_total` (from `scrape.candidates.derived` where `confidence_max > 0.3`; target = 0 forever). Both non-zero = P0 bug.

### How the developer reads it
- **CI**: GitHub Actions checks; release-blocking (observability §10).
- **Runtime**: `openlore stats --scraper` shows `Unsigned residue: 0` and `Confidence auto-inflate: 0` (anything else is P0).

### Aggregation across cohort
CI signal IS the cohort signal — if both ATs pass, EVERY user's binary has the
human-gate property (same logic as foundation KPI-4).

### Feasibility: GREEN
Both CI gates designed in `ci-cd-pipeline.md` delta §3.2. Both runtime counters
designed in `observability.md` delta §4.

## 4. KPI-SCR-3 — Auditability: every candidate names its source signal

### What
Every proposed candidate traces to the exact public GitHub signal(s) that
produced it. A candidate that collapses multiple signals lists ALL of them (no
truncation). The derivation is deterministic over the SSOT signal→predicate
mapping (WD-53; no ML inference). Auditability is a precondition for KPI-SCR-1
trust (users will not sign a candidate they cannot trace).

### Where the data lives
- **CI signal**: AT `candidate_names_source_signal` (`ci-cd-pipeline.md` delta §3.3): asserts every `CandidateClaim.source_signals` is non-empty; asserts the collapsed-multi-signal candidate lists ALL contributing signals; asserts determinism over the SSOT mapping.
- **Runtime signal**: counter `scraper_candidate_missing_source_total` (from `scrape.candidate.rendered` where `source_signal_count == 0`; target = 0 forever; non-zero is P0 — analogous to slice-03 `peer_render_attribution_missing_total`) + the `scrape.candidates.derived.source_signal_coverage` field (MUST be `"all"`).
- **Day-30 think-aloud**: 5-user manual session (PO) confirms users perceive the candidates as traceable ("a strong starting point I trusted enough to edit and sign").

### How the developer reads it
- **CI**: GitHub Actions check; release-blocking-adjacent (blocking, not a guardrail — a missing-source candidate is a correctness bug, not a trust-model collapse).
- **Runtime**: `openlore stats --scraper` shows `Candidates missing source: 0`.
- **Renderer-review backstop (D-D28)**: the release-time renderer-review checklist (D-D19) gains one line — "candidate renderer lists ALL source signals (no truncation)" — the human-in-the-loop backstop for future renderers that the AT doesn't yet cover (same reasoning as slice-03's KPI-FED-2 checklist line).

### Aggregation across cohort
CI signal IS cohort signal.

### Feasibility: GREEN
CI gate designed (§3.3). Runtime counter + coverage field designed
(observability §4, §2.1). Checklist line is a one-line documentation edit.

## 5. KPI-SCR-4 — Public-data-only: zero private endpoint calls (GUARDRAIL)

### What
The scraper reads ONLY public GitHub data. Private/non-existent/inaccessible
targets are REFUSED (not silently emptied) with "scraper only reads public
data". NO authenticated-private endpoint is ever reachable. The target is the
SUBJECT of a possible claim, never a controller. This is the no-surveillance
guardrail (unshippable if violated).

### Where the data lives
- **CI signal (PRIMARY — two complementary layers)**:
  - **Probe + AT layer**: AT `scraper_only_reads_public_data` (`ci-cd-pipeline.md` delta §3.4) + probe step 2 (architecture-design §6.3) against the `private_refusal` FakeGithub fixture: a private/inaccessible path 404s and MUST be refused with `GithubError::NotPublic` (NOT a silent empty harvest); zero candidates rendered; non-zero exit.
  - **Contract layer (THE release-gate mechanism)**: the `contract-pact-github` **public-endpoint allowlist assertion** (`ci-cd-pipeline.md` delta §3.6; D-D22): across ALL scrape operations exercised, the provider (FakeGithub in PR/nightly; recorded/real GitHub at release) records every requested path; the test asserts `requested_paths ⊆ public_allowlist`. ANY observed off-allowlist (authenticated-private) endpoint call fails CI. **Zero private-endpoint calls.** This is the slice-02 KPI-SCR-4 release-gate.
- **Runtime signal**: counter `scrape_refusals_total{reason: NotPublic}` (the refuse-private path firing — a NotPublic refusal is the system working correctly) + the no-token-leak assertion (probe step 5 + the contract assertion that `GITHUB_TOKEN` never appears in any captured path/event/log).

### The public-endpoint allowlist (the contract subject)
The allowlist (architecture-design §6.2) — the test asserts NO call falls
outside it:
- `GET /repos/{owner}/{repo}`
- `GET /repos/{owner}/{repo}/contents/{path}`
- tags/releases, languages
- `GET /users/{user}`, `GET /users/{user}/repos`
- (or the GraphQL public equivalents if DELIVER picks GraphQL — Q-DELIVER-2; the assertion adapts to the chosen transport)

No authenticated-private endpoint (e.g. anything requiring repo-scope read of a
private repo) is on the allowlist; any such call fails the assertion.

### How the developer reads it
- **CI**: GitHub Actions checks (the AT + the contract allowlist assertion); both release-blocking.
- **Runtime**: `openlore stats --scraper` shows the refusal breakdown (`NotPublic: N` is expected/normal when a user scrapes a private target; it means the guardrail fired).

### Aggregation across cohort
CI gate IS cohort signal — if the allowlist assertion + the refuse-private AT
pass, EVERY user's binary only touches public endpoints. The runtime refusal
counter is observational (how often a user scrapes a private target), not a
guarantee — the guarantee is in the CI gate.

### Feasibility: GREEN
The two-layer CI gate (probe/AT + contract allowlist) is fully designed
(`ci-cd-pipeline.md` delta §3.4 + §3.6). The FakeGithub `private_refusal`
fixture + the recorded-real provider are the load-bearing infrastructure (D-D24,
D-D25). Runtime counter + no-token-leak assertion designed.

## 6. KPI-SCR-5 — Edit-rate ≥50% of signed-from-scraper claims

### What
≥50% of signed-from-scraper claims show ≥1 field edited (predicate, evidence, or
confidence) from the proposed default before signing. Proves the
human-in-the-loop is REAL — not rubber-stamping machine proposals.

### Where the data lives
- **Per-user behavioral signal** (always captured): `tracing` event
  `claim.signed.from_scraper{proposed_confidence, signed_confidence, fields_edited, edit_count}` (observability §2.3). The per-field diff between the proposed candidate and the signed payload is computed at sign time and recorded. Ratio metric `scraper_edit_rate` aggregates over a 30-day window.
- **Per-user qualitative signal**: the same one-shot Likert as KPI-SCR-1 (the day-30 think-aloud asks whether the candidate was "a strong starting point I trusted enough to edit and sign").
- **Cohort behavioral signal** (requires future telemetry endpoint OR PO outreach): edit-rate across users + the 30-day cohort denominator.

### How the developer reads it
- **Per-user**: `openlore stats --scraper` shows `Edit-rate (30d): N%`.
- **Per-user fallback**: `scripts/kpi-scr-5.jq` (% of `claim.signed.from_scraper` with `edit_count ≥ 1`, 30-day window; works offline).
- **Cohort**: NOT in slice-02. Luna (PO) coordinates direct outreach to dogfood users at day-30 (the slice-01/03 model).

### Disprover wiring
The kill-criterion "KPI-SCR-5 < 20%" (outcome-kpis.md disprovers §4 — near-zero
edit rate = human-in-the-loop is theatre) is a PO-reviewed day-30 signal, not an
auto-block. The per-user `edit_count` field is the raw measurement.

### Aggregation across cohort
- Per-user: GREEN.
- Cohort rate (across all users): requires future telemetry endpoint OR PO out-of-band outreach. **Telemetry rule**: ONLY the ratio + denominator count can ever be sent; `predicate` text, `fields_edited` field-values, `target` strings, and `scrape_id` are NEVER sent (observability §7).

### Feasibility: GREEN per-user / YELLOW cohort
Per-user diff capture is solid and ships in slice-02. Cohort aggregation
requires the future endpoint (same constraint as slice-03 KPI-FED-3 cohort).

## 7. Cross-references

- `observability.md` delta §2 — event names that back KPI-SCR-1..5 measurement
- `observability.md` delta §4 — metric rows derived from those events
- `ci-cd-pipeline.md` delta §3.2 (KPI-SCR-2), §3.3 (KPI-SCR-3), §3.4 + §3.6 (KPI-SCR-4), §3.5 (single-publish-path), §4 (mutation scope)
- Foundation `kpi-instrumentation.md` (KPI-1..6) + slice-03 `kpi-instrumentation.md` delta (KPI-FED-1..6) — unchanged
- `discuss/outcome-kpis.md` — authoritative KPI-SCR definitions + the Handoff to DEVOPS section
- ADR-010 — telemetry-opt-in (governs the YELLOW path for KPI-SCR-1 cohort and KPI-SCR-5 cohort)

## 8. Map to slice-01 KPIs (per `outcome-kpis.md` §Mapping)

| slice-01 KPI | Status in slice-02 instrumentation |
|---|---|
| KPI-4 (zero silent normalization, 100% round-trip identity) | Inherited UNCHANGED. A signed-from-scraper claim is byte-identical in shape to a hand-authored claim (display-only provenance, WD-62 — no new field, no new CID path); the existing `kpi-4-roundtrip` gate covers it without change. The scraper adds no new normalization path. |
| KPI-5 (local-first invariant, network-disabled correctness) | Inherited with one SCOPED exception (per outcome-kpis.md). `scrape github` REQUIRES network (the harvest step); the `adapter-github` probe is `--offline`-skipped and `scrape` refuses offline with `GithubError::Network`. Everything AFTER harvest (review, edit, sign) follows the slice-01 local-first rule — sign succeeds with network disabled; only publish needs network. The existing `kpi-5-offline` test scope is unchanged (the scrape step is explicitly out of the offline-correctness envelope). |
| KPI-6 (claims that would NOT have been blog posts) | EXTENDED. The scraper lowers the activation cost; KPI-SCR-1 is the leading indicator for this lagging slice-01 north star. Same cohort-aggregation deferral as KPI-6. |
| KPI-1 (under 2 min e2e for the slice-01 walking skeleton) | Slice-02 introduces KPI-SCR-1 as the parallel measurement for the slice-02 walking skeleton (same 2-min budget, but INCLUDING vocabulary discovery, which the candidate list removes). Independent histogram (`scrape_to_sign_seconds`); coexists with KPI-1's measurement. |

## 9. KPI-SCR sign-off readiness (per slice-03 §Phase 6 precedent)

| KPI-SCR | Per-user instrumented in slice-02? | Cohort aggregated? | Status |
|---|---|---|---|
| KPI-SCR-1 | YES (post-hoc jq / `scrape_to_sign_seconds` + day-30 Likert) | NO (future telemetry endpoint for cohort percentiles) | **GREEN per-user / YELLOW cohort** |
| KPI-SCR-2 | YES (two CI gates + two runtime counters) | YES (CI = cohort property) | **GREEN** |
| KPI-SCR-3 | YES (CI gate + runtime counter + coverage field) | YES (CI = cohort property) | **GREEN** |
| KPI-SCR-4 | YES (probe/AT + contract allowlist assertion + runtime counter) | YES (CI = cohort property) | **GREEN** |
| KPI-SCR-5 | YES (`claim.signed.from_scraper` diff event + ratio metric) | NO (future telemetry endpoint OR PO day-30 outreach) | **GREEN per-user / YELLOW cohort** |

**No KPI-SCR is RED.** All five are at minimum per-user-readable from the
slice-02 binary. The two YELLOW items (KPI-SCR-1 cohort percentile, KPI-SCR-5
cohort rate) reflect the same deferred-cohort-endpoint constraint that slice-03
KPI-FED-3/5 and foundation KPI-3/6 carry — not a capture gap. The two
release-blocking guardrails (KPI-SCR-2, KPI-SCR-4) are both fully GREEN.
