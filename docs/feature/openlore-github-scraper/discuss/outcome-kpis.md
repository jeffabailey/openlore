# Outcome KPIs — openlore-github-scraper (slice-02)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

## Feature: openlore-github-scraper

### Objective

A user evaluating a contributor or repo through a philosophy lens produces a
well-evidenced, signed claim in a fraction of the time it would take to author
it by hand — while always remaining the sole signer, never letting the tool
assert anything on their behalf, and only ever touching public data.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-SCR-1 | Contributor-evaluator users (P-002 + P-001 in evaluator hat) | Produce an evidence-backed signed claim about a target by reviewing scraper candidates | In under 2 minutes from `scrape github` to signed claim (vs the slice-01 hand-authoring "under 2 min once the user knows the predicate vocabulary" — the scraper removes the vocabulary-recall cost) | Hand-authoring baseline: J-001 success signal "claim in <2 min once predicate vocabulary is known"; scraper target is <2 min INCLUDING vocabulary discovery | Author-side timing telemetry (`scrape.to_sign.duration_seconds`) + 30-day think-aloud study | Leading (Outcome) |
| KPI-SCR-2 | Contributor-evaluator users | Run `scrape github` WITHOUT `--sign` and observe that NOTHING was signed, persisted as a claim, or published | 100% (zero unsigned persistence; zero auto-publish) | n/a (new behavior) | Acceptance test `scraper_never_persists_unsigned` + `candidate_confidence_no_autoinflate` | Leading (Guardrail) |
| KPI-SCR-3 | Contributor-evaluator users | Trace every proposed candidate to the exact public GitHub signal that produced it | 100% (every candidate names its source signal) | n/a (new behavior) | Acceptance test `candidate_names_source_signal` + 5-user think-aloud at day-30 | Leading (Outcome) |
| KPI-SCR-4 | Contributor-evaluator users | Scrape only PUBLIC data; never trigger a private-data read or a surveillance affordance | 100% (zero private endpoint calls; private/non-existent targets refused) | n/a (new behavior) | Acceptance test `scraper_only_reads_public_data` + adapter contract test asserting only public GitHub endpoints are used | Leading (Guardrail / Trust) |
| KPI-SCR-5 | Contributor-evaluator users | Edit at least one scraper-proposed candidate (predicate, evidence, or confidence) before signing it | >=50% of signed-from-scraper claims show >=1 field edited from the proposed default | n/a (new behavior) | Author-side telemetry: per-signed-claim diff between proposed candidate and signed payload | Leading (Outcome) |

### Metric Hierarchy

- **North Star**: **KPI-SCR-1** — cost-to-first-signed-claim via the scraper,
  under 2 minutes. This is the behavioral validation of the whole feature
  thesis: the scraper EXISTS to lower the cost of producing well-evidenced
  claims. If users can technically scrape but it is no faster than hand-authoring,
  the feature has no reason to exist.
- **Leading Indicators**: KPI-SCR-3 (auditability — users will not sign a
  candidate they cannot trace to a signal, so auditability is a precondition for
  KPI-SCR-1), KPI-SCR-5 (edit rate — proves the human-in-the-loop is real, which
  is what makes users trust the candidates enough to sign quickly).
- **Guardrail Metrics**: KPI-SCR-2 (human-gate: zero unsigned persistence /
  auto-publish) and KPI-SCR-4 (public-data-only). Both MUST hold; any failure is
  unshippable.

### Guardrail interpretation

| Guardrail | If it fails | Consequence |
|---|---|---|
| KPI-SCR-2 (human-gate) | The scraper signed or published something the human did not explicitly sign | Violates the slice-01 "claims are signed human assertions" invariant. UNSHIPPABLE; the entire trust model collapses. |
| KPI-SCR-4 (public-data-only) | The scraper read private data or exposed a surveillance affordance | Violates the J-004 anxiety mitigation ("will this become a surveillance / blacklist tool?"). UNSHIPPABLE. |

### Mapping to slice-01 KPIs

| slice-01 KPI | Status in slice-02 |
|---|---|
| KPI-4 (zero silent normalization, 100% round-trip identity) | Inherited UNCHANGED — a signed-from-scraper claim is byte-identical in shape to a hand-authored claim once signed; the scraper adds no new normalization path. |
| KPI-5 (local-first invariant, network-disabled correctness) | Inherited with one scoped exception — `scrape github` REQUIRES network (it is the harvest step). Everything AFTER harvest (review, edit, sign) follows the slice-01 local-first rule. Sign succeeds with network disabled (only publish needs network). |
| KPI-6 (claims that would NOT have been blog posts) | Extended — the scraper lowers the activation cost, which should INCREASE the rate of claims that would not otherwise have been written. KPI-SCR-1 is the leading indicator for this lagging slice-01 north star. |

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| KPI-SCR-1 | author-side timing telemetry + dogfood think-aloud | tracing histogram `scrape.to_sign.duration_seconds` + manual session (day-30) | continuous + once | DEVOPS (telemetry), nw-product-owner (session) |
| KPI-SCR-2 | acceptance test | automated test (CI) — `scraper_never_persists_unsigned`, `candidate_confidence_no_autoinflate` | continuous | DEVOPS |
| KPI-SCR-3 | acceptance test + dogfood think-aloud | automated test (CI) `candidate_names_source_signal` + manual session | continuous + once | DEVOPS (CI), nw-product-owner (session) |
| KPI-SCR-4 | acceptance test + adapter contract test | automated test (CI) `scraper_only_reads_public_data` + Pact-style contract asserting only public endpoints | continuous | DEVOPS |
| KPI-SCR-5 | author-side telemetry (candidate-vs-signed diff) | tracing event `scrape.candidate.signed{fields_edited}` on each sign-from-scraper | continuous | DEVOPS (telemetry), nw-product-owner (analysis) |

### Hypothesis

We believe that **a CLI that harvests a target's public GitHub signals and
proposes small, auditable, conservative-confidence candidate claims the human
edits and signs** for **contributor-evaluator users (P-002 + P-001 in the
evaluator hat)** will achieve **a cost-to-first-signed-claim under 2 minutes
including predicate-vocabulary discovery**.

We will know this is true when **dogfood users produce a signed claim about a
target via the scraper in under 2 minutes, AND when interviewed at day-30 they
describe the candidate list as "a strong starting point I trusted enough to edit
and sign," AND >=50% of their signed-from-scraper claims show at least one
field edited from the proposed default.**

### Disprovers (kill criteria for the cost-lowering hypothesis)

These outcomes would kill the slice-02 hypothesis and force re-design before
slice-04:

1. **KPI-SCR-2 != 100%**: any unsigned persistence or auto-publish is a fatal
   failure of the human-gate. The feature is pulled until fixed.
2. **KPI-SCR-4 != 100%**: any private-data read or surveillance affordance is a
   fatal failure of the no-surveillance promise. The feature is pulled until fixed.
3. **KPI-SCR-1 > 4 minutes (2x budget)**: if scraping is no faster than
   hand-authoring, the feature has no reason to exist; re-investigate the
   candidate-review UX before slice-04.
4. **KPI-SCR-5 < 20%**: a near-zero edit rate suggests users are rubber-stamping
   machine proposals — the human-in-the-loop is theatre, not real. Re-investigate
   whether candidates are over-confident or the edit affordance is too hidden.

### Handoff to DEVOPS

The platform-architect needs these from this document to plan instrumentation:

1. **Data collection requirements**:
   - Author-side tracing histogram `scrape.to_sign.duration_seconds` from
     `scrape github` invocation to first successful sign (for KPI-SCR-1).
   - Author-side tracing event `scrape.candidate.signed{target, predicate,
     fields_edited, proposed_confidence, signed_confidence}` on each
     sign-from-scraper (for KPI-SCR-5).
   - Adapter contract fixture: a GitHub API recording / contract test that
     asserts the scraper calls ONLY public endpoints (for KPI-SCR-4).

2. **Dashboard/monitoring needs**:
   - KPI-SCR-1 dashboard: P50 + P95 of `scrape.to_sign.duration_seconds` per
     target-type bucket (small repo, large repo, contributor/user).
   - KPI-SCR-5 dashboard: edit-rate (% of signed-from-scraper claims with >=1
     edited field) over a 30-day window.

3. **Alerting thresholds**:
   - Alert if any CI run reports KPI-SCR-2 != 100% (release-blocking).
   - Alert if any CI run reports KPI-SCR-4 != 100% (release-blocking).
   - Alert if P95 of KPI-SCR-1 exceeds 4 minutes (informational; do NOT block
     release alone, but escalate to PO).

4. **Baseline measurement**: no baselines needed; all KPIs are for new behavior.
   KPI-SCR-1's comparison baseline is the slice-01 hand-authoring time (J-001
   success signal); collect it from the slice-01 dogfood cohort if available.
