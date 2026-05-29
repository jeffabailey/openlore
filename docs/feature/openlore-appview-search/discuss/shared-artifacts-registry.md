# Shared Artifacts Registry — openlore-appview-search (slice-05)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

This registry tracks every `${variable}` that flows across the
discover-across-the-network journey steps, its single source of truth, and the
integration gate that verifies consistency. slice-05 introduces a NEW corpus (the
network index) but NO new write surface and NO new authoring path. Every indexed
record is a VERIFIED copy of an author's published signed claim; the index
normalizes nothing and stores no derived merged aggregate. The new artifacts
(`verified_marker`, `share_link`) are DERIVED + DISPLAY-ONLY (the marker is an
ingest-gate guarantee; the link encodes a query, never a stored snapshot).

## Artifact table

| Artifact | Source of truth | Consumers | Risk | Validation |
|---|---|---|---|---|
| `subject` | the author's published signed claim (verified into the network index); ultimately `claims.subject` on the author's side | step 1 search arg, step 3 contributor/subject search, step 4 share-link encoding | HIGH — drift breaks result identity across local/network | byte-equal across local + network surfaces; `search_subject_round_trip` |
| `object` (philosophy) | the author's published signed claim (verified into the index) | step 1 `--object` search, step 3 result rows, step 4 share-link encoding | HIGH — drift breaks philosophy grouping + share-link resolution | byte-equal; near-match suggestion engine on miss |
| `author_did` | the SIGNED payload's `author` (verified against the signature at ingest); non-`Option` in every indexed record | every result row header (steps 1-4), `--show`, share-link results | HIGH — drift = attribution loss (anti-merging at network scale) | `network_result_preserves_attribution` + `xtask check-arch` no-elide-author rule extended to the index query path |
| `claim_cid` | recomputed at ingest from the canonicalized signed record; matched against the author's published CID | step 1 row, step 2 `--show` CID-match line | HIGH — the verified, addressable unit; the CID-recompute-matches-published check IS the tamper detector | `indexer_rejects_unverified_claim` (CID-mismatch fixture rejected); `--show` displays recomputed == published |
| `confidence` (numeric) | the author's signed claim (verified into the index) | step 1 row, step 2 `--show` | HIGH — numeric-only persisted/indexed (WD-10 / I-6) | numeric-only; display bucket render-only; the value shown == the value indexed |
| `confidence_bucket` | DERIVED display-only (`claim-domain::confidence_bucket`) | every confidence display in results | MEDIUM — must never be persisted/indexed | inherited WD-10 / I-6: bucket strings never serialized into the index |
| `verified_marker` (`[verified]`) | DERIVED at INGEST by the pure `claim-domain` verification (signature verify + CID recompute); an ingest-gate guarantee, **not a per-result runtime guess** | every result row (steps 1-3), step 2 `--show` verification lines | HIGH — the trust contract; an unverified claim is never indexed, so the marker is universal by construction | `indexer_rejects_unverified_claim` (tampered/unsigned/CID-mismatch fixtures never indexed); every result carries `[verified]` |
| `relationship_label` ((not subscribed)/(subscribed peer)/(you)) | DERIVED from the local `peer_subscriptions` (slice-03) joined against the result's `author_did` | every result row, drives the follow affordance (step 4) | MEDIUM — must match slice-03 labeling; mislabel breaks the discovery→federation funnel | reuses slice-03 relationship-labeling; `(not subscribed)` triggers the `peer add` affordance |
| `share_link` (query-encoding) | DERIVED from the query (dimension + value); **encodes the QUERY, NOT a stored result snapshot** (WD-extends-72) | step 4 `--share` output; resolves back to a step-1 search | MEDIUM — must never freeze a merged snapshot or lose attribution | `share_link_encodes_query_not_snapshot`: opening resolves to current verified per-author results, never a stored merged view |

## Integration gates (handed to DISTILL as acceptance tests)

These are the cross-step consistency checks DESIGN must preserve and DISTILL must
turn into executable acceptance tests.

### Gate 1 — `network_result_preserves_attribution` (LOAD-BEARING, guardrail)

Every network search / aggregate / shared-link result row MUST carry exactly one
`author_did`. No result row may represent a multi-author merged "network
consensus." Two claims with identical (subject, object) by different authors MUST
render as two separate rows. This is the anti-merging-in-NETWORK-aggregates
invariant (extends slice-03 I-FED-1 and slice-04 I-GRAPH-1/2). The
`xtask check-arch` `no_cross_table_join_elides_author` rule extends to cover the
index query path. (KPI-AV-2; release-blocking.)

### Gate 2 — `indexer_rejects_unverified_claim` (LOAD-BEARING, guardrail)

Before any claim enters the index, the indexer MUST verify the author's signature
AND recompute the CID against the published record. A tampered-signature,
unsigned, or CID-mismatched claim MUST be rejected and MUST never enter the index
nor appear in any search result. Drives adversarial fixtures (tampered signature,
CID mismatch, unsigned). Mirrors the slice-03 pull-time verification gate (PP-3/PP-4
precedent) at network scale. (KPI-AV-3; release-blocking.)

> DESIGN dependency / risk: true network-scale verification needs production
> multibase (z6Mk...) PLC DID-document pubkey decode, which slice-03 left as a
> test-only seam (DV-4). DESIGN MUST resolve this for KPI-AV-3 to hold against real
> network data. Flagged in feature-delta risks; the gate is written against the
> same verification contract regardless of the pubkey-decode mechanism.

### Gate 3 — `verified_marker_is_universal` (trust display)

Every search result MUST carry a `[verified]` marker, because verification is an
INGEST precondition (Gate 2), not a per-result runtime check the user must
interpret. There is no `[unverified]` / `[unknown signature]` state in results.
`openlore search --show <cid>` MUST display "Signature: VERIFIED against
<author_did>" and "CID recomputed, matches published record" using the SAME pure
verification result computed at ingest (single source of truth for "verified").

### Gate 4 — `public_data_banner_shown` (honesty framing)

A public-data banner MUST be printed up front for every search session, stating
that indexing covers ONLY public signed claims, each verified before indexing, and
that nothing private is read or aggregated. Realizes the ADR-014-deferred
"claims-are-public" framing. (KPI-AV-5.)

### Gate 5 — `discovery_follow_reuses_slice03_path` (funnel integrity)

A search result that includes an unfollowed author MUST end with a
`openlore peer add <did>` affordance that reuses the slice-03 subscription path
verbatim. There MUST be NO new or parallel subscription mechanism: subscription
state created via the affordance is indistinguishable from a slice-03 `peer add`
(same `peer remove`/`--purge` semantics, zero parallel state). Discovery MUST
never auto-subscribe. (KPI-AV-4.)

### Gate 6 — `share_link_encodes_query_not_snapshot` (shareable + anti-merging across the boundary)

A `--share` link MUST encode the QUERY (dimension + value), NOT a frozen merged
result snapshot. Opening the link MUST re-run the encoded query and resolve to the
CURRENT per-author-attributed, signature-verified results (newly-ingested matching
claims appear); it MUST never resolve to a stored merged view that loses
attribution. (KPI-AV-6; extends Gate 1 across the share boundary.)

### Gate 7 — `local_first_preserved` (the architectural-shift guardrail)

The network discovery surface MUST be additive: compose / sign / own-claim /
local `graph query` flows MUST continue to succeed with the network disabled
(KPI-5). When the index is unreachable, `search` MUST degrade to a clear
local-only/unavailable message and MUST NOT hang, fatally error, or block the
local-first flows. The indexer MUST hold no signing/publishing capability by
construction. (KPI-5 inherited guardrail; release-blocking.)

## Validation questions (checklist)

- Does every `${variable}` in the step mockups have a documented source above? YES.
- Could any network search row hide an author / show a merged consensus? NO — Gate 1 forbids it; the index has no merged-record schema.
- Could an unverified or tampered claim be indexed or shown? NO — Gate 2 rejects it at ingest; Gate 3 makes the `[verified]` marker universal by construction.
- Is any derived value (verified marker, share link) at risk of being persisted as a stored merged aggregate? NO — the marker is an ingest-gate guarantee; the share link encodes a query, not a snapshot (Gate 6).
- Does adding the network service compromise local-first authoring? NO — Gate 7 keeps compose/sign offline-capable and the indexer signing-incapable; `search` degrades gracefully.
- Does discovery introduce a parallel subscription path? NO — Gate 5 reuses slice-03 `peer add` verbatim.
- Is the public-data expectation surfaced honestly? YES — Gate 4 (the ADR-014-deferred framing).
