<!-- markdownlint-disable MD024 -->
# User Stories: htmx-scraper-viewer (slice-06)

> **DELTA** on slices 01/02/03. Read-only htmx viewer for the **node operator** on
> **localhost**. Every story traces to **Job 1** (store inspection, north star) or
> **Job 2** (live-scrape browsing). `job_id` references `jtbd-job-stories.md` in this
> feature directory (Job 1 / Job 2). Signing stays in the CLI (I-SCR-1).

## System Constraints (cross-cutting — apply to every story)

- **Read-only**: no story introduces a write or sign path (NFR-VIEW-1, I-VIEW-1).
- **No key in web process** (NFR-VIEW-2, I-VIEW-2 / I-SCR-1).
- **Localhost only** (NFR-VIEW-3, I-VIEW-4).
- **Local-first**: Job 1 views work offline (NFR-VIEW-4, I-VIEW-6 / slice-01 KPI-5).
- **Same store as CLI**: no second store/schema (BR-VIEW-4).
- **Provenance honesty**: derived-from only on live candidates (BR-VIEW-3, WD-62).
- Tech choices deferred to DESIGN (OD-VIEW-1..7).

---

## US-VIEW-001: See my store in the browser (Walking Skeleton)

- **job_id**: Job 1 (see-what-is-in-my-store)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1

### Elevator Pitch

- **Before**: Maria can only learn what her node holds by opening a DuckDB shell and hand-writing SQL against `claims`.
- **After**: Maria runs `openlore viewer` and opens `http://127.0.0.1:8788/claims` in her browser to see her signed claims rendered as an HTML list — zero SQL.
- **Decision enabled**: Maria can confirm at a glance whether her node's persisted claims match what she believes she signed, and decide whether anything is missing or wrong.

### Problem

Maria Santos is a node operator who has been signing claims for weeks. She is blind to
her own node's persisted contents without writing raw SQL. To answer "what does my node
hold?" she must open a DuckDB shell, know the schema, and hand-write `SELECT` queries —
so she rarely checks, and her uncertainty about node state grows.

### Who

- Node operator | on localhost, on her own machine | motivated to trust that her node's state matches her mental model.

### Solution

A read-only local web process (`openlore viewer`) that binds a localhost address, opens
the same local DuckDB store the CLI uses, loads **no** signing key, and serves
`GET /claims` rendering the `claims` rows as one HTML list page viewable in a browser.
This is the thinnest end-to-end thread: HTTP in → DuckDB query → HTML out, read-only,
offline.

### Domain Examples

#### 1: Happy path
Maria runs `openlore viewer`; it reports listening on `http://127.0.0.1:8788`. She opens
`/claims` and sees her 312 claims, including `("rust-lang/rust","is-maintained-by","The
Rust Project")` at confidence `0.90` with CID `bafyrei...1`. She typed no SQL.

#### 2: Edge — empty store
A fresh operator, Tom, has signed nothing. He opens `/claims` and sees "You have not
signed any claims yet — claims you sign with the CLI will appear here," not a blank page.

#### 3: Error — store unreadable
Another OpenLore process holds `~/.openlore/store.duckdb`. Maria opens `/claims` and sees
"Could not open your store at ~/.openlore/store.duckdb. Is another process using it?" —
no raw stack trace.

### UAT Scenarios (BDD)

#### Scenario: Operator sees their persisted claims in a browser with zero SQL
Given Maria has signed 312 claims including ("rust-lang/rust","is-maintained-by","The Rust Project") at confidence 0.90
When she runs `openlore viewer` and opens the My Claims page in her browser
Then she sees that claim as a row with subject, predicate, object, confidence 0.90, and its CID
And she did not write any SQL

#### Scenario: Viewer starts read-only on localhost with no signing key
Given Maria has a local store
When she runs `openlore viewer`
Then it reports a localhost listen URL and states the view is read-only
And no signing key is loaded into the process

#### Scenario: Empty store guides a first-run operator
Given Tom has signed no claims
When he opens the My Claims page
Then he sees guidance that signed claims appear here and are created via the CLI

#### Scenario: Unreadable store shows a helpful error
Given the store file is locked by another process
When Maria opens the My Claims page
Then she sees a plain-language message naming the path and asking if another process uses it
And no raw stack trace is shown

### Acceptance Criteria

- [ ] `openlore viewer` binds a loopback address and prints its URL and a read-only notice.
- [ ] The process loads no signing key (verified: no key read path reachable).
- [ ] `GET /claims` renders persisted `claims` rows (subject, predicate, object, confidence numeric, cid).
- [ ] Confidence renders as the stored numeric (0.90), not reformatted.
- [ ] Empty store shows guided message pointing to the CLI.
- [ ] Unreadable store shows plain-language error with path and next step, no stack trace.
- [ ] The page renders with no network access (offline).

### Outcome KPIs

- **Who**: node operator | **Does what**: views persisted claims in a browser | **By how much**: in < 10 s from cold start, zero SQL | **Measured by**: KPI-VIEW-1 (timing) + KPI-VIEW-2 (route audit) | **Baseline**: today requires opening a DuckDB shell + writing SQL (effectively never done routinely).

### Technical Notes

- Read against existing `adapter-duckdb`; same store as CLI (BR-VIEW-4).
- Rendering/HTTP approach is OD-VIEW-1/OD-VIEW-2; read-only connection seam is OD-VIEW-6.
- Inherits I-SCR-1 (no key, no signing), slice-01 KPI-5 (offline).
- Depends on: `adapter-duckdb` (exists), `claims` table (slice-01, exists).

---

## US-VIEW-002: Inspect one claim's full evidence

- **job_id**: Job 1 (see-what-is-in-my-store)
- **Release**: R1 | **MoSCoW**: Must | **Priority**: P2

### Elevator Pitch

- **Before**: The list view shows summary fields, but Maria cannot see the evidence backing a specific claim without SQL.
- **After**: Maria clicks a claim (or opens `/claims/{cid}`) and sees its full detail including every evidence URL.
- **Decision enabled**: Maria can verify that a claim carries the evidence she intended, and decide whether it is well-supported.

### Problem

Maria is a node operator who needs to verify the evidence behind a specific claim. The
list view summarizes; she cannot confirm a claim's full evidence array without dropping
back to SQL.

### Who

- Node operator | reviewing a specific claim on localhost | motivated to confirm a claim is properly evidenced.

### Solution

A read-only `GET /claims/{cid}` detail page rendering the full persisted claim — all
fields plus the complete evidence[] array — addressed by CID, with a link back to the list.

### Domain Examples

#### 1: Happy path
Maria opens `/claims/bafyrei...1` and sees subject `rust-lang/rust`, confidence `0.90`,
author `did:plc:maria...`, composed `2026-04-18T09:12:03Z`, and both evidence URLs.

#### 2: Edge — claim with no evidence
Maria opens a claim she signed without evidence; the page shows "no evidence attached"
rather than a blank evidence section.

#### 3: Error — CID not found
Maria mistypes a CID; the page shows "No claim with that identifier in your store" and a
link back to My Claims.

### UAT Scenarios (BDD)

#### Scenario: Operator views the full evidence behind one claim
Given Maria's claim with CID bafyrei...1 has two evidence URLs
When she opens its detail page
Then she sees all claim fields and both evidence URLs

#### Scenario: Claim with no evidence is shown clearly
Given Maria has a claim signed without evidence
When she opens its detail page
Then she sees "no evidence attached" rather than a blank section

#### Scenario: Unknown CID guides the operator back
Given no claim with CID bafyrei...zzz exists in the store
When Maria opens that detail page
Then she sees "No claim with that identifier in your store" and a link back to the list

### Acceptance Criteria

- [ ] `GET /claims/{cid}` renders the full claim incl. complete evidence[].
- [ ] A claim with empty evidence shows an explicit "no evidence attached" state.
- [ ] An unknown CID shows a guided not-found message with a back link.
- [ ] The detail page renders offline.

### Outcome KPIs

- **Who**: node operator | **Does what**: confirms a claim's evidence | **By how much**: 100% of a claim's stored evidence URLs visible on one page | **Measured by**: KPI-VIEW-1 (legibility) | **Baseline**: evidence only inspectable via SQL today.

### Technical Notes

- Column→field mapping is OD-VIEW-3. Depends on US-VIEW-001.

---

## US-VIEW-003: Distinguish federated peer claims from my own

- **job_id**: Job 1 (see-what-is-in-my-store)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P3

### Elevator Pitch

- **Before**: Maria has no browser view of the `peer_claims` she has federated, and no easy way to tell federated claims from her own.
- **After**: Maria opens `/peer-claims` and sees federated claims with their peer origin, on a surface distinct from her own claims.
- **Decision enabled**: Maria can tell what her node has pulled from peers versus what she authored, and decide whether her federation set looks right.

### Problem

Maria is a node operator whose store holds both her signed claims and 1,840 federated
`peer_claims` from 4 peers. Without a distinct view she cannot tell, in the browser, what
came from peers versus what she authored — risking confusion about authorship.

### Who

- Node operator on a federated node | localhost | motivated to keep "mine vs federated" unambiguous.

### Solution

A read-only `GET /peer-claims` view rendering `peer_claims` rows with peer provenance
(peer_origin), presented on a surface clearly separate from own claims.

### Domain Examples

#### 1: Happy path
Maria opens `/peer-claims` and sees `("axum/axum","has-license","MIT")` at 0.88 from
`peer-A`, distinct from her own claims tab.

#### 2: Edge — no peers yet
A node that has federated nothing shows "No federated claims yet" guidance.

#### 3: Boundary — missing peer origin
A peer_claim with absent origin still renders, labeled origin "unknown," not dropped.

### UAT Scenarios (BDD)

#### Scenario: Operator distinguishes federated peer claims from their own
Given Maria has federated 1,840 peer claims from 4 peers
When she opens the Peer Claims view
Then she sees federated claims with their peer origin, separate from her own claims

#### Scenario: No federated claims yet is guided
Given Maria has federated no peer claims
When she opens the Peer Claims view
Then she sees "No federated claims yet" guidance

#### Scenario: Peer claim with unknown origin still renders
Given a federated peer claim has no recorded origin
When Maria opens the Peer Claims view
Then that claim still renders with origin shown as "unknown"

### Acceptance Criteria

- [ ] `GET /peer-claims` renders `peer_claims` rows with peer_origin.
- [ ] Peer claims are visually/structurally distinct from own claims.
- [ ] Empty peer set shows guided message.
- [ ] Missing peer_origin renders as "unknown" rather than dropping the row.
- [ ] The view renders offline.

### Outcome KPIs

- **Who**: federated node operator | **Does what**: inspects + distinguishes federated vs own claims | **By how much**: 100% of peer rows show origin and are separable from own | **Measured by**: KPI-VIEW-3 | **Baseline**: no browser view of peer_claims today.

### Technical Notes

- Depends on US-VIEW-001 and `peer_claims` (slice-03). Mapping is OD-VIEW-3.

---

## US-VIEW-004: Navigate a large store with pagination

- **job_id**: Job 1 (see-what-is-in-my-store)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P3

### Elevator Pitch

- **Before**: Rendering all of a 1,840+ row store on one page is slow and unscannable.
- **After**: Maria pages through her claims and peer claims in fixed-size pages with a position indicator.
- **Decision enabled**: Maria can browse a real-sized store without the page hanging, and locate a region of her claims.

### Problem

Maria is a node operator with a large store (312 own + 1,840 peer claims). An unbounded
single-page render is slow and unscannable, undermining the "see at a glance" outcome at
real scale.

### Who

- Node operator with a real-sized store | localhost | motivated to browse without performance pain.

### Solution

Read-only pagination on the claims and peer-claims list views: fixed page size, position
indicator (e.g. "1–50 of 312"), and next/previous navigation. Strategy is OD-VIEW-4.

### Domain Examples

#### 1: Happy path
Maria's 312 claims render 50 per page; she clicks Next and sees "51–100 of 312."

#### 2: Boundary — last page
On the final page she sees "301–312 of 312" and Next is disabled/absent.

#### 3: Edge — single page
A 12-claim store renders one page with no pagination controls shown.

### UAT Scenarios (BDD)

#### Scenario: Operator pages through a large store
Given Maria has 312 signed claims and a page size of 50
When she opens My Claims and clicks Next
Then she sees claims 51–100 of 312 with a position indicator

#### Scenario: Last page is bounded correctly
Given Maria is on the last page of 312 claims
Then she sees 301–312 of 312 and no further Next action

#### Scenario: Small store needs no pagination controls
Given Maria has 12 claims and a page size of 50
When she opens My Claims
Then all 12 render and no pagination controls are shown

### Acceptance Criteria

- [ ] List views render at most one page-size of rows per request.
- [ ] A position indicator ("X–Y of N") is shown.
- [ ] Next/previous navigate correctly; bounds are respected at first/last page.
- [ ] Stores smaller than one page show no pagination controls.
- [ ] Paginated views render offline.

### Outcome KPIs

- **Who**: node operator with a large store | **Does what**: browses the full store | **By how much**: first page renders < 10 s regardless of store size | **Measured by**: KPI-VIEW-1 (at scale) | **Baseline**: unbounded render not viable.

### Technical Notes

- Pagination strategy/page size is OD-VIEW-4. Depends on US-VIEW-002 (rendering).

---

## US-VIEW-005: Browse live scrape proposals before signing in the CLI

- **job_id**: Job 2 (browse-scrape-proposals)
- **Release**: R3 | **MoSCoW**: Could | **Priority**: P4

### Elevator Pitch

- **Before**: Deciding which scrape candidates matter is awkward in CLI batch text.
- **After**: Maria enters a target on `/scrape` and sees the proposed candidate claims as a scannable HTML list with their derived-from provenance — nothing signed or saved.
- **Decision enabled**: Maria can visually triage which candidates are worth signing, then run the CLI sign command for the ones she chooses.

### Problem

Maria is a node operator who, before signing scraped claims, wants to weigh which
candidates matter. In the CLI the proposals arrive as a wall of batch text that is awkward
to scan and compare — especially each candidate's derived-from provenance.

### Who

- Node operator triaging scrape candidates | localhost, network available | motivated to review before signing in the CLI.

### Solution

A read-only `/scrape` view: a target form that runs the slice-02 propose step live (no
persistence), and renders the resulting in-memory `CandidateClaim` values as HTML rows
with display-only derived-from. **No sign control**; the page directs the operator to the
CLI to sign. Requires network.

### Domain Examples

#### 1: Happy path
Maria enters `tokio-rs/tokio`; 7 candidates render, e.g. `("tokio-rs/tokio","has-license",
"MIT")` at 0.95 with derived-from "LICENSE @ HEAD". Nothing is saved; she later signs two
via `openlore scrape github tokio-rs/tokio --sign`.

#### 2: Edge — no candidates derived
Maria enters `some-org/empty-repo`; the page shows "No candidate claims could be derived"
with a suggestion to check license/manifest data.

#### 3: Error — network unavailable
Offline, Maria submits `tokio-rs/tokio`; the page reports GitHub could not be reached and
notes her store view still works offline.

### UAT Scenarios (BDD)

#### Scenario: Operator browses live proposals without signing anything
Given a live scrape of "tokio-rs/tokio" would propose 7 candidate claims
When Maria submits that target on the Live Scrape view
Then she sees 7 candidates with subject, predicate, object, confidence, and derived-from
And the page states none are signed or saved
And no candidate is persisted
And she is directed to the CLI to sign any candidate

#### Scenario: Provenance appears only on live proposals, never on persisted claims
Given Maria is viewing live proposals for "tokio-rs/tokio" showing derived-from
When she opens her persisted My Claims view
Then no persisted claim shows derived-from

#### Scenario: Target yielding no candidates guides the operator
Given a live scrape of "some-org/empty-repo" derives no candidates
When Maria submits that target
Then she sees "No candidate claims could be derived" with a suggested alternative

#### Scenario: Network failure clarifies the store view still works offline
Given Maria cannot reach GitHub
When she submits "tokio-rs/tokio" on the Live Scrape view
Then she sees that GitHub could not be reached
And she is told her store view still works offline

### Acceptance Criteria

- [ ] `/scrape` form accepts a target and runs the live propose step (no persistence).
- [ ] Candidates render with subject, predicate, object, confidence, and derived-from.
- [ ] The page states nothing is signed/saved and renders **no** sign control.
- [ ] No candidate is written to the store; refresh re-harvests.
- [ ] derived-from never appears on persisted-claim views.
- [ ] Zero-candidate and network-failure states show guided messages (network failure notes offline store view works).

### Outcome KPIs

- **Who**: node operator | **Does what**: reviews proposals in browser, then signs in CLI | **By how much**: candidate triage shifts from CLI batch text to a scannable browser list | **Measured by**: KPI-VIEW-4 | **Baseline**: only CLI batch-text review exists today.

### Technical Notes

- Requires network (NFR-VIEW-7). Calls slice-02 propose step; **no** sign path (I-SCR-1).
- Whether this shares the store-view binary or is separate is OD-VIEW-5.
- Depends on US-VIEW-001 (HTTP/render foundation) and slice-02 propose pipeline.
