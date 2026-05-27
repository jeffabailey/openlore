# Visual Journey — author-counter-claim

- **Feature**: openlore-federated-read (slice-03)
- **Wave**: DISCUSS
- **Date**: 2026-05-27
- **Owner**: Luna (nw-product-owner)
- **Persona**: P-002 (federation-reader hat) primary; P-001 also exercises
- **Job**: J-003 (sub-job J-003b — counter-claim as first-class disagreement)
- **Structured schema**: `docs/product/journeys/author-counter-claim.yaml`
- **Precondition**: user has completed steps 1-3 of `subscribe-and-read-federated`

## Flow at a glance

```
       Trigger: Maria disagrees with Rachel's claim about cargo
                              |
                              v
+------------+   +------------+   +------------+   +------------+
| Step 1     |-->| Step 2     |-->| Step 3     |-->| Step 4     |
| identify   |   | compose    |   | sign +     |   | observe in |
| target CID |   | counter    |   | publish    |   | federated  |
|            |   | (--reason) |   |            |   | query      |
+------------+   +------------+   +------------+   +------------+
  Irritated       Targeted        Considered      Publicly-staked
                                                        |
                                                        v
                                                   Validated
```

## Emotional arc — irritation-to-considered-public-stake

The load-bearing emotional transition is **step 2** (compose with `--reason`).
The `--reason` flag is REQUIRED and forces the user to articulate their
disagreement in writing before they sign anything. This converts irritation
into a considered public stake.

This is the J-003b hypothesis under test: making disagreement a first-class
structured artifact, rather than a reply thread, will change how engineers
disagree. Slice-03 ships the mechanism; the behavioral validation lives at
day-30 (see KPI-FED-3 in `outcome-kpis.md`).

The counter-claim flow deliberately REUSES the slice-01 compose-sign-publish
pipeline (US-001..US-003) — not a separate code path. This is enforced by
ADR-003's single-publish-path invariant. The user-visible difference is:

- Extra required flag `--reason`.
- Sugar verb `claim counter <peer_cid>` (which internally constructs an
  `--counters` reference via the existing `references[]` mechanism from
  ADR-008).
- Preview shows the target's author DID and the "counter-claims coexist,
  never overwrite" framing.

## Step 1 — identify the target claim

```
$ openlore graph query --subject github:rust-lang/cargo --federated

...
Author: did:plc:rachel-test (subscribed peer)
  - embodiesPhilosophy  org.openlore.philosophy.dependency-pinning
    confidence  0.91  (triangulated)
    evidence    https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
    cid         bafy...n4ka                       <-- counter-claim target
    composed_at 2026-05-22T09:18:44Z

Tip: `openlore claim counter bafy...n4ka --reason ...` to publish a counter-claim.
```

**Why the tip line lists a real CID**: making the counter-claim CLI
discoverable in-context lowers the activation energy for J-003b. Without it,
the user has to memorize the verb structure AND type the CID — friction the
journey cannot afford if disagreement is to feel as light as it must.

**Feels**: entry Irritated -> exit Targeted.

## Step 2 — compose the counter-claim

```
$ openlore claim counter bafy...n4ka \
    --reason   "Cargo's dependency pinning is opt-in, not philosophical; pinning is a tool, not a value." \
    --subject  github:rust-lang/cargo \
    --predicate embodiesPhilosophy \
    --object   org.openlore.philosophy.dependency-pinning \
    --evidence https://github.com/rust-lang/cargo/blob/master/CONTRIBUTING.md \
    --confidence 0.72

Composing counter-claim (not yet signed, not yet published)
-----------------------------------------------------------
  counters    : bafy...n4ka  (by did:plc:rachel-test)
  subject     : github:rust-lang/cargo
  predicate   : embodiesPhilosophy
  object      : org.openlore.philosophy.dependency-pinning
  evidence    : https://github.com/rust-lang/cargo/blob/master/CONTRIBUTING.md
  confidence  : 0.72  (weighted)
  reason      : Cargo's dependency pinning is opt-in, not philosophical;
                pinning is a tool, not a value.
  author      : did:plc:maria-test
  composed_at : 2026-05-27T10:14:32Z
  references  : [{ cid: bafy...n4ka, type: counters }]

This is YOUR reasoning. It will be signed and published as a claim,
not as truth. The peer's original claim REMAINS visible; counter-claims
coexist, never overwrite.

Press Enter to sign, Ctrl-C to cancel.
```

**Why "not as truth" appears again**: SAME literal text as US-001 from
openlore-foundation. This is content-frozen across slices. Counter-claim
authoring is still claim authoring; the J-001 anxiety mitigation applies.

**Why "counter-claims coexist, never overwrite" appears**: this addresses the
J-001 brigading anxiety from the counter-author's side. The user about to
publish a counter-claim should be reassured the peer's original is not
destroyed.

**Why `--reason` is REQUIRED**: silent counter-claims are anti-social. The
reason is the artifact's disagreement intent; without it the system cannot
distinguish a structured disagreement from a duplicate claim. This is a hard
gate enforced pre-compose.

**Feels**: entry Targeted -> exit Considered.

## Step 3 — sign and publish

```
Signing with did:plc:maria-test ... ok
Computing claim CID            ... bafy...new

Written to local store:
  path : ~/.local/share/openlore/claims/bafy...new.json
  cid  : bafy...new
  size : 587 bytes
  type : counter-claim (counters bafy...n4ka)

Publish to your PDS now? [Y/n] y

Publishing to https://pds.example.com ...
  record collection : org.openlore.claim
  record rkey       : bafy...new
  ... ok (HTTP 200, 138ms)

Published.
  at-uri : at://did:plc:maria-test/org.openlore.claim/bafy...new

Your counter-claim is now public. Rachel's original claim
(bafy...n4ka) remains visible on her PDS and in federated queries;
nothing has been overwritten.

Tip: `openlore claim retract bafy...new` if you change your mind.
```

**Why this reuses the slice-01 publish path**: ADR-003 single-publish-path
invariant. The architecture forbids a parallel publish code path for
counter-claims; same VerbClaimPublish internals, same idempotency contract,
same failure semantics. The counter-claim is just a claim with
`references[].type == counters`.

**Why the success message reminds about retract**: counter-claims are also
retractable. If Maria changes her mind, she can issue a `claim retract` that
references her own counter-claim CID. The Lexicon supports this without any
new fields.

**Feels**: entry Considered -> exit Publicly-staked.

## Step 4 — observe the counter-relationship

```
$ openlore graph query --subject github:rust-lang/cargo --federated

Claims about github:rust-lang/cargo (4 found across 2 authors)
===============================================================

Author: did:plc:maria-test (you)
  - embodiesPhilosophy  org.openlore.philosophy.reproducible-builds
    ...
    cid         bafy...m9pq

  - embodiesPhilosophy  org.openlore.philosophy.dependency-pinning
    confidence  0.72  (weighted)
    evidence    https://github.com/rust-lang/cargo/blob/master/CONTRIBUTING.md
    counters    bafy...n4ka by did:plc:rachel-test
    reason      Cargo's dependency pinning is opt-in, not philosophical;
                pinning is a tool, not a value.
    cid         bafy...new

Author: did:plc:rachel-test (subscribed peer)
  - embodiesPhilosophy  org.openlore.philosophy.dependency-pinning
    confidence  0.91  (triangulated)
    evidence    https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
    countered-by bafy...new by did:plc:maria-test
    cid         bafy...n4ka

  - embodiesPhilosophy  org.openlore.philosophy.reproducible-builds
    confidence  0.65  (weighted)
    evidence    https://github.com/rust-lang/cargo/issues/5359
    cid         bafy...x7ts

4 claims displayed; 1 counter-relationship; 0 merged.
```

**Why bidirectional annotation**: the counter-claim row shows "counters X" AND
the target row shows "countered-by X". The user can scan the output and see
the disagreement structure without re-querying.

**Why "0 merged" is in the summary line**: same anti-merging guarantee as the
subscribe-and-read journey. Even with a counter-relationship between two
claims, the system does NOT collapse them into a single "consensus" row.

**Feels**: entry Publicly-staked -> exit Validated.

## Shared artifacts highlighted

| Artifact | First appears | Reused at | Risk |
|---|---|---|---|
| `target_cid` | step 1 | steps 2, 3, 4 | HIGH — drift breaks counter-relationship |
| `counter_claim_reason` | step 2 | steps 2, 4 | MEDIUM — display fidelity matters |
| `counter_claim_cid` | step 3 | step 4 | HIGH — same as any claim CID |
| `at_uri` | step 3 | step 4 | MEDIUM — derived value, must match |

Full registry: `shared-artifacts-registry.md` (this directory).

## Cross-journey consistency invariants

- The `--reason` text is persisted in the signed claim payload as a first-class field; it is NOT a side-channel comment.
- The `counters` reference type is the SAME enum value used by `claim retract` (per ADR-008): `pub enum ReferenceType { Retracts, Counters, ... }`. No new enum variant added — slice-03 just uses an existing one.
- The compose preview's "not as truth" literal is the SAME wording as US-001. Content-frozen across slices.
- The single publish path (ADR-003) is preserved: `VerbClaimCounter` constructs a counter-claim, then calls into `VerbClaimPublish` internals. No new publish code path.

## Failure scenarios summary

| Step | Mode | User-visible behavior |
|---|---|---|
| 1 | Target CID typo'd or stale | "No claim found with CID <X>"; non-zero exit |
| 2 | --reason missing | Pre-compose rejection; "counter-claims require --reason" |
| 2 | --reason too long (>1000 chars) | Pre-compose rejection; size hint; suggest separate evidentiary claim |
| 2 | User counters own claim | Pre-compose rejection; "use `claim retract` instead" |
| 2 | User has already countered this CID | Warn + confirm to add a second counter-claim |
| 3 | PDS unreachable | Local counter-claim preserved; retry hint (mirrors US-003) |
| 3 | PDS rejects record | Raw error + actionable hint; local intact |
| 3 | Signing key inaccessible | Prompt to unlock keychain |
| 4 | Counter target peer purged | Annotation degrades to "counters bafy...n4ka (peer not subscribed)" |
| 4 | Counter-claim signature invalid on re-read | Row marked "[signature invalid]"; exit non-zero |
