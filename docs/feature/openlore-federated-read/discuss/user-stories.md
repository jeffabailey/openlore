<!-- markdownlint-disable MD024 -->

# User Stories — openlore-federated-read (slice-03)

All stories in this file belong to **slice-03-federated-read** (the second
sibling feature after openlore-foundation). Every story carries a `job_id`
traceable to `docs/product/jobs.yaml` per Decision 1. Stories US-FED-001..005
carry mandatory Elevator Pitches; US-FED-006 is `@infrastructure` and carries
an `infrastructure_rationale` instead.

## System Constraints

These are cross-cutting constraints that apply to every story in this feature.
The first six are **inherited verbatim from openlore-foundation/user-stories.md**
and are repeated here for the reviewer's convenience. They are NOT relitigated.

- **Local-first**: every flow must remain functional with the network offline up to the publication boundary OR the peer-pull boundary. `peer add` and `peer pull` are the only network operations introduced by this slice. Federated queries on already-pulled peer claims work offline.
- **Solution-neutral**: stories describe user-observable behavior. The choice between schema additions for peer_claims (separate DuckDB table vs separate database file vs separate store crate) is reserved for DESIGN.
- **Claims-not-truth invariant**: the literal text "not as truth" still appears in any compose preview (including counter-claim compose). No UI surface frames any claim as a truth assertion.
- **Attribution-preserving**: every claim shown anywhere must show its author DID. No "merged consensus" rendering across authors, ever. This is the load-bearing J-003a invariant — `federation_attribution_preserved` integration test enforces it.
- **Retraction without deletion**: counter-claims are also claims; they are retractable but not deletable. Soft-retract semantics from WD-11 apply.
- **CLI-first**: the CLI remains the canonical interface; no web UI in slice-03.

Constraints introduced new by this slice:

- **Separate stores for author and peer claims**: author's own claims live in `author_claims` (same as slice-01); peer claims live in `peer_claims`. No JOIN that collapses the author column is permitted. (Implementation: DESIGN picks single-DB-two-tables OR two-database-files; the contract is "no cross-table query may emit a row without the author DID.")
- **Per-claim signature verification at pull time**: every peer claim's signature MUST verify against the peer's DID-document public key before being stored. Verification failure rejects that claim only; other claims in the pull proceed.
- **Per-claim CID recomputation at pull time**: every peer claim's CID MUST be recomputed locally and byte-match the peer's published rkey. Mismatch rejects that claim.
- **Counter-claim verb shape**: `openlore claim counter <target_cid> --reason ...` is sugar for constructing a claim with `references[].type == Counters` pointing at `target_cid`. The verb invokes the SAME compose-sign-publish pipeline as `claim add` (no parallel publish path — preserves ADR-003 invariant).
- **`--reason` is REQUIRED on counter-claims**: silent counter-claims are forbidden. Reason text length: 1..=1000 chars.
- **`peer remove --purge` requires interactive confirmation**: no `--yes` flag in slice-03 (defer to slice-04 when scripting needs justify it).

### Glossary (terms introduced by this slice)

- **Peer**: another developer the user has subscribed to via their ATProto DID.
- **Subscription**: a persisted record that this user has expressed interest in pulling claims from a specific peer DID. Stored in `peer_subscriptions`.
- **Peer claim**: a signed claim authored by a peer (NOT the current user), ingested into `peer_claims` after passing signature + CID verification.
- **Federated query**: `openlore graph query --federated`; includes peer claims with explicit attribution. Without `--federated`, behavior is identical to slice-01.
- **Counter-claim**: a claim with `references[].type == Counters` pointing at a target claim's CID. The target may be the current user's own claim or a peer's. The counter-claim coexists with the target; neither is hidden.

---

## US-FED-001: Subscribe to a peer's claim stream

### Job link

- `job_id`: J-003 (sub-job J-003c addressed indirectly via revocability framing)

### Elevator Pitch

- **Before**: I want to read another developer's signed philosophical claims, but the only way today is to manually scrape their PDS — there is no subscription concept and no separation between their claims and mine.
- **After**: I run `openlore peer add did:plc:rachel-test`, see "Resolving DID ... ok / Adding peer to subscription list ... ok / next pull: on-demand", and the CLI tells me peer claims will appear in `openlore graph query --federated`. Total time: under 5 seconds.
- **Decision enabled**: I can now commit to following a peer's claim stream without their claims polluting my own, and I know exactly how to unsubscribe later — which means I'll actually subscribe to people I'm only mildly curious about.

### Problem

Maria Lopez (P-002) is a tech lead evaluating Rust libraries for a team
project. She has heard that Rachel Chen (did:plc:rachel-test) publishes
thoughtful, evidence-anchored philosophical claims about Rust libraries on
OpenLore. Maria wants to read Rachel's claims alongside her own without
Rachel's opinions silently merging into Maria's view. There is no
subscription concept today: the only path is to manually fetch Rachel's
PDS records and grep them — which is friction-heavy and provides no
ongoing way to refresh.

### Who

- Researcher / Tech Lead (P-002) wearing the federation-reader hat
- Already authenticated against their own ATProto identity (per slice-01 init)
- Comfortable with DIDs and CLI flags
- Has identified a specific peer DID to follow (out-of-band discovery; slice-03 does not own discovery)

### Solution

A `openlore peer add <did>` CLI command that resolves the peer's DID
document, validates that the peer's PDS exposes the `org.openlore.claim`
collection, and persists a subscription record. Subscribing does NOT pull
claims — pull is a separate explicit step (US-FED-002). Re-subscribing to
the same DID is idempotent.

### Domain Examples

#### Example 1 (Happy Path)

Maria Lopez (`did:plc:maria-test`) wants to follow Rachel Chen. She runs:

```
openlore peer add did:plc:rachel-test
```

The CLI resolves the DID (handle: rachel.example.com, PDS: https://pds.example.com),
confirms the claim collection is published, and writes the subscription
record. Output ends with: `Tip: peer claims will appear in 'openlore graph
query --federated <subject>'. To unsubscribe later: 'openlore peer remove
did:plc:rachel-test'.` Total elapsed time: ~3 seconds.

#### Example 2 (Edge / Idempotent re-subscribe)

Tobias Weber (`did:plc:tobias-test`) re-runs `openlore peer add
did:plc:rachel-test` after already subscribing yesterday. The CLI prints
`already subscribed since 2026-05-26T14:02:11Z` and exits 0. The
subscription record is not duplicated.

#### Example 3 (Error / Peer DID does not resolve)

Aanya Krishnan (`did:plc:aanya-test`) accidentally types
`openlore peer add did:plc:not-a-real-did`. The DID resolution fails. The CLI
prints `error: could not resolve did:plc:not-a-real-did (HTTP 404 from
plc.directory)`. No subscription is recorded. Exit code is non-zero.

#### Example 4 (Edge / Peer has no claim collection yet)

Maria adds `did:plc:newbie-test`, a peer who has an ATProto identity but
has never published an OpenLore claim. The CLI warns `peer has not published
any org.openlore.claim records yet; subscription added but will return zero
claims on first pull. Proceed? [y/N]:` On confirmation, the subscription is
recorded.

### UAT Scenarios (BDD)

```gherkin
Scenario: Researcher subscribes to a peer's claim stream
  Given Maria has authenticated as did:plc:maria-test
  And the peer did:plc:rachel-test resolves to handle rachel.example.com
  When Maria runs `openlore peer add did:plc:rachel-test`
  Then the CLI prints a subscription confirmation naming the peer DID and handle
  And a subscription record is persisted in the peer-subscriptions store
  And no peer claims have been pulled yet
  And the CLI displays the next-pull hint and the unsubscribe command

Scenario: Subscribing to an already-subscribed peer is idempotent
  Given Maria has already subscribed to did:plc:rachel-test
  When Maria runs `openlore peer add did:plc:rachel-test` a second time
  Then the CLI prints "already subscribed since" with the original subscription timestamp
  And the subscription record is unchanged
  And exit code is 0

Scenario: Peer DID resolution failure does not record subscription
  Given the peer did:plc:not-a-real-did fails to resolve
  When Aanya runs `openlore peer add did:plc:not-a-real-did`
  Then the CLI exits with a non-zero status
  And the error message names the DID and the resolution failure cause
  And no subscription record is written

Scenario: Subscribing to one's own DID is rejected
  Given Maria has authenticated as did:plc:maria-test
  When Maria runs `openlore peer add did:plc:maria-test`
  Then the CLI exits with a non-zero status
  And the error message is "you are already your own author; cannot subscribe to yourself"
  And no subscription record is written
```

### Acceptance Criteria

- [ ] `openlore peer add <did>` resolves the peer DID via ATProto identity resolution before persisting any subscription.
- [ ] Subscription record contains: peer DID, peer handle, peer PDS endpoint, subscribed_at timestamp.
- [ ] Re-subscribing to an already-subscribed DID is idempotent: no duplicate record, exit 0, message names the original subscription timestamp.
- [ ] DID resolution failure leaves no subscription record and exits non-zero.
- [ ] Subscribing to the current user's own DID is rejected with a clear error.
- [ ] Output includes the unsubscribe hint and the federated-query hint.

### Outcome KPIs

See `outcome-kpis.md` KPI-FED-1 (attribution baseline established at subscription) and KPI-FED-5 (e2e latency includes subscribe step).

### Technical Notes

- Depends on US-FED-006 (peer_subscriptions schema in place).
- ATProto DID resolution reuses `adapter-atproto-did` from slice-01 (no new adapter; new method `resolve_peer(did) -> PeerInfo`).
- The "peer has no claim collection yet" warning requires the adapter to know whether the peer's PDS exposes `org.openlore.claim`. DESIGN's call whether to do a probe-record-fetch or rely on the PDS's collection list endpoint.
- Subscription persistence: new `peer_subscriptions` table/store; DESIGN owns whether this is a new DuckDB table in the existing DB or a separate file.

---

## US-FED-002: Pull peer claims with signature and CID verification

### Job link

- `job_id`: J-003 (sub-job J-003a anti-merging via stored attribution)

### Elevator Pitch

- **Before**: After subscribing to a peer, there is no way to fetch their claims into my local store; I would have to write a custom script against the ATProto API and trust whatever it returns.
- **After**: I run `openlore peer pull` and see "Pulling claims from 2 subscribed peers... rachel: 5 new, 5/5 verified, stored in peer_claims. tobias: 3 new, 3/3 verified, stored in peer_claims. None merged with your own claims." I can now query them in the next story.
- **Decision enabled**: I can ingest a peer's claims with cryptographic confidence that every claim is genuinely from that peer's DID and that nothing has been silently merged into my own claims store — which means I'll actually pull from people whose authority I want to weigh.

### Problem

After subscribing to a peer, Maria needs a way to fetch their published
claims into her local store. The fetch must (a) verify every claim's
signature against the peer's DID-document key so Maria knows the claims
genuinely come from that DID, (b) recompute every claim's CID locally to
detect canonicalization disagreements that would break federation
determinism, and (c) write claims to a SEPARATE store from her own claims
so the anti-merging guarantee holds at the storage layer.

### Who

- Researcher / Tech Lead (P-002), already subscribed to ≥1 peer via US-FED-001
- Online (pull requires network)
- Comfortable with the idea that some peer claims may be rejected at pull time (signature failure, CID mismatch, schema version unknown)

### Solution

A `openlore peer pull` CLI command that iterates every subscribed peer,
fetches their `org.openlore.claim` records via ATProto, verifies each
record's signature against the peer's DID-document key, recomputes each
record's CID locally, and stores verified records in the `peer_claims`
store with attribution preserved per record. Pull is idempotent (re-pulls
skip records already in peer_claims by CID).

### Domain Examples

#### Example 1 (Happy Path)

Maria has subscribed to Rachel (5 claims published) and Tobias (3 claims
published). She runs `openlore peer pull`. The CLI fetches all 8 records,
verifies all 8 signatures successfully, recomputes all 8 CIDs successfully,
and stores all 8 in `peer_claims`. Output reports `Pulled 8 new peer claims
in 1.4s. None merged with your own claims.` Re-running `openlore peer pull`
30 seconds later reports `Pulled 0 new peer claims; 8 already in
peer_claims.`

#### Example 2 (Edge / Some claims rejected, others stored)

Aanya has subscribed to a peer who has 5 published claims, one of which has
a tampered signature (an adversary modified the record after the peer
published). She runs `openlore peer pull`. The CLI verifies 4/5 signatures
successfully and stores those 4. The tampered claim is rejected with
`rejected 1 (signature invalid)`. The 4 valid claims are stored normally.
Exit code is non-zero to flag the rejection.

#### Example 3 (Error / Peer PDS unreachable)

Tobias has subscribed to 3 peers. One peer's PDS is currently down. He runs
`openlore peer pull`. The CLI reports `peer did:plc:down-test: PDS
unreachable (connection refused); skipping`. The other 2 peers proceed
normally. Exit code is non-zero overall to flag that not every peer pull
succeeded.

### UAT Scenarios (BDD)

```gherkin
Scenario: Pulling claims from a subscribed peer
  Given Maria has subscribed to did:plc:rachel-test
  And Rachel's PDS contains 5 org.openlore.claim records
  When Maria runs `openlore peer pull`
  Then the CLI fetches all 5 records from Rachel's PDS
  And each record's signature is verified against Rachel's DID document
  And each record's CID is recomputed locally and matches the peer-published CID
  And all 5 records are stored in the peer_claims store attributed to Rachel
  And none of Maria's own claims (author_claims) are modified
  And the pull summary reports "5 stored, 0 rejected"

Scenario: Peer claim with invalid signature is rejected at ingest
  Given Maria has subscribed to did:plc:rachel-test
  And one of Rachel's 5 records has been tampered (signature does not verify)
  When Maria runs `openlore peer pull`
  Then the tampered record is rejected and NOT stored in peer_claims
  And the other 4 records are stored normally
  And the pull summary reports "4 stored, 1 rejected (signature invalid)"
  And exit code is non-zero

Scenario: Peer claim with CID mismatch is rejected at ingest
  Given Maria has subscribed to did:plc:rachel-test
  And one of Rachel's records has a rkey that does not match its locally-recomputed CID
  When Maria runs `openlore peer pull`
  Then the mismatched record is rejected and NOT stored in peer_claims
  And the rejection reason is reported as "CID mismatch (possible adversarial input)"
  And exit code is non-zero

Scenario: Pull is idempotent — re-pulling skips already-stored claims
  Given Maria has subscribed to did:plc:rachel-test
  And a previous `openlore peer pull` has stored 5 of Rachel's claims
  When Maria runs `openlore peer pull` again with no new records on Rachel's PDS
  Then the CLI reports "0 new, 5 already in peer_claims, skipped"
  And no duplicate records are created
  And exit code is 0
```

### Acceptance Criteria

- [ ] `openlore peer pull` iterates every subscribed peer (from peer_subscriptions store).
- [ ] Every fetched record's signature is verified against the peer's DID-document key BEFORE storage.
- [ ] Every fetched record's CID is recomputed locally and MUST byte-match the peer's published rkey BEFORE storage.
- [ ] Records that fail signature verification OR CID match are rejected; rejection is reported in the pull summary with cause; other records in the pull proceed normally.
- [ ] Stored records go to the `peer_claims` store, NOT to `author_claims`. The current user's own claims are unaffected by any pull.
- [ ] Pull is idempotent: re-running with no new records on peer PDSes reports `0 new` and exits 0.
- [ ] If ANY peer's pull was skipped or had rejections, overall exit code is non-zero (to flag CI / scripts).
- [ ] Pull summary includes per-peer counts (fetched / new / verified / rejected).

### Outcome KPIs

See `outcome-kpis.md` KPI-FED-1 (attribution fidelity), KPI-FED-2 (zero merge), KPI-FED-6 (zero invalid signatures stored).

### Technical Notes

- Depends on US-FED-001 (subscriptions exist) and US-FED-006 (peer_claims schema).
- New port surface: extends `PdsPort` (existing) with `list_peer_records(peer_did) -> Vec<RecordRef>` and `get_peer_record(peer_did, rkey) -> SignedClaim`. OR new `PeerPort` trait (DESIGN's call per ADR-009).
- Signature verification reuses `claim-domain::verify` (existing pure function); the public key resolution is new (from peer's DID document).
- CID recomputation reuses `claim-domain::compute_cid` (existing pure function).
- `peer_claims` store schema: DESIGN owns. Constraint: must support `query_by_subject_across_authors(subject) -> Vec<(author_did, SignedClaim)>` for US-FED-003.

---

## US-FED-003: Read federated graph with per-author attribution

### Job link

- `job_id`: J-003 (sub-job J-003a anti-merging — the load-bearing surface)

### Elevator Pitch

- **Before**: I can query my own claims with `openlore graph query --subject X`, but peer claims I have pulled are invisible to that query — and there is no way to ask "show me what everyone I follow has said about X" without it merging into a faceless aggregate.
- **After**: I run `openlore graph query --subject github:rust-lang/cargo --federated` and see "Claims about github:rust-lang/cargo (3 found across 2 authors)" with my claim under "did:plc:maria-test (you)" and Rachel's two claims under "did:plc:rachel-test (subscribed peer)", each row showing confidence and CID. The footer says "Each claim is attributed to its author DID. No claims are merged."
- **Decision enabled**: I can synthesize a defensible view of multiple developers' opinions on a subject without inheriting any of them silently — which means I'll trust this query enough to base architectural decisions on it.

### Problem

After pulling peer claims, Maria needs to read them in a way that
preserves per-claim attribution so she can weigh each claim against its
author's authority. The query MUST NOT collapse multiple authors' claims
into a single "consensus" row — that would reproduce the exact failure mode
of HN/Reddit aggregators that Maria currently distrusts.

### Who

- Researcher / Tech Lead (P-002), has subscribed to ≥1 peer (US-FED-001) and pulled their claims (US-FED-002)
- May be online or offline (query reads local stores only)
- Wants to read multi-author claims with per-author attribution

### Solution

Extend `openlore graph query` with a `--federated` flag. Without the flag,
behavior is identical to slice-01 (author's own claims only). With the
flag, the query includes peer claims from `peer_claims` AND author claims
from `author_claims`, grouped by author DID in the output. Every claim row
shows the author DID, the confidence, the evidence, the CID, and the
composed_at timestamp. NO output row represents a multi-author aggregate.

### Domain Examples

#### Example 1 (Happy Path)

Maria has 1 of her own claims about `github:rust-lang/cargo` and has pulled
2 of Rachel's claims about the same subject. She runs
`openlore graph query --subject github:rust-lang/cargo --federated`. The
output has 3 rows grouped under 2 author headers. Her claim appears under
"did:plc:maria-test (you)". Rachel's 2 claims appear under
"did:plc:rachel-test (subscribed peer)". Footer: "Showing your claims +
claims from 1 subscribed peer with claims on this subject. Each claim is
attributed to its author DID. No claims are merged."

#### Example 2 (Edge / `--federated` but no peers subscribed)

Tobias runs `openlore graph query --subject github:rust-lang/tokio
--federated` but has not yet subscribed to any peers. The output shows his
own claims (if any) plus a footer: "No peers subscribed. Use `openlore peer
add <did>` to follow a peer's claim stream."

#### Example 3 (Edge / Same subject + predicate + object across two authors)

Aanya has her own claim that `github:rust-lang/cargo` embodies
`reproducible-builds` with confidence 0.78. She has pulled Rachel's claim
asserting the SAME (subject, predicate, object) with confidence 0.65. The
federated query displays BOTH claims as distinct rows under their
respective author headers. There is NO single "Both authors agree" row.

#### Example 4 (Edge / Author-only query unchanged)

Maria has subscribed to and pulled Rachel's claims. She runs
`openlore graph query --subject github:rust-lang/cargo` (WITHOUT `--federated`).
The output shows ONLY her own claims. Footer: "Use --federated to include
1 subscribed peer."

### UAT Scenarios (BDD)

```gherkin
Scenario: Federated query shows attribution-preserved peer claims
  Given Maria has published 1 claim about github:rust-lang/cargo
  And Maria has pulled 2 claims about github:rust-lang/cargo from did:plc:rachel-test
  When Maria runs `openlore graph query --subject github:rust-lang/cargo --federated`
  Then the output displays 3 claims grouped by author DID
  And Maria's claim appears under "did:plc:maria-test (you)"
  And Rachel's 2 claims appear under "did:plc:rachel-test (subscribed peer)"
  And no claim row is labeled as a "merged" or "consensus" entry
  And every claim row displays the author DID, the confidence, and the CID
  And the footer states the count of authors and the no-merge guarantee

Scenario: Identical-content claims by different authors are displayed as separate rows
  Given Aanya has published claim bafy...aanya asserting subject S, predicate P, object O with confidence 0.78
  And Aanya has pulled Rachel's claim bafy...rachel asserting the SAME S, P, O with confidence 0.65
  When Aanya runs `openlore graph query --subject S --federated`
  Then the output displays exactly 2 rows
  And one row is bafy...aanya under "did:plc:aanya-test (you)"
  And the other row is bafy...rachel under "did:plc:rachel-test (subscribed peer)"
  And there is NO row that represents both claims combined

Scenario: Author-only query is unchanged from slice-01 behavior
  Given Maria has subscribed to and pulled Rachel's claims
  When Maria runs `openlore graph query --subject github:rust-lang/cargo`  (without --federated)
  Then the output shows only Maria's own claims
  And the output footer mentions "Use --federated to include 1 subscribed peer"
  And exit code is 0

Scenario: --federated with no peers subscribed
  Given Tobias has not subscribed to any peers
  When Tobias runs `openlore graph query --subject github:rust-lang/tokio --federated`
  Then the output shows only Tobias's own claims
  And the output footer is "No peers subscribed. Use `openlore peer add <did>` to follow a peer's claim stream."
```

### Acceptance Criteria

- [ ] `openlore graph query --subject <S> --federated` returns author claims + peer claims for subject S.
- [ ] Without `--federated`, behavior is identical to slice-01 (author claims only). The flag default is OFF.
- [ ] Output is grouped by author DID, with a header per author identifying the author DID and the relationship (`(you)` or `(subscribed peer)` or `(unsubscribed cache)`).
- [ ] Every claim row displays: author DID, predicate, object, confidence (numeric + display-only bucket label per WD-10), evidence URL, CID, composed_at timestamp.
- [ ] NO output row represents a multi-author aggregate. Two claims with identical (subject, predicate, object) but different authors appear as TWO rows.
- [ ] Footer states the count of distinct authors AND the no-merge guarantee.
- [ ] If `--federated` is requested but no peers are subscribed, output gracefully degrades to author-only with a hint to `peer add`.
- [ ] If `--federated` is requested AND the user has counter-claims, the output annotates them per US-FED-004's `counters` / `countered-by` bidirectional pairing.

### Outcome KPIs

See `outcome-kpis.md` KPI-FED-1 (attribution fidelity — this story is the load-bearing surface) and KPI-FED-2 (zero merged rows — guardrail).

### Technical Notes

- Depends on US-FED-002 (peer_claims populated).
- New query path: extends existing `query_by_subject` to optionally include `peer_claims`. DESIGN owns whether to add a `query_federated_by_subject` method on `StoragePort` or to add a `federated: bool` parameter to the existing method.
- The output renderer is the load-bearing surface for the anti-merging guarantee. DESIGN should consider a renderer-level invariant: "every output row has exactly one author_did; never emit a row without one."
- Counter-claim annotation (`countered-by ...`) requires a join on the `references[]` field across `peer_claims` and `author_claims`. DESIGN owns query shape; consumer-facing contract is "annotation is bidirectional and appears in the same query call, not a separate one."

---

## US-FED-004: Author and publish a counter-claim referencing a peer's claim

### Job link

- `job_id`: J-003 (sub-job J-003b counter-claim as first-class disagreement) + J-001 (the underlying publish flow)

### Elevator Pitch

- **Before**: When I disagree with a peer's claim, my only public option is to write a blog post or post a reply on social media — both of which dissolve provenance and feel like flame-war invitations.
- **After**: I see Rachel's claim CID in my federated query output, run `openlore claim counter bafy...n4ka --reason "..." --subject ... --predicate ... --object ... --evidence ... --confidence 0.72`, see a compose preview that names the counter target's author DID and contains both "not as truth" and "counter-claims coexist, never overwrite", press Enter, then Y to publish. Subsequent federated queries show my counter-claim and Rachel's original side-by-side with bidirectional `counters` / `countered-by` annotations.
- **Decision enabled**: I can publicly stake a structured disagreement with another developer's claim as a permanent attributable artifact rather than a reply thread — which means I'll actually engage with claims I disagree with, instead of muttering "well, that's wrong" privately.

### Problem

Maria has read Rachel's federated claim asserting that
`github:rust-lang/cargo` embodies the `dependency-pinning` philosophy with
high confidence. Maria disagrees: she believes Cargo's dependency pinning
is an opt-in tool, not a philosophical value. Today Maria has no path to
publicly disagree with Rachel's claim in a structured, attributable way.
Her options are blog post (loses structure) or social-media reply (loses
the public-stake quality). She needs a way to publish her disagreement as
its own signed claim that references Rachel's.

### Who

- Researcher / Tech Lead (P-002), reads federated claims regularly, has identified a specific peer claim to disagree with
- Author-engineers (P-001) who occasionally encounter peer claims they disagree with
- Both have the same flow

### Solution

A `openlore claim counter <target_cid> --reason "..." --subject ...
--predicate ... --object ... --evidence ... --confidence ...` CLI command
that constructs a claim with `references[].type == Counters` pointing at
`<target_cid>`, runs it through the same compose-sign-publish pipeline as
`claim add` (preserving the ADR-003 single-publish-path invariant), and
publishes. The compose preview includes counter-specific framing
("counters: <target_cid> (by <peer_author_did>)" + "counter-claims
coexist, never overwrite"). `--reason` is REQUIRED (1..=1000 chars).

### Domain Examples

#### Example 1 (Happy Path)

Maria sees Rachel's bafy...n4ka in her federated query. She runs:

```
openlore claim counter bafy...n4ka \
  --reason   "Cargo's dependency pinning is opt-in, not philosophical; pinning is a tool, not a value." \
  --subject  github:rust-lang/cargo \
  --predicate embodiesPhilosophy \
  --object   org.openlore.philosophy.dependency-pinning \
  --evidence https://github.com/rust-lang/cargo/blob/master/CONTRIBUTING.md \
  --confidence 0.72
```

The compose preview names Rachel's DID, displays "not as truth" and
"counter-claims coexist, never overwrite", shows the reason verbatim,
shows the references[] entry. She presses Enter, then Y. The counter-claim
is signed (CID: bafy...new), persisted locally, published to her PDS.
Subsequent federated query annotates both her counter and Rachel's original
with `counters` / `countered-by` pairs.

#### Example 2 (Edge / Counter one's own claim is rejected)

Aanya accidentally runs `openlore claim counter <her_own_cid> --reason ...
...`. The CLI rejects pre-compose with `error: cannot counter your own
claim; use 'openlore claim retract <cid>' instead`. No file is written.

#### Example 3 (Error / --reason missing)

Tobias runs `openlore claim counter bafy...n4ka --subject ... --predicate
... --object ... --evidence ... --confidence 0.7` (no --reason). The CLI
rejects pre-compose with `error: counter-claims require --reason; explain
your disagreement`. No file is written.

#### Example 4 (Edge / Already-countered claim)

Maria has previously countered Rachel's bafy...n4ka with her own
bafy...prev. She runs `openlore claim counter bafy...n4ka --reason ...`
again. The CLI warns `you have already countered this claim (your previous
counter CID: bafy...prev). Continue and add a second counter-claim? [y/N]:`
On confirmation, a second counter-claim is composed. Both counters are
preserved.

### UAT Scenarios (BDD)

```gherkin
Scenario: Counter-claim compose preview shows target attribution and required framing
  Given Maria has Rachel's claim bafy...n4ka in her peer_claims store
  When Maria runs `openlore claim counter bafy...n4ka --reason "..." --subject S --predicate P --object O --evidence E --confidence 0.72`
  Then the CLI prints a compose preview block
  And the preview lists "counters: bafy...n4ka (by did:plc:rachel-test)"
  And the preview contains the literal text "not as truth"
  And the preview contains the literal text "counter-claims coexist, never overwrite"
  And the preview shows the --reason text verbatim
  And the preview lists the references[] field with one entry of type "counters" pointing at bafy...n4ka
  And no file has been written under ~/.local/share/openlore/
  And no network call has been made

Scenario: Counter-claim requires --reason
  Given Maria has Rachel's claim bafy...n4ka in her peer_claims store
  When Maria runs `openlore claim counter bafy...n4ka` without --reason (other flags valid)
  Then the CLI exits with a non-zero status
  And the error message is "counter-claims require --reason; explain your disagreement"
  And no file has been written
  And no network call has been made

Scenario: Countering one's own claim is rejected
  Given Aanya has her own claim bafy...aanya in her author_claims store
  When Aanya runs `openlore claim counter bafy...aanya --reason "..."` (other flags valid)
  Then the CLI exits with a non-zero status
  And the error message includes "cannot counter your own claim"
  And the error message suggests `openlore claim retract bafy...aanya` as the correct verb
  And no file has been written

Scenario: Counter-claim signs and publishes via the slice-01 publish pipeline
  Given Maria has composed a valid counter-claim against bafy...n4ka
  When Maria confirms the sign prompt and then the publish prompt
  Then the counter-claim is signed with Maria's DID
  And the counter-claim's CID is computed deterministically
  And the counter-claim is persisted to ~/.local/share/openlore/claims/<cid>.json
  And the counter-claim is published via the SAME VerbClaimPublish code path as a regular claim
  And the publish success message reminds Maria that Rachel's original claim remains visible

Scenario: Counter-relationship is annotated bidirectionally in subsequent federated query
  Given Maria has published counter-claim bafy...new countering Rachel's bafy...n4ka
  When Maria runs `openlore graph query --subject <subject> --federated`
  Then Maria's bafy...new row shows "counters bafy...n4ka by did:plc:rachel-test"
  And Rachel's bafy...n4ka row shows "countered-by bafy...new by did:plc:maria-test"
  And the summary line states the count of counter-relationships explicitly
```

### Acceptance Criteria

- [ ] `openlore claim counter <target_cid> --reason "..." [other claim flags]` constructs a counter-claim with `references[].type == Counters` pointing at `<target_cid>`.
- [ ] `--reason` is required; 1..=1000 chars; missing or empty exits non-zero with a clear error.
- [ ] Compose preview displays "counters: <target_cid> (by <target_author_did>)".
- [ ] Compose preview contains the literal text "not as truth" (inherited from US-001).
- [ ] Compose preview contains the literal text "counter-claims coexist, never overwrite".
- [ ] Compose preview displays the --reason text verbatim, wrapped at 78 cols.
- [ ] Countering one's own claim is rejected pre-compose with a hint to `claim retract`.
- [ ] Already-countered targets prompt for confirmation before allowing a second counter-claim.
- [ ] Sign and publish reuse the slice-01 pipeline (no parallel publish code path).
- [ ] Subsequent `graph query --federated` annotates both the counter-claim and the target with bidirectional `counters` / `countered-by` text.

### Outcome KPIs

See `outcome-kpis.md` KPI-FED-3 (counter-claim publication rate — the J-003b behavioral validation).

### Technical Notes

- Depends on US-FED-002 (peer_claims populated, target CIDs locally resolvable) and US-FED-003 (federated query renders counter-relationships).
- The Lexicon `org.openlore.claim` needs a new optional field `reason` for counter-claims. DESIGN owns whether to (a) add the field directly to the claim schema as optional, (b) introduce a sub-type, or (c) keep `reason` in the references[] entry next to the type. The contract is: signed payload preserves the reason text byte-equal, and the field is publicly federated (not local-only).
- Counter-claim authoring reuses `claim-domain::reference_rules_validate` (existing pure function) — must add a check "if reference type is Counters AND target_cid resolves to a claim with the current user's author_did, reject as 'self-counter'."

---

## US-FED-005: Remove a peer subscription cleanly, with optional purge of cached claims

### Job link

- `job_id`: J-003 (sub-job J-003c revocability without residue)

### Elevator Pitch

- **Before**: Once I subscribe to a peer, there is no way to undo the subscription, and the peer's claims sit in my local store forever — subscription is a one-way commitment, which makes me hesitant to subscribe to anyone I'm only mildly curious about.
- **After**: I run `openlore peer remove did:plc:rachel-test` (soft) or `openlore peer remove did:plc:rachel-test --purge` (hard); the soft form removes the subscription but keeps cached claims; the hard form prompts "Proceed? [y/N]" and on confirmation removes the subscription AND deletes all of Rachel's cached claims from my peer_claims store; my own claims are unaffected.
- **Decision enabled**: I can subscribe freely to peers I'm only mildly curious about, knowing I can leave cleanly without trace — which means I'll explore more peer claim streams than I would under a one-way-commitment model.

### Problem

Without a revocation path, subscription is a one-way commitment. Maria
would have to manually grep her local store to clean up after an
unwanted subscription, and she has no guarantee that nothing lingers
(subscription record, cached claims, derived data). This violates J-003c
(subscription must be revocable without residue) and pushes Maria
toward never subscribing to anyone unless she is already confident she
wants to follow them indefinitely.

### Who

- Researcher / Tech Lead (P-002), has subscribed to ≥1 peer they now want to unsubscribe from
- Author-engineers (P-001) in the same situation
- Online or offline (peer remove is local-only)

### Solution

A `openlore peer remove <did> [--purge]` CLI command with two modes:

- **Soft (default)**: removes the subscription record from `peer_subscriptions`. Cached claims in `peer_claims` are retained (annotated as "unsubscribed cache" in subsequent federated queries). No confirmation prompt.
- **Hard (`--purge`)**: removes the subscription AND deletes all cached peer claims attributed to the removed peer from `peer_claims`. REQUIRES interactive confirmation prompt (no `--yes` flag in slice-03). The current user's own claims in `author_claims` are NEVER affected, including counter-claims the user authored against the removed peer's claims (those remain as the user's own published artifacts).

### Domain Examples

#### Example 1 (Happy Path / Soft remove)

Maria runs `openlore peer remove did:plc:rachel-test` (no --purge). The CLI
prints `Removed subscription. 5 cached peer claims retained (use --purge to
delete them).` Subsequent `openlore graph query --subject ... --federated`
still shows Rachel's claims but annotated `(unsubscribed cache)` instead of
`(subscribed peer)`.

#### Example 2 (Happy Path / Hard purge)

Maria runs `openlore peer remove did:plc:rachel-test --purge`. The CLI
shows the cached-record count and asks `This is irreversible. Re-running
'openlore peer add ...' and 'openlore peer pull' will re-fetch them.
Proceed? [y/N]:` She types `y`. The CLI removes the subscription, deletes
all 5 of Rachel's cached claims from peer_claims, and confirms `Purged 5
cached peer claims attributed to did:plc:rachel-test. Your own claims (in
author_claims) are unaffected.` Subsequent federated query returns zero
claims from Rachel.

#### Example 3 (Edge / Hard purge declined)

Tobias runs `openlore peer remove did:plc:tobias-favorite --purge`. The CLI
asks "Proceed? [y/N]:". He types `n`. The CLI prints `Cancelled. Subscription
and cached peer claims unchanged.` and exits 0. Both the subscription
record and the cached claims remain.

#### Example 4 (Edge / Removing a peer the user never subscribed to)

Aanya runs `openlore peer remove did:plc:stranger-test`. The CLI prints
`Not subscribed to did:plc:stranger-test; nothing to remove.` and exits 0.
No error; idempotent.

#### Example 5 (Edge / User has counter-claims against the removed peer)

Maria has counter-claims (published by her, in `author_claims`) against
two of Rachel's claims. She runs `openlore peer remove did:plc:rachel-test
--purge`. After confirmation, Rachel's cached claims are deleted from
`peer_claims`, but Maria's counter-claims remain in `author_claims` (those
are Maria's own published artifacts). Subsequent federated queries show
Maria's counter-claims with annotation "counters bafy...n4ka (peer not
subscribed)" — degraded gracefully.

### UAT Scenarios (BDD)

```gherkin
Scenario: Soft unsubscribe leaves cached peer claims intact
  Given Maria has subscribed to did:plc:rachel-test
  And Maria has pulled 5 of Rachel's claims into peer_claims
  When Maria runs `openlore peer remove did:plc:rachel-test` (without --purge)
  Then the subscription record is removed from peer_subscriptions
  And Rachel's 5 claims remain in peer_claims
  And subsequent `openlore graph query --subject ... --federated` shows Rachel's claims annotated "(unsubscribed cache)"
  And exit code is 0

Scenario: Hard purge removes peer claims and asks for confirmation
  Given Maria has subscribed to did:plc:rachel-test
  And Maria has pulled 5 of Rachel's claims
  When Maria runs `openlore peer remove did:plc:rachel-test --purge`
  Then the CLI shows the cached-record count
  And the CLI asks "Proceed? [y/N]"
  And on confirmation, the subscription record is removed
  And all 5 of Rachel's cached claims are deleted from peer_claims
  And Maria's own claims in author_claims are unaffected
  And subsequent `openlore graph query --subject ... --federated` returns zero claims from Rachel
  And exit code is 0

Scenario: Hard purge declined leaves everything unchanged
  Given Maria has subscribed to did:plc:rachel-test
  And Maria has pulled 5 of Rachel's claims
  When Maria runs `openlore peer remove did:plc:rachel-test --purge` and answers "n" to the prompt
  Then the subscription record remains
  And all 5 of Rachel's cached claims remain in peer_claims
  And the CLI prints "Cancelled. Subscription and cached peer claims unchanged."
  And exit code is 0

Scenario: Removing a not-subscribed peer is idempotent
  Given Aanya has not subscribed to did:plc:stranger-test
  When Aanya runs `openlore peer remove did:plc:stranger-test`
  Then the CLI prints "Not subscribed to did:plc:stranger-test; nothing to remove."
  And no peer_subscriptions or peer_claims state changes
  And exit code is 0

Scenario: User's counter-claims survive a hard purge of the countered peer
  Given Maria has published counter-claim bafy...new against Rachel's bafy...n4ka
  And Maria runs `openlore peer remove did:plc:rachel-test --purge` and confirms
  When the purge completes
  Then Rachel's bafy...n4ka is deleted from peer_claims
  And Maria's bafy...new remains in author_claims
  And subsequent federated query shows bafy...new with annotation "counters bafy...n4ka (peer not subscribed)"
```

### Acceptance Criteria

- [ ] `openlore peer remove <did>` removes the subscription record from `peer_subscriptions`.
- [ ] Without `--purge`: cached peer claims for the removed peer are RETAINED in `peer_claims`; subsequent federated queries annotate them `(unsubscribed cache)`.
- [ ] With `--purge`: an interactive confirmation prompt is REQUIRED (no `--yes` flag in slice-03); on confirmation, the subscription is removed AND all of that peer's cached claims are deleted from `peer_claims`.
- [ ] `--purge` declined leaves both subscription record and cached claims unchanged.
- [ ] `--purge` NEVER deletes anything from `author_claims`, including counter-claims authored by the current user against the removed peer's claims.
- [ ] Removing a not-subscribed peer is idempotent: exits 0 with a clear message; no state changes.
- [ ] Post-purge federated queries gracefully degrade counter-claim annotations to "counters <cid> (peer not subscribed)".

### Outcome KPIs

See `outcome-kpis.md` KPI-FED-4 (revocation cleanliness — zero residue).

### Technical Notes

- Depends on US-FED-001 (subscriptions exist), US-FED-002 (cached claims exist).
- The "(unsubscribed cache)" annotation in federated queries (US-FED-003) requires the query layer to distinguish "peer is currently subscribed" from "peer has cached claims but no subscription." DESIGN owns the schema (likely: `peer_claims` has a foreign-key-like relationship to `peer_subscriptions` that is allowed to dangle).
- Confirmation prompt reuses the same TTY-io helper as the slice-01 publish prompt.

---

## US-FED-006 `@infrastructure`: Bootstrap peer_subscriptions schema, peer_claims store, and PeerPort wiring

### `infrastructure_rationale`

This story exists to extend the slice-01 storage schema and adapter layer
to support peer subscriptions and peer claims. It is an `@infrastructure`
story because it has no end-user-observable behavior on its own — every
user-visible behavior is in US-FED-001..005. Without this story, those five
stories cannot ship. It is grouped with them in Release 1 (the walking
skeleton release) because US-FED-001..003 (the slice-03 walking-skeleton
trio) all depend on it.

The slice satisfies the BLOCKING slice-level Elevator Pitch check (per
`nw-po-review-dimensions` Dimension 0 §5): five user-visible stories
(US-FED-001..005) accompany this one infrastructure story. The slice is
NOT 100% `@infrastructure`.

### Job link

- `job_id`: `infrastructure-only`

### Problem (infra perspective)

Slice-01 created `author_claims` storage. Slice-03 introduces TWO new
storage surfaces: `peer_subscriptions` (which DIDs are subscribed,
when, with what cached PDS endpoint) and `peer_claims` (signed claims
fetched from those peers, attributed per claim). The Lexicon for
`org.openlore.claim` needs a new optional field for counter-claim reason
text. The `ports` crate needs either an extension to `PdsPort` or a new
`PeerPort` trait. None of these changes are user-visible on their own,
but every US-FED-001..005 story depends on them.

### Solution (infra)

- Extend `lexicon` crate: add optional `reason: String` field to `org.openlore.claim` (length 1..=1000 when present; permitted on any claim but semantically only meaningful when `references[].type == Counters`).
- Extend `ports` crate: either add methods `list_peer_records(peer_did)`, `get_peer_record(peer_did, rkey)` to `PdsPort` (and a new `PeerStoragePort` with `read_peer_claims`, `write_peer_claim`, `delete_peer_claims_by_author`, `list_subscriptions`, `add_subscription`, `remove_subscription`), OR introduce a single `PeerPort` trait combining both surfaces. DESIGN's call. Either way, MUST include `probe()` per ADR-009.
- Extend `adapter-duckdb` (or new `adapter-peer-store`): implement the storage port for `peer_subscriptions` and `peer_claims`. Schema migration is forward-only and idempotent. Probe asserts schema-version match, sentinel round-trip, fsync honored on storage medium.
- Extend `adapter-atproto-pds`: add the peer-read methods. Probe asserts that a fixture peer DID's listed records round-trip CID-stably.
- Extend `xtask check-arch`: ensure the new `peer_claims` table cannot be JOINed with `author_claims` in a way that elides the author DID column. (A clippy lint or test-time fixture is acceptable.)

### Acceptance Criteria

- [ ] Lexicon `org.openlore.claim` accepts optional `reason` field of length 1..=1000 when present; absent field is permitted.
- [ ] New port surface (extension of `PdsPort` + storage port additions, or new `PeerPort`) compiles and has stub implementations in DELIVER's RED phase.
- [ ] All new port methods have probe() coverage per ADR-009.
- [ ] `peer_subscriptions` and `peer_claims` schemas are created idempotently at `openlore init` (extending the slice-01 init flow).
- [ ] DuckDB schema migration is forward-only; running `openlore init` on an existing slice-01 database upgrades it without data loss.
- [ ] `xtask check-arch` enforces that no query may JOIN `author_claims` and `peer_claims` in a way that elides the author DID column.

### UAT Scenarios (BDD — infrastructure surface)

```gherkin
Scenario: Init on a fresh database creates slice-01 + slice-03 schemas
  Given no openlore database exists
  When Jeff runs `openlore init --handle jeff.test --app-password ...`
  Then ~/.local/share/openlore/openlore.duckdb is created
  And the database contains tables: author_claims, peer_subscriptions, peer_claims
  And the schema-version metadata records slice-03 version

Scenario: Init on an existing slice-01 database migrates forward without data loss
  Given an openlore database exists with N author_claims rows (slice-01 schema)
  When Jeff runs `openlore init` after upgrading to a slice-03 build
  Then the schema is upgraded to slice-03 version
  And all N author_claims rows are preserved byte-for-byte
  And the new peer_subscriptions and peer_claims tables are created and empty
```

### Outcome KPIs

n/a — supports KPI-FED-1, KPI-FED-2, KPI-FED-4, KPI-FED-6 indirectly.

### Technical Notes

- Depends on slice-01 schema being present (this story extends, does not replace).
- Coordinates closely with US-FED-002 (peer pull) and US-FED-005 (peer remove) on the exact contract surface of the storage port.
- The new `reason` field in the Lexicon is FORWARD-COMPATIBLE: a slice-01-era reader receiving a slice-03 counter-claim will see an unknown optional field and (per ADR-005's optionality rule) MUST ignore it gracefully. No data loss; no wire break.

---

## Summary table

| Story | Title | Job link | Right-sized? | DoR status |
|---|---|---|---|---|
| US-FED-001 | Subscribe to a peer's claim stream | J-003 | YES (1 day, 4 scenarios) | PASS (see DoR section in feature-delta.md) |
| US-FED-002 | Pull peer claims with sig + CID verification | J-003 | YES (2 days, 4 scenarios) | PASS |
| US-FED-003 | Read federated graph with per-author attribution | J-003 | YES (2 days, 4 scenarios) | PASS |
| US-FED-004 | Author and publish a counter-claim | J-003 + J-001 | YES (2 days, 5 scenarios) | PASS |
| US-FED-005 | Remove a peer subscription with optional purge | J-003 | YES (1.5 days, 5 scenarios) | PASS |
| US-FED-006 | Bootstrap peer storage + PeerPort (`@infrastructure`) | `infrastructure-only` | YES (1.5 days, 2 scenarios) | PASS (with infra rationale) |

Total estimated effort: ~10 days at moderate confidence. Slice composition
gate: PASS — 5 user-visible stories + 1 infrastructure story; slice is NOT
100% `@infrastructure` (per `nw-po-review-dimensions` Dimension 0 §5).
