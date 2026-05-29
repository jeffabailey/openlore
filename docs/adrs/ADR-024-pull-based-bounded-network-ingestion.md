# ADR-024: Pull-Based Bounded Network Ingestion — the ADR-016 Firehose Re-Evaluation (Pull, not Firehose, for slice-05)

- **Status**: Proposed
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-108 / OD-AV-4 for openlore-appview-search (slice-05)
- **Feature**: openlore-appview-search (slice-05)
- **Revisits / affirms**: ADR-016 (pull-on-demand; "push subscriptions locked OUT for slice-03 — re-evaluate at slice-05"). This is that re-evaluation.
- **Resolves**: OD-AV-4 (ingestion model).

## Context

ADR-016 locked OUT push subscriptions (ATProto Firehose) for slice-03 with an
explicit "re-evaluate at slice-05 when network-scale discovery arrives" note.
slice-05's indexer needs a way for PUBLIC signed claims to ARRIVE so they can be
verified + indexed. Two models:

1. **Pull-based bounded ingestion**: the indexer pulls public records from a
   bounded, enumerable set of network sources (DID documents → PDSes / a
   configurable relay), verifies each, and indexes. Re-runs on demand or on a
   simple interval; no persistent connection.
2. **ATProto Firehose (push)**: a persistent subscription to a relay's event
   stream (`com.atproto.sync.subscribeRepos`), reacting to records in real time.

The forces (carried from ADR-016's reasoning, now at network scale):

- **Walking-skeleton sufficiency** (WD-108): the slice's job is to validate that
  trustworthy network discovery surfaces unfollowed authors (KPI-AV-1). The corpus
  needs to EXIST, be VERIFIED, and be SEARCHABLE. HOW claims arrive (pull vs push)
  is invisible to the user-visible contract.
- **Firehose is heavy**: a persistent connection implies reconnection logic,
  cursor/sequence management, back-pressure, ordering, and ATProto firehose
  framing — substantial protocol complexity orthogonal to the J-005 hypothesis.
- **Daemon-shape creep**: Firehose pushes the indexer toward an always-on,
  reconnecting daemon. ADR-016 deliberately kept OpenLore a non-daemon CLI;
  slice-05's indexer is already a long-running process, but a pull loop is far
  simpler to reason about, test, and bound than a firehose consumer.
- **Bounded ingest is verifiable**: a pull over an enumerable source set is
  deterministic and hermetically testable (a fixture relay returns a known record
  set) — which the verified-before-index gate (WD-104 / KPI-AV-3) needs.

DESIGN owns:

1. The ingestion model (this ADR).
2. The bounded source-discovery strategy for the walking skeleton.
3. The per-record fault-isolation contract (reusing ADR-016's discipline).
4. The freshness/staleness window and its observability.

## Decision

**slice-05 ingests via PULL-BASED BOUNDED ingestion. ATProto Firehose is a
documented FUTURE option, NOT slice-05's mechanism.** The indexer pulls public
signed-claim records from a bounded source set, verifies each (ADR-026 + the pure
`claim-domain` core), and indexes the verified, attributed records. The pull loop
reuses the slice-03 pull-time verification discipline (ADR-016 / KPI-FED-6) at
network scale — there is NO second verification path.

### Bounded source-discovery strategy (walking skeleton)

The indexer's ingest source set is BOUNDED and ENUMERABLE — never "the whole
firehose". For the walking skeleton, the source set is the union of:

1. **Seed DIDs**: a configured list of network author DIDs (the dogfood seed —
   e.g., the user's own federation graph exported, plus a small curated seed). The
   indexer resolves each DID document → PDS → `listRecords` for the
   `org.openlore.claim` collection (the same read surface slice-03 `peer pull`
   uses, extended to arbitrary network authors).
2. **A configurable relay endpoint** (optional): if configured, the indexer
   queries a relay's `listRecords`-equivalent enumeration for `org.openlore.*`
   records. This is still PULL (request/response), not a push subscription.

Source discovery is deliberately conservative: bounded, enumerable, re-runnable.
Growing coverage (the KPI-AV-1 sparsity risk) is a seed-set + relay-config
concern, not an architectural one.

### Pull cadence (non-daemon-leaning)

- The indexer runs an ingest pass on startup, then on a SIMPLE interval
  (`--ingest-interval`, default conservative; DELIVER tunes), and on an explicit
  `openlore-indexer ingest` one-shot subcommand.
- NO persistent connection, NO firehose cursor, NO reconnection state machine.
  Each pass is a bounded enumeration + verify + upsert. This keeps the indexer
  "a process that runs a pull loop" rather than "a firehose daemon".
- De-dup by CID against the index (a record already indexed is skipped), exactly
  as slice-03 `peer pull` de-dups by CID against `peer_claims`.

### Per-record / per-source fault isolation (reuses ADR-016)

The pull loop reuses the ADR-016 fault-isolation contract verbatim, at network
scale:

| Failure | Granularity | Behavior |
|---|---|---|
| A source DID's PDS unreachable | Per-source | Skip that source; record `indexer.ingest.source_skipped{reason}`; continue with other sources. |
| Per-record signature verification fails (ADR-026) | Per-record | Reject that record only; `indexer.ingest.rejected{reason: bad_signature}`; NEVER indexed; continue. |
| Per-record CID recomputation mismatch | Per-record | Reject that record only; `indexer.ingest.rejected{reason: cid_mismatch}`; NEVER indexed; continue. |
| Per-record unsigned / missing signature | Per-record | Reject; `indexer.ingest.rejected{reason: unsigned}`; continue. |
| Per-record Lexicon schema unknown-required-field | Per-record | Reject `schema version unknown`; continue (ADR-005 forward-compat for OPTIONAL fields). |
| A record's `author` field is the indexer-operator's own DID | Per-record | Indexed normally (the operator's own published claims ARE public network claims; the indexer has no special "self"). Distinct from slice-03's SelfAttribution defense, which protected a user's local store; the indexer has no user identity to impersonate. |

The verified-before-index gate (WD-104) is the load-bearing reuse: a record
enters the index ONLY after `claim_domain::verify(record, author_pubkey)` AND
`claim_domain::compute_cid(record) == published_cid` both pass. This is the SAME
pure core slice-01/03 use — no second verification path (KPI-AV-3).

### Freshness / staleness window (observable)

- Like ADR-016's pull model, there is a staleness window: a claim published to the
  network is invisible to search until the next ingest pass indexes it.
- This is OBSERVABLE: the index-coverage / freshness dashboard (DEVOPS handoff)
  surfaces `indexer.ingest.lag` and `distinct_authors_indexed`. An ingest lag
  exceeding a DESIGN-defined freshness budget is an informational alert (feeds the
  KPI-AV-1 sparsity diagnosis).
- The staleness window is acceptable for the discovery thesis: KPI-AV-1 is a
  "did you discover an unfollowed author this session" metric, not a real-time
  freshness metric. Firehose's real-time advantage is not on the slice-05 critical
  path.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **ATProto Firehose (push subscription via `com.atproto.sync.subscribeRepos`)** | Deferred (WD-108 / re-affirms ADR-016). Real-time but heavy: reconnection, cursor/sequence management, back-pressure, ordering, firehose framing — substantial protocol complexity orthogonal to the J-005 discovery hypothesis. Pushes the indexer toward an always-on reconnecting daemon. Real-time freshness is not on the slice-05 critical path (KPI-AV-1 is a per-session discovery-rate metric). Documented future option (revisit trigger below). |
| **Unbounded firehose-style "index everything on the network"** | Rejected. Unbounded ingest is untestable (no hermetic fixture), unbounded in storage/cost, and tempts indexing low-quality / spam records. Bounded, enumerable source sets keep ingest deterministic, hermetically testable (a fixture relay), and aligned with the verified-before-index gate. |
| **Pull triggered ONLY by the CLI (no indexer-side loop)** | Rejected. The CLI is local-first and must not become responsible for populating a network index; coupling ingest to CLI invocation would (a) make coverage depend on how often a user runs `search`, and (b) drag network-ingest into the local-first binary. The indexer owns its own ingest cadence (ADR-023). |
| **Re-verify at query time instead of at ingest** | Rejected (mirrors ADR-016's same rejection). Every search would do N Ed25519 verifications; the verified-before-index gate centralizes the trust decision so a search can trust every indexed row was verified. The `[verified]` marker is therefore a construction guarantee, not a per-result runtime check (WD-104 / US-AV-004). |

## Consequences

### Positive

- Far simpler than Firehose: no persistent connection, no cursor/reconnect state
  machine, no back-pressure. The indexer is "a process that runs a bounded pull
  loop", testable hermetically against a fixture relay.
- Reuses the slice-03 verification discipline (ADR-016 / KPI-FED-6) and the pure
  `claim-domain` core verbatim — no second verification path (KPI-AV-3 holds by
  construction).
- Bounded + enumerable ingest is deterministic and hermetically testable — exactly
  what the `indexer_rejects_unverified_claim` release gate needs (fixture sources
  with tampered/unsigned/CID-mismatch records).
- Coverage is a config concern (seed set + relay), reversible and tunable without
  an architectural change.

### Negative

- **Staleness window**: a network claim is invisible to search until the next
  ingest pass. Mitigation: observable ingest lag (DEVOPS dashboard); acceptable for
  the per-session discovery thesis; revisit toward Firehose if real-time freshness
  becomes a JTBD.
- **Coverage depends on the seed/relay config** (the KPI-AV-1 sparsity risk).
  Mitigation: index-coverage dashboard; seed the source set from the user's
  federation graph + a curated seed; a shared/community indexer + relay is the
  documented scale path.
- **Re-fetch cost grows with source count × record count** (same as ADR-016's pull
  cost). Bounded for the walking skeleton; per-source cursors (the ADR-016 `--since`
  deferral) are the natural optimization when it bites.

### Earned Trust

The ingest source is a network dependency the indexer MUST probe (principle 12).
The `AtProtoIngestAdapter` (the driven port for the ingest source) ships a
`probe()` within the 250ms budget (ADR-009 I-4/I-5) that exercises:

1. **Source reachability + enumeration shape**: the adapter can resolve a fixture
   source DID document and enumerate its `org.openlore.claim` records; a
   misconfigured / unreachable source produces a structured
   `health.startup.refused{reason: indexer.ingest_source_unreachable}` (the
   composition root refuses to start an indexer that cannot reach its only source;
   a configured-but-down OPTIONAL relay degrades to seed-DIDs-only, not refusal).
2. **The substrate-lie scenario (catalogued)**: a fixture source that returns a
   record whose signature does NOT verify and a record whose CID does NOT match —
   the probe asserts the ingest path REJECTS both before they reach the index
   (the verified-before-index gate is exercised by the probe, not just trusted).
   This is the slice-05 "what happens if the network lies?" check: the network
   WILL serve tampered/fabricated records; the design refuses to trust them and
   the probe proves the rejection path works in the real environment.

The pull loop itself reuses the ADR-016 Earned-Trust contracts (idempotency,
per-record fault isolation, per-source fault isolation) at network scale, in the
indexer's acceptance suite (`indexer_rejects_unverified_claim` is the
release-gate; KPI-AV-3).

## Revisit Trigger

- Real-time freshness becomes a primary JTBD (dogfood evidence: users want
  newly-published claims discoverable within seconds, not within an ingest
  interval). Add a Firehose subscription mode — additive to the bounded-pull model,
  with the daemon/reconnection complexity scoped to its own ADR.
- Index coverage from bounded pull is too sparse (KPI-AV-1 < 20% disprover, and
  the diagnosis is coverage). Expand the source set / relay config first; consider
  Firehose only if the bottleneck is freshness, not coverage.
- Per-source re-fetch cost becomes observably slow. Add per-source cursors
  (`--since`) — the ADR-016 deferral, now at network scale.
