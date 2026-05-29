# Visual Journey — discover-across-the-network (verified, attributed, network-scale)

- **Feature**: openlore-appview-search (slice-05)
- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)
- **Persona**: P-002 Researcher / Tech Lead (network-discovery hat) — primary; P-001 (Solo Builder) wears the same hat when discovering aligned projects/maintainers before committing to a stack
- **Job**: J-005 (sub-jobs J-005a search-at-network-scale, J-005b index-only-verified-attributed-claims, J-005c discovery-feeds-federation)
- **Structured schema**: `docs/product/journeys/discover-across-the-network.yaml`

This document is the human-readable companion to the YAML schema. It captures the
visual flow, the emotional arc, and the per-step TUI mockups in one place so the
reviewer and DESIGN wave can read the journey without context-switching.

The corpus being searched is the NETWORK INDEX: PUBLIC signed claims aggregated
from across the network by the `openlore-indexer` service — beyond the user's own
claims (slice-01) and beyond the peers they manually subscribed to (slice-03).
**This journey adds a NETWORK SERVICE (the architectural shift from local-first)**,
but the CLI + local signed claims REMAIN the source of truth: the AppView is a
read/discovery surface that never overwrites, merges, signs, or publishes.
Compose/sign/own-claim flows stay 100% local and offline-capable.

## Flow at a glance

```
   Trigger: Maria cares about reproducible-builds but follows NOBODY who claims it
                              |
                              v
+------------+   +------------+   +------------+   +------------+
| Step 1     |-->| Step 2     |-->| Step 3     |-->| Step 4     |
| search the |   | trust the  |   | read before|   | act:       |
| network by |   | result     |   | following  |   | follow /   |
| philosophy |   | (verified, |   | (--contrib |   | share      |
| (--object) |   |  --show)   |   |  / subject)|   |            |
+------------+   +------------+   +------------+   +------------+
  Cold-start       Reassured        Orienting       Connected /
  hopeful          (trust built)    (read trail)    Defensible
```

## Emotional arc — cold-start-hope-to-connected-trust (with a verification buffer)

Pattern: **Discovery Joy** layered over **Problem Relief** (the cold-start
frustration relieved) and **Confidence Building** (trust built before action).

- **Entry**: Cold-start-hopeful-but-wary. Maria cares about reproducible-builds
  but does NOT already follow anyone who claims it — the slice-03/04 local surfaces
  are a dead-end for her. She is hopeful the network has the reasoning she needs,
  but wary: "Is this just another centralized aggregator that collapses
  provenance? Will it serve me a tampered claim? Does a network service betray the
  local-first promise I bought into?"
- **Middle (the verification buffer — load-bearing)**: the FIRST thing she sees is
  a public-data banner ("indexes only PUBLIC signed claims, each verified before
  indexing"). Then results arrive — including authors she does not follow — each
  carrying a `[verified]` marker and a visible author DID. `--show` lets her
  confirm "Signature: VERIFIED against did:plc:priya-test" and "CID recomputed,
  matches published record." This buffer converts the aggregator-distrust anxiety
  into trust BEFORE she acts on anything. (Mirrors the slice-04 transparency
  buffer: trust the data before trusting the discovery.)
- **The "aha"**: a relevant, well-evidenced claim by Priya — someone Maria had
  never heard of — appears in her first search. This is the Discovery-Joy peak:
  the cold-start problem is relieved; she found aligned reasoning without first
  knowing whom to follow.
- **End**: Connected + defensible. Maria reads Priya's whole verified reasoning
  trail (`--contributor`), decides to follow her (`openlore peer add`, the slice-03
  path she already knows), pulls her claims into her LOCAL graph, and optionally
  shares the discovery as a link for her ADR. The AppView strengthened her
  local-first graph rather than replacing it — the discovery→federation funnel
  closed.

The arc deliberately puts TRUST (step 2) immediately AFTER the first results,
BEFORE any action (follow/share, step 4). The user builds trust in the verified,
attributed data BEFORE acting on it. A discovery the user cannot trust — or a
network service that quietly undermined local-first — would re-trigger the
aggregator anxiety the whole product exists to avoid.

## Step 1 — search the network by philosophy (--object)

```
$ openlore search --object org.openlore.philosophy.reproducible-builds

Discovery indexes only PUBLIC signed claims published to authors' PDSs.
Each result is the author's own signed record, signature-verified before indexing.
Nothing private is read or aggregated.

Network results for org.openlore.philosophy.reproducible-builds
(12 signed claims across 7 subjects, 9 distinct authors — all signature-verified)
=================================================================================

Author: did:plc:priya-test (priya.example.com)            (not subscribed)
  - github:bazelbuild/bazel   confidence 0.82 (well-evidenced)  [verified]  bafy...k2
  - github:nixos/nixpkgs      confidence 0.71 (well-evidenced)  [verified]  bafy...q8

Author: did:plc:rachel-test (rachel.example.com)          (subscribed peer)
  - github:nixos/nixpkgs      confidence 0.88 (well-evidenced)  [verified]  bafy...nx99

Author: did:plc:sven-test (sven.example.com)              (not subscribed)
  - github:denoland/deno      confidence 0.65 (weighted)        [verified]  bafy...d3

  ... 6 more authors ...

12 signed claims, 9 distinct authors, all signature-verified.
Every result is one author's signed claim; nothing is merged.

Tip: read an author's whole trail before following:
     `openlore search --contributor did:plc:priya-test`
     Follow a discovered author: `openlore peer add did:plc:priya-test`
```

**Note**: this is the NET-NEW surface of slice-05 — `openlore search` over the
NETWORK INDEX (distinct from the local `openlore graph query` of slice-01/03/04).
The dimensions mirror slice-04 (object/subject/contributor) but the corpus is the
network index, not the local store. Results include authors the user does NOT
follow, labeled `(not subscribed)` — the cold-start discovery payoff. DESIGN owns
whether discovery is a top-level `search` verb or a `--network` flag on
`graph query` (OD-AV-5).

**Feels**: entry Cold-start-hopeful-but-wary -> exit Reassured-and-curious (the
public-data banner + `[verified]` markers begin building trust; the unfollowed-
author hit is the start of the "aha").

## Step 2 — trust the result (verified marker + --show + public-data honesty)

Inspect a discovered result to confirm it is the author's genuine signed record:

```
$ openlore search --object org.openlore.philosophy.reproducible-builds --show bafy...k2

Discovered claim (full record)
==============================
subject     github:bazelbuild/bazel
predicate   embodiesPhilosophy
object      org.openlore.philosophy.reproducible-builds
confidence  0.82  (well-evidenced)
evidence    https://github.com/bazelbuild/bazel/blob/master/docs/...
author      did:plc:priya-test (priya.example.com)   (not subscribed)
cid         bafy...k2

Signature:  VERIFIED against did:plc:priya-test
CID:        bafy...k2  (recomputed, matches published record)

This is Priya's own signed claim, exactly as she published it. It was
signature-verified before being indexed. Discovery never serves an unverified or
fabricated claim.

Follow this author: `openlore peer add did:plc:priya-test`
```

**Why the verification buffer is load-bearing**: discovery at network scale only
has value if the user can trust it. The J-005 anxiety is sharp — "is this just
another aggregator? is this tampered?" The mitigation is the SAME trust contract
slice-03 established for peer pulls (KPI-FED-6: verify signature + recompute CID
before accepting), now made VISIBLE in the discovery surface. Verification happens
at INGEST (US-AV-001), so there is no `[unverified]` state in results to reason
about — every result is `[verified]` by construction. `--show` lets the user
confirm the signature against the author DID and the CID against the published
record.

**Why the public-data banner is load-bearing**: ADR-014 deferred the
"claims-are-public" framing to slice-05. The banner surfaces honestly, BEFORE any
results, that indexing covers ONLY public signed claims and reads nothing private
— mitigating the "did I expose data I did not mean to?" anxiety and the
"surveillance tool?" distrust.

**Feels**: entry Reassured-and-curious -> exit Trusting (the discovery is a
genuine, verified, attributed signed claim — Problem Relief: it is NOT an
aggregator).

## Step 3 — read before following (--contributor / --subject at network scale)

Read a discovered author's whole public reasoning trail BEFORE committing to
follow them:

```
$ openlore search --contributor did:plc:priya-test

Network claims authored by did:plc:priya-test (priya.example.com)   (not subscribed)
(8 verified claims across 6 subjects)
====================================================================================

github:bazelbuild/bazel   embodiesPhilosophy  reproducible-builds   0.82 (well-evidenced) [verified] bafy...k2
github:bazelbuild/bazel   embodiesPhilosophy  hermetic-builds       0.77 (well-evidenced) [verified] bafy...k9
github:facebook/buck2     embodiesPhilosophy  reproducible-builds   0.69 (weighted)       [verified] bafy...b2
github:nixos/nixpkgs      embodiesPhilosophy  reproducible-builds   0.71 (well-evidenced) [verified] bafy...q8
github:pantsbuild/pants   embodiesPhilosophy  hermetic-builds       0.58 (weighted)       [verified] bafy...p4
...

All claims authored by ONE DID (did:plc:priya-test). This is one developer's
reasoning trail, not a community consensus. You do not follow this author.

Follow this author: `openlore peer add did:plc:priya-test`
```

Survey what the network says about a specific PROJECT:

```
$ openlore search --subject github:bazelbuild/bazel

Network claims about github:bazelbuild/bazel (from 5 distinct authors)
======================================================================

Author: did:plc:priya-test  (not subscribed)
  - reproducible-builds   0.82 (well-evidenced)  [verified]  bafy...k2
  - hermetic-builds       0.77 (well-evidenced)  [verified]  bafy...k9

Author: did:plc:dana-test   (not subscribed)
  - documentation-first   0.60 (weighted)        [verified]  bafy...dz

  ... 3 more authors ...

Grouped by author; every claim retains its author DID. No claims are merged.
```

**Why "one developer's reasoning trail, not a community consensus" is load-bearing
here too**: the contributor lens at network scale is exactly where the
anti-merging anxiety is sharpest — a long list of one unfollowed person's claims
could be mistaken for authoritative network truth. The footer keeps the J-005
framing honest. Reading the trail BEFORE following is the "read before you commit"
step that makes the subscribe decision evidence-based, not reputation-based.

**Feels**: entry Trusting -> exit Orienting-toward-a-decision (Maria has read
enough of Priya's trail to decide whether to follow).

## Step 4 — act on the discovery (follow / share)

Follow a discovered author — discovery becomes the front-door to the slice-03
federation flow:

```
$ openlore peer add did:plc:priya-test

Added did:plc:priya-test (priya.example.com) as a subscription.
Next `openlore peer pull` will ingest their claims into your LOCAL graph.

$ openlore peer pull
Pulled 8 claims from did:plc:priya-test (all signature-verified). 8 added to your local graph.

$ openlore graph query --contributor did:plc:priya-test
# Priya's claims now appear in the LOCAL graph and participate in
# `--weighted` and `--traverse` views, exactly like any pulled peer.
```

Share a discovery as a stable link for a teammate / an ADR:

```
$ openlore search --object org.openlore.philosophy.reproducible-builds --share

Shareable link:
  openlore://search?object=org.openlore.philosophy.reproducible-builds

Anyone who opens this runs the same network search and sees the same attributed,
signature-verified claims, each under its author DID. The link encodes the QUERY,
not a frozen snapshot — it always resolves to the current verified results and
never collapses authors into a merged view.
```

**Why the follow funnel is load-bearing**: discovery without a follow path is a
dead-end read. Reusing the slice-03 `openlore peer add` (no parallel subscription
path) makes the AppView STRENGTHEN the local-first graph instead of competing with
it. The funnel is: discover (search) -> read trail (`--contributor`) -> follow
(`peer add`) -> pull (`peer pull`) -> local graph. Following is ALWAYS an explicit
human action; discovery never auto-subscribes.

**Why the shareable link encodes the query, not a snapshot**: a frozen merged
snapshot would lose attribution and go stale. Encoding the QUERY keeps the shared
artifact attribution-preserving and always-current — realizing the J-004
"shareable as a link to a query" success signal without becoming an aggregator.

**Feels**: entry Orienting-toward-a-decision -> exit Connected + Defensible (the
trusted local graph grew from a network discovery; the discovery is shareable as a
decision artifact).

## Shared artifacts highlighted

| Artifact | First appears | Reused at | Risk |
|---|---|---|---|
| `subject` (project URI) | step 1 | steps 1, 3, 4 (share) | HIGH — drift breaks result identity across local/network |
| `object` (philosophy URI) | step 1 | steps 1, 3, 4 (share) | HIGH — drift breaks philosophy grouping + share-link encoding |
| `author_did` (contributor) | step 1 | every result row (steps 1-4) | HIGH — drift = attribution loss (anti-merging at network scale) |
| `claim_cid` | step 1 | step 2 (`--show`), step 4 (share encodes none — query only) | HIGH — the verified, addressable unit; CID-recompute-matches-published is the trust check |
| `confidence` (numeric) | step 1 | step 2 (`--show`) | HIGH — numeric-only persisted/indexed (WD-10); display bucket render-only |
| `verified_marker` (`[verified]`) | step 1 | every result row | HIGH — the trust contract; guaranteed by ingest gate (US-AV-001), never a per-result runtime guess |
| `relationship_label` ((not subscribed)/(subscribed peer)/(you)) | step 1 | steps 1, 3; drives the follow affordance (step 4) | MEDIUM — must match slice-03 labeling; drives the discovery→federation funnel |
| `share_link` (query-encoding) | step 4 | resolves back to a step-1 search | MEDIUM — must encode the QUERY, never a frozen merged snapshot |

Full registry: `shared-artifacts-registry.md` (this directory).

## Trust + anti-merging + local-first guarantees (cross-cutting)

Three load-bearing invariants span the whole slice — called out separately because
they are not step-local concerns.

### Signature-verified-before-index (J-005 trust precondition)

- **At ingest**: the indexer verifies the author's signature AND recomputes the
  CID against the published record BEFORE the claim enters the index. An unsigned,
  tampered, or CID-mismatched claim is rejected and never indexed. Reuses the pure
  `claim-domain` verification core (no second verification path); mirrors slice-03
  pull-time verification (KPI-FED-6) at network scale.
- **At display**: every result carries a `[verified]` marker (guaranteed by the
  ingest gate — there is no `[unverified]` state to interpret); `--show` confirms
  "Signature: VERIFIED against <did>" + "CID recomputed, matches published record".
- **At test time**: `indexer_rejects_unverified_claim` drives tampered-signature
  and CID-mismatch fixtures and asserts they never enter the index nor a search
  result (KPI-AV-3, release-blocking).
- **Public-data honesty**: a banner states indexing covers ONLY public signed
  claims; nothing private is read (the ADR-014-deferred framing).

### Anti-merging in NETWORK aggregates (slice-03 I-FED-1 → slice-04 I-GRAPH-1/2 → network scale)

A network search result is an AGGREGATE OF MANY AUTHORS' CLAIMS, never a merge that
loses attribution.

- **At ingest**: every indexed record carries a non-`Option` author DID; the index
  has NO schema for a merged multi-author "consensus" record.
- **At query**: an aggregate (e.g., all claims for an object) is composed from
  individually-attributed records, never a stored merged row; identical claims by
  different authors stay separate.
- **At display**: every result row retains its author DID and `(not subscribed)` /
  `(subscribed peer)` / `(you)` label; no "the network says X" consensus row.
- **At the share boundary**: a shared link encodes the QUERY (not a frozen merged
  snapshot), so opening it re-composes the per-author-attributed result.
- **At test time**: `network_result_preserves_attribution` asserts every result
  row carries one author DID and identical-content-different-author claims render
  as separate rows; the `xtask check-arch` `no_cross_table_join_elides_author`
  rule (slice-03 I-FED-1) extends to the index query path (KPI-AV-2,
  release-blocking).

### Local-first preserved despite the network-service shift (KPI-5)

The AppView is inherently a network service — the genuine architectural shift this
slice introduces. The local-first promise survives because:

- **Authoring stays local**: compose / sign / own-claim flows are unchanged and
  succeed with the network disabled. The indexer holds no signing capability by
  construction (it cannot author, sign, mutate, or publish — mirrors slice-02
  `adapter-github`).
- **The CLI + signed claims remain the source of truth**: the AppView is additive,
  a read/discovery layer; it never overwrites or merges a local claim.
- **Discovery degrades gracefully**: when the index is unreachable, `search`
  prints a clear local-only/unavailable message and points to the local
  `graph query` surface; it never hangs, errors fatally, or blocks the local
  flows.
- **DESIGN owns the shift's mechanics** (self-hostable vs hosted indexer;
  CLI→indexer transport; degraded-mode mechanism; pull-vs-Firehose). These are
  Open Decisions OD-AV-1..4. The PRODUCT requirement is: local-first authoring is
  never compromised by the network discovery surface.

## Failure scenarios summary

| Step | Mode | User-visible behavior |
|---|---|---|
| 1 | `--object` philosophy URI typo / unknown | "No network claims found for object org.openlore.philosophy.foo. Did you mean ...?" (suggest near-matches); exit 0 |
| 1 | Network index unreachable | "Network index unavailable. Showing LOCAL results only (own + subscribed peers). Run `openlore graph query --object ...` for the local graph." degrades cleanly; never hangs (KPI-5) |
| 1 | Search matches only authors the user already follows | Results shown (still valid), but the KPI-AV-1 unfollowed-author-hit does not fire — diagnostic that the index is too sparse / too local-biased |
| 2 | `--show <cid>` for a CID not in the result set | "CID bafy...nothere is not in this search result. Run the search without --show first." exit non-zero |
| 2 | (cannot happen) an unverified result shown | IMPOSSIBLE by construction — verification is an ingest gate (US-AV-001); unverified claims are never indexed, so never shown. The adversarial fixture proves rejection at ingest, not at display |
| 3 | `--contributor` DID not in the index | "No network claims found for contributor github:... . They may not publish OpenLore claims, or the indexer has not ingested them." exit 0 |
| 3 | `--subject` project with no network claims | "No network claims found for github:... ." exit 0 |
| 4 | Follow an author already subscribed | No redundant follow affordance shown; result labeled `(subscribed peer)` |
| 4 | User never runs `peer add` | No subscription created; discovery is read-only; `openlore peer list` unchanged (no auto-follow) |
| 4 | `--share` link opened after new matching claims ingested | Resolves to CURRENT verified results (includes new claims); never a stale merged snapshot |
| any | Mixed own + subscribed-peer + unfollowed-network authors | All sources participate; each row keeps its author DID, `[verified]` marker, and correct relationship label |
