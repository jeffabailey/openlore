<!-- markdownlint-disable MD024 -->

# User Stories — openlore-appview-search (slice-05)

All stories in this file belong to **slice-05-appview-search** (the fifth and
FINAL sibling feature in the OpenLore umbrella). Every story carries a `job_id`
traceable to `docs/product/jobs.yaml` per Decision 1. Stories US-AV-002..006
carry mandatory Elevator Pitches; US-AV-001 is `@infrastructure` and carries an
`infrastructure_rationale` instead.

This slice adds an **AppView / indexer service (a separate binary,
`openlore-indexer`)** that ingests PUBLIC signed claims from ACROSS the network
— beyond the user's own claims and beyond the peers they manually subscribed to
in slice-03 — and delivers **query-driven discovery ("search by philosophy") at
network scale**. The headline is: a developer can discover a well-evidenced
signed claim by an author they have never heard of, without first knowing whose
DID to follow.

The AppView is a READ / discovery surface. The CLI + signed claims remain the
source of truth. The indexer NEVER becomes an authority that overwrites or merges
claims; it never signs or publishes on a user's behalf. Discovery is a front-door
that feeds the slice-03 federation flow (`openlore peer add`), not a replacement
for it.

## System Constraints

These are cross-cutting constraints that apply to every story in this feature.
The first six are **inherited from the prior slices' user-stories.md** and are
repeated here for the reviewer's convenience. They are NOT relitigated.

- **CLI-first, local-first source of truth**: the CLI + the local signed claims
  remain the source of truth. Compose, sign, and own-claim flows continue to work
  with the network disabled (KPI-5). The AppView/indexer is a discovery surface
  layered ON TOP; it is never the authority. When the index is unreachable,
  discovery degrades to a clear unavailable/local-only message and never blocks
  the local-first flows. (This is the load-bearing product tension this slice
  introduces — see the Local-first ↔ network-service section below.)
- **Solution-neutral**: stories describe user-observable behavior. The indexer's
  deployment shape (self-hostable single binary vs hosted service), its transport
  to the CLI (HTTP/XRPC), and whether ingestion is pull-based or Firehose-based
  are reserved for DESIGN.
- **Claims-not-truth invariant**: no surface frames any claim — or any network
  search result — as a truth assertion. A discovered claim is still one author's
  reasoning, displayed with explicit confidence; confidence never reads as "the
  network thinks this is true."
- **Attribution-preserving (anti-merging, extended to NETWORK aggregates)**:
  every claim shown anywhere — including in a network search result aggregated
  from many authors across the network — retains its author DID. There is NO
  "network consensus" row that hides the individual authors. This is the
  load-bearing slice-03 I-FED-1 invariant (carried into aggregates by slice-04
  I-GRAPH-1/2) now carried into network-scale aggregates. The
  `network_result_preserves_attribution` test enforces it.
- **Confidence numeric-only; buckets display-only**: numeric `[0.0, 1.0]` is the
  only persisted/indexed confidence (WD-10 / I-6). Display buckets are render-only,
  in search results exactly as in local query.
- **Single signing/publish path**: the AppView adds NO write surface. A user
  subscribing to a discovered author reuses the slice-03 `openlore peer add` path;
  there is no parallel subscription mechanism. The indexer signs/publishes nothing.

Constraints introduced new by this slice:

- **Signature-verified-before-index (load-bearing)**: the indexer MUST verify
  each claim's signature AND recompute its CID before the claim enters the index.
  No unsigned or unverified claim is ever indexed or returned by a search. This
  mirrors the slice-03 pull-time verification gate (KPI-FED-6) at network scale.
- **Public-data-only / claims-are-public framing**: indexing aggregates ONLY
  PUBLIC signed claims (records the author published to their PDS). The
  "your peer-published claims are public and network-discoverable" expectation is
  surfaced honestly to the user (the framing ADR-014 deferred to slice-05). The
  indexer reads no private data and exposes no surveillance affordance.
- **Network scope (this is the architectural shift)**: unlike slice-04 (local
  graph only), the index aggregates claims from across the network. This is a
  genuine local-first ↔ network-service tension, surfaced as the headline product
  decision for DESIGN (see below). The slice keeps the tension honest: discovery
  is network-scale, but ownership and authoring stay local-first.

### Local-first ↔ network-service tension (flagged for DESIGN, not resolved here)

The whole product has been CLI-first and local-first (KPI-5: compose + sign
succeed offline). An AppView/indexer is inherently a NETWORK service. This is a
real architectural shift, NOT a contradiction to paper over. DISCUSS frames the
product requirement and its forces; DESIGN resolves the architecture:

- **Is the indexer self-hostable?** (a single-binary the user runs themselves,
  preserving sovereignty) vs a hosted service the CLI queries.
- **How does the CLI reach the index?** (HTTP/XRPC to a local or remote indexer).
- **Is there a degraded local-only mode** when the index is unreachable? (Product
  requirement: YES — discovery must degrade gracefully and never block the
  local-first flows. The mechanism is DESIGN's.)
- **Pull-based vs Firehose ingestion?** (ADR-016 locked OUT push subscriptions for
  slice-03 with a "re-evaluate at slice-05" note. Firehose is a DESIGN OPTION, not
  a slice-05 requirement; pull-based indexing may suffice for the walking skeleton.)

These are captured as Open Decisions OD-AV-1..4 in `feature-delta.md`. The
user-visible contracts in these stories hold regardless of how DESIGN resolves
them.

### Glossary (terms introduced by this slice)

- **AppView**: the read/discovery surface over the network index. In OpenLore it
  is a query layer, not an authority — it never overwrites, merges, or signs.
- **Indexer**: the separate binary (`openlore-indexer`) that ingests PUBLIC signed
  claims from across the network, verifies each one, and serves search queries.
- **Network index**: the searchable corpus of signature-verified, attributed
  public claims aggregated from many authors — the corpus a `search` query runs
  against (distinct from the LOCAL graph that slice-04 `graph query` runs against).
- **Network search**: a query over the network index by subject, object, or
  contributor. The dimensions mirror slice-04, but the corpus is the network
  index, not the local store.
- **Verified-signature marker**: a per-result indicator that the indexer verified
  the author's signature and the CID matches the published record. A result
  without it is never shown (unverified claims are never indexed).
- **Discovery → federation funnel**: the path from discovering an author's claim
  in a network search to subscribing to them via the slice-03 `openlore peer add`,
  growing the user's trusted local graph.

---

## US-AV-001 `@infrastructure`: Bootstrap the indexer service + signature-verified, attributed ingest pipeline

### `infrastructure_rationale`

This story exists to add the new `openlore-indexer` binary and its ingest
pipeline: aggregate PUBLIC signed claims from across the network, verify each
claim's signature and recompute its CID before indexing, persist every indexed
record with a non-`Option` author DID, and expose a search/query surface the CLI
consumes. It is an `@infrastructure` story because it has no end-user-observable
behavior on its own — every user-visible behavior is in US-AV-002..006. Without
this story, those five stories cannot ship. It is grouped with US-AV-002 and
US-AV-004 in Release 1 (the walking-skeleton release) because the walking-skeleton
stories depend on it.

The slice satisfies the BLOCKING slice-level Elevator Pitch check (per
`nw-po-review-dimensions` Dimension 0 §5): five user-visible stories
(US-AV-002..006) accompany this one infrastructure story. The slice is NOT 100%
`@infrastructure`.

### Job link

- `job_id`: `infrastructure-only`

### Problem (infra perspective)

Slices 01–04 produced signed, structured, federated, locally-scored claims, but
discoverability stops at the local store (own + manually-subscribed peers +
scraper-signed). A claim by an author the user has never subscribed to is
invisible. Closing the J-001 "undiscoverable" gap at network scale needs a
service that aggregates many authors' PUBLIC signed claims into a searchable
index. The hard constraint: the index must never become an untrusted aggregator
— every claim must be signature-verified + CID-recomputed BEFORE indexing
(mirroring slice-03 pull-time verification), and every indexed record must keep
its author DID (anti-merging at the ingest layer). None of this is user-visible
on its own, but every US-AV-002..006 story depends on it. The deployment shape,
transport, and pull-vs-Firehose ingestion are DESIGN decisions, not this story's.

### Solution (infra)

- New `openlore-indexer` binary (separate from the `openlore` CLI; the brief's
  Component Inventory gains a row at finalize). It aggregates PUBLIC signed claim
  records from across the network and maintains a searchable index keyed by
  subject, object, and contributor.
- **Verified ingest gate**: before any claim enters the index, the indexer
  verifies the author's signature and recomputes the CID against the published
  record (reusing the pure `claim-domain` verification + the slice-03 verification
  discipline). An unsigned, tampered, or CID-mismatched claim is rejected and
  never indexed. This reuses the existing pure verification core — no second
  verification path.
- **Attribution-at-ingest**: every indexed record carries a non-`Option`
  `author_did` (mirroring slice-03's `FederatedRow` discipline). The index has NO
  schema for a multi-author "consensus" record; an aggregate is computed at query
  time from individually-attributed records, never stored merged.
- **Search/query surface**: the indexer exposes a query API (by subject, object,
  contributor) that the CLI consumes. The transport (HTTP/XRPC), the deployment
  shape (self-hostable vs hosted), and the ingestion mode (pull-based vs Firehose,
  ADR-016 re-evaluation) are DEFERRED to DESIGN; this story's contract is
  transport- and deployment-neutral.
- **Local-first preservation**: the indexer is additive. The existing `openlore`
  CLI compose/sign/local-query flows are unchanged and continue to work with the
  network disabled. The indexer holds no signing/publishing capability by
  construction (it CANNOT author or mutate a claim — the human-gate at the
  architecture layer, mirroring slice-02's `adapter-github`).
- **Probe**: any new port/adapter surface ships a `probe()` per ADR-009 within the
  250ms budget (I-4/I-5).

### Acceptance Criteria

- [ ] A new `openlore-indexer` binary builds as a separate workspace member, distinct from the `openlore` CLI.
- [ ] The ingest pipeline verifies each claim's signature AND recomputes its CID before indexing; an unsigned, tampered, or CID-mismatched claim is rejected and never enters the index. (Reuses the pure `claim-domain` verification core — no second verification path.)
- [ ] Every indexed record carries a non-`Option` `author_did`; the index has no schema for a merged multi-author record (anti-merging at the ingest layer; compile-error if author DID is dropped).
- [ ] The indexer exposes a query surface (by subject, object, contributor) consumable by the CLI; the transport and deployment shape are DESIGN's choice and do not change this contract.
- [ ] The indexer holds NO signing/publishing capability by construction (it cannot author, sign, mutate, or publish a claim).
- [ ] The existing `openlore` CLI compose/sign/local-query flows are unchanged and succeed with the network disabled (KPI-5 preserved).
- [ ] Any new port/adapter surface ships `probe()` coverage per ADR-009 within the 250ms budget.
- [ ] An `xtask check-arch` rule (extending the slice-03 `no_cross_table_join_elides_author`) covers the index query path: no aggregate query elides the author DID.

### UAT Scenarios (BDD — infrastructure surface)

```gherkin
Scenario: The indexer rejects an unsigned or tampered claim before it is searchable
  Given a network record whose signature does not verify against its author DID
  When the indexer attempts to ingest the record
  Then the record is rejected and does not enter the index
  And a subsequent search never returns that record
  And a valid signed record from the same author is ingested and becomes searchable

Scenario: Every indexed record retains its author DID and is never merged
  Given two distinct authors have each published a public claim about the same subject and object
  When both records are ingested and a search matches that subject+object
  Then the index stores two individually-attributed records
  And no merged multi-author "consensus" record exists in the index
  And the anti-merging check rule covers the index query path
```

### Outcome KPIs

n/a — supports KPI-AV-1, KPI-AV-2, KPI-AV-3, KPI-AV-4 indirectly.

### Technical Notes

- Depends on the slice-01 `claim-domain` verification core (signature verify + CID recompute) and the slice-03 pull-time-verification precedent (KPI-FED-6). This story extends the verification discipline to network-scale ingest; it does NOT add a second verification path.
- The deployment shape (self-hostable single binary vs hosted service), the CLI→indexer transport (HTTP/XRPC), and the ingestion mode (pull-based vs ATProto Firehose, ADR-016 re-evaluation) are the headline DESIGN decisions (OD-AV-1..4). The contract here is deliberately transport/deployment/ingestion-neutral.
- Functional Rust paradigm (ADR-007): verification is pure core (reused); network I/O and index storage stay behind ports in the effect shell.
- The index store choice (DuckDB FTS vs a search engine vs a graph store) is DESIGN's call; the user-visible contracts (verified, attributed, searchable) hold regardless.

---

## US-AV-002: Search by philosophy (object) at network scale, attribution preserved

### Job link

- `job_id`: J-005 (sub-job J-005a search-at-network-scale — the headline surface)

### Elevator Pitch

- **Before**: I can query my LOCAL graph by philosophy
  (`openlore graph query --object dependency-pinning`), but it only sees my own
  claims and the peers I already chose to follow — so a great signed claim about
  dependency-pinning by someone I have never heard of is invisible, and I am stuck
  guessing whose DID to subscribe to first.
- **After**: I run
  `openlore search --object org.openlore.philosophy.dependency-pinning` and see
  "Network results for org.openlore.philosophy.dependency-pinning (12 signed
  claims across 7 subjects, 9 distinct authors — all signature-verified)" grouped
  by author, each claim under its author DID with numeric confidence, evidence,
  CID, and a [verified] marker — including authors I do not yet follow. The footer
  says "every result is one author's signed claim; nothing is merged. Follow an
  author with `openlore peer add <did>`."
- **Decision enabled**: I can discover well-evidenced reasoning and the people
  behind it across the whole network WITHOUT first knowing whom to follow — which
  means I will find aligned developers and projects I would never have reached by
  name, and choose whom to subscribe to from evidence rather than reputation.

### Problem

Maria Lopez (P-002) is a tech lead choosing a build-tooling stack and cares about
reproducible-builds as a philosophy, but she does not yet follow anyone who claims
it. The slice-04 local query (`graph query --object`) only sees her own claims
and her manually-pulled peers — a cold-start dead-end. She needs to search across
the network's PUBLIC signed claims to discover who claims reproducible-builds and
on which projects. The danger: any tool that answered this by collapsing authors
into a faceless "the network values X" count would reproduce the exact aggregator
failure mode (HN/Reddit/awesome-lists) the whole product exists to avoid. And she
must be able to trust that what she discovers is a real signed claim, not a
fabricated or tampered one.

### Who

- Researcher / Tech Lead (P-002) wearing the network-discovery hat — primary
- Senior Engineer Solo Builder (P-001) discovering aligned projects/maintainers
  before committing to a stack — secondary
- Does NOT already follow the relevant authors (cold-start discovery)
- Comfortable with philosophy URIs and DIDs; wants attribution + verification

### Solution

Add a new `openlore search` verb with an `--object <philosophy>` dimension that
queries the NETWORK INDEX (not the local graph). It returns signature-verified,
attributed public claims about that philosophy from across the network, grouped
by author (or by subject under an author), each with author DID, numeric
confidence + display-only bucket, evidence, CID, and a [verified] marker.
Authors the user does not follow are included and labeled `(not subscribed)`. NO
row represents a multi-author aggregate; identical claims by different authors are
separate rows. A footer states the no-merge guarantee and points to
`openlore peer add` to follow a discovered author.

### Domain Examples

#### Example 1 (Happy Path — discover claims by unfollowed authors)

Maria runs `openlore search --object org.openlore.philosophy.reproducible-builds`.
The network index returns 12 signed claims across 7 subjects by 9 authors —
including Priya Nair (`did:plc:priya-test`, whom Maria does not follow) on
`github:bazelbuild/bazel` (0.82) and Rachel (`did:plc:rachel-test`, a peer Maria
already follows) on `github:nixos/nixpkgs` (0.88). Each result shows the author
DID, confidence, evidence, CID, and `[verified]`. Unfollowed authors are labeled
`(not subscribed)`. The footer reads "12 signed claims, 9 distinct authors, all
signature-verified. Every result is one author's signed claim; nothing is merged.
Follow an author with `openlore peer add <did>`."

#### Example 2 (Edge — identical claim by two authors stays two rows)

Tobias runs `openlore search --object org.openlore.philosophy.dependency-pinning`.
Two authors he does not follow both claim `github:denoland/deno` embodies
dependency-pinning (Priya at 0.70, Sven at 0.65). The results show BOTH as
distinct rows under `github:denoland/deno`, one under `did:plc:priya-test
(not subscribed)` and one under `did:plc:sven-test (not subscribed)`. There is NO
"deno: 2 authors agree" merged row.

#### Example 3 (Edge — index unreachable degrades to local-only)

Maria runs `openlore search --object reproducible-builds` while the indexer is
unreachable (offline or service down). The CLI prints "Network index unavailable.
Showing LOCAL results only (own + subscribed peers). Run
`openlore graph query --object ...` for the local graph." It falls back to the
local surface cleanly and never hangs or errors out fatally. (Local-first
preserved.)

#### Example 4 (Error/Edge — unknown philosophy URI)

Maria typos `openlore search --object org.openlore.philosophy.reproducable-builds`
(misspelled). The index finds zero matches and prints "No network claims found
for object org.openlore.philosophy.reproducable-builds. Did you mean
org.openlore.philosophy.reproducible-builds?" (near-match suggestion). Exit code
is 0 (a valid empty result, not an error).

### UAT Scenarios (BDD)

```gherkin
Scenario: Searching a philosophy surfaces signed claims by authors I do not follow
  Given the network index has 12 verified claims asserting org.openlore.philosophy.reproducible-builds across 7 subjects by 9 authors
  And Maria does not subscribe to most of those authors
  When Maria runs `openlore search --object org.openlore.philosophy.reproducible-builds`
  Then the results include claims by authors Maria does not follow, labeled "(not subscribed)"
  And every result shows its author DID, numeric confidence, evidence, CID, and a [verified] marker
  And no result row collapses multiple authors into a single entry
  And the footer states nothing is merged and points to `openlore peer add` to follow an author

Scenario: Identical-content claims by different authors are separate network rows
  Given two unfollowed authors each published a verified claim asserting github:denoland/deno embodies dependency-pinning
  When Tobias runs `openlore search --object org.openlore.philosophy.dependency-pinning`
  Then both claims appear as distinct rows under github:denoland/deno
  And each is attributed to a distinct author DID labeled "(not subscribed)"
  And there is NO row that represents both claims combined

Scenario: An unreachable index degrades to local-only without blocking
  Given the network indexer is unreachable
  When Maria runs `openlore search --object org.openlore.philosophy.reproducible-builds`
  Then the output states the network index is unavailable
  And the output points to the local `openlore graph query --object` surface
  And the command exits without a fatal error

Scenario: Unknown philosophy URI returns an empty result with a suggestion
  Given the network index has no claims for object org.openlore.philosophy.reproducable-builds
  When Maria runs `openlore search --object org.openlore.philosophy.reproducable-builds`
  Then the output states no network claims were found for that object
  And the output suggests a near-matching philosophy URI
  And exit code is 0
```

### Acceptance Criteria

- [ ] `openlore search --object <philosophy>` queries the NETWORK INDEX (not the local graph) and returns matching signed claims.
- [ ] Results include claims by authors the user does NOT subscribe to, labeled `(not subscribed)`; followed authors are labeled per the slice-03 relationship labels.
- [ ] Every result row shows: author DID, predicate, subject/object, numeric confidence + display-only bucket (WD-10), evidence, CID, and a `[verified]` marker.
- [ ] Results are grouped by author (or by subject under an author); NO row represents a multi-author aggregate. Two claims with identical (subject, object) by different authors appear as TWO rows (anti-merging at network scale).
- [ ] The footer states the count of distinct authors AND the no-merge guarantee AND points to `openlore peer add <did>` to follow a discovered author.
- [ ] An unknown/unmatched philosophy URI returns an empty result with a near-match suggestion and exit code 0.
- [ ] When the index is unreachable, the command degrades to a clear local-only/unavailable message and does not block or fatally error (local-first preserved).

### Outcome KPIs

See `outcome-kpis.md` KPI-AV-1 (discover a claim by an unfollowed author — the north star), KPI-AV-2 (anti-merging at network scale — guardrail), KPI-AV-3 (signature-verified before index — guardrail).

### Technical Notes

- Depends on US-AV-001 (indexer + verified ingest + query surface in place).
- New CLI verb `openlore search` (distinct from the local `openlore graph query`). DESIGN owns whether discovery is a new top-level verb (`search`) or a `--network` flag on `graph query`; the product requirement is that the corpus is the network index, clearly distinct from the local graph (OD-AV-5).
- The renderer reuses the slice-03/04 anti-merging discipline: every output row carries exactly one `author_did`.
- DESIGN owns the index store and search mechanism (DuckDB FTS, a search engine, etc.); invisible to this story's contract.

---

## US-AV-003: Search by contributor or subject at network scale

### Job link

- `job_id`: J-005 (sub-job J-005a search-at-network-scale; relates to J-004 contributor lens at scale)

### Elevator Pitch

- **Before**: When a network search surfaces a compelling claim by someone I do
  not follow, I cannot ask "what else does this person claim across the whole
  network?" or "what does the network say about THIS specific project?" without
  first subscribing to them and pulling — a commitment I have to make before I
  even know if their reasoning is consistent.
- **After**: I run `openlore search --contributor github:priya` and see
  "Network claims authored by did:plc:priya-test (8 verified claims across 6
  subjects)" — her whole public reasoning trail — without following her first; or
  I run `openlore search --subject github:bazelbuild/bazel` and see every
  network author's verified claims about that project, each attributed.
- **Decision enabled**: I can evaluate a discovered contributor's whole public
  reasoning trail, or survey what the network says about a specific project,
  BEFORE deciding whether to follow — which means I subscribe from evidence, not
  from a single claim I happened to see.

### Problem

Maria's network search by philosophy (US-AV-002) surfaced a compelling claim by
Priya, whom she does not follow. Before committing to subscribe-and-pull (a
slice-03 relationship), she wants to read Priya's whole public reasoning trail to
judge consistency — and separately, she wants to survey what the network says
about `github:bazelbuild/bazel` specifically. Today she would have to subscribe
to Priya first (a premature commitment) just to read her, or query each subject
one at a time. She needs contributor-first and subject-first network search
dimensions, with the same honest framing (one developer's trail, not consensus;
each subject's claims attributed per author).

### Who

- Researcher / Tech Lead (P-002, network-discovery hat) evaluating a discovered
  contributor before following, or surveying a project at network scale
- Senior Engineer Solo Builder (P-001) vetting a dependency's maintainer across
  the network
- Wants to read before committing to a subscription; values attribution + the
  honest "not consensus" framing

### Solution

Extend `openlore search` with `--contributor <did|handle>` and `--subject
<project>` dimensions over the network index. `--contributor` lists every verified
network claim authored by that DID/handle, across all subjects, with the honest
"one developer's reasoning trail, not a community consensus" footer.
`--subject` lists every verified network claim about that project, grouped by
author. Both mirror the slice-04 local dimensions but over the network corpus;
both preserve per-author attribution and the `[verified]` marker; both work
without the user following the authors.

### Domain Examples

#### Example 1 (Happy Path — contributor trail before following)

Maria runs `openlore search --contributor github:priya` (resolves to
`did:plc:priya-test`). The index returns 8 verified claims across 6 subjects
(bazel x2, buck2, nixpkgs, pants, please, ninja). Each shows
subject/object/confidence/CID/`[verified]`. The footer reads "8 verified claims by
ONE DID (did:plc:priya-test). This is one developer's reasoning trail, not a
community consensus. You do not follow this author — `openlore peer add
did:plc:priya-test` to subscribe."

#### Example 2 (Happy Path — subject survey at network scale)

Tobias runs `openlore search --subject github:bazelbuild/bazel`. The index returns
verified claims about bazel from 5 distinct network authors, grouped by author,
each with its philosophy/confidence/CID/`[verified]`. No "bazel: the network
thinks X" merged row appears.

#### Example 3 (Edge — contributor not in the index)

Aanya runs `openlore search --contributor github:nobody-here`. The index has no
verified claims by that contributor. The CLI prints "No network claims found for
contributor github:nobody-here. They may not publish OpenLore claims, or the
indexer has not yet ingested them." Exit code is 0.

#### Example 4 (Edge — a followed author appears with the correct label)

Maria runs `openlore search --contributor github:rachel` (a peer she already
follows). The results are labeled `(subscribed peer)` rather than
`(not subscribed)`, preserving the slice-03 relationship labeling even in network
search — so Maria can tell which discovered authors she already follows.

### UAT Scenarios (BDD)

```gherkin
Scenario: Searching a contributor surfaces their full network reasoning trail before following
  Given did:plc:priya-test has 8 verified network claims across 6 subjects and Maria does not follow her
  When Maria runs `openlore search --contributor github:priya`
  Then all 8 verified claims are listed under did:plc:priya-test with subject, object, confidence, CID, and [verified]
  And the footer states this is one developer's reasoning trail, not a community consensus
  And the footer offers `openlore peer add did:plc:priya-test` to subscribe

Scenario: Searching a subject surfaces every network author's verified claims, attributed
  Given github:bazelbuild/bazel has verified network claims from 5 distinct authors
  When Tobias runs `openlore search --subject github:bazelbuild/bazel`
  Then the claims are grouped by author, each with its philosophy, confidence, CID, and [verified]
  And no row collapses multiple authors into a single "network consensus" entry

Scenario: A contributor absent from the index degrades gracefully
  Given the network index has no verified claims by contributor github:nobody-here
  When Aanya runs `openlore search --contributor github:nobody-here`
  Then the output states no network claims were found for that contributor
  And exit code is 0

Scenario: A followed author is labeled correctly in network search
  Given Maria already subscribes to did:plc:rachel-test
  When Maria runs `openlore search --contributor github:rachel`
  Then Rachel's network claims are labeled "(subscribed peer)" rather than "(not subscribed)"
  And every claim retains its author DID and [verified] marker
```

### Acceptance Criteria

- [ ] `openlore search --contributor <did|handle>` lists every verified network claim authored by that DID/handle, across all subjects, without requiring the user to follow them.
- [ ] `openlore search --subject <project>` lists every verified network claim about that project, grouped by author.
- [ ] Each row shows subject, predicate, object, numeric confidence + display-only bucket, CID, and a `[verified]` marker.
- [ ] The author-relationship label is correct: `(not subscribed)` for unfollowed authors, `(subscribed peer)` for followed, `(you)` for the user's own claims (preserving slice-03 labeling).
- [ ] The `--contributor` footer states it is one developer's reasoning trail, not a community consensus, and offers `openlore peer add` to subscribe.
- [ ] A contributor/subject with no network claims returns an empty result with a clear message and exit code 0.
- [ ] When the index is unreachable, the command degrades to a clear local-only/unavailable message (local-first preserved).

### Outcome KPIs

See `outcome-kpis.md` KPI-AV-1 (discovery — the contributor lens is a primary discovery path), KPI-AV-2 (anti-merging at network scale), KPI-AV-4 (discovery→federation funnel — `--contributor` is the read-before-follow step).

### Technical Notes

- Depends on US-AV-001 and US-AV-002 (the network search verb + index query surface).
- Reuses the slice-03 author-relationship labeling (subscribed / not-subscribed / you) and the slice-02/04 contributor-handle→DID resolution.
- The renderer reuses the anti-merging discipline: every row carries one `author_did` and one `[verified]` marker.

---

## US-AV-004: Trust a discovered result — verified-signature + CID-match marker and public-data honesty

### Job link

- `job_id`: J-005 (sub-job J-005b index-only-verified-attributed-claims — the trust precondition)

### Elevator Pitch

- **Before**: When a search surfaces a claim from someone I have never heard of, I
  have no way to know whether it is a real claim that author signed or a fabricated
  / tampered row an aggregator made up — so I cannot trust a network result enough
  to act on it, and the whole discovery surface feels like just another
  unaccountable aggregator.
- **After**: Every search result carries a `[verified]` marker, and
  `openlore search --object ... --show <cid>` prints the full record with
  "Signature: VERIFIED against did:plc:priya-test" and "CID: bafy...k2 (recomputed,
  matches published record)", plus a one-line banner up front: "Discovery indexes
  only PUBLIC signed claims. Each result is the author's own signed record,
  signature-verified before indexing."
- **Decision enabled**: I can trust that a discovered claim is exactly what the
  named author signed and published — verified, attributed, unfabricated — which
  means I will act on network discoveries the same way I act on a peer I pulled
  myself, instead of dismissing them as aggregator noise.

### Problem

Discovery at network scale only has value if the user can trust it. The J-005
anxiety is sharp: "Is this just another centralized aggregator that collapses
provenance? Will it serve me a tampered or fabricated claim?" The mitigation is
the same trust contract slice-03 established for peer pulls (KPI-FED-6: verify
signature + recompute CID before accepting), now made VISIBLE in the discovery
surface. The user must be able to see, per result, that the indexer verified the
signature against the author DID and that the CID matches the published record —
and must be told honestly, up front, that indexing covers only PUBLIC signed
claims (the framing ADR-014 deferred to slice-05). Without this visible trust
contract, the AppView is indistinguishable from the aggregators the product
exists to replace.

### Who

- Researcher / Tech Lead (P-002, network-discovery hat) deciding whether to act on
  a discovered claim from an unfollowed author
- Senior Engineer Solo Builder (P-001) auditing a surprising network result before
  citing it
- Carries the J-005 "is this a trustworthy aggregator?" + "is this tampered?"
  anxieties; needs visible verification + honest public-data framing

### Solution

(a) Every `openlore search` result carries a `[verified]` marker (guaranteed by
the US-AV-001 ingest gate — only verified claims are ever indexed). (b) A
`--show <cid>` option on `search` prints the full discovered record with an
explicit "Signature: VERIFIED against <author_did>" line and a "CID: <cid>
(recomputed, matches published record)" line, so the user can confirm the result
is the author's own signed record. (c) A short public-data banner is printed at
the top of every search session: "Discovery indexes only PUBLIC signed claims.
Each result is the author's own signed record, verified before indexing." No
unverified claim is ever shown; if a record fails verification it is not in the
index (US-AV-001), so there is no "unverified" state to display in results.

### Domain Examples

#### Example 1 (Happy Path — inspect a verified discovered record)

Maria runs `openlore search --object reproducible-builds --show bafy...k2` for a
result by Priya (whom she does not follow). The output prints the full record:
subject `github:bazelbuild/bazel`, object `reproducible-builds`, confidence 0.82,
evidence URL, author `did:plc:priya-test`, "Signature: VERIFIED against
did:plc:priya-test", "CID: bafy...k2 (recomputed, matches published record)". She
trusts it as Priya's genuine signed claim.

#### Example 2 (Happy Path — public-data banner up front)

Tobias runs any `openlore search` query. Before the results, the CLI prints
"Discovery indexes only PUBLIC signed claims published to authors' PDSs. Each
result is the author's own signed record, signature-verified before indexing.
Nothing private is read or aggregated." He is never surprised about what is
discoverable.

#### Example 3 (Boundary — verification happens at index, so no unverified result exists)

Aanya inspects many results and never sees an `[unverified]` or `[unknown
signature]` marker, because the US-AV-001 ingest gate rejects unverified claims
before indexing. Every result is `[verified]` by construction; there is no
mixed-trust result list to reason about.

#### Example 4 (Error — `--show` a CID not in the result set)

Maria runs `--show bafy...nothere` for a CID absent from the current result set.
The CLI prints "CID bafy...nothere is not in this search result. Run the search
without --show to list results, then --show a listed CID." and exits non-zero (a
usage error, unlike an empty search which exits 0).

### UAT Scenarios (BDD)

```gherkin
Scenario: A discovered result can be inspected to confirm it is the author's verified signed record
  Given a search returned a claim by did:plc:priya-test with CID bafy...k2
  When Maria runs `openlore search --object org.openlore.philosophy.reproducible-builds --show bafy...k2`
  Then the full record is printed with subject, object, confidence, evidence, and author DID
  And the output states "Signature: VERIFIED against did:plc:priya-test"
  And the output states the CID was recomputed and matches the published record

Scenario: A public-data banner sets the indexing expectation honestly before results
  Given the network index is reachable
  When Tobias runs any `openlore search` query
  Then a banner states that discovery indexes only PUBLIC signed claims
  And the banner states each result is the author's own signed record verified before indexing
  And the banner states nothing private is read or aggregated

Scenario: No unverified result is ever shown because verification happens at index time
  Given the network index contains only signature-verified claims (per the ingest gate)
  When Aanya inspects many search results
  Then every result carries a [verified] marker
  And no result is shown with an unverified or unknown-signature state

Scenario: --show on a CID absent from the result set is a usage error
  Given the current search result does not contain CID bafy...nothere
  When Maria runs `openlore search --object ... --show bafy...nothere`
  Then the output states the CID is not in this search result
  And exit code is non-zero
```

### Acceptance Criteria

- [ ] Every `openlore search` result carries a `[verified]` marker (guaranteed by the US-AV-001 ingest gate; unverified claims are never indexed, so never shown).
- [ ] `openlore search --show <cid>` prints the full discovered record including subject, object, confidence, evidence, and author DID.
- [ ] `--show` output states the signature was VERIFIED against the author DID and that the recomputed CID matches the published record.
- [ ] A public-data banner is printed up front for every search session, stating that indexing covers ONLY public signed claims, each verified before indexing, and that nothing private is read or aggregated.
- [ ] No result is ever shown in an unverified / unknown-signature state (verification is an ingest precondition, not a per-result runtime check the user must interpret).
- [ ] `--show` for a CID not in the current result set is a usage error with a clear message and non-zero exit.
- [ ] All display is read-only; inspecting a result creates, signs, or mutates nothing.

### Outcome KPIs

See `outcome-kpis.md` KPI-AV-3 (signature-verified before index — guardrail; this story is the load-bearing visible surface for it) and KPI-AV-5 (public-data framing comprehension).

### Technical Notes

- Depends on US-AV-001 (the ingest gate guarantees the `[verified]` invariant) and US-AV-002 (the search surface that renders the marker).
- The `--show` verification lines render the SAME pure `claim-domain` verification result the indexer computed at ingest; no second verification path (single source of truth for "verified").
- The public-data banner copy is a product default; PO owns the wording. The framing realizes the ADR-014-deferred "claims-are-public" expectation.
- The CID-recompute-matches-published display reuses the slice-03 CID-mismatch detection discipline (PP-4 precedent).

---

## US-AV-005: Subscribe to a discovered author straight from a search result (discovery → federation)

### Job link

- `job_id`: J-005 (sub-job J-005c discovery-feeds-federation)

### Elevator Pitch

- **Before**: When I discover a great claim by someone I do not follow, the
  search result is a dead-end — to actually start following them I have to copy
  their DID out of the result, switch context, and run a separate `peer add`,
  which is enough friction that I often just close the tab and forget.
- **After**: A search result that includes an unfollowed author ends with
  "Follow this author: `openlore peer add did:plc:priya-test`", and running it
  subscribes me via the exact slice-03 federation flow I already know — after
  which `openlore peer pull` brings Priya's claims into my LOCAL graph and they
  show up in my `openlore graph query` and `--weighted` views.
- **Decision enabled**: I can turn a network discovery into a followed peer in one
  step, growing my trusted LOCAL graph from evidence I discovered — which means
  the AppView strengthens my local-first graph instead of replacing it, and the
  discovery→subscribe→local-graph funnel actually closes.

### Problem

Discovery has no lasting value if it is a dead-end read. The point of finding a
well-evidenced claim by an unfollowed author is to start following them so their
reasoning flows into the user's trusted LOCAL graph (where slice-03 federation and
slice-04 scoring already work). Today, acting on a discovery means manually
copying a DID and context-switching to a separate command — enough friction to
break the funnel. The product requirement: a discovered author is one step away
from `openlore peer add`, reusing the slice-03 path (no parallel subscription
mechanism), so discovery becomes the front-door to federation rather than a
competing surface.

### Who

- Researcher / Tech Lead (P-002, network-discovery hat) who discovered an aligned
  author and wants to start following them
- Senior Engineer Solo Builder (P-001) growing a trusted graph from network
  discoveries
- Values the discovery→federation funnel; wants the local-first graph to grow from
  what they discover

### Solution

Every `openlore search` result that includes an author the user does NOT follow
ends with a follow affordance: "Follow this author: `openlore peer add <did>`".
The affordance reuses the slice-03 `openlore peer add` verb verbatim — there is NO
new subscription path. After following, the existing `openlore peer pull` brings
the author's claims into the LOCAL graph, where they participate in
`graph query`, `--weighted`, and `--traverse` exactly like any pulled peer. The
funnel is: discover (search) → follow (`peer add`) → pull (`peer pull`) → local
graph. Discovery never auto-subscribes; following is always an explicit human
action.

### Domain Examples

#### Example 1 (Happy Path — discover, follow, pull into local graph)

Maria discovers Priya's reproducible-builds claim via `openlore search --object
reproducible-builds`. The result ends with "Follow this author:
`openlore peer add did:plc:priya-test`". Maria runs it; slice-03 acknowledges
"Added did:plc:priya-test; next pull will ingest their claims." She runs
`openlore peer pull`, then `openlore graph query --contributor did:plc:priya-test`
— Priya's claims are now in her LOCAL graph and participate in `--weighted` views.

#### Example 2 (Edge — already-followed author shows no redundant follow prompt)

Tobias's search result includes Rachel, whom he already follows. Her result is
labeled `(subscribed peer)` and shows NO "Follow this author" affordance (he
already follows her). The funnel affordance appears only for unfollowed authors.

#### Example 3 (Edge — discovery never auto-subscribes)

Aanya runs many searches and inspects results without ever running `peer add`. No
subscription is created; her `openlore peer list` is unchanged. Discovery is
read-only; following is always an explicit, separate human action (no auto-follow,
no "subscribe to all results").

#### Example 4 (Edge — follow reuses the slice-03 path verbatim)

Maria follows a discovered author and later runs `openlore peer remove
did:plc:priya-test --purge`. The slice-03 purge semantics apply unchanged (the
author was added via the same `peer add` path), leaving zero residue. Discovery
introduced no parallel subscription state to leak.

### UAT Scenarios (BDD)

```gherkin
Scenario: A discovered author can be followed in one step and their claims flow into the local graph
  Given Maria discovered a verified claim by did:plc:priya-test whom she does not follow
  When Maria runs the `openlore peer add did:plc:priya-test` affordance shown in the result
  Then did:plc:priya-test is added as a subscription via the slice-03 federation path
  And after `openlore peer pull` Priya's claims appear in Maria's local `graph query --contributor` results
  And those claims participate in local `--weighted` and `--traverse` views

Scenario: An already-followed author shows no redundant follow affordance
  Given Tobias already subscribes to did:plc:rachel-test
  When Tobias sees Rachel in a search result
  Then her result is labeled "(subscribed peer)"
  And no "Follow this author" affordance is shown for her

Scenario: Discovery never auto-subscribes
  Given Aanya runs several searches and inspects results
  When Aanya does not run any `openlore peer add`
  Then no subscription is created and `openlore peer list` is unchanged

Scenario: Following a discovered author reuses the slice-03 path with no parallel state
  Given Maria followed a discovered author via `openlore peer add`
  When Maria later runs `openlore peer remove <did> --purge`
  Then the slice-03 purge semantics apply unchanged and leave zero residue
```

### Acceptance Criteria

- [ ] Every search result that includes an unfollowed author ends with a "Follow this author: `openlore peer add <did>`" affordance.
- [ ] The affordance reuses the slice-03 `openlore peer add` verb verbatim; there is NO new or parallel subscription path.
- [ ] After following + `openlore peer pull`, the author's claims appear in the LOCAL graph and participate in `graph query`, `--weighted`, and `--traverse`.
- [ ] An already-followed author is labeled `(subscribed peer)` and shows NO follow affordance.
- [ ] Discovery never auto-subscribes; following is always an explicit, separate human action (no "subscribe to all results").
- [ ] Subscription state created via the affordance is indistinguishable from a slice-03 `peer add` (same `peer remove`/`--purge` semantics, zero parallel state).

### Outcome KPIs

See `outcome-kpis.md` KPI-AV-4 (discovery→federation funnel — the funnel-closing behavior; this story is its load-bearing surface).

### Technical Notes

- Depends on US-AV-002/003 (search results that label followed vs unfollowed authors) and the slice-03 `peer add`/`peer pull`/`peer remove` verbs (reused verbatim).
- The follow affordance is a render-only hint that prints the existing slice-03 command; it does NOT execute it (no auto-follow). The human runs the command.
- DESIGN owns whether the affordance can be made executable interactively (e.g., a confirm prompt) in a later release; the slice-05 requirement is the printed, copy-pasteable affordance reusing `peer add`.

---

## US-AV-006: Share a network search result as a stable link

### Job link

- `job_id`: J-005 (sub-job J-005a; realizes the J-004 "shareable as a link to a query" success signal deferred from slice-02/04)

### Elevator Pitch

- **Before**: When a network search surfaces exactly the evidence I need to
  justify a stack choice, I cannot hand it to a teammate as anything better than a
  pasted block of terminal text — they cannot re-run it, cannot see the same
  verified attribution, and have no way to follow the authors I found.
- **After**: I run `openlore search --object reproducible-builds --share` and get
  a stable link (e.g. `openlore://search?object=...` or an https URL — DESIGN's
  call) that a teammate opens to see the SAME attributed, signature-verified
  claims, each still showing its author DID and `[verified]` marker.
- **Decision enabled**: I can hand a teammate a reproducible, attributed,
  verified discovery they can open and act on themselves — which means a network
  discovery becomes a shareable decision artifact (an ADR reference, a Slack link)
  instead of dying as terminal scrollback.

### Problem

The J-004 success signal "shareable as a link to a query, not just a static dump"
was explicitly deferred to slice-05. Maria's network search produced exactly the
evidence she needs for an architecture decision record, but she can only paste
terminal text — a teammate cannot re-run it, cannot verify the attribution, and
cannot follow the discovered authors. The product requirement: a network search
result is shareable as a stable link that resolves to the same attributed,
verified results. The link must NOT become a new authority or a merged snapshot
that loses attribution — it resolves to the same per-author, `[verified]`,
anti-merging-preserving result the originating user saw.

### Who

- Researcher / Tech Lead (P-002, network-discovery hat) writing an ADR or
  justifying a decision to a team
- Senior Engineer Solo Builder (P-001) sharing a discovery with a collaborator
- Wants a reproducible, attributed, verified shareable artifact — not a lossy dump

### Solution

Add a `--share` option to `openlore search` that emits a stable link encoding the
query (dimension + value). Opening the link (re-running the encoded query against
the index, via CLI or a future web AppView — DESIGN's call) yields the SAME
attributed, signature-verified results: every claim under its author DID with a
`[verified]` marker, anti-merging preserved. The link encodes the QUERY, not a
frozen merged snapshot — so it always resolves to current verified results and
never becomes a stale or attribution-losing artifact. DESIGN owns the link scheme
(`openlore://` deep link vs https URL) and whether a web AppView renders it.

### Domain Examples

#### Example 1 (Happy Path — share a philosophy query)

Maria runs `openlore search --object org.openlore.philosophy.reproducible-builds
--share`. The CLI prints "Shareable link:
openlore://search?object=org.openlore.philosophy.reproducible-builds" plus
"Anyone who opens this runs the same network search and sees the same attributed,
verified claims. The link encodes the query, not a frozen snapshot." She pastes
it into her team's ADR.

#### Example 2 (Happy Path — teammate opens the link, sees attributed verified results)

Tobias opens the link Maria shared. He runs the encoded query against the index
and sees the same results: each claim under its author DID with a `[verified]`
marker, grouped by author, anti-merging preserved. He can follow any discovered
author with the same `openlore peer add` affordance.

#### Example 3 (Edge — share a contributor query)

Maria runs `openlore search --contributor github:priya --share` and gets
"openlore://search?contributor=did:plc:priya-test". The shared link resolves to
Priya's verified network reasoning trail with the same "one developer's reasoning
trail, not a community consensus" framing.

#### Example 4 (Edge — link resolves to current results, never a stale merged snapshot)

Maria shares a query link. A week later Priya publishes two more reproducible-builds
claims and the indexer ingests them. When Tobias opens the same link, he sees the
updated result set (including the new verified claims) — because the link encodes
the QUERY, not a frozen snapshot, and never collapses authors into a stored merged
view.

### UAT Scenarios (BDD)

```gherkin
Scenario: A network search result is shareable as a stable link
  Given Maria ran `openlore search --object org.openlore.philosophy.reproducible-builds`
  When Maria runs the same search with `--share`
  Then the output prints a stable link encoding the query dimension and value
  And the output states the link encodes the query, not a frozen snapshot

Scenario: Opening a shared link yields the same attributed, verified results
  Given Maria shared a network search link
  When Tobias opens the link and the encoded query runs against the index
  Then he sees the same claims grouped by author, each with its author DID and a [verified] marker
  And no result is collapsed into a merged "network consensus" row
  And he can follow any discovered author with `openlore peer add`

Scenario: A shared link resolves to current results, not a stale snapshot
  Given Maria shared a philosophy query link
  And the indexer later ingests two more verified claims matching that query
  When Tobias opens the link after the new claims are ingested
  Then the result set includes the newly ingested verified claims
  And the link never resolves to a stored merged snapshot that loses attribution
```

### Acceptance Criteria

- [ ] `openlore search --share` (combinable with `--object`/`--subject`/`--contributor`) emits a stable link encoding the query dimension and value.
- [ ] Opening the link re-runs the encoded query against the index and yields the SAME attributed, signature-verified results (every claim under its author DID with a `[verified]` marker).
- [ ] The shared result preserves anti-merging: no result is collapsed into a merged "network consensus" row; identical claims by different authors stay separate.
- [ ] The link encodes the QUERY, not a frozen merged snapshot; it resolves to CURRENT verified results (newly-ingested matching claims appear).
- [ ] The output states the link encodes the query (not a snapshot) so the user understands the sharing semantics.
- [ ] The link scheme (`openlore://` deep link vs https URL) and whether a web AppView renders it are DESIGN's call; this story's contract (stable, attributed, verified, anti-merging-preserving, query-not-snapshot) holds regardless.

### Outcome KPIs

See `outcome-kpis.md` KPI-AV-6 (shared-link usage — realizes the J-004 shareable-link success signal) and KPI-AV-2 (anti-merging preserved across the share boundary).

### Technical Notes

- Depends on US-AV-002/003 (the search surface whose query is encoded into the link).
- The link encodes the query parameters, not a result snapshot — so resolving it always re-verifies attribution and never persists a merged view (anti-merging across the share boundary).
- DESIGN owns: the link scheme, the resolver (CLI re-run vs a web AppView), and whether the web AppView is in slice-05 or deferred. The slice-05 product requirement is the shareable, query-encoding, attribution-preserving link; a full web UI is OUT of scope unless DESIGN scopes a minimal resolver (OD-AV-6).
- This is the slice's most scope-creep-prone surface (a web AppView could balloon it). Kept to a stable shareable link + resolver contract; full presentational web UI is deferred (see story-map "deferred / out of scope").

---

## Summary table

| Story | Title | Job link | Right-sized? | DoR status |
|---|---|---|---|---|
| US-AV-001 | Bootstrap the indexer service + verified, attributed ingest (`@infrastructure`) | `infrastructure-only` | YES (3 days, 2 scenarios) | PASS (with infra rationale) |
| US-AV-002 | Search by philosophy (object) at network scale, attribution preserved | J-005 | YES (2.5 days, 4 scenarios) | PASS |
| US-AV-003 | Search by contributor or subject at network scale | J-005 | YES (2 days, 4 scenarios) | PASS |
| US-AV-004 | Trust a discovered result — verified marker + public-data honesty | J-005 | YES (1.5 days, 4 scenarios) | PASS |
| US-AV-005 | Subscribe to a discovered author (discovery → federation) | J-005 | YES (1.5 days, 4 scenarios) | PASS |
| US-AV-006 | Share a network search result as a stable link | J-005 | YES (2 days, 4 scenarios) | PASS |

Total estimated effort: ~12.5 days at moderate confidence. Slice composition
gate: PASS — 5 user-visible stories + 1 infrastructure story; slice is NOT 100%
`@infrastructure` (per `nw-po-review-dimensions` Dimension 0 §5).

> Scope-creep note (this slice is "easiest to scope-creep"): a full web AppView UI
> is explicitly OUT of scope (see `story-map.md` deferred table). slice-05 delivers
> the indexer + CLI `search` discovery surface + a shareable-link contract. Any web
> presentational layer beyond a minimal link resolver is a future slice.
