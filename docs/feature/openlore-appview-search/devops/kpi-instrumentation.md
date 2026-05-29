# KPI-AV Instrumentation — openlore-appview-search (slice-05)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex
- **Source-of-truth KPIs**: `docs/feature/openlore-appview-search/discuss/outcome-kpis.md`
- **Foundation cross-link**: foundation KPI-1..6; slice-03 KPI-FED-1..6; slice-02 KPI-SCR-1..5; slice-04 KPI-GRAPH-1..6 — all in force, unchanged

This document traces each of the 6 outcome KPI-AV targets to **what** measures it,
**where** the data lives, **how** it is read, the **per-user vs cohort split**, and a
**feasibility tag** (GREEN/YELLOW/RED) for slice-05. The two cardinal GUARDRAILS
(KPI-AV-2, KPI-AV-3) + the inherited KPI-5 guardrail get **release-blocking** treatment
— each is a DISCUSS disprover (any failure is UNSHIPPABLE).

## 1. Summary table

| KPI | Type | Instrumentation | Read mechanism | Per-user / cohort split | Feasibility |
|---|---|---|---|---|---|
| KPI-AV-1 (≥60% of discovery sessions surface an unfollowed-author hit in 30d) | Leading / **North Star** | CLI event `search.discovery.unfollowed_author_hit` + post-hoc `scripts/kpi-av-1.jq` + 30-day think-aloud (reuse D-D18 survey) | per-user: `openlore stats --discovery`; cohort: future endpoint OR PO day-30 outreach | per-user GREEN / cohort YELLOW | **GREEN per-user / YELLOW cohort** |
| KPI-AV-2 (100% — zero attribution loss in any network result/aggregate/share) | Leading (**Guardrail**) | AT `at-network-result-preserves-attribution` + the `contract-pact-indexer-query` wire pin + runtime counter `indexer_query_attribution_missing_total` + 5-user day-30 think-aloud | CI status (RELEASE-BLOCKING) + `openlore-indexer stats` | per-user = cohort (CI = cohort property) | **GREEN** |
| KPI-AV-3 (100% — zero unverified/unsigned/CID-mismatched claim ever indexed or returned) | Leading (**Guardrail**) | AT `at-indexer-rejects-unverified-claim` (adversarial fixtures) + the `contract-pact-pds-network` record/DID-doc pin + the ingest/identity probes + runtime counters `indexer_ingest_rejected_total{reason}` | CI status (RELEASE-BLOCKING) + `openlore-indexer stats` | per-user = cohort (CI = cohort property) | **GREEN** |
| KPI-AV-4 (≥30% of discovery cohort subscribes to ≥1 discovered author in 30d) | Leading (Outcome) | CLI event `search.discovery.follow_funnel` + post-hoc `scripts/kpi-av-4.jq` + 30-day survey | per-user: `openlore stats --discovery`; cohort: future endpoint OR PO outreach | per-user GREEN / cohort YELLOW | **GREEN per-user / YELLOW cohort** |
| KPI-AV-5 (≥4/5 public-data framing comprehension, first 50 sessions) | Leading (Outcome) | up-front banner (`at-public-data-banner-shown`) + the one-shot comprehension prompt (D-D18 reuse; `search.public_data_banner_shown` hook) | one-shot CLI prompt + PO aggregation | per-user GREEN (prompt) / cohort YELLOW (PO) | **GREEN per-user prompt / YELLOW cohort** |
| KPI-AV-6 (≥1 shared discovery link per cohort in 30d) | Leading (Outcome) | CLI events `search.share.link_emitted` / `link_opened` + post-hoc `scripts/kpi-av-6.jq` + 30-day survey | per-user: `openlore stats --discovery`; cohort: future endpoint OR PO outreach | per-user GREEN / cohort YELLOW | **GREEN per-user / YELLOW cohort** |

**No KPI-AV is RED.** Every one has a designed capture mechanism that ships in
slice-05. The two cardinal GUARDRAILS (KPI-AV-2/3) + the inherited KPI-5 are fully
GREEN and release-blocking. The YELLOW items (KPI-AV-1/4/5/6 cohort) reflect the SAME
deferred-cohort-endpoint constraint that foundation KPI-3/6, slice-03 KPI-FED-3/5,
slice-02 KPI-SCR-1/5, and slice-04 KPI-GRAPH-1/5/6 cohort carry — NOT a slice-05
capture gap. This is the D-D17 / D-D26 / D-D32 GREEN/YELLOW policy applied to KPI-AV
(locked as D-D40).

## 2. The per-user vs cohort split (the load-bearing instrumentation decision — D-D40)

Per the slice-03 D-D17 / slice-04 D-D32 framework + `outcome-kpis.md` §4 (baselines:
all KPI-AV are new behavior, implicit baseline 0):

- **Per-user signals are FULLY captured in slice-05** for ALL six KPI-AV — the CLI
  emits the privacy-preserving `search.*` events into the author-side log; the
  operator reads `openlore stats --discovery` or the `scripts/kpi-av-*.jq` fallbacks.
  The two GUARDRAILS (KPI-AV-2/3) are CI-gate signals where CI-pass = the property
  holds for EVERY user's binary (the cohort signal IS the CI signal). KPI-5 is the
  same.
- **Cohort aggregation (the % across the dogfood discovery cohort) is YELLOW** for the
  four OUTCOME metrics (KPI-AV-1/4/5/6) — it requires the future opt-in telemetry
  endpoint (ADR-010 revisit; NOT stood up in slice-05 — see `platform-design.md` §11
  Upstream Issue 2) OR PO day-30 outreach to the dogfood cohort. This is the EXACT
  constraint every prior slice carries.
- **The indexer-OPERATOR surface is a NEW per-instance signal** distinct from the
  per-user split: the index-coverage/freshness dashboard (`openlore-indexer stats`) is
  read by the single self-hosted operator, NOT per-user. It feeds the KPI-AV-1 sparsity
  diagnosis (`observability.md` §9). It is per-INSTANCE (one indexer), not per-USER —
  the indexer is per-user-neutral (it does not know who searched).

**The cohort SPLIT-BY rule (the slice-05 privacy tightening)**: where slice-04's
cohort telemetry could send the surfaced-connection BOOLEAN, slice-05's KPI-AV-1/4
cohort rollup may send ONLY the per-session boolean ("surfaced-≥1-unfollowed-author"
/ "followed-a-discovered-author") + the session count — NEVER the dimension VALUE,
NEVER the surfaced/followed `author_did` (a DID stream over telemetry would reveal the
cohort's discovery + subscription graph — the slice-03 peer_did rule + slice-04
connection-target rule, tightened: slice-05 events carry NO DID at all, §observability
§7.1). This is the cohort = "pending opt-in endpoint" split, mirroring slice-03 D-D17
+ slice-04 D-D32, with the slice-05 DID-omission tightening.

## 3. KPI-AV-1 — Unfollowed-author discovery in a session (NORTH STAR)

### What
Per `outcome-kpis.md` §North Star: ≥60% of dogfood discovery sessions surface ≥1
relevant signed claim by an author the user does NOT already follow, within 30 days.
The behavioral validation of J-005 — the AppView's entire point is to close the J-001
"undiscoverable" gap at network scale (find aligned reasoning WITHOUT first knowing
whom to follow). The slice's value evaporates if users CAN search but never discover
anything beyond their local graph.

### Where the data lives
- **Per-user behavioral signal**: CLI event `search.discovery.unfollowed_author_hit{dimension, unfollowed_author_count}` (`observability.md` §2.2), emitted when a `search` result includes ≥1 unfollowed author the user inspects. Counter `search_discovery_unfollowed_hit_total` (`observability.md` §4.1).
- **Session denominator**: count of discovery sessions (`search.executed` with `indexer_reachable=true`) per 30-day window.
- **The coverage diagnosis (the KPI-AV-1 enabler)**: the index-coverage/freshness dashboard (`indexer_distinct_authors_indexed`, `indexer_claims_indexed_total`, `indexer_ingest_lag_seconds`; `observability.md` §9) — the operator's tool to diagnose a LOW KPI-AV-1 (is the index too sparse / too biased toward already-followed authors?). This is the DISCUSS-requested sparsity-diagnosis instrument.
- **Per-user qualitative**: the 30-day think-aloud study (PO-owned) + the D-D18 one-shot Likert survey after the first `search.discovery.unfollowed_author_hit`.

### Read + per-user/cohort split
- **Per-user**: `openlore stats --discovery` shows `Unfollowed-author hits: N`; the % needs the session join (`scripts/kpi-av-1.jq`, post-hoc). GREEN.
- **Cohort %**: requires the future opt-in endpoint OR PO day-30 outreach. YELLOW. Telemetry rule: ONLY the per-session boolean + the session count can ever be sent; the dimension VALUE + the unfollowed `author_did` are NEVER sent (§2 split-by rule).

### Feasibility: GREEN per-user / YELLOW cohort
Event + survey + the coverage dashboard ship in slice-05. The KPI-AV-1 < 20% disprover
triggers a coverage/UX re-investigation (NOT a release gate — it is a post-release
outcome metric; informational alert at day-30 < 30%, `observability.md` §10).

## 4. KPI-AV-2 — Zero attribution loss at network scale (GUARDRAIL, RELEASE-BLOCKING)

### What
100% attribution fidelity in any network search / aggregate / shared result; ZERO
faceless network-consensus rows. Extends slice-03 KPI-FED-1/2 + slice-04 KPI-GRAPH-2
into NETWORK scale. A cardinal disprover (any failure UNSHIPPABLE).

### Where the data lives (the three-layer enforcement, WD-120)
- **CI signal (PRIMARY)**: `at-network-result-preserves-attribution` (`ci-cd-pipeline.md` §3.2) — two distinct-author claims on one (subject,object) → two attributed result rows; `distinct_author_count == 2`; no merged row; no `consensus`/`merged` table; aggregation in pure Rust not SQL.
- **Contract signal**: `contract-pact-indexer-query` pins that EVERY wire result element carries `author_did` (the anti-merging-across-the-transport pin; B1, `contract-test-ownership.md` §2).
- **Structural signal**: `xtask check-arch` `no_cross_table_join_elides_author` EXTENDED to the `adapter-index-store` SQL literals (WD-120 layer 2; `ci-cd-pipeline.md` §2.1 rule 1).
- **Type-level signal**: `IndexedClaim`/`NetworkResultRow.author_did` is non-`Option<Did>`; `compose_results` returns a per-author structure with no merged-row API (WD-120 layer 1; compile error if dropped).
- **Runtime signal**: counter `indexer_query_attribution_missing_total` (target 0 forever; non-zero is a P0 bug — mirror of slice-03/04 attribution-missing counters).

### Read + per-user/cohort split
- **CI**: GitHub Actions check; RELEASE-BLOCKING via branch protection. A failing AT is the KPI-AV-2 disprover alert.
- **Per-user = cohort**: CI-pass = EVERY user's binary has the property (the three-layer enforcement means a single-layer bypass is caught by ≥1 other). No cohort-endpoint dependency.

### Feasibility: GREEN. Release-blocking.

## 5. KPI-AV-3 — Verified-before-index (GUARDRAIL, RELEASE-BLOCKING)

### What
100% — zero unverified/unsigned/CID-mismatched claim ever indexed or returned by a
search. Extends slice-03 KPI-FED-6 (pull-time verification) to network-scale INGEST,
against REAL network data (the ADR-026 production PLC decode resolving the slice-03
DV-4 seam). A cardinal disprover (any failure UNSHIPPABLE).

### Where the data lives
- **CI signal (PRIMARY)**: `at-indexer-rejects-unverified-claim` (`ci-cd-pipeline.md` §3.1) — adversarial fixtures (tampered-sig / CID-mismatch / unsigned / `did_unresolvable`) are REJECTED; none enter the index; legitimately-signed records DO enter (a false-positive reject is ALSO a failure); the gate reuses `claim_domain::verify` (no second path); uses the REAL `decode_ed25519_multibase` (a seam-only pass fails).
- **Contract signal**: `contract-pact-pds-network` pins the `listRecords` record shape + the PLC DID-doc/`publicKeyMultibase` `z6Mk...` shape the gate + the ADR-026 decode depend on (B2, `contract-test-ownership.md` §3); release-tag re-verifies against REAL `bsky.social` + `plc.directory` (the confirmation KPI-AV-3 holds against real data — the cardinal slice-05 concern the DV-4 seam left open).
- **Probe signal**: the `adapter-atproto-ingest` probe rejects a fixture tampered/CID-mismatch record at startup; the `adapter-atproto-did` resolve-only probe decodes a real `z6Mk...` + verifies/rejects correctly (`observability.md` §2.8).
- **Mutation signal**: `appview-domain` (`ingest_decision`) + the `claim-domain` decode helper in the nightly mutation scope (≥95% kill rate; `ci-cd-pipeline.md` §4) — a surviving gate/decode mutant means a tampered record could slip past without test failure.
- **Structural signal**: `no_pubkey_seam_in_release_build` (`ci-cd-pipeline.md` §2.1 rule 3) — production uses the real decode; the test seam is release-forbidden (I-AV-6).
- **Runtime signal**: counters `indexer_ingest_rejected_total{reason}` + `indexer_ingest_verified_total` (`observability.md` §4.2) — the verified-vs-rejected ratio; the rejected-for-real-claims count must be 0; the `did_unresolvable` count flags PLC-resolution trouble.

### Read + per-user/cohort split
- **CI**: GitHub Actions check; RELEASE-BLOCKING. A failing AT (or a mutation regression in `ingest_decision`/decode) is the KPI-AV-3 disprover alert.
- **Per-user = cohort**: CI-pass = the property holds for every indexer; the verify gate is the same pure core for every instance. The operator reads `openlore-indexer stats` for the runtime verified/rejected ratio.

### Feasibility: GREEN. Release-blocking.

## 6. KPI-AV-4 — Discovery → federation funnel

### What
≥30% of the dogfood discovery cohort subscribes to ≥1 newly-discovered author within
30 days, then sees those claims in their LOCAL graph. The funnel that makes the
AppView STRENGTHEN the local-first graph (the KPI-AV-1 leading indicator).

### Where the data lives
- **Per-user behavioral signal**: CLI event `search.discovery.follow_funnel{time_from_search_to_add_seconds, was_previously_unfollowed}` (`observability.md` §2.3), linking a `search`-surfaced unfollowed author to a subsequent `openlore peer add`. The affordance reuses `peer add` verbatim (WD-122/I-AV-7); the AT `at-discovery-follow-reuses-slice03-path` asserts the reuse (no parallel state; claims appear in local `graph query` after `peer add` + `peer pull`).
- **Per-user qualitative**: 30-day survey (PO-owned).

### Read + per-user/cohort split
- **Per-user**: `openlore stats --discovery` shows the funnel count (`scripts/kpi-av-4.jq`). GREEN.
- **Cohort %**: future endpoint OR PO outreach. YELLOW. Telemetry: only the per-session funnel boolean + the elapsed-time histogram; NEVER the DID (§2 split-by rule).

### Feasibility: GREEN per-user / YELLOW cohort.

## 7. KPI-AV-5 — Public-data framing comprehension

### What
≥4/5 average on a one-shot post-search comprehension prompt ("what data does discovery
index?") for the first 50 search sessions. Proves the honesty framing (public-data-only,
WD-105) landed.

### Where the data lives
- **The always-on framing**: the up-front public-data banner (every search; `at-public-data-banner-shown`; KPI-AV-5/I-AV-4).
- **The measurement**: a one-shot, dismissible, opt-in CLI comprehension prompt after the FIRST `search` (the `search.public_data_banner_shown{first_session: true}` hook, `observability.md` §2.5, §8), reusing the D-D18 one-shot file-presence survey mechanism. Response at `$XDG_DATA_HOME/openlore/surveys/post-search-comprehension.response.json`; free-text optional, never telemetry-sent.

### Read + per-user/cohort split
- **Per-user**: the prompt response file. GREEN.
- **Cohort (avg ≥4/5 over first 50 sessions)**: PO out-of-band aggregation (the survey scores; never the free text). YELLOW — same posture as foundation KPI-3 felt-framing.

### Feasibility: GREEN per-user prompt / YELLOW cohort. There is no honest way to
instrument "comprehension" without the prompt; the survey is the right mechanism.

## 8. KPI-AV-6 — Shareable discovery link

### What
≥1 shared discovery link per dogfood discovery cohort within 30 days (realizing the
J-004 shareable-link signal deferred from slice-02/04). The KPI-AV-1 leading indicator
(a discovery became a shareable decision artifact).

### Where the data lives
- **Per-user behavioral signal**: CLI events `search.share.link_emitted` / `search.share.link_opened` (`observability.md` §2.4). `--share` encodes the query, not a snapshot (WD-122/I-AV-8; `at-share-link-encodes-query-not-snapshot` asserts the re-resolution to current per-author results).
- **Per-user qualitative**: 30-day survey ("did you share an openlore discovery with a teammate?").

### Read + per-user/cohort split
- **Per-user**: `openlore stats --discovery` (`scripts/kpi-av-6.jq`). GREEN.
- **Cohort (≥1 per cohort)**: future endpoint OR PO outreach. YELLOW. `link_opened` is observable only on the SAME machine (a teammate opening it is on THEIR machine — that is the survey's job, not telemetry).

### Feasibility: GREEN per-user / YELLOW cohort.

## 9. Mapping to inherited KPIs (per `outcome-kpis.md` §Mapping)

| Inherited KPI | Status in slice-05 instrumentation |
|---|---|
| KPI-4 (slice-01: zero silent normalization, 100% round-trip identity) | Inherited UNCHANGED. A discovered claim's displayed fields match the author's published record byte-for-byte (the indexer normalizes nothing; it indexes the signed record as-is, verified). The `--show` CID-recompute-matches-published display (`at-search-show-trust`) enforces it at network scale; the foundation round-trip discipline is unchanged. |
| KPI-5 (slice-01: local-first, network-disabled correctness) | Inherited and REINFORCED-WITH-A-TENSION (the headline). The AppView IS a network service, BUT compose/sign/own-claim/local-query still succeed network-disabled, and `search` degrades gracefully. **RELEASE-BLOCKING** via `at-local-first-preserved` (`ci-cd-pipeline.md` §3.3); any release breaking offline compose/sign is blocked. The CLI links no indexer code (the `xtask check-arch` CLI-dep-graph exclusion). |
| KPI-FED-1 (slice-03: 100% attribution fidelity) | Inherited and EXTENDED to network scale — KPI-AV-2 carries anti-merging into network aggregates (per-author-attributed, never a faceless consensus row). The slice-03 runtime counter pattern is mirrored by `indexer_query_attribution_missing_total`. |
| KPI-FED-2 (slice-03: zero merged-consensus rows) | Inherited and EXTENDED — no "the network says X" merged row exists anywhere in search or shared-link output; identical claims by different authors stay separate (KPI-AV-2). The renderer-review checklist gains the slice-05 line (D-D41). |
| KPI-FED-6 (slice-03: pull-time verification; zero invalid signatures stored) | Inherited and EXTENDED to network-scale INGEST — KPI-AV-3 carries verify-signature-and-recompute-CID-before-accepting from peer pulls to network indexing, against REAL network data (the ADR-026 production decode resolving the DV-4 seam). |
| KPI-GRAPH-2 (slice-04: anti-merging in aggregates) | Inherited and EXTENDED — the `no_cross_table_join_elides_author` xtask discipline extends from the local scoring/traversal SQL to the index-store SQL (KPI-AV-2). |
| WD-10 / I-6 (display-only buckets; numeric-only persistence) | Inherited UNCHANGED — the index stores/serves numeric confidence; display buckets are render-only in search results exactly as in local query (the no-persist scan extends to `index.duckdb` + `indexed_claims/` artifacts; `design/data-models.md` §"Confidence buckets stay UNPERSISTED"). |

## 10. KPI-AV sign-off readiness

| KPI-AV | Per-user instrumented in slice-05? | Cohort aggregated? | Status |
|---|---|---|---|
| KPI-AV-1 | YES (`search.discovery.unfollowed_author_hit` + survey + coverage dashboard) | NO (future endpoint OR PO day-30 outreach) | **GREEN per-user / YELLOW cohort** |
| KPI-AV-2 | YES (CI gate + contract pin + type guarantee + structural rule + runtime counter) | YES (CI = cohort property) | **GREEN** (release-blocking) |
| KPI-AV-3 | YES (CI gate + contract pin + probes + mutation scope + structural rule + runtime counters) | YES (CI = cohort property) | **GREEN** (release-blocking) |
| KPI-AV-4 | YES (`search.discovery.follow_funnel` + AT + survey) | NO (future endpoint OR PO outreach) | **GREEN per-user / YELLOW cohort** |
| KPI-AV-5 | YES (one-shot comprehension prompt + banner AT) | NO (PO survey aggregation) | **GREEN per-user prompt / YELLOW cohort** |
| KPI-AV-6 | YES (`search.share.link_*` + AT + survey) | NO (future endpoint OR PO outreach) | **GREEN per-user / YELLOW cohort** |
| KPI-5 (inherited guardrail) | YES (`at-local-first-preserved` CI gate) | YES (CI = cohort property) | **GREEN** (release-blocking) |

**No KPI-AV is RED.** All six (+ the inherited KPI-5) are at minimum per-user-readable
or PO-surveyable from the slice-05 release. The two cardinal GUARDRAILS (KPI-AV-2/3) +
KPI-5 are fully GREEN AND release-blocking. The YELLOW items (KPI-AV-1/4/5/6 cohort)
reflect the same deferred-cohort-endpoint / PO-outreach constraint every prior slice
carries — the D-D17 / D-D26 / D-D32 policy applied to KPI-AV (D-D40). The slice IS
shippable with these YELLOWs: the per-user signal suffices for dogfood PO outreach
within 30 days; cohort aggregation is the future telemetry endpoint's job (NOT
slice-05 — `platform-design.md` §11 Upstream Issue 2 corrects the slice-04
forward-expectation).

## 11. Cross-references

- `observability.md` delta §2 — event names backing KPI-AV-1..6 measurement; §2.6 the indexer-operator events; §7 the per-user/cohort telemetry rules; §9 the coverage dashboard
- `observability.md` delta §4 — metric rows; §10 alerting
- `ci-cd-pipeline.md` delta §3.1 (KPI-AV-3), §3.2 (KPI-AV-2), §3.3 (KPI-5), §3.4 (search ATs), §3.5 (contract sub-jobs), §4 (mutation scope)
- `contract-test-ownership.md` — the two boundary contracts pinning KPI-AV-2 (B1 wire) + KPI-AV-3 (B2 record/DID-doc)
- `platform-design.md` §11 — Upstream Issue 2 (slice-05 does NOT stand up the cohort telemetry endpoint)
- `wave-decisions.md` delta — D-D35..D-D43
- `discuss/outcome-kpis.md` — authoritative KPI-AV definitions + §Disprovers + §Handoff to DEVOPS
- ADR-010 — telemetry-opt-in (governs the YELLOW cohort path; the indexer-operator surface is distinct)
