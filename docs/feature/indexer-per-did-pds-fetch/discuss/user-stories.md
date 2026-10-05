<!-- markdownlint-disable MD024 -->
# User Stories: indexer-per-did-pds-fetch

> Every story has a `job_id` from `docs/product/jobs.yaml`, or is `infrastructure-only` with a
> rationale. Personas and FR/NFR IDs are in `requirements.md`. AC IDs (AC-NNN.n) are
> consolidated in `acceptance-criteria.md`. Data such as DIDs and PDS hosts is illustrative but
> realistic.

## System Constraints (cross-cutting, apply to every story)

- **I-IPF-1 Verdict unchanged (ADR-071).** Self-attested is admitted only when the read-from
  URL exactly equals the PDS freshly resolved for the repo DID in the same pass. The CID is
  checked against the rkey from the `at://` URI. App-signed records use the unchanged
  verify-before-index gate (WD-104).
- **I-IPF-2 Fallback is never own-PDS.** Records read through `OPENLORE_INDEXER_SOURCE_URL`
  are relay-origin. If the fallback URL equals a DID's resolved PDS, that DID was resolvable,
  so the fallback is not used for it.
- **I-IPF-3 Per-DID fault isolation (ADR-024).** No single DID failure aborts the pass or
  prevents other DIDs from being indexed. Skipping never deletes indexed rows.
- **I-IPF-4 Bounded (ADR-024).** There is a fixed cap on concurrent outbound requests, the
  existing per-repo page bound, and a per-DID time budget. The cadence is unchanged.
- **I-IPF-5 Anti-merging (ADR-025).** Every indexed row is attributed to the repo DID it was
  listed for. There is no consensus or merge.
- **I-IPF-6** Capability boundary (ADR-023) unchanged. Observability is structural only
  (WD-105: DIDs, URLs, counts and reasons, never claim content).
- Technology and event schema choices are deferred to DESIGN (OQ-IPF-1..6).

---

## US-IPF-001: Find self-attested claims from authors on any PDS

- **job_id**: J-005 (beneficiary: J-009, where Priya's approved claims become discoverable)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1

### Elevator Pitch

- **Before**: Maria runs `openlore search --object org.openlore.philosophy.reproducible-builds` and never sees Priya, because Priya's self-attested claims live on bsky.social and the indexer refuses them as relay-origin.
- **After**: Maria runs the same `openlore search` and sees `Author: did:plc:priyaraman7x2k (priyaraman.bsky.social) … github:priyaraman/cargo-pin confidence 0.82 [verified]`.
- **Decision enabled**: Maria decides whether to look at `cargo-pin` or follow Priya (`openlore peer add`), based on a claim she could not see before.

### Problem

Maria cares about reproducible builds and follows nobody who claims it. Priya Raman approved 3
self-attested claims in the review app, and they are stored on her bsky.social PDS. The indexer
reads every repo from the single URL Jeff configured, which is his own PDS. Priya's records
therefore look relay-origin and are refused, so Maria's network search is missing exactly the
new authors the review app was built to bring in.

### Who

- Maria | searcher who does not know whom to follow | wants every public claim on the network.
- Priya Raman | bsky.social author | wants her approved claims discoverable.

### Solution

Each pass resolves every configured repo DID to the PDS its DID document names, and lists that
repo's claims there (with `repo=<DID>` and cursor paging). The unchanged ADR-071 verdict then
admits self-attested records read from the author's own PDS.

### Domain Examples

#### 1: Happy path, two authors on two PDSes

Priya (`did:plc:priyaraman7x2k`, `https://morel.us-east.host.bsky.network`, 3 self-attested
claims) and Dmitri (`did:plc:dvolkov3m9q`, `https://pds.volkov.dev`, 2 app-signed claims) are
both configured. One pass indexes all 5. Priya's 3 are stored as self-attested and Dmitri's 2
as app-signed.

#### 2: Edge: Priya moved PDS

Between passes Priya migrates to `https://pds.priyaraman.dev`. Her DID document is updated.
The next pass reads from `pds.priyaraman.dev`, and her claims are still admitted. Nothing is
read from the old host.

#### 3: Error: a PDS serves a different repo

`did:plc:mallory4k1z`'s DID document names a PDS whose listing returns records with
`at://did:plc:priyaraman7x2k/...` URIs. None of those records are indexed under Mallory, and
each one is counted as refused.

#### 4: Error: tampered rkey

One of Priya's records has an rkey that does not equal its recomputed CID. That record is
refused (`cid_mismatch`), and her other two are indexed.

### UAT Scenarios (BDD)

#### Scenario: Authors on different PDSes are all found in one pass

Given Priya's DID document names https://morel.us-east.host.bsky.network and she has 3 self-attested claims there
And Dmitri's DID document names https://pds.volkov.dev and he has 2 app-signed claims there
When one ingest pass runs
Then all 5 claims are indexed, each attributed to its own author
And Priya's 3 are recorded as self-attested and Dmitri's 2 as app-signed

#### Scenario: Maria finds Priya through network search

Given Priya's self-attested claim that github:priyaraman/cargo-pin embodies reproducible-builds was indexed
When Maria runs openlore search --object org.openlore.philosophy.reproducible-builds
Then Maria sees that claim attributed to did:plc:priyaraman7x2k

#### Scenario: An author who moved PDS is followed to the new one

Given Priya's claims were indexed from morel.us-east.host.bsky.network on the previous pass
And her DID document now names https://pds.priyaraman.dev
When the next ingest pass runs
Then her claims are read from pds.priyaraman.dev and admitted as self-attested

#### Scenario: Records of another repo are never attributed to the requested author

Given the PDS named for did:plc:mallory4k1z answers with records of did:plc:priyaraman7x2k
When one ingest pass runs
Then no record is indexed under did:plc:mallory4k1z
And each such record is counted as refused

#### Scenario: Tampered self-attested records are still refused

Given one of Priya's 3 records has an rkey that does not match its content
When one ingest pass runs
Then that record is refused as a CID mismatch and her other 2 are indexed

### Acceptance Criteria

- [ ] AC-001.1 In one pass, each configured DID is listed on the PDS its DID document names, and claims from several PDSes are indexed with correct attribution.
- [ ] AC-001.2 Self-attested records read from the author's freshly resolved PDS are indexed as self-attested. App-signed records are indexed exactly as today.
- [ ] AC-001.3 An indexed self-attested claim is returned by `openlore search` attributed to its author.
- [ ] AC-001.4 Resolution happens every pass. After a PDS move, records are read from the new PDS and still admitted.
- [ ] AC-001.5 Records whose `at://` repo differs from the requested DID are never indexed for that DID and are counted as refused.
- [ ] AC-001.6 The CID == rkey check still refuses tampered records. The other records of the same repo are unaffected.

### Outcome KPIs

- **Who**: authors with self-attested claims on a PDS other than the fallback | **Does what**: appear in network search | **By how much**: 100% of configured, resolvable and reachable authors after one pass | **Measured by**: `provenance` refusals for own-PDS records (must be 0) and the self-attested indexed count | **Baseline**: 0% (all refused).

### Technical Notes

- Brownfield: `crates/openlore-indexer/src/run.rs` `ingest` lists from `wiring.source_url`, and `origin_for` already compares `fetched_from` with the resolved PDS. The resolved PDS should be resolved once and used for both the listing and the verdict (registry IC-2).
- Reuses `RepoListingPort` (repo + cursor paging) and `IdentityLookupPort`. No schema migration (provenance column exists).
- Today `origin_for` resolves only when the listing holds self-attested records. Resolution now always happens, because it decides where to list.

---

## US-IPF-002: One unreachable author never blocks the others

- **job_id**: J-005
- **Release**: Release 1 | **MoSCoW**: Must | **Priority**: P2

### Elevator Pitch

- **Before**: When `pds.volkov.dev` returns 502, `openlore-indexer ingest` stops with `listing did:plc:dvolkov3m9q failed` and exit code 2, and nobody's new claims reach `openlore search` that pass.
- **After**: `openlore-indexer ingest` exits 0 and prints `{"event":"indexer.ingest.source_skipped","did":"did:plc:dvolkov3m9q","reason":"pds_unreachable",...}`, and Maria's `openlore search` shows Priya's new claim from the same pass.
- **Decision enabled**: Jeff decides whether to wait for the next pass, contact Dmitri, or drop the DID from the list. Maria keeps getting fresh results.

### Problem

Fetching from each author's own PDS multiplies the number of hosts the indexer depends on. Today
a single failed listing aborts the whole pass. As more bsky.social and self-hosted authors are
added, the chance that some PDS is down during a pass approaches certainty. Jeff cannot tell
which DID failed or why without reading stderr. Maria sees stale results.

### Who

- Jeff Bailey | indexer operator with 14 repo DIDs | wants clear per-DID reasons and a pass that completes.
- Maria | searcher | wants results that stay fresh when one author's host is down.

### Solution

Each DID is handled independently. An unresolvable DID with no fallback, an unreachable PDS, a
timeout or a listing error skips that DID with a structured reason, and the pass continues and
completes. Skipped DIDs are retried next pass, and their previously indexed claims stay
searchable. Outbound requests are bounded in concurrency and time.

### Domain Examples

#### 1: PDS down

`pds.volkov.dev` returns 502. Priya's and Jeff's claims are indexed. Dmitri is skipped with
reason `pds_unreachable`. His 2 claims indexed yesterday still appear in search.

#### 2: DID unresolvable, no fallback

`did:plc:ghost0000` returns 404 from `plc.directory`, and no fallback is configured. It is
skipped with reason `did_unresolvable`. The other 13 DIDs are indexed.

#### 3: Slow PDS

`pds.slowhost.example` accepts the connection but never answers. After `${per_did_time_budget}`
it is skipped with reason `pds_timeout`, and the pass finishes.

#### 4: Recovery

On the next pass `pds.volkov.dev` is back. Dmitri's new third claim is indexed and no skip is
reported for him.

### UAT Scenarios (BDD)

#### Scenario: One PDS being down does not hide the others

Given pds.volkov.dev returns 502
When one ingest pass runs
Then Priya's and Jeff's claims are indexed
And Jeff sees did:plc:dvolkov3m9q skipped with reason pds_unreachable
And the pass completes successfully

#### Scenario: An unresolvable author is skipped with a reason

Given did:plc:ghost0000 cannot be resolved and no fallback is configured
When one ingest pass runs
Then the other configured authors are indexed
And Jeff sees did:plc:ghost0000 skipped with reason did_unresolvable

#### Scenario: A skipped author's earlier claims stay searchable

Given Dmitri's 2 claims were indexed on an earlier pass
And pds.volkov.dev is down for this pass
When Maria searches for reproducible-builds after this pass
Then she still sees Dmitri's 2 claims

#### Scenario: A hanging PDS cannot stall the pass

Given pds.slowhost.example never answers
When one ingest pass runs
Then that author is skipped with reason pds_timeout within the per-author time budget
And every other author is indexed

#### Scenario: Skipped authors are retried on the next pass

Given Dmitri was skipped on the previous pass because his PDS was down
And pds.volkov.dev is reachable again with 3 claims
When the next ingest pass runs
Then all 3 of Dmitri's claims are searchable

#### Scenario: Many authors never cause unbounded concurrent requests

Given 40 authors spread across 12 PDS hosts are configured
When one ingest pass runs
Then at no moment are more than the configured maximum of outbound requests in flight
And the pass summary shows 40 configured authors accounted for as own-PDS, fallback or skipped

### Acceptance Criteria

- [ ] AC-002.1 A DID whose PDS is unreachable, returns an error, or whose listing fails is skipped. The other DIDs are indexed, and the pass ends successfully (exit 0).
- [ ] AC-002.2 An unresolvable DID with no fallback is skipped with reason `did_unresolvable`.
- [ ] AC-002.3 Each skip emits one structured event with the DID, the reason (`did_unresolvable` | `pds_unreachable` | `pds_timeout` | `listing_failed`), and, when known, the PDS URL. It contains no claim content.
- [ ] AC-002.4 Skipping never removes previously indexed rows for that DID.
- [ ] AC-002.5 A PDS that does not answer is abandoned after the per-DID time budget with reason `pds_timeout`.
- [ ] AC-002.6 A DID skipped on one pass is attempted again on the next pass.
- [ ] AC-002.7 Concurrent outbound requests never exceed `${max_concurrent_fetches}`. The pass summary satisfies own_pds + fallback + skipped = configured.

### Outcome KPIs

- **Who**: indexer passes | **Does what**: complete and index every reachable DID despite individual failures | **By how much**: 0 passes aborted by a single DID failure. 100% of skips carry a reason | **Measured by**: pass exit codes and `source_skipped` events vs the summary | **Baseline**: any single listing failure aborts the pass (`run.rs`, exit 2).

### Technical Notes

- Brownfield gap: today `ingest` returns 2 on the first listing error. ADR-024 already specifies `indexer.ingest.source_skipped{reason}`. This story realizes it.
- Index store upsert failures remain pass-fatal. That is a local store fault, not a source fault. DESIGN to confirm (OQ-IPF-5).
- 6 scenarios is the top of the range. Split option, if DESIGN estimates more than 3 days: 002a isolation and reasons, 002b time and concurrency bounds.

---

## US-IPF-003: Unresolvable authors fall back without weakening provenance

- **job_id**: J-005
- **Release**: Release 1 | **MoSCoW**: Should | **Priority**: P4

### Elevator Pitch

- **Before**: Jeff's `OPENLORE_INDEXER_SOURCE_URL` is the only place any repo is read from. Under per-DID fetch with no fallback, an author whose DID briefly fails to resolve would contribute nothing to `openlore search`.
- **After**: `openlore-indexer ingest` prints `{"event":"indexer.ingest.source_skipped","did":"did:plc:ghost0000","reason":"did_unresolvable","fallback_used":true}`, and that author's app-signed claims appear in Maria's `openlore search` while the self-attested ones are refused (`by_reason.provenance`).
- **Decision enabled**: Jeff decides whether to keep a fallback source configured, knowing it can only add app-signed claims and never self-attested ones.

### Problem

Some configured DIDs cannot be resolved at a given moment. The PLC directory may hiccup, or a
`did:web` host may have stopped serving its document. Jeff's deployment has always had a single
source URL that hosts some of these repos. Dropping those DIDs entirely would lose app-signed
claims that are fully verifiable by signature. Treating the fallback as the author's PDS would
break ADR-071.

### Who

- Jeff Bailey | operator migrating from a single-source setup | wants no lost coverage.
- Maria | searcher | wants app-signed claims to stay findable without being misled about self-attested ones.

### Solution

If a DID cannot be resolved and a fallback source is configured, list it there. Those records
are always relay-origin. App-signed claims go through the normal signature gate, and
self-attested claims are refused for provenance.

### Domain Examples

#### 1: Fallback adds app-signed claims

`did:plc:ghost0000` does not resolve. The fallback `https://pds.jeffbailey.us` holds its 2
app-signed claims and 1 self-attested claim. The 2 app-signed claims are indexed, the 1
self-attested claim is refused (`provenance`), and the skip event shows `fallback_used: true`.

#### 2: Fallback never used for a resolvable DID

Priya resolves to `morel.us-east.host.bsky.network`. The fallback is never contacted for her,
even if it happens to hold copies of her records.

#### 3: Fallback also down

`did:plc:ghost0000` does not resolve and the fallback returns 503. The DID is skipped with
reason `did_unresolvable` and `fallback_used: true`, along with the fallback failure. The other
DIDs are unaffected.

### UAT Scenarios (BDD)

#### Scenario: An unresolvable author's app-signed claims come through the fallback

Given did:plc:ghost0000 cannot be resolved
And the fallback source holds 2 app-signed claims and 1 self-attested claim for it
When one ingest pass runs
Then the 2 app-signed claims are indexed
And the self-attested claim is refused for provenance and does not appear in search

#### Scenario: A resolvable author is never read from the fallback

Given Priya resolves to morel.us-east.host.bsky.network
And the fallback source also holds copies of her records
When one ingest pass runs
Then Priya's records are read only from morel.us-east.host.bsky.network

#### Scenario: A failing fallback is isolated like any other source

Given did:plc:ghost0000 cannot be resolved and the fallback source returns 503
When one ingest pass runs
Then did:plc:ghost0000 is skipped with a reason that shows the fallback was tried
And every other author is indexed

### Acceptance Criteria

- [ ] AC-003.1 An unresolvable DID with a configured fallback is listed from the fallback. Its app-signed records pass the normal gate, and its self-attested records are refused for provenance.
- [ ] AC-003.2 The fallback is never contacted for a DID that resolved in this pass.
- [ ] AC-003.3 A fallback failure skips only the affected DIDs, and the skip event records `fallback_used: true`.

### Outcome KPIs

- **Who**: configured DIDs that are temporarily unresolvable | **Does what**: still contribute their app-signed claims | **By how much**: 100% of app-signed claims available at a reachable fallback are indexed. 0 self-attested claims are admitted through the fallback | **Measured by**: `fallback` count in the pass summary and the `provenance` refusals for fallback-read DIDs | **Baseline**: n/a (new path). Today every DID is read from the source URL.

### Technical Notes

- Depends on US-IPF-002 (skip events). `RecordOrigin::of(fetched_from, resolved_pds)` cannot match when there is no resolved PDS, so the fallback is relay-origin by construction. DESIGN should make that a type-level guarantee (OQ-IPF-3).

---

## US-IPF-004: Indexer configuration is explained at startup; single-source deployments unchanged (@infrastructure)

- **job_id**: infrastructure-only
- **infrastructure_rationale**: Operator-only. It produces no searcher decision on its own. It
  makes the new optional-fallback config safe to operate and proves no regression for existing
  deployments. The slice contains 4 user-visible J-005 stories.
- **Release**: Release 1 | **MoSCoW**: Should | **Priority**: P5

### Problem

`OPENLORE_INDEXER_SOURCE_URL` changes meaning from "the source" to "an optional fallback", and
DIDs are now parsed and resolved one by one. Jeff needs mistakes caught at startup with a
message he can act on. Existing deployments must keep returning the same search results.

### Who

- Jeff Bailey | operator upgrading a running indexer | wants actionable errors and no surprises.

### Solution

Validate repo DIDs and the fallback URL at startup. Refuse with a message that names the
variable and the bad value. Report (but do not refuse) an empty DID list or an absent fallback.
Prove that a deployment whose DIDs all live on the configured source behaves as before.

### Domain Examples

#### 1: Malformed DID

`OPENLORE_INDEXER_REPO_DIDS="did:plc:priyaraman7x2k,priya"` → refuse start with a message
naming `OPENLORE_INDEXER_REPO_DIDS` and `"priya"`, and `health.startup.refused`.

#### 2: Malformed fallback

`OPENLORE_INDEXER_SOURCE_URL="pds.jeffbailey.us"` (no scheme) → refuse start, naming the
variable and the value.

#### 3: No fallback (new normal)

The source URL is unset. Startup reports "no fallback source; unresolvable DIDs will be
skipped" and runs.

#### 4: Single-source deployment

All 14 DIDs live on `https://pds.jeffbailey.us`, which is also the configured source. The
indexed claim set and `openlore search` output for existing data match the pre-upgrade output.
The only difference is the added `[self-attested]` label on rows that were already
self-attested.

### UAT Scenarios (BDD)

#### Scenario: A malformed repo DID is explained at startup

Given OPENLORE_INDEXER_REPO_DIDS contains "did:plc:priyaraman7x2k,priya"
When Jeff starts openlore-indexer ingest
Then the indexer refuses to start
And the message names OPENLORE_INDEXER_REPO_DIDS and the entry "priya"

#### Scenario: A malformed fallback URL is explained at startup

Given OPENLORE_INDEXER_SOURCE_URL is "pds.jeffbailey.us"
When Jeff starts openlore-indexer ingest
Then the indexer refuses to start, naming OPENLORE_INDEXER_SOURCE_URL and its value

#### Scenario: Running without a fallback is reported, not refused

Given OPENLORE_INDEXER_SOURCE_URL is unset
When Jeff starts openlore-indexer ingest
Then Jeff is told that unresolvable authors will be skipped
And the pass runs

#### Scenario: A single-source deployment returns the same results

Given all 14 configured authors live on https://pds.jeffbailey.us, which is also the configured source
When one ingest pass runs after the upgrade
Then the indexed claims and the search results for existing data match those before the upgrade

### Acceptance Criteria

- [ ] AC-004.1 A malformed repo DID refuses start with `health.startup.refused` and a message naming the variable and the entry.
- [ ] AC-004.2 A malformed fallback URL refuses start, naming the variable and the value.
- [ ] AC-004.3 An unset fallback or an empty DID list is reported in plain words and does not refuse start.
- [ ] AC-004.4 For a single-source deployment, the indexed set and search output for existing data are unchanged, apart from the provenance label (US-IPF-005).

### Outcome KPIs

- **Who**: operators upgrading | **Does what**: start a correctly configured indexer the first time | **By how much**: 100% of config errors named at startup (variable + value). 0 search-result diffs on the single-source regression fixture | **Measured by**: startup acceptance tests and the before/after search fixture | **Baseline**: today a bad DID string is accepted silently and fails later.

### Technical Notes

- ADR-024 Earned Trust says the probe refuses when the "only source" is unreachable. With per-DID sources, the probe semantics change (OQ-IPF-1).
- Existing acceptance suites run with a fixture source URL and likely without a resolvable PLC fixture. Under the fallback rule they keep their behavior. DISTILL should confirm.

---

## US-IPF-005: Search says which claims are self-attested

- **job_id**: J-005 (beneficiary: J-009, where Priya's claims are shown honestly)
- **Release**: Release 1 | **MoSCoW**: Must | **Priority**: P3

### Elevator Pitch

- **Before**: `openlore search --object org.openlore.philosophy.reproducible-builds` shows Priya's claim as `[verified]` only. The wire result has no provenance, so it reads as app-signed.
- **After**: the same command shows `github:priyaraman/cargo-pin confidence 0.82 (well-evidenced) [verified] [self-attested] bafy...r7`, and Dmitri's row keeps plain `[verified]`.
- **Decision enabled**: Maria decides how much weight to give a claim, knowing whether its author's repo attested it or an OpenLore app key signed it.

### Problem

ADR-071 §5 requires the search DTO to carry an optional `provenance`, but the wire result does
not have it today. Once US-IPF-001 makes self-attested claims routine in search, Maria would see
them as app-signed. That misstates how they are attested.

### Who

- Maria | searcher weighing claims | wants honest provenance.

### Solution

Search results carry an optional provenance (`app-signed` | `self-attested`), and the CLI
labels self-attested rows. An absent value is read as app-signed, so old indexers still work.

### Domain Examples

#### 1: Mixed results

Priya (self-attested) and Dmitri (app-signed) both match reproducible-builds. Priya's row shows
`[self-attested]` and Dmitri's does not.

#### 2: Old indexer

Maria's CLI queries an indexer that predates this change, so no provenance is on the wire. All
rows render exactly as today, with no label and no error.

#### 3: Contributor search

`openlore search --contributor did:plc:priyaraman7x2k` shows all 3 of her claims with
`[self-attested]`.

### UAT Scenarios (BDD)

#### Scenario: Self-attested claims are labeled in search results

Given Priya's self-attested claim and Dmitri's app-signed claim both match reproducible-builds
When Maria runs openlore search --object org.openlore.philosophy.reproducible-builds
Then Priya's claim shows [self-attested] next to [verified]
And Dmitri's claim shows [verified] without [self-attested]

#### Scenario: Results from an older indexer render as before

Given the indexer Maria queries does not report provenance
When Maria runs the same search
Then every result renders exactly as it did before this change

#### Scenario: Searching by contributor labels every self-attested claim

Given Priya has 3 indexed self-attested claims
When Maria runs openlore search --contributor did:plc:priyaraman7x2k
Then all 3 results show [self-attested]

### Acceptance Criteria

- [ ] AC-005.1 Each search result carries the provenance stored at ingest, and the CLI shows `[self-attested]` for self-attested rows only.
- [ ] AC-005.2 An absent provenance is treated as app-signed, and the output is unchanged for old servers.
- [ ] AC-005.3 Provenance is consistent across the object, subject and contributor dimensions.

### Outcome KPIs

- **Who**: searchers seeing self-attested results | **Does what**: see the correct provenance label | **By how much**: 100% of self-attested rows labeled. 0 app-signed rows mislabeled | **Measured by**: acceptance fixture with mixed provenance | **Baseline**: 0% (field absent on the wire).

### Technical Notes

- `lexicon::SearchResultDto` lacks `provenance`. The index store already reads it (`COALESCE(provenance,'app-signed')`). The `flat_attributed_rows` projection in `run.rs` must carry it.
- This is an additive optional field per ADR-071 §5 and ADR-005 forward-compat. The viewer's network surface, if any, follows the same rule.
