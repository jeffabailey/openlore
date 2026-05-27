# ADR-016: Pull Semantics — Pull-on-Demand Only; No Auto-Pull, No Push, No Background Daemon

- **Status**: Accepted
- **Date**: 2026-05-27
- **Deciders**: Morgan (nw-solution-architect), per WD-18 lock from Luna (nw-product-owner) for openlore-federated-read
- **Feature**: openlore-federated-read (slice-03)

## Context

slice-03 ingests federated peer claims via the `openlore peer pull` verb
(ADR-013). When and how that pull happens shapes the failure-mode surface,
the user's mental model of "when does my view change," and the operational
profile of the CLI (daemon? cron? interactive only?).

DISCUSS locked the answer (WD-18): pull-on-demand only. Auto-pull and push
are both deferred. DESIGN records the architectural commitments this lock
implies and the assumptions it bakes into the rest of the system, so
future slices know what they're amending if they relax it.

DESIGN owns:

1. The exact pull verb semantics (global vs per-peer; concurrency model;
   network failure handling).
2. The boundary between pull-time validation and query-time validation
   (per anxiety-scenario-2 from the gherkin expansion).
3. The first-pull-orientation message trigger (per OD-FED-2 default
   verdict: once-per-user via identity.toml state).
4. The pull's interaction with the probe gauntlet at startup.

## Decision

**Pull is a single explicit verb invoked by the user; it operates over
ALL active subscriptions in one batch; it fails-soft per-peer and
per-claim; it stores only verified records; it runs no background process.**

### Pull verb shape (slice-03)

```
openlore peer pull
```

- No arguments: pull from every peer in `peer_subscriptions WHERE removed_at IS NULL`.
- No `--peer <did>` filter in slice-03: defer to slice-04 if dogfeed reveals a use case.
- No `--since <ts>` filter: every pull re-fetches every peer's full record list and de-dupes by CID against `peer_claims`. Optimization deferred (atrium-api's listRecords pagination is the natural place to add a `since` cursor; not slice-03's problem).
- No `--dry-run` flag: would add a code path that does network I/O but no storage write; not justified for slice-03 (failure surface confined to `--purge` per WD-21).

### Concurrency model

- **Per-peer pulls run sequentially**, not concurrently. Slice-03 priority is correctness and observable behavior; concurrent peer pulls would interleave per-peer progress output and complicate failure aggregation.
- Within a single peer's pull, records are fetched, verified, and inserted **one at a time** in a single DuckDB transaction per record (or per batch of ≤100 records — DELIVER's call). Per-record signature verification is the dominant cost; batching write amortizes COMMIT overhead.
- Slice-04 may revisit concurrency once a baseline single-peer-pull profile exists.

### Failure modes (per-peer / per-record)

| Failure | Granularity | Behavior |
|---|---|---|
| Peer's PDS unreachable | Per-peer | Skip that peer with `peer did:plc:X: PDS unreachable (connection refused); skipping`. Continue with other peers. Overall exit code is non-zero. |
| Peer's DID document re-resolution failure | Per-peer | Same as above; the cached `peer_pds_endpoint` is NOT used as fallback (security: a peer who rotated to a different PDS may have done so to revoke a compromised key). |
| Per-record signature verification fails | Per-record | Reject that record only; report in summary `rejected 1 (signature invalid)`; continue with remaining records. |
| Per-record CID recomputation mismatch | Per-record | Reject that record only; report in summary `rejected 1 (CID mismatch — possible adversarial input)`; continue. |
| Per-record schema version unknown (record uses a Lexicon field this binary doesn't recognize) | Per-record | If the unknown field is OPTIONAL, ingest the record and ignore the field (per ADR-005). If the unknown field is REQUIRED by a future schema, reject with `schema version unknown — upgrade openlore`. |
| Disk full during write | Per-record | Roll back that record's transaction; report the disk error; the remaining records in the pull MAY also fail subsequent writes; exit non-zero. |
| Local user's DID appears as the `author` field on a peer's record | Per-record | Reject with `PeerStorageError::SelfAttribution`. Peer published a record claiming to be from the current user. The slice-03 trust model says "we trust signatures, not author fields"; if such a record DID verify against the current user's key, it would mean the user's key was compromised — out of slice-03 scope. Reject defensively. |

### Pull-time vs query-time validation

| Validation | Pull time | Query time |
|---|---|---|
| Signature verifies against peer's DID-doc key | REQUIRED — verified or rejected | OPTIONAL (slice-03: not re-verified; deferred to slice-04 if performance permits). Per anxiety scenario 2's `# DISTILL: confirm`: slice-03 trusts the local disk; later slices may add `peer verify --all`. |
| CID recomputation matches peer-published rkey | REQUIRED — verified or rejected | NOT re-checked (CID is the row PK; mismatch at insert is impossible by construction) |
| Lexicon schema validates | REQUIRED — rejected or ingested | NOT re-checked (already validated; ingested rows are well-formed) |
| Author DID matches a subscribed peer | The current user's pull walks `peer_subscriptions WHERE removed_at IS NULL`; each peer's records arrive attributed to THAT peer's DID. Cross-attributed records (record author != subscribed peer; see anxiety scenario 1) are REJECTED at pull time per the SelfAttribution check extended for "any DID other than the subscribed peer" — DELIVER confirms this. | n/a |

The slice-03 design intentionally chooses **pull-time validation, trust
local disk at query time**. Re-verification at query time would be
correct-er but at performance cost; the disk-tampering failure mode
(anxiety scenario 2) is real but rare and is mitigated by:

- The pull-time verification (a tampered record never enters peer_claims).
- The on-disk file integrity (signed-record JSON files are also
  content-addressable; a tampered file's local CID would not match the row
  PK; a future `peer verify --all` verb in slice-04 surfaces this).
- The filesystem permissions on `~/.local/share/openlore/peer_claims/` —
  user-owned, no group write, no world access. Tampering requires either
  the user's own action or malware running as the user.

### First-pull orientation message (per OD-FED-2)

The first-EVER `openlore peer pull` invocation by a given user (NOT first
per session; first per-install) prints a one-line orientation:

```
First federated pull complete. From now on, run `openlore peer pull`
on demand — claims do not auto-refresh.
```

State storage: a new key `federation.first_pull_completed_at` (RFC3339
UTC timestamp) in `~/.config/openlore/identity.toml`. The orientation
fires when this key is absent or empty; on a successful pull, the key is
set and never cleared. The check is local-only; no telemetry implication.

### Pull's interaction with probe gauntlet

The probe gauntlet (ADR-009 "wire then probe then use") at startup runs
ALL adapter probes BEFORE dispatching to any verb, including `peer pull`.
This means a `peer pull` invocation fails fast if:

- `adapter-atproto-did`'s probe refused (e.g., keychain unreachable),
- `adapter-atproto-pds`'s probe refused (e.g., TLS handshake fails on the
  user's own PDS — a precondition for any network operation),
- `adapter-duckdb`'s probe refused (e.g., schema mismatch).

The pull itself does NOT probe each peer's PDS at startup; peer PDS
reachability is checked at pull time per-peer (different probe; different
failure semantics — peer down is per-peer-soft-fail, not global-hard-fail).

### Push and daemon explicitly rejected (architectural commitment)

- **No long-running process**: the CLI binary exits after every command.
  A future "background pull" feature would add a daemon, a service-manager
  integration (systemd / launchd / Task Scheduler), and a new operational
  surface. Slice-03 commits to remaining a non-daemon CLI.
- **No push subscriptions**: no ATProto Firehose, no WebSockets, no
  Server-Sent Events. Slice-03 trades freshness for simplicity. The
  staleness window (time between a peer publishing and the user seeing
  the claim) is bounded only by how often the user runs `peer pull`.
- **No cron integration**: slice-03 does not ship a cron snippet or
  systemd timer unit. A user who wants scheduled pulls writes their own
  cron entry calling `openlore peer pull` — supported but uninstalled by
  default.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Auto-pull on `peer add`** | Locked rejected by WD-18. Conflates subscribe-success with pull-success; widens subscribe's failure surface; subscribing to a slow PDS would slow `peer add` unpredictably. |
| **Auto-pull on every CLI invocation** | Considered briefly. Rejected: violates the predictability principle (network activity on every command would surprise users running `graph query --subject ...` offline); makes the local-first KPI-FED-5 unmeasurable. |
| **Push subscriptions via ATProto Firehose** | Locked rejected by WD-18. Requires daemon; violates CLI-first; adds substantial protocol complexity (reconnection, ordering, ATProto firehose semantics) orthogonal to the J-003 hypothesis under test. Re-evaluate post-MVP if real-time freshness becomes a JTBD. |
| **`--peer <did>` filter on `peer pull`** | Defer to slice-04. The use case (re-pull a specific peer after they republished a claim) is real but slice-03 batches are small enough that "pull all" is acceptable. Adds CLI surface for marginal value at slice-03 stage. |
| **`--since <ts>` filter (only fetch new records)** | Defer. Atrium-api's listRecords supports a cursor; using it would require persisting a per-peer cursor (small schema add). For slice-03 peer-claim cardinality (≤100 per peer in practice), full re-fetch + CID-dedup is acceptable. |
| **Re-verify signatures at query time** | Considered for anxiety scenario 2 protection. Rejected for slice-03 on performance grounds (every federated query would do N Ed25519 verifications). Mitigation: filesystem perms + future `peer verify --all` in slice-04. |
| **Pull concurrency (N peers in parallel)** | Premature; slice-03 peer counts in dogfeed are small (handful per user). Sequential pull keeps output ordered and failure aggregation simple. Revisit at slice-04. |

## Consequences

### Positive

- The verb surface stays small: one `peer pull` verb, no flags in slice-03.
- Network activity is predictable: user knows exactly when bytes cross
  the wire (subscribe-time DID resolution + pull-time PDS fetch). Offline
  workflows (`graph query --federated` after pull) work unchanged.
- No daemon = no operational surface = no orphan process bugs, no
  systemd-vs-launchd compatibility matrix, no "is the daemon running?"
  diagnostic verbs.
- Failure isolation is per-peer-and-per-record; a bad actor or down peer
  does not block other peers' pulls.
- Pull-time validation centralizes the trust-decision: bad records never
  enter the local store. Query-time logic can trust that every row in
  `peer_claims` was verified at some point.

### Negative

- **Staleness window**: a peer publishing a claim is invisible until the
  user runs `peer pull`. This is the trade-off for the simplicity; the
  first-pull orientation message and the per-verb output hints
  ("To pull updates: `openlore peer pull`") mitigate by anchoring the
  user's mental model.
- **No notification mechanism**: the user has no way to know "Rachel
  published 3 new claims since yesterday" without pulling. Acceptable for
  slice-03 dogfeed; revisit per JTBD if friction emerges.
- **Re-fetch every pull**: bandwidth cost grows with peer count × claim
  count. Bounded for slice-03 (a few peers × a few hundred claims max);
  scales poorly to many peers × many claims. Slice-04+ optimization via
  per-peer cursors.
- **Disk-tampering window**: between pull and next pull, a tampered
  on-disk file would be queried as if valid. Real but contained by
  filesystem perms; explicit `peer verify --all` deferred to slice-04.

### Earned Trust

The pull verb's `probe()`-equivalent contract surfaces are:

1. **Pull idempotency**: re-running `peer pull` against a peer with N
   records that were already pulled MUST report `0 new, N already in
   peer_claims, skipped` and write nothing. Test: integration test
   `peer_pull_idempotent` populates a fixture peer with 5 claims, runs
   pull, asserts 5 written; runs pull again, asserts 0 written.

2. **Per-record fault isolation**: a fixture peer with 5 records, 1 of
   which has a tampered signature, MUST result in 4 stored + 1 rejected;
   the rejected record's CID is NOT in `peer_claims`. Test:
   `peer_tampered_signature_rejected` (mandated by KPI-FED-6).

3. **Per-peer fault isolation**: 2 fixture peers, one's PDS unreachable,
   the other reachable with N records, MUST result in N records pulled
   from the reachable peer + a "skipping" line for the unreachable.
   Exit code is non-zero overall to flag the partial failure.

4. **No-self-attribution defense**: a fixture peer that publishes a
   record whose `author` field is the local user's DID MUST be rejected
   at pull time with `PeerStorageError::SelfAttribution`. Test:
   `peer_self_attribution_rejected`.

5. **First-pull orientation idempotency**: the orientation message
   appears EXACTLY ONCE across all subsequent pulls by the same user.
   Test: capture stdout across 3 consecutive `peer pull` invocations;
   assert the orientation line appears only in the first.

These five tests are mandatory for slice-03's pull surface and live in
the acceptance suite per the DISTILL handoff in feature-delta.md.

## Revisit Trigger

- A JTBD validation surfaces real-time freshness as a primary need
  (post-MVP). Add a push subscription mode (likely via ATProto Firehose);
  introduces daemon surface and operational complexity.
- A scripting use case requires per-peer or per-claim filters on pull
  (`--peer <did>`, `--since <ts>`). Both are additive flag extensions to
  the existing verb; no architectural amendment beyond ADR-013.
- Peer counts in dogfeed grow such that sequential pull is observably
  slow (KPI-FED-5 P95 > 180s threshold from outcome-kpis.md). Add
  concurrent peer pulls with bounded parallelism (e.g., 4-wide); revisit
  ordering of output.
- The trust model evolves to require periodic re-verification of cached
  peer claims. Add `peer verify --all` verb (slice-04 candidate);
  amendment to ADR-013 verb list.
