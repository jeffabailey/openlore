# Outcome KPIs — openlore-appview-search (slice-05)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

## Feature: openlore-appview-search

### Objective

A developer who does NOT already know whom to follow can discover well-evidenced
signed claims — and the people behind them — from across the whole network in a
single search session, trust each result because it is signature-verified and
attributed to its author (never a faceless network-consensus row), and turn a
discovery into a followed peer that grows their trusted LOCAL graph — all without
the AppView ever becoming an authority that overwrites the CLI-first, local-first
source of truth.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-AV-1 | Network-discovery users (P-002 + P-001 in discovery hat) | Discover a relevant signed claim by an author they do NOT already subscribe to, via a network search | >=60% of dogfood discovery sessions surface >=1 relevant claim by an unfollowed author within 30 days | 0 (no network discovery surface exists today; local query sees only own + subscribed peers) | Author-side telemetry: `search.discovery.unfollowed_author_hit` event when a result includes an unfollowed author the user inspects; 30-day think-aloud study | Leading (Outcome) |
| KPI-AV-2 | Network-discovery users | Correctly identify the author DID behind any network search result they are shown (zero faceless network-consensus rows) | 100% — zero attribution loss in any network search / aggregate / shared result | n/a (new behavior; extends KPI-FED-1/2 + KPI-GRAPH-2 to network scale) | Release-gate acceptance test `network_result_preserves_attribution` (every result row carries one author DID; identical claims by different authors render as separate rows) + adversarial review of every search/share renderer + 5-user think-aloud at day-30 | Leading (Guardrail) |
| KPI-AV-3 | Network-discovery users | Trust that every discovered claim is the author's own signed record (signature-verified + CID-matched before indexing) | 100% — zero unverified/unsigned/CID-mismatched claims ever indexed or returned by a search | n/a (new behavior; extends KPI-FED-6 to network-scale ingest) | Release-gate adversarial fixture acceptance test `indexer_rejects_unverified_claim` (tampered-signature + CID-mismatch fixtures rejected before indexing) + the `[verified]` marker on every result | Leading (Guardrail) |
| KPI-AV-4 | Network-discovery users | Turn a network discovery into a followed peer (discovery → federation funnel), then see those claims in their LOCAL graph | >=30% of dogfood discovery cohort subscribes to >=1 newly-discovered author within 30 days | 0 (no discovery→follow path exists today) | Author-side telemetry: `search.discovery.follow_funnel` event linking a search result to a subsequent `openlore peer add` of a previously-unfollowed author + 30-day survey | Leading (Outcome) |
| KPI-AV-5 | Network-discovery users | Correctly state that discovery indexes only PUBLIC signed claims (public-data framing comprehension) | >=4/5 average on a one-shot post-search comprehension prompt ("what data does discovery index?") for the first 50 search sessions | n/a (new framing; ADR-014 deferred this to slice-05) | One-time post-search CLI prompt (dismissible, opt-in) + the up-front public-data banner | Leading (Outcome) |
| KPI-AV-6 | Network-discovery users | Share a network search result as a stable link a teammate opens to see the same attributed, verified claims | >=1 shared discovery link per dogfood discovery cohort within 30 days (realizing the J-004 shareable-link signal) | 0 (no shareable query link exists today; J-004 success signal deferred from slice-02/04) | Author-side telemetry: `search.share.link_emitted` + `search.share.link_opened` events + 30-day survey ("did you share an openlore discovery with a teammate?") | Leading (Outcome) |

### Metric Hierarchy

- **North Star**: **KPI-AV-1** — % of dogfood discovery sessions that surface a
  relevant signed claim by an author the user does NOT already follow. This is the
  behavioral validation of J-005: the entire point of the AppView is to close the
  J-001 "undiscoverable" gap at network scale — letting a developer find aligned
  reasoning WITHOUT first knowing whom to follow. The slice's value evaporates if
  users CAN search the network but never discover anything beyond what their local
  graph already showed them.
- **Leading Indicators**: KPI-AV-4 (discovery→federation funnel — proves discovery
  is decision-relevant enough to grow the trusted local graph, not just idle
  curiosity), KPI-AV-6 (shared-link usage — proves a discovery became a shareable
  decision artifact), KPI-AV-5 (public-data framing comprehension — proves the
  honesty framing landed).
- **Guardrail Metrics**: **KPI-AV-2 (anti-merging in NETWORK aggregates — zero
  attribution loss)** and **KPI-AV-3 (signature-verified-before-index — zero
  unverified claims indexed)**. Both MUST hold; any failure is unshippable. These
  are the two cardinal trust guarantees that distinguish the OpenLore AppView from
  the centralized aggregators the whole product exists to replace. (Also inherits
  KPI-5 local-first and KPI-4 round-trip as guardrails — see mapping below.)

### Mapping to inherited KPIs

| Inherited KPI | Status in slice-05 |
|---|---|
| KPI-4 (slice-01: zero silent normalization, 100% round-trip identity) | Inherited UNCHANGED — a discovered claim's displayed fields match the author's published record byte-for-byte; the indexer normalizes nothing (it indexes the signed record as-is, verified). The `--show` CID-recompute-matches-published display enforces it at network scale. |
| KPI-5 (slice-01: local-first, network-disabled correctness) | Inherited and REINFORCED-WITH-A-TENSION — the AppView is a network service (the architectural shift), BUT compose/sign/own-claim/local-query flows still succeed with the network disabled, and `search` degrades to a clear local-only message when the index is unreachable. KPI-5 remains a guardrail: any release that breaks offline compose/sign is blocked. (This is the load-bearing local-first↔network-service tension flagged for DESIGN.) |
| KPI-FED-1 (slice-03: 100% attribution fidelity) | Inherited and EXTENDED to NETWORK scale — KPI-AV-2 carries the anti-merging guarantee into network aggregates: a network search result is always per-author-attributed, never a faceless consensus row. |
| KPI-FED-2 (slice-03: zero merged-consensus rows) | Inherited and EXTENDED — no "the network says X" merged row exists anywhere in search or shared-link output; identical claims by different authors stay separate (KPI-AV-2). |
| KPI-FED-6 (slice-03: zero invalid signatures stored, pull-time verification) | Inherited and EXTENDED to NETWORK-scale ingest — KPI-AV-3 carries the verify-signature-and-recompute-CID-before-accepting gate from peer pulls to network indexing. No unverified claim is ever indexed. (CAVEAT inherited: production multibase pubkey decode from a real PLC DID document was a slice-03 deferred TODO; slice-05 DESIGN must resolve it for true network-scale verification — flagged as a DESIGN dependency / risk.) |
| KPI-GRAPH-2 (slice-04: anti-merging in aggregates) | Inherited and EXTENDED — slice-04 carried anti-merging into LOCAL aggregates; slice-05 carries it into NETWORK aggregates (the same `no_cross_table_join_elides_author` xtask discipline extends to the index query path). |
| WD-10 / I-6 (display-only buckets; numeric-only persistence) | Inherited UNCHANGED — the index stores/serves numeric confidence; display buckets are render-only in search results exactly as in local query. |

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| KPI-AV-1 | author-side telemetry + day-30 think-aloud | tracing event `search.discovery.unfollowed_author_hit` + manual session | continuous + once | DEVOPS (telemetry), nw-product-owner (session) |
| KPI-AV-2 | release-gate acceptance test + adversarial review + dogfood think-aloud | automated test (CI) + manual review + manual session (day-30) | continuous + once per release + once | DEVOPS (CI), nw-product-owner (review/session) |
| KPI-AV-3 | release-gate adversarial fixture acceptance test | automated test (CI) driving tampered/CID-mismatch fixtures | continuous | DEVOPS (CI) |
| KPI-AV-4 | author-side telemetry + 30-day survey | tracing event `search.discovery.follow_funnel` + survey form | continuous + once | DEVOPS (telemetry), nw-product-owner (survey) |
| KPI-AV-5 | one-shot post-search comprehension prompt | dismissible opt-in CLI prompt | first 50 sessions | nw-product-owner (copy), DEVOPS (delivery hook) |
| KPI-AV-6 | author-side telemetry + 30-day survey | tracing events `search.share.link_emitted` / `link_opened` + survey | continuous + once | DEVOPS (telemetry), nw-product-owner (survey) |

### Hypothesis

We believe that **an indexer service that aggregates many authors' PUBLIC signed
claims from across the network — verifying each signature and recomputing each CID
before indexing, preserving per-author attribution on every result, and exposing a
`search` surface by philosophy / subject / contributor with a one-step path back
into the slice-03 follow flow** will achieve **a 30-day dogfood discovery cohort
that discovers relevant claims by previously-unfollowed authors in >=60% of
sessions and subscribes to >=1 newly-discovered author — without ever being shown
a faceless network-consensus row or an unverified claim, and without the network
service compromising the local-first compose/sign flows.**

We will know this is true when **dogfood discovery users surface >=1 claim by an
unfollowed author in 60% of sessions within 30 days, AND >=30% subscribe to a
discovered author (the discovery→federation funnel closes), AND every result they
were shown was signature-verified and per-author-attributed, AND their offline
compose/sign flows still worked throughout.**

### Disprovers (kill criteria for the appview-search hypothesis)

These outcomes would kill the slice-05 hypothesis and force a re-design:

1. **KPI-AV-2 < 100%**: any attribution loss / faceless network-consensus row in a
   search or shared result is a fatal failure of the trust model carried from
   slice-03/04 into network scale. The AppView would be indistinguishable from the
   aggregators the product exists to replace. UNSHIPPABLE.
2. **KPI-AV-3 < 100%**: any unverified / unsigned / CID-mismatched claim entering
   the index or being returned by a search breaks the J-005 trust precondition.
   Discovery would serve potentially fabricated reasoning. UNSHIPPABLE.
3. **KPI-5 regression**: if adding the network service breaks offline compose/sign
   (the local-first guardrail), the architectural shift has compromised the core
   promise. UNSHIPPABLE (the AppView must be additive, never load-bearing for
   authoring).
4. **KPI-AV-1 < 20%**: a near-zero unfollowed-author-discovery rate suggests the
   index is too sparse, too biased toward already-followed authors, or the search
   does not actually surface anything beyond the local graph — the J-005 value
   thesis is weakened. Re-investigate index coverage and the search UX before
   investing further (e.g., before any web AppView).
5. **KPI-AV-4 < 10%**: a near-zero discovery→follow funnel suggests discovery is
   idle curiosity that never grows the trusted local graph — the AppView is not
   strengthening local-first. Re-investigate the follow affordance friction.

### Handoff to DEVOPS

The platform-architect needs these from this document to plan instrumentation:

1. **Data collection requirements**:
   - Author-side tracing event `search.discovery.unfollowed_author_hit{object_or_subject_or_contributor, unfollowed_author_count}` when a search result includes an unfollowed author the user inspects (KPI-AV-1).
   - Author-side tracing event `search.discovery.follow_funnel{discovered_did, time_from_search_to_add}` linking a search result to a subsequent `openlore peer add` of a previously-unfollowed author (KPI-AV-4).
   - Author-side tracing events `search.share.link_emitted` / `search.share.link_opened` (KPI-AV-6).
   - Indexer-side counters `indexer.ingest.verified` vs `indexer.ingest.rejected{reason: bad_signature|cid_mismatch|unsigned}` (KPI-AV-3 — the rejection counter must stay > 0 only for adversarial fixtures, never for legitimately-indexed claims).
   - NO telemetry on the CONTENTS of discovered claims (only structural counts + DIDs the user already saw); the public-data framing does not extend to user-behavior surveillance.

2. **Dashboard/monitoring needs**:
   - KPI-AV-1 dashboard: % of discovery sessions with >=1 unfollowed-author hit per 30-day window (dogfood-only initially).
   - KPI-AV-4 dashboard: discovery→follow funnel conversion rate per 30-day window.
   - KPI-AV-3 dashboard: indexer ingest verified-vs-rejected ratio; the rejected-for-real-claims count must be 0.
   - Index freshness / coverage dashboard: claims indexed, distinct authors indexed, ingest lag (feeds the KPI-AV-1 sparsity diagnosis).

3. **Alerting thresholds**:
   - Alert if any CI run reports KPI-AV-2 != 100% (release-blocking).
   - Alert if any CI run reports KPI-AV-3 != 100% (a legitimately-signed claim rejected, OR any unverified claim indexed — release-blocking).
   - Alert if KPI-5 regression detected (offline compose/sign breaks — release-blocking).
   - Informational alert if KPI-AV-1 < 30% at day-30 (escalate to PO; informs whether index coverage is the bottleneck) and if index ingest lag exceeds a DESIGN-defined freshness budget.

4. **Baseline measurement**: no baselines needed; all KPIs are for new behavior.
   KPI-AV-1, KPI-AV-4, and KPI-AV-6 baselines are implicitly 0 (no network
   discovery, no discovery→follow funnel, and no shareable query link exist today).
   KPI-AV-5 (public-data framing) is a new comprehension metric; baseline n/a.
