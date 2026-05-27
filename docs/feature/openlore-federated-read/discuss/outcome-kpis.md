# Outcome KPIs — openlore-federated-read (slice-03)

- **Wave**: DISCUSS
- **Date**: 2026-05-27
- **Owner**: Luna (nw-product-owner)

## Feature: openlore-federated-read

### Objective

A subscribed reader of another developer's signed claims walks away from
each session with a defensible view of WHO claimed WHAT with WHAT evidence,
never inheriting their conclusions silently, and disagreement is one verb
away when warranted.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-FED-1 | Federated-read users (P-002 + P-001 in reader hat) | Correctly identify the author DID of any claim shown in a federated query | 100% (zero attribution loss) | n/a (new behavior) | Integration test `federation_attribution_preserved` + 5-user think-aloud study at day-30 | Leading (Outcome) |
| KPI-FED-2 | Federated-read users | Encounter ZERO "merged consensus" rendering of multi-author claims across all UI surfaces | 0 occurrences | n/a (new behavior) | Adversarial review of every output renderer + acceptance test `no_merged_rows` | Leading (Guardrail) |
| KPI-FED-3 | Federated-read users (P-002 wearing the federation-reader hat) | Publish at least one counter-claim in response to a peer's claim they disagree with | ≥30% of dogfood cohort within 30 days of slice-03 release | 0 (no counter-claim verb exists today) | Author-side telemetry: count of `claim counter` invocations per active reader-user; 30-day survey "did the counter-claim feel as light as a comment?" | Leading (Outcome) |
| KPI-FED-4 | Federated-read users who run `peer remove --purge` | Verify zero residual peer claims from the removed peer in subsequent federated queries | 100% (zero residue) | n/a (new behavior) | Acceptance test `peer_remove_purge_zero_residue` + dogfood log audit | Leading (Outcome / Guardrail) |
| KPI-FED-5 | Federated-read users | Complete subscribe -> pull -> federated query end-to-end | In under 90 seconds for a peer publishing ≤20 claims | n/a (new behavior; comparable to slice-01 KPI-1 "under 2 min e2e") | Author-side timing telemetry (start-to-first-query-result histogram) | Leading (Outcome) |
| KPI-FED-6 | Federated-read users | Pull a peer's claims without any signature-verification false positives accepted into peer_claims | 100% (no invalid signatures stored) | n/a (new behavior) | Adversarial fixture: a peer that publishes deliberately tampered records; acceptance test `peer_tampered_signature_rejected` | Leading (Guardrail / Security) |

### Metric Hierarchy

- **North Star**: **KPI-FED-3** — % of dogfood cohort that publishes ≥1 counter-claim within 30 days. This is the behavioral validation of J-003b (disagreement as first-class structured artifact). The slice's value evaporates if engineers technically CAN counter-claim but never DO.
- **Leading Indicators**: KPI-FED-1 (attribution fidelity — without this, KPI-FED-3 cannot happen because users do not see WHO to counter), KPI-FED-5 (end-to-end latency — friction kills behavior change).
- **Guardrail Metrics**: KPI-FED-2 (zero merged rows), KPI-FED-4 (zero purge residue), KPI-FED-6 (zero invalid signatures stored). All three MUST hold; any failure is unshippable.

### Mapping to slice-01 KPIs

| slice-01 KPI | Status in slice-03 |
|---|---|
| KPI-4 (zero silent normalization, 100% round-trip identity) | Inherited and EXTENDED — applies now to peer-sourced claims too. The peer pull MUST preserve every field byte-equal during the federation round-trip. |
| KPI-5 (local-first invariant, network-disabled correctness) | Inherited UNCHANGED — federated queries on locally-cached peer claims work without network. Only `peer add` and `peer pull` require network. |
| KPI-1 (under 2 min e2e for slice-01 walking skeleton) | New KPI-FED-5 mirrors this for the slice-03 walking skeleton (90s budget given that pull is the only network step and peer claim sets are bounded). |

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| KPI-FED-1 | acceptance test + dogfood think-aloud session | automated test (CI) + manual session (day-30) | continuous + once | DEVOPS (CI), nw-product-owner (session) |
| KPI-FED-2 | acceptance test `no_merged_rows` | automated test (CI) | continuous | DEVOPS |
| KPI-FED-3 | author-side telemetry (count of `claim counter` invocations) + 30-day survey | telemetry via tracing event `claim.counter.published` + survey form | continuous + once at day-30 | DEVOPS (telemetry), nw-product-owner (survey) |
| KPI-FED-4 | acceptance test + dogfood log audit (random 5% sample of `peer remove --purge` events) | automated test + manual audit | continuous + monthly | DEVOPS |
| KPI-FED-5 | author-side timing telemetry (`federation.e2e.duration_seconds` histogram) | tracing histogram event emitted on `peer pull` completion + on first `graph query --federated` after | continuous | DEVOPS |
| KPI-FED-6 | adversarial fixture in acceptance suite | automated test (CI) | continuous | DEVOPS |

### Hypothesis

We believe that **a CLI that surfaces peer claims with explicit per-claim
attribution and offers a first-class `claim counter` verb** for federated
readers will achieve **a 30-day dogfood cohort that publishes ≥1 counter-claim
in 30% of cases**.

We will know this is true when **dogfood users in the federation-reader cohort
publish at least one counter-claim each within 30 days of slice-03 release,
AND when interviewed at day-30 they describe the counter-claim experience as
"as light as posting a comment, but more structured."**

### Disprovers (kill criteria for the federation hypothesis)

These outcomes would kill the slice-03 hypothesis and force re-design before
slice-04:

1. **KPI-FED-1 < 100%**: any attribution loss is a fatal failure of the trust model. The Lexicon shape would need to change.
2. **KPI-FED-2 > 0**: any "merged consensus" rendering means a renderer is collapsing authors; the rendering layer needs a redesign.
3. **KPI-FED-3 < 10%**: a near-zero counter-claim rate suggests the verb is too friction-heavy or the behavior change is not happening — either way, the J-003b hypothesis is weakened. Re-investigate the UX before slice-04.
4. **KPI-FED-6 < 100%**: any tampered-signature acceptance is a security failure; halts release.

### Handoff to DEVOPS

The platform-architect needs these from this document to plan instrumentation:

1. **Data collection requirements**:
   - Author-side tracing event `claim.counter.published{counter_cid, target_cid, target_author_did, reason_len}` emitted on every successful counter-claim publish.
   - Author-side tracing histogram `federation.e2e.duration_seconds` from start of `peer pull` to first row rendered by `graph query --federated`.
   - Adversarial fixture peer: a tampered-record fixture PDS used in CI acceptance tests for KPI-FED-6.

2. **Dashboard/monitoring needs**:
   - KPI-FED-3 dashboard: count of `claim.counter.published` events per active reader-user per 30-day window. Dogfood-only initially; revisit at slice-05 for broader rollout.
   - KPI-FED-5 dashboard: P50 + P95 of `federation.e2e.duration_seconds` per peer-cardinality bucket (≤5, 6-20, 21-100, >100 peer claims).

3. **Alerting thresholds**:
   - Alert if any CI run reports KPI-FED-1 != 100% (release-blocking).
   - Alert if any CI run reports KPI-FED-2 > 0 (release-blocking).
   - Alert if any CI run reports KPI-FED-6 != 100% (release-blocking).
   - Alert if P95 of KPI-FED-5 exceeds 180 seconds (informational; do NOT block release alone, but escalate to PO).

4. **Baseline measurement**: no baselines needed; all KPIs are for new behavior. KPI-FED-3 baseline is implicitly 0 (no counter-claim verb exists today).
