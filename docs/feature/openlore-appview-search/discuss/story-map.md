# Story Map: openlore-appview-search (slice-05)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

## User: P-002 Researcher / Tech Lead (network-discovery hat)

Secondary persona: P-001 Senior Engineer Solo Builder (wears the network-discovery
hat when discovering aligned projects/maintainers before committing to a stack).

## Goal

Discover signed claims — and the people behind them — from across the WHOLE
network, without first knowing whom to follow: search by philosophy, subject, or
contributor; trust each result because it is signature-verified and per-author
attributed; and turn a discovery into a followed peer that grows the trusted LOCAL
graph — all while the CLI + local store remain the source of truth.

## Backbone

| Index the network | Search the network | Trust a result | Act on a discovery |
|------|------|------|------|
| Ingest public signed claims | Search by object (philosophy) | `[verified]` marker on every result | Follow a discovered author (`peer add`) |
| Verify signature + recompute CID | Search by contributor / subject | `--show <cid>` signature + CID-match | Share a result as a stable link |
| Keep per-author attribution | (Anti-merging at network scale) | Public-data banner up front | (Discovery → federation funnel) |

The backbone is the lifetime of a single discovery: the network is indexed (and
kept trustworthy), the user searches it, trusts what they find, and acts on it
(follow / share). This is Activity 5 ("Explore") of the umbrella story map, now
delivered at NETWORK scale.

---

## Walking Skeleton (slice-05 walking skeleton)

The thinnest slice that exercises discover-trustworthily end-to-end at network
scale:

1. **Index the network** — `openlore-indexer` ingests PUBLIC signed claims from
   across the network, verifying each signature + recomputing each CID before
   indexing, keeping a non-`Option` author DID on every record (US-AV-001).
2. **Search by philosophy** — `openlore search --object <philosophy>` returns
   signature-verified, per-author-attributed claims from across the network,
   including authors the user does NOT follow (US-AV-002).
3. **Trust the result** — every result carries a `[verified]` marker and a
   public-data banner; `--show <cid>` confirms "Signature: VERIFIED against
   <did>" + "CID recomputed, matches published record" (US-AV-004).

This skeleton validates:

- The indexer can aggregate many authors' public claims and serve a search,
  WITHOUT ever indexing an unsigned/tampered claim (KPI-AV-3 guardrail).
- A network search surfaces claims by authors the user does NOT already follow —
  the J-005 discoverability-at-scale thesis (KPI-AV-1 north star, baseline).
- Anti-merging holds at NETWORK scale: every result is per-author attributed; no
  faceless consensus row (KPI-AV-2 guardrail, baseline).
- Local-first survives the architectural shift: compose/sign still work offline;
  `search` degrades to local-only when the index is unreachable (KPI-5 preserved).

It does NOT include `--contributor`/`--subject` search, the follow funnel, or the
shareable link — those are deliberately later releases. The walking skeleton is
the thinnest proof that "trustworthy network discovery by philosophy" holds.

---

## Release 1 — Walking Skeleton (target outcome: trustworthy network discovery by philosophy works end-to-end)

| Story | Target outcome | KPI |
|---|---|---|
| US-AV-001 | Bootstrap the indexer + verified, attributed ingest pipeline (`@infrastructure`) | supports KPI-AV-1..4 (the trust + attribution + discovery preconditions) |
| US-AV-002 | Search by philosophy (object) at network scale, attribution preserved | KPI-AV-1 (discover an unfollowed author — north star, baseline), KPI-AV-2 (anti-merging at scale, baseline) |
| US-AV-004 | Trust a discovered result — `[verified]` marker + `--show` + public-data banner | KPI-AV-3 (signature-verified before index — guardrail), KPI-AV-5 (public-data framing) |

**Rationale**: this is the minimum bundle that disproves the slice-05 hypothesis
if it fails. Without an indexer that aggregates the network AND a search that
surfaces unfollowed authors AND a visible trust contract, J-005 ("discover
across the network without knowing whom to follow") is unmet. The riskiest
assumptions — "an index can be trustworthy (verified + attributed) at network
scale" and "search surfaces something beyond the local graph" — are validated
here. US-AV-004 is in the walking skeleton (not deferred) because a discovery
surface the user cannot trust has no value — verification visibility is
load-bearing, not a nicety.

**Demo gate (Phase 3.5)**: User runs
`openlore search --object org.openlore.philosophy.reproducible-builds` over an
index populated with claims from authors they do NOT follow. The output shows a
public-data banner, lists verified claims by unfollowed authors (each with author
DID + `[verified]`), no merged consensus row, and `--show <cid>` confirms the
signature is verified against the author DID and the CID matches the published
record. Compose/sign still succeed with the network disabled.

---

## Release 2 — Discovery dimensions + funnel (target outcome: discovery feeds the trusted local graph)

| Story | Target outcome | KPI |
|---|---|---|
| US-AV-003 | Search by contributor or subject at network scale (read before following) | KPI-AV-1 (discovery via contributor/subject), KPI-AV-2 (anti-merging) |
| US-AV-005 | Subscribe to a discovered author straight from a result (discovery → federation) | KPI-AV-4 (discovery→federation funnel — the funnel-closing behavior) |

**Rationale**: Release 2 completes the discovery dimensions (contributor + subject,
mirroring slice-04 over the network corpus) and CLOSES the funnel — the behavior
that makes the AppView strengthen the local-first graph instead of competing with
it. It is sequenced AFTER the walking skeleton because the funnel (US-AV-005)
depends on search results that distinguish followed vs unfollowed authors
(US-AV-002/003), and the contributor lens (US-AV-003) is the natural "read before
you follow" step that precedes a subscribe. If Release 1 had a latent attribution
or verification bug, it surfaces there first rather than corrupting the funnel.

**Demo gate**: Maria discovers Priya (an unfollowed author) via `openlore search
--contributor github:priya`, reads Priya's whole verified network reasoning trail,
then runs the `openlore peer add did:plc:priya-test` affordance from the result;
after `openlore peer pull`, Priya's claims appear in Maria's LOCAL `graph query`
and `--weighted` views.

---

## Release 3 — Shareable discovery (target outcome: a discovery becomes a shareable decision artifact)

| Story | Target outcome | KPI |
|---|---|---|
| US-AV-006 | Share a network search result as a stable link | KPI-AV-6 (shared-link usage — realizes the J-004 shareable-link signal), KPI-AV-2 (anti-merging across the share boundary) |

**Rationale**: `--share` ships LAST because discovery is fully usable without it
(Releases 1–2 deliver trustworthy discovery + the follow funnel). Sharing realizes
the J-004 "shareable as a link to a query" success signal that was deferred from
slice-02/04, turning a discovery into an ADR-citable artifact. The worst case
without it ("I can discover and follow, but can only paste terminal text to a
teammate") is survivable; the worst case for Release 1 (an untrusted or
attribution-losing index) is not. It is also the surface most prone to scope creep
(a web AppView could balloon it), so it is isolated last and kept to a
shareable-link + resolver contract — a full presentational web UI is explicitly
deferred (see the deferred table).

**Demo gate**: Maria runs `openlore search --object reproducible-builds --share`,
gets a stable link, and Tobias opens it to see the same attributed, verified
claims (each with author DID + `[verified]`), including the ability to follow any
discovered author.

---

## Priority Rationale

Priority order: **Release 1 (Walking Skeleton) > Release 2 (Dimensions + funnel) > Release 3 (Shareable discovery)**.

The ordering is set by outcome impact and risk-of-failure consequence, NOT by
feature volume or implementation order:

1. **Release 1 first** because if a trustworthy network index + a search that
   surfaces unfollowed authors does not work — or if it loses attribution or
   indexes an unverified claim — the slice-05 thesis (J-005: discover across the
   network without knowing whom to follow, trustworthily) is disproven. The two
   riskiest assumptions are "an index can be verified + attributed at network
   scale" and "search surfaces something beyond the local graph." Validating both
   is the walking skeleton (per `nw-user-story-mapping` "Riskiest Assumption
   First"). US-AV-001 (`@infrastructure`) is bundled here because the indexer +
   verified-attributed ingest it provides are prerequisites for every user-visible
   story, and US-AV-004 (trust marker) is bundled because an untrustable discovery
   surface has no value.

2. **Release 2 second** because the contributor/subject dimensions + the
   discovery→federation funnel deliver the behavior that makes the AppView
   STRENGTHEN the local-first model (KPI-AV-4) rather than compete with it. It is
   the highest-value behavior change after the walking skeleton, but it benefits
   from being built on a stable, trusted search foundation — if Release 1 has a
   latent attribution/verification bug, it surfaces during Release 1 rather than
   feeding bad data into a follow decision.

3. **Release 3 third** because the shareable link is a discovery-amplification
   deepening, not a primary outcome. Discovery is usable and trustworthy without
   it for the first weeks of dogfooding. The worst case ("I can discover + follow
   but can only paste terminal text") is survivable; the worst case for Release 1
   (untrusted or attribution-losing discovery) is unsurvivable. Isolating it last
   also fights scope creep: the shareable-link contract is held to a stable,
   query-encoding, attribution-preserving link + resolver — a full web AppView is
   out of scope.

This ordering preserves the carpaccio principle: each release is independently
demo-able and delivers a verifiable working behavior. Release 1 alone is a
shippable end-to-end slice (trustworthy network discovery by philosophy). Release
2 adds the dimensions + funnel. Release 3 adds shareability.

---

## What is NOT in scope (explicitly deferred — fighting scope creep)

slice-05 is the umbrella's "easiest to scope-creep" slice (per the foundation
story-map). This table is the hard line. Most of these are deferred to a FUTURE
slice or are deliberate permanent exclusions, NOT just later releases of this
feature.

| Out-of-scope | Why deferred | Future home |
|---|---|---|
| A full presentational WEB AppView / browser UI | The slice delivers the indexer + CLI `search` discovery surface + a shareable-link contract. A full web UI is the single biggest scope-creep risk and is presentational over the same index; the CLI must prove the discovery model first | Future slice (post-slice-05); DESIGN may scope only a MINIMAL link resolver if it keeps Release 3 right-sized (OD-AV-6) |
| ATProto Firehose / real-time push ingestion | ADR-016 locked OUT push subscriptions for slice-03 with a "re-evaluate at slice-05" note. Firehose is a DESIGN OPTION, not a slice-05 requirement; pull-based indexing may suffice for the walking skeleton | DESIGN decision this slice (OD-AV-4); if not chosen now, a future ingestion-mode slice |
| Cross-user / cohort SCORING across many users' graphs | slice-04 scored the LOCAL graph; the indexer enables cross-user aggregation, but a full cohort-scoring surface (weighted/triangulated across the network) is a distinct large outcome. slice-05 ships network SEARCH + per-author attribution; network-scale WEIGHTING is its own future slice | Future slice (network-scoring); slice-05 surfaces attributed results, not network-wide adherence weights |
| Persisting / federating a derived score or ranking | Inherited from slice-04 WD-72: weights/scores are DERIVED + DISPLAY-ONLY. The index stores VERIFIED CLAIMS, never derived aggregates. A shareable link encodes a QUERY, never a frozen merged snapshot | Never (deliberate) |
| A "network consensus" / merged-view feature | The cardinal anti-merging invariant (I-FED-1 → I-GRAPH-1/2 → KPI-AV-2): every result preserves per-author attribution. A merged network-consensus row is forbidden, not deferred | Never (deliberate) |
| Indexing PRIVATE or non-signed data | Public-data-only framing (ADR-014 deferred to slice-05). The indexer ingests ONLY public signed claims; private data and surveillance affordances are forbidden | Never (deliberate) |
| The indexer signing / publishing / mutating claims | The AppView is a READ/discovery surface; it holds no signing capability by construction (mirrors slice-02 `adapter-github`). The CLI + signed claims remain the source of truth | Never (deliberate) |
| Auto-subscribe / "follow all results" | Following is always an explicit human action (US-AV-005). Auto-follow would betray the slice-03 sovereignty model | Never (deliberate) |
| Trust/reputation scoring by author identity | A per-author trust score is a separate JTBD (inherited slice-04 deferral); slice-05 ranks/filters by evidence + verification, not by reputation | Post-slice-05 |
| Resolving production multibase (z6Mk...) PLC DID-document pubkeys | A slice-03 deferred TODO (DV-4 test-only seam). True network-scale verification (KPI-AV-3) needs real PLC pubkey decode; this is a DESIGN dependency/risk for slice-05, not a separate user story | DESIGN dependency this slice (flagged as a risk); see feature-delta risks |
| New write surface of any kind beyond reusing slice-03 `peer add` | slice-05 is discovery (read) only; the only state change a user makes is following a discovered author via the existing slice-03 path | Out of scope by design |
