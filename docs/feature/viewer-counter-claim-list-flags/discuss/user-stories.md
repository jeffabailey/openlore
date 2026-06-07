<!-- markdownlint-disable MD024 -->
# User Stories: viewer-counter-claim-list-flags (slice-12)

> Every story traces to **J-003b** (`docs/product/jobs.yaml`). One infra story
> (`infrastructure-only`) + two user-visible stories. Persona: **P-001** ("Maria",
> the node operator, counter-claim-scanner hat). Each AC names the driving port —
> the HTTP route (`GET /claims`) — port-to-port (no direct calls to `viewer-domain`).

---

## US-LF-001: Read-only BATCH counter-presence read capability in the viewer process

`job_id: infrastructure-only`
`infrastructure_rationale:` Adds the read-only `counter_presence_for(target_cids: &[String])`
capability to `StoreReadPort` (+ its `adapter-duckdb` impl) — the batch presence
plumbing US-LF-002/003 consume. Produces no user-visible output on its own (no new
route, no rendered page; it returns a set of countered CIDs), so it enables a user
decision only THROUGH US-LF-002. The slice contains two user-visible stories, so it
has release value.
`@infrastructure` — no Elevator Pitch (BLOCKS the slice only if it were the sole story; it is not).

### Problem

The viewer can today answer "is THIS ONE claim countered?" via the slice-11
`query_counter_claims(target_cid)` per-CID read. But the `/claims` list renders up to
50 rows per page. Asking "is each of these 50 claims countered?" by calling the
per-CID read once per row would be 50 queries per page (N+1) — a performance and
correctness anti-pattern. There is NO batch capability that answers "which of THESE
CIDs are countered?" in one aggregate read.

### Who

- P-001 ("Maria"), the node operator — but this is the plumbing layer; her decision
  is enabled by US-LF-002 which consumes this read.

### Solution

A read-only `StoreReadPort::counter_presence_for(target_cids: &[String]) ->
Result<HashSet<String>, StoreReadError>` (collection type DESIGN's call). ONE
aggregate query over the LOCAL INDEXED `claim_references ∪ peer_claim_references`
tables: `WHERE referenced_cid IN (...) AND ref_type = 'counters'`, `DISTINCT
referenced_cid`. Returns the SET of CIDs from `target_cids` that have ≥1 counter. NO
per-row artifact read (the flag carries no reason text — the reason is the slice-11
thread's job). Anti-merging by construction (a presence set, no JOIN/GROUP-BY that
elides authors). Widens the slice-11 `query_counter_claims` Step-A indexed lookup
from one CID to a CID set.

### Domain Examples

#### 1: Happy Path — Maria's 3-claim page, 1 countered

Maria's `/claims` page 1 has 3 of her own claims with CIDs
`bafyMariaRust`, `bafyMariaDoc`, `bafyMariaSemver`. Tobias has authored a counter
referencing `bafyMariaRust` (a peer counter, in `peer_claim_references`).
`counter_presence_for(["bafyMariaRust","bafyMariaDoc","bafyMariaSemver"])` runs ONE
query and returns `{"bafyMariaRust"}`.

#### 2: Edge Case — none countered

Maria's page has `bafyMariaA`, `bafyMariaB` — neither has any counter.
`counter_presence_for(["bafyMariaA","bafyMariaB"])` runs ONE query and returns the
EMPTY set `{}` (the renderer shows no flags).

#### 3: Boundary — own + peer counters on the same page

Maria's page has `bafyMariaX` (countered by her OWN later counter, in
`claim_references`) and `bafyMariaY` (countered by Rachel's peer counter, in
`peer_claim_references`). `counter_presence_for(["bafyMariaX","bafyMariaY"])` runs
ONE query (UNION ALL across both ref tables) and returns `{"bafyMariaX","bafyMariaY"}`.

### UAT Scenarios (BDD)

> Driving port: this read is exercised THROUGH `GET /claims` by US-LF-002/003. The
> scenarios below assert its observable contract (presence set, single-query,
> empty-when-none, read-only) on the rendered list surface, port-to-port — no
> direct-call test (the contract is the rendered flag set).

### Scenario: The batch presence read flags only genuinely-countered claims on a page

Given Maria's store holds three own claims `bafyMariaRust`, `bafyMariaDoc`, `bafyMariaSemver`
And Tobias has signed a counter referencing `bafyMariaRust`
When Maria opens `GET /claims`
Then the `bafyMariaRust` row carries a "Countered" marker
And the `bafyMariaDoc` and `bafyMariaSemver` rows carry no marker

### Scenario: The presence lookup is one aggregate query regardless of page size (N+1 guard)

Given Maria's store holds a full 50-claim `/claims` page
When Maria opens `GET /claims`
Then exactly ONE counter-presence query runs for the whole page
And the number of presence queries does not grow with the number of rows

### Scenario: A page with no countered claims yields an empty presence set

Given Maria's store holds claims none of which are countered
When Maria opens `GET /claims`
Then no row carries a "Countered" marker
And the page renders identically to slice-06

### Scenario: The presence read is local and renders offline

Given the network is unreachable
And Maria's store holds a countered claim `bafyMariaRust`
When Maria opens `GET /claims`
Then the `bafyMariaRust` row still carries the "Countered" marker
And no network call is made

### Scenario: The presence read leaves the store unchanged (read-only)

Given Maria's store holds a known set of claims and counters
When Maria opens `GET /claims`
Then the store's claim and counter row counts are byte-identical afterward

### Acceptance Criteria

- [ ] `counter_presence_for(&[cid])` returns exactly the subset of input CIDs that have ≥1 `ref_type='counters'` reference in `claim_references ∪ peer_claim_references`.
- [ ] The read runs ONE aggregate query for any input slice (no per-CID query; query count invariant to page size).
- [ ] Returns an empty set when no input CID is countered.
- [ ] Reads the LOCAL indexed ref tables only — no network, no per-row artifact read; renders with the network down.
- [ ] The read mutates nothing (read-only gold: store row-count universe unchanged).

### Outcome KPIs

- **Who**: the viewer process (plumbing for P-001's list view)
- **Does what**: answers "which of these page CIDs are countered?" in one query
- **By how much**: query count for a page is exactly 1, independent of page size (no N+1)
- **Measured by**: gold/`@property` acceptance test asserting single-query invariant
- **Baseline**: capability does not exist (only per-CID `query_counter_claims`)

### Technical Notes

- Widens the slice-11 `query_counter_claims` Step-A indexed UNION-ALL ref lookup from one CID to `referenced_cid IN (...)`, projecting `DISTINCT referenced_cid` (no Step-B artifact read).
- DESIGN owns: exact collection type; DuckDB `IN (...)` param binding (array param vs expanded placeholders); ADR-046 amendment if needed.
- Depends on: the indexed `claim_references` / `peer_claim_references` tables (slice-03 / existing). No new crate (workspace stays 21).

---

## US-LF-002: See a neutral "Countered" flag on each countered `/claims` row

`job_id: J-003b`

### Problem

Maria has signed dozens of claims. Some have drawn counter-claims (her own follow-up
counters, or peers' counters pulled via `peer pull`). Today, the only way she can
find out WHICH of her claims are contested is to open each `/claims/{cid}` detail
page one-by-one and look for the slice-11 thread. Scanning her list, she is blind to
disagreement — she cannot triage where to spend attention.

### Who

- P-001 ("Maria"), the node operator, counter-claim-scanner hat | scanning her own
  `/claims` list in the browser viewer | motivated to know, at a glance, which claims
  drew pushback so she can decide which thread to read first.

### Solution

On the `GET /claims` list, each row whose claim has ≥1 counter shows a neutral
"Countered" marker (the slice-11 `COUNTERED_PRESENCE_FLAG` constant, REUSED
verbatim), rendered as a render-only `<a href="/claims/{cid}">Countered</a>` one-hop
link to that claim's slice-11 thread. Un-countered rows show nothing. The flag is
driven by the US-LF-001 batch presence set projected onto `ClaimRowView`
(`is_countered = presence.contains(&row.cid)`).

### Elevator Pitch

Before: Maria cannot tell which of her signed claims have been countered without opening each `/claims/{cid}` detail page one-by-one.
After: open `http://127.0.0.1:<port>/claims` → each row whose claim has ≥1 counter shows a neutral "Countered" marker linking to that claim's thread; un-countered rows show nothing.
Decision enabled: Maria decides WHICH contested claim to open and read the disagreement on first — triaging her attention from the list instead of blind-opening every claim.

### Domain Examples

#### 1: Happy Path — Tobias counters Maria's Rust claim

Maria signed `subject=github:rust-lang/cargo predicate=embodiesPhilosophy
object=memory-safety confidence=0.90` (CID `bafyMariaRust`). Tobias pulled it and
signed a counter (`references[].type=counters`, target `bafyMariaRust`, reason "cargo
allows `unsafe` in build scripts"). Maria `peer pull`s Tobias. She opens `/claims`:
the `bafyMariaRust` row shows subject/predicate/object/`0.90`/CID AS BEFORE, plus a
"Countered" link. She clicks it → lands on `/claims/bafyMariaRust` → reads Tobias's
slice-11 thread.

#### 2: Edge Case — own counter (self-correction across distinct claims)

Maria signed `bafyMariaDoc` (a documentation-first claim), then later signed her OWN
counter to a DIFFERENT claim of hers, `bafyMariaSemver`, referencing it. On `/claims`,
`bafyMariaSemver` shows the "Countered" link (her own counter is a real counter);
`bafyMariaDoc` shows no marker.

#### 3: Error/Boundary — claim with many counters renders ONE neutral flag

Rachel and Tobias both counter Maria's `bafyMariaTDD` claim. On `/claims`, the
`bafyMariaTDD` row shows a SINGLE neutral "Countered" marker (presence-only) — NOT
"disputed by 2", NOT a count, NOT a verdict. The two authors + their reasons are on
the slice-11 thread the marker links to.

### UAT Scenarios (BDD)

### Scenario: A countered claim's list row shows the neutral Countered marker

Given Maria's store holds her claim `bafyMariaRust` countered by Tobias
When Maria opens `GET /claims`
Then the `bafyMariaRust` row shows the "Countered" marker
And the marker text is exactly "Countered" (never "disputed", "refuted", or "false")

### Scenario: The Countered marker links to that claim's thread

Given Maria's store holds her claim `bafyMariaRust` countered by Tobias
When Maria opens `GET /claims`
Then the "Countered" marker on the `bafyMariaRust` row is a link to `/claims/bafyMariaRust`
And following it shows the slice-11 counter thread for that claim

### Scenario: A claim with multiple counters shows a single presence marker, not a count

Given Maria's claim `bafyMariaTDD` is countered by both Rachel and Tobias
When Maria opens `GET /claims`
Then the `bafyMariaTDD` row shows exactly one "Countered" marker
And the list shows no count, no "disputed by N", and no aggregate verdict

### Scenario: The flag renders identically under htmx fragment and no-JS full page

Given Maria's store holds a countered claim `bafyMariaRust`
When Maria loads `/claims` as a full page (no JS) and as an htmx fragment
Then the `bafyMariaRust` row shows the same "Countered" marker in both shapes

### Acceptance Criteria

- [ ] A row whose claim CID is in the presence set shows the `COUNTERED_PRESENCE_FLAG` ("Countered") marker; the exact text is "Countered".
- [ ] The marker is a render-only `<a href="/claims/{cid}">` one-hop link to that claim's slice-11 thread.
- [ ] A claim with N counters still shows ONE neutral marker (presence-only — no count, no "disputed by N", no verdict).
- [ ] The flag renders identically in the htmx fragment and the no-JS full page (parity — same fragment fn).
- [ ] The marker is never a verdict word ("disputed"/"refuted"/"false") and never a sort/filter control.

### Outcome KPIs

- **Who**: P-001 dogfood operators scanning `/claims`
- **Does what**: navigate from a list-row "Countered" flag to that claim's thread
- **By how much**: operators who author/pull a counter open its thread via the list flag within the same session (leading indicator of KPI-FED-3), vs. only chance drill-in before slice-12
- **Measured by**: opt-in telemetry (list-flag → detail navigation) + dogfood report
- **Baseline**: 0 (no list flag exists; counters are discoverable only by drill-in)

### Technical Notes

- Extends `ClaimRowView` (`is_countered: bool`, projected from the US-LF-001 presence set) + `render_claim_row` (emit the flag when `is_countered`).
- REUSES `COUNTERED_PRESENCE_FLAG` (slice-11) verbatim — no new flag string.
- Depends on: US-LF-001 (the batch presence read). No new route (extends `GET /claims`). No new crate.

---

## US-LF-003: An un-countered row shows no flag; the flag never changes list order, paging, counts, or confidence

`job_id: J-003b`

### Problem

An at-a-glance flag is only trustworthy if it is ADDITIVE. Maria needs assurance that
the "Countered" marker does NOT silently re-order her list (pushing contested claims
to the top), filter it (hiding un-countered ones), re-weight confidence, or change
paging/counts — and that an un-countered claim looks EXACTLY as it did in slice-06
(no badge, no "0 counters" noise). A flag that quietly reorders or re-scores would
pick a triage order FOR her and break the "shown, never applied" contract.

### Who

- P-001 ("Maria"), the node operator, counter-claim-scanner hat | needs the list to
  remain a faithful, un-reordered view | anxious that an automated flag might
  silently transform her data or its order.

### Solution

The flag is purely additive. An un-countered row renders byte-identically to
slice-06 (no marker, no empty-state). The slice-06 ordering (`composed_at DESC, cid`),
page boundaries, total count, and every row's verbatim confidence are UNCHANGED by
the presence of the flag — the presence read is a SEPARATE set lookup that the pure
projection maps onto rows WITHOUT touching `list_claims`' ordering/paging/count.

### Elevator Pitch

Before: Maria worries an at-a-glance flag might silently re-order, filter, or re-weight her claims list — picking a triage order for her, or making un-countered claims look "wrong".
After: open `http://127.0.0.1:<port>/claims` → un-countered rows show NO marker (no badge, no "0 counters"); the list order, page boundaries, total count, and every row's confidence are byte-identical to the pre-flag render.
Decision enabled: Maria trusts the list as a faithful, un-reordered view of her claims — she scans it the same way she always has, now with a neutral countered marker where (and only where) disagreement actually exists.

### Domain Examples

#### 1: Happy Path — un-countered claim renders as slice-06

Maria signed `bafyMariaDoc` (no counters). On `/claims`, the `bafyMariaDoc` row shows
subject/predicate/object/`0.90`/CID with NO marker — byte-identical to slice-06.

#### 2: Edge Case — mixed page, order preserved

Maria's page 1 (newest first) is `bafyMariaSemver` (countered), `bafyMariaDoc`
(un-countered), `bafyMariaRust` (countered) in `composed_at DESC` order. With the flag
ON, the rows appear in the SAME order — `bafyMariaSemver`, `bafyMariaDoc`,
`bafyMariaRust` — with markers only on the first and third. The flag did NOT pull the
countered rows together or to the top.

#### 3: Boundary — confidence untouched

Maria's countered claim `bafyMariaTDD` has `confidence=0.30`. With the "Countered"
marker shown, the confidence cell still renders `0.30` verbatim — the counter did NOT
re-weight or re-score it (shown-never-applied).

### UAT Scenarios (BDD)

### Scenario: An un-countered claim's row shows no marker and no noise

Given Maria's store holds her un-countered claim `bafyMariaDoc`
When Maria opens `GET /claims`
Then the `bafyMariaDoc` row shows no "Countered" marker
And the row shows no "0 counters" or empty-state text — it renders exactly as slice-06

### Scenario: The flag does not change list order, paging, or counts

Given Maria's store holds a mix of countered and un-countered claims spanning two pages
When Maria opens `GET /claims` with the flag feature active
Then the row order is `composed_at DESC, cid` exactly as slice-06
And the page boundaries and the total count are byte-identical to slice-06
And the countered rows are not pulled to the top or grouped together

### Scenario: A flagged claim's confidence is shown verbatim, never re-weighted

Given Maria's countered claim `bafyMariaTDD` has confidence `0.30`
When Maria opens `GET /claims`
Then the `bafyMariaTDD` row shows the "Countered" marker
And its confidence cell renders `0.30` (byte-identical to a no-flag render)

### Scenario: A mixed page flags only the countered rows

Given Maria's page holds `bafyMariaSemver` (countered), `bafyMariaDoc` (un-countered), `bafyMariaRust` (countered)
When Maria opens `GET /claims`
Then `bafyMariaSemver` and `bafyMariaRust` show the "Countered" marker
And `bafyMariaDoc` shows no marker

### Acceptance Criteria

- [ ] An un-countered row renders byte-identically to slice-06 (no marker, no empty-state, no "0 counters").
- [ ] The list ordering (`composed_at DESC, cid`), page boundaries, and total count are byte-identical with and without the flag.
- [ ] The countered rows are NOT re-ordered, grouped, or pulled to the top by the flag.
- [ ] A flagged claim's confidence renders verbatim (byte-identical to a no-flag render — shown-never-applied).
- [ ] On a mixed page, only the genuinely-countered rows carry the marker.

### Outcome KPIs

- **Who**: P-001 operators relying on `/claims` as a faithful list
- **Does what**: scan a list whose order/paging/counts/confidence are unaffected by the flag
- **By how much**: 100% byte-identical list ordering/paging/count/confidence vs slice-06 (zero tolerance — gold test)
- **Measured by**: gold acceptance test comparing flagged vs un-flagged render of the same store
- **Baseline**: slice-06 list behavior (the reference render)

### Technical Notes

- The presence set is mapped onto rows by the PURE projection AFTER `list_claims` produces the page — `list_claims`' SQL (ordering/paging/count) is UNTOUCHED.
- Shown-never-applied (I-LF-2) is the load-bearing invariant; pinned by a gold test asserting confidence + order byte-identity.
- Depends on: US-LF-001, US-LF-002. No new route, no new crate.
