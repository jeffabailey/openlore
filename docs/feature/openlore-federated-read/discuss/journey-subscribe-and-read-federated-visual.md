# Visual Journey — subscribe-and-read-federated

- **Feature**: openlore-federated-read (slice-03)
- **Wave**: DISCUSS
- **Date**: 2026-05-27
- **Owner**: Luna (nw-product-owner)
- **Persona**: P-002 Researcher / Tech Lead (federation-reader hat) — primary; P-001 (Solo Builder) also wears this hat
- **Job**: J-003 (sub-jobs J-003a anti-merging, J-003c revocability)
- **Structured schema**: `docs/product/journeys/subscribe-and-read-federated.yaml`

This document is the human-readable companion to the YAML schema. It captures
the visual flow, the emotional arc, and the per-step TUI mockups in one place
so the reviewer and DESIGN wave can read the journey without context-switching.

## Flow at a glance

```
                Trigger: Maria wants to read Rachel's takes on Cargo
                              |
                              v
+------------+   +------------+   +------------+   +------------+
| Step 1     |-->| Step 2     |-->| Step 3     |-->| Step 4     |
| peer add   |   | peer pull  |   | graph query|   | peer remove|
|            |   |            |   | --federated|   | --purge    |
|            |   |            |   |            |   | (optional) |
+------------+   +------------+   +------------+   +------------+
  Curious-but-     Acknowledged    Trust-          Sovereign-
  cautious                         building        confident
                                                       |
                                                       v
                                                  Reversed-cleanly
```

## Emotional arc — discovery-with-explicit-sovereignty-buffer

The load-bearing emotional moment is **step 2 -> step 3**. Step 2 (pull) writes
peer claims to a SEPARATE store from the user's own claims; step 3 (query)
displays them under explicit author DID groupings. Without that separation +
attribution, J-003 collapses into "yet another aggregator that hides
provenance" — the exact thing P-002 currently uses (HN, Reddit, awesome-lists)
and is dissatisfied with.

The journey deliberately does NOT auto-pull on subscribe. The pull is a separate
explicit step so the user can subscribe to many DIDs cheaply and decide later
when to fetch. This also lets the trust-building moment happen at the user's
pace.

Step 4 is optional (end-of-journey). It exists so the user feels subscription
is reversible (J-003c — the revocability anxiety). Half the journeys this
feature must support will end at step 3; that is fine. Step 4 is the safety
valve.

## Step 1 — peer add

```
$ openlore peer add did:plc:rachel-test

Resolving DID did:plc:rachel-test ... ok
  handle           : rachel.example.com
  PDS              : https://pds.example.com
  claim collection : org.openlore.claim  (lexicon ok)

Adding peer to subscription list ... ok
  subscribed_at    : 2026-05-27T10:14:32Z
  next pull        : on-demand (`openlore peer pull`)
  local layer      : peer_claims (separate from your own claims)

Tip: peer claims will appear in `openlore graph query --federated <subject>`.
     To unsubscribe later: `openlore peer remove did:plc:rachel-test`.
```

**Why the "separate from your own claims" hint is load-bearing**: it
front-loads the anti-merging guarantee. The user learns at the moment of
subscription that this will NOT contaminate their own claims.

**Feels**: entry Curious-but-cautious -> exit Acknowledged.

## Step 2 — peer pull

```
$ openlore peer pull

Pulling claims from 2 subscribed peers...

  did:plc:rachel-test (rachel.example.com)
    fetched   : 7 records
    new       : 5 (2 already in peer_claims, skipped)
    verified  : 5/5 signatures valid against rachel's DID document
    stored    : peer_claims (attribution preserved per record)

  did:plc:tobias-test (tobias.example.com)
    fetched   : 3 records
    new       : 3
    verified  : 3/3 signatures valid
    stored    : peer_claims

Pulled 8 new peer claims in 1.4s.
None merged with your own claims; query with --federated to see them.
```

**Why per-peer signature verification is load-bearing**: it is the only thing
that protects the user from an adversarial peer publishing claims with someone
ELSE's DID in the author field. The pull MUST recompute the CID locally and
verify the signature against the peer's DID-document key. Anything that fails
this check is rejected at ingest, not silently stored.

**Feels**: entry Acknowledged -> exit Trust-building.

## Step 3 — graph query --federated

```
$ openlore graph query --subject github:rust-lang/cargo --federated

Claims about github:rust-lang/cargo (3 found across 2 authors)
===============================================================

Author: did:plc:maria-test (you)
  - embodiesPhilosophy  org.openlore.philosophy.reproducible-builds
    confidence  0.78  (well-evidenced)
    evidence    https://doc.rust-lang.org/cargo/reference/build-cache.html
    cid         bafy...m9pq
    composed_at 2026-05-26T14:02:11Z

Author: did:plc:rachel-test (subscribed peer)
  - embodiesPhilosophy  org.openlore.philosophy.dependency-pinning
    confidence  0.91  (triangulated)
    evidence    https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
    cid         bafy...n4ka
    composed_at 2026-05-22T09:18:44Z

  - embodiesPhilosophy  org.openlore.philosophy.reproducible-builds
    confidence  0.65  (weighted)
    evidence    https://github.com/rust-lang/cargo/issues/5359
    cid         bafy...x7ts
    composed_at 2026-05-20T11:33:02Z

Showing your claims + claims from 1 subscribed peer with claims on this subject.
Each claim is attributed to its author DID. No claims are merged.

Tip: `openlore claim counter bafy...x7ts --reason ...` to publish a counter-claim.
```

**Why the "No claims are merged." line is load-bearing**: this is the
anti-merging guarantee made visible. Even when two authors have claims with
the SAME subject + predicate + object (Rachel and Maria both claim
`reproducible-builds`), the output displays them as separate author-attributed
rows, NEVER as a single "Both authors agree" row.

**Why the tip line at the bottom names a specific CID**: it makes
counter-claim authoring discoverable in-context. The user does not need to
remember the `claim counter` verb syntax; they copy-paste the suggestion.

**Feels**: entry Trust-building -> exit Sovereign-confident.

## Step 4 — peer remove --purge (optional)

```
$ openlore peer remove did:plc:rachel-test --purge

Removing peer did:plc:rachel-test ...
  subscribed_at  : 2026-05-27T10:14:32Z
  peer_claims    : 5 records cached
  --purge active : will delete cached records

This is irreversible. Re-running `openlore peer add did:plc:rachel-test`
and `openlore peer pull` will re-fetch them.

Proceed? [y/N]: y

Removed subscription.
Purged 5 cached peer claims attributed to did:plc:rachel-test.
Your own claims (in author_claims) are unaffected.
```

**Why confirmation is REQUIRED for `--purge`**: this is the only destructive
operation in the slice-03 surface. Slice-03 does NOT ship a `--yes` flag for
this — defer to slice-04 when scripting needs justify it. The hard
confirmation is the user's "I really mean this" moment.

**Why `peer remove` WITHOUT `--purge` is also offered**: soft-remove (cancel
subscription, retain cache) is the safer default. The user can decide later
whether to delete the cache. This is the JTBD anxiety mitigation for J-003c:
unsubscribe is cheap and never destroys data by accident.

**Feels**: entry Sovereign-confident -> exit Reversed-cleanly.

## Shared artifacts highlighted

| Artifact | First appears | Reused at | Risk |
|---|---|---|---|
| `peer_did` | step 1 | steps 2, 3, 4 | HIGH — drift would corrupt attribution |
| `peer_claim_cid` | step 2 | step 3, counter-claim journey | HIGH — drift = federation thesis broken |
| `peer_pds_endpoint` | step 1 | step 2 | MEDIUM — peer can rotate PDS legitimately |
| `subscribed_at` | step 1 | step 4 (diagnostics) | LOW |

Full registry: `shared-artifacts-registry.md` (this directory).

## Anti-merging guarantee (cross-cutting)

This is the single most load-bearing invariant of the whole feature. It is
called out separately because it spans the slice — not a step-local concern.

- **At ingest (step 2)**: peer claims go to a SEPARATE `peer_claims` store, not the author's `author_claims`. The schema MUST enforce that no cross-table JOIN may collapse the author column.
- **At query (step 3)**: output is grouped by author DID. Even when two authors have identical (subject, predicate, object), they appear as separate rows.
- **At display (any UI surface)**: NO summary line says "consensus" or "agreement" without the author DIDs being visible adjacent.
- **At test time (acceptance suite)**: a dedicated test `federation_attribution_preserved` asserts every output row has a distinct (author_did, claim_cid) tuple.

## Failure scenarios summary

| Step | Mode | User-visible behavior |
|---|---|---|
| 1 | Peer DID does not resolve | Subscription NOT added; non-zero exit; resolution error printed |
| 1 | Peer has no claim collection yet | Warn but allow subscription (peer may publish later) |
| 1 | Same peer added twice | Idempotent; exits 0 with "already subscribed since X" |
| 2 | Peer PDS unreachable | Other peers proceed; this peer's pull skipped; overall exit non-zero |
| 2 | Peer claim signature invalid | That claim rejected; others stored; non-zero exit to flag |
| 2 | Peer claim CID mismatch | That claim rejected; "possible adversarial input" message |
| 2 | Peer schema version unknown | That claim rejected; "upgrade openlore" hint |
| 3 | `--federated` but no peers | Author-only output + hint to `openlore peer add` |
| 3 | Subscribed peer but never pulled | Output flags "1 subscribed peer with no claims pulled" |
| 4 | Peer not subscribed | "Nothing to remove"; exit 0 |
| 4 | `--purge` disk error | Transaction rolls back; subscription remains; retry hint |
