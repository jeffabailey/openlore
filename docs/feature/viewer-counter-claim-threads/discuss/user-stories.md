<!-- markdownlint-disable MD024 -->
# User Stories: viewer-counter-claim-threads (slice-11)

> Every story traces to **J-003b** (`docs/product/jobs.yaml`) except US-CT-001
> (`infrastructure-only` with rationale). Acceptance criteria are written
> port-to-port: each AC names the DRIVING PORT — the HTTP route `GET /claims/{cid}`
> on the real `openlore ui` viewer — and asserts an observable HTML outcome.
> Persona: **P-001 "Maria"** (the node operator, counter-claim-reader hat).
> Real data throughout: `did:plc:maria-test` (you), `did:plc:rachel-test` (peer),
> `did:plc:tobias-test` (peer), claim CIDs `bafy...n4ka` (target), `bafy...new`,
> `bafy...t0bi` (counters).

## System Constraints

Cross-cutting constraints binding on ALL stories below (the slice-11 commitments,
see `feature-delta.md` for full statements):

- **Read-only** (I-CT-1 / KPI-VIEW-2): no write/sign/counter control on any
  surface; `query_counter_claims` is read-only on a no-mutation trait; no signing
  key in the viewer process. Authoring stays EXCLUSIVELY in the CLI.
- **Shown, never applied** (I-CT-2 / ADR-015): the countered claim renders VERBATIM
  with its ORIGINAL confidence; counters never filter/merge/re-weight/re-rank it.
- **Attribution without merging** (I-CT-3): every counter shows its own author DID +
  its own CID + its verbatim reason; two counters = two thread items, never a merged
  aggregate.
- **Verbatim confidence** (I-CT-4 / KPI-4): any shown confidence renders `0.90`,
  never `0.9`/`90%`.
- **LOCAL-only / offline** (I-CT-5 / KPI-5): reads `claims ∪ peer_claims`; NO
  network on this route; renders with the network down; only the vendored local
  `/static/htmx.min.js` (no CDN).
- **Progressive enhancement** (I-CT-6): `HX-Request` → fragment; no-JS / bookmark /
  direct-URL → full page = chrome + the SAME fragment (structural parity).
- **No new crates** (I-CT-7): extend `viewer-domain` + `adapter-http-viewer` +
  `adapter-duckdb` + `ports` + `cli` + `xtask`; workspace stays 21; functional (ADR-007).

---

## US-CT-001: Read-only counter-claim thread READ capability in the viewer process

> **job_id**: `infrastructure-only`
> **infrastructure_rationale**: Adds the read-only `query_counter_claims(target_cid)`
> capability to `StoreReadPort` (+ its `adapter-duckdb` read impl) and the pure
> `viewer-domain` thread view-model — the plumbing US-CT-002/003 consume. It produces
> no user-visible output on its own (no new route, no rendered page); it enables a
> user decision only THROUGH US-CT-002. The slice contains two non-infra user-visible
> stories (US-CT-002/003), so the slice retains release value (Dimension-0 slice-level
> check passes). This is the only `@infrastructure` story in the slice.

### Problem

Maria is a senior engineer running `openlore ui` on her own node. The viewer can read
her claims, her peer claims, scores, and graph surveys — but it has NO way to read the
COUNTER-CLAIMS that target a given claim. The `StoreReadPort` exposes
`get_claim`, `list_claims`, `list_peer_claims`, `query_contributor_scoring_feed`,
`query_project_survey`, `query_philosophy_survey` — none of which surface, for a CID,
the signed claims that COUNTER it. Without a read capability, the disagreement around
a claim is invisible to the browser surface.

### Who

- P-001 Maria (node operator) | reading her local store via `openlore ui` | wants
  the viewer to be able to SURFACE disagreement, but only as a READ (never a write).

### Solution

Add `StoreReadPort::query_counter_claims(target_cid: &str) -> Result<Vec<CounterClaimRow>, StoreReadError>`:
a read-only SELECT over `claims ∪ peer_claims` (UNION ALL, explicit `author_did` +
`cid`, no merging JOIN/GROUP BY/AVG) returning every signed claim that carries a
`references[]` entry of type `counters` whose target `cid == target_cid`. Each row
carries `author_did`, the counter's own `cid`, its verbatim `reason`, its `confidence`
(DOUBLE), `composed_at` (ordering only), and `origin` (`PeerOrigin`). Returns an
EMPTY vec when nothing counters the target. Plus a pure `viewer-domain` `CounterThread`
ADT projecting these rows. NO mutation method is added (the trait stays no-write);
the `adapter-duckdb` impl shares the CLI's connection (BR-VIEW-4).

### Domain Examples

#### 1: Happy Path — Rachel's claim has one counter
`query_counter_claims("bafy...n4ka")` over a store where Maria's own claim `bafy...new`
counters Rachel's `bafy...n4ka` returns one `CounterClaimRow {author_did:
"did:plc:maria-test", cid: "bafy...new", reason: "Cargo's dependency pinning is
opt-in, not philosophical; pinning is a tool, not a value.", confidence: 0.72,
origin: Own}`.

#### 2: Edge Case — two counters from two authors
`query_counter_claims("bafy...n4ka")` over a store where both Maria (`bafy...new`,
origin Own) AND peer Tobias (`bafy...t0bi`, origin `Known{author_did:
"did:plc:tobias-test", fetched_from_pds: "https://pds.example.com"}`) counter Rachel's
claim returns TWO rows — never one merged "disputed by 2" row.

#### 3: Error/Boundary — un-countered claim
`query_counter_claims("bafy...solo")` over a store where nothing references
`bafy...solo` as a `counters` target returns an EMPTY vec (`Ok(vec![])`), never an
error — the renderer (US-CT-003) then shows the claim alone.

### UAT Scenarios (BDD)

#### Scenario: The viewer can read the counters targeting a claim
Given Maria's local store contains her claim `bafy...new` which counters Rachel's
`bafy...n4ka` with reason "Cargo's dependency pinning is opt-in, not philosophical;
pinning is a tool, not a value." and confidence 0.72
When the viewer calls `StoreReadPort::query_counter_claims("bafy...n4ka")` (the driving
read port)
Then the result is one `CounterClaimRow` whose `author_did` is "did:plc:maria-test",
`cid` is "bafy...new", `reason` is the verbatim text, and `confidence` is 0.72

#### Scenario: Counters from two authors are returned as two attributed rows
Given Maria's local store contains her own counter `bafy...new` AND peer Tobias's
counter `bafy...t0bi`, both targeting Rachel's `bafy...n4ka`
When the viewer calls `StoreReadPort::query_counter_claims("bafy...n4ka")`
Then the result is two `CounterClaimRow`s, one attributed to "did:plc:maria-test"
(origin Own) and one to "did:plc:tobias-test" (origin Known with its PDS)
And neither row is a merged or averaged aggregate

#### Scenario: A claim with no counters returns an empty result, not an error
Given Maria's local store contains the claim `bafy...solo` and NO claim references it
as a `counters` target
When the viewer calls `StoreReadPort::query_counter_claims("bafy...solo")`
Then the result is `Ok` with an empty list of counter rows

#### Scenario: The read capability adds no write/sign surface
Given the `openlore ui` viewer process is composed with the read-only store port
When the route table and the viewer process are audited (xtask check-arch viewer
capability rule + key-access audit)
Then `query_counter_claims` is a read-only method on a trait with NO mutation method
And the viewer process reads no signing key

### Acceptance Criteria

- [ ] `StoreReadPort::query_counter_claims(target_cid)` exists, is read-only, and
  returns `Vec<CounterClaimRow>` (own + peer counters via UNION ALL, explicit
  `author_did` + `cid`, no merging JOIN/GROUP BY/AVG).
- [ ] For a target with counters, each returned row carries `author_did`, the
  counter's own `cid`, its verbatim `reason`, its `confidence` DOUBLE, and its
  `origin` (`PeerOrigin`).
- [ ] For a target with no counters, the method returns `Ok(empty vec)`, never an error.
- [ ] No mutation method is added to `StoreReadPort`; xtask check-arch viewer
  capability rule and key-access audit pass (read-only, no key).
- [ ] Workspace stays 21 members (no new crate).

### Outcome KPIs

- **Who**: the viewer process (infrastructure enabling P-001's reading) | **Does
  what**: can read the counters targeting any CID from the LOCAL store |
  **By how much**: 100% of counters in `claims ∪ peer_claims` for a target CID are
  returned, attributed, with zero merged rows | **Measured by**: acceptance test
  over the real read port + the anti-merging gold | **Baseline**: 0 (no counter-read
  capability existed before slice-11).

### Technical Notes

- Reuses the slice-03 counter-claim model: a counter is an ordinary signed claim with
  a `references[]` entry of `type == counters` (ADR-015 / ADR-008). DESIGN owns the
  exact SQL to match `references[].cid == target_cid AND references[].type == counters`.
- Anti-merging UNION-ALL pattern mirrors slice-10 `query_project_survey` / slice-09
  `query_contributor_scoring_feed`. Reuses `PeerOrigin` (Own vs Known/Unknown).
- Depends on: the slice-03 `peer_claims` table + the `claims` table (both present);
  the `references[]` storage shape (present). No external dependency.

---

## US-CT-002: See the counter-claim thread beneath a countered claim

> **job_id**: `J-003b`

### Elevator Pitch

- **Before**: Maria opens a claim in the viewer and sees it ALONE — she has no idea
  anyone disagreed with it, who, or why; disagreement is invisible on her local
  browser surface.
- **After**: Maria opens `http://127.0.0.1:<port>/claims/bafy...n4ka` and, BENEATH the
  original claim (rendered verbatim with its original confidence 0.91), sees a
  "Counter-claims" thread — each counter showing its author DID, its own CID, and the
  full verbatim reason — so she can read BOTH sides at a glance.
- **Decision enabled**: Maria decides whether to trust, cite, or counter the claim
  herself (via the CLI), now that she can SEE the existing disagreement and read the
  countering reasoning in full.

### Problem

Maria is a senior engineer who reads other developers' signed claims to make tool
decisions. When she opens a claim in the viewer, she sees only the claim. If Rachel's
claim "cargo embodies dependency-pinning, confidence 0.91" has already been countered
— by Maria herself or by a peer — Maria cannot see that on the local browser surface.
She finds it disorienting to evaluate a claim she cannot tell is disputed, and tedious
to drop back to the CLI `graph query --federated` to discover the counter-relationship.

### Who

- P-001 Maria (node operator, counter-claim-reader hat) | drilling into a claim on
  the local `openlore ui` viewer | wants to read the disagreement around a claim
  WITHOUT the disagreement changing the claim.

### Solution

Extend `GET /claims/{cid}` so that, after the existing claim fields + evidence
section, the detail page/fragment renders a "Counter-claims" thread: for each counter
returned by `query_counter_claims(cid)`, one thread item showing its author DID, its
own CID (linked to `/claims/{counter_cid}`), and its verbatim reason. The original
claim is rendered EXACTLY as today (verbatim confidence, untouched). Counters are
ordered deterministically (e.g. `composed_at` then CID tiebreak — DESIGN owns the
exact order). The thread is the SAME fragment in both the htmx and no-JS shapes.

### Domain Examples

#### 1: Happy Path — one counter from a peer
Maria opens `/claims/bafy...n4ka` (Rachel's claim, confidence 0.91). Beneath it, a
"Counter-claims" section shows one item: author `did:plc:maria-test (you)`, CID
`bafy...new`, reason "Cargo's dependency pinning is opt-in, not philosophical; pinning
is a tool, not a value." Rachel's claim still shows confidence 0.91, unchanged.

#### 2: Edge Case — two counters from two authors
Maria opens `/claims/bafy...n4ka` where both her own `bafy...new` and peer Tobias's
`bafy...t0bi` (author `did:plc:tobias-test`) counter it. The thread shows TWO items,
each with its own author DID + CID + verbatim reason. No "disputed by 2" merged badge.

#### 3: Error/Boundary — a counter whose reason came from a non-OpenLore client (blank)
Maria opens `/claims/bafy...x7ts` countered by a peer record that satisfies the
Lexicon but carries an empty `reason` (the ADR-015 asymmetry: optional at wire,
required only at the OpenLore verb). The thread item shows the author DID + CID and an
explicit "no reason provided" state (never a crash, never a blank line that looks like
a render bug).

### UAT Scenarios (BDD)

#### Scenario: A countered claim shows the counter thread beneath it
Given Maria's store contains Rachel's claim `bafy...n4ka` (confidence 0.91) countered
by Maria's own `bafy...new` with reason "Cargo's dependency pinning is opt-in, not
philosophical; pinning is a tool, not a value."
When Maria requests `GET /claims/bafy...n4ka` from the `openlore ui` viewer (full page)
Then the response renders Rachel's claim fields with confidence "0.91"
And beneath the claim a "Counter-claims" section renders one item showing author
"did:plc:maria-test", CID "bafy...new", and the verbatim reason text
And the original claim's confidence is still "0.91" (unchanged by the counter)

#### Scenario: Two counters render as two attributed thread items, never merged
Given Rachel's claim `bafy...n4ka` is countered by Maria's `bafy...new` AND peer
Tobias's `bafy...t0bi` (author "did:plc:tobias-test")
When Maria requests `GET /claims/bafy...n4ka` from the viewer
Then the "Counter-claims" section renders exactly two items
And one item shows author "did:plc:maria-test" with CID "bafy...new"
And the other shows author "did:plc:tobias-test" with CID "bafy...t0bi"
And no single row aggregates the two counters into a "disputed by 2" or consensus row

#### Scenario: Each counter's CID links to that counter's own detail
Given Rachel's claim `bafy...n4ka` is countered by Maria's `bafy...new`
When Maria requests `GET /claims/bafy...n4ka` from the viewer
Then the counter thread item for "bafy...new" includes a link to `/claims/bafy...new`
So Maria can drill into the counter's own detail (reusing the existing detail route)

#### Scenario: A counter with no reason renders an explicit empty-reason state
Given Rachel's claim `bafy...x7ts` is countered by a peer record whose `reason` is
empty (a non-OpenLore client; ADR-015 wire-optional asymmetry)
When Maria requests `GET /claims/bafy...x7ts` from the viewer
Then the counter thread item shows the author DID and CID
And it shows an explicit "no reason provided" state rather than a blank line or a crash

#### Scenario: The counter thread renders identically under htmx and no-JS
Given Rachel's claim `bafy...n4ka` is countered by Maria's `bafy...new`
When Maria requests `GET /claims/bafy...n4ka` WITH `HX-Request: true` (the htmx swap)
And Maria requests `GET /claims/bafy...n4ka` WITHOUT `HX-Request` (no-JS full page)
Then the htmx response returns the `#claim-detail` fragment containing the claim + the
counter thread
And the no-JS response returns the full page whose `#claim-detail` region embeds the
SAME fragment (structural parity)

#### Scenario: The counter thread renders with the network disabled
Given Rachel's claim `bafy...n4ka` is countered by peer Tobias's `bafy...t0bi`
(already pulled into `peer_claims`)
And the network is disabled
When Maria requests `GET /claims/bafy...n4ka` from the viewer
Then the counter thread renders Tobias's counter fully (author DID + CID + reason)
And the page references only the local `/static/htmx.min.js` (no CDN, no network call)

### Acceptance Criteria

- [ ] `GET /claims/{cid}` renders, beneath the existing claim fields + evidence, a
  "Counter-claims" section listing every counter from `query_counter_claims(cid)`.
- [ ] The original claim is rendered VERBATIM with its ORIGINAL confidence — the
  presence of counters changes none of the claim's fields (shown, never applied).
- [ ] Each counter thread item shows its `author_did`, its own `cid` (linked to
  `/claims/{counter_cid}`), and its verbatim `reason`.
- [ ] Two counters render as two distinct attributed items; no merged/aggregate
  "disputed" row exists anywhere in the response (anti-merging gold).
- [ ] A counter with an empty reason renders an explicit "no reason provided" state,
  never a blank line or a crash.
- [ ] The thread is the SAME fragment under `HX-Request` (fragment) and no-JS (full
  page embeds it) — structural parity.
- [ ] The thread renders with the network disabled, referencing only the vendored
  local htmx asset (offline / no-CDN).

### Outcome KPIs

- **Who**: P-001 Maria (counter-claim-reader hat) | **Does what**: reads the full
  disagreement around a claim (who countered, with what CID, and the verbatim reason)
  from the local browser surface | **By how much**: 100% of counters in the local
  store for a CID are shown, each attributed, with zero merged rows; original
  confidence unchanged | **Measured by**: acceptance test over the real `GET
  /claims/{cid}` route + the anti-merging + shown-never-applied gold | **Baseline**: 0
  (no local browser counter-thread existed before slice-11). Leading indicator OF the
  inherited KPI-FED-3 (counter-claim publication rate) — see `feature-delta.md`.

### Technical Notes

- Extends `render_claim_detail` / `render_claim_detail_fragment` (viewer-domain) and
  `claim_detail_page` (adapter-http-viewer). Reuses `Shape::from_request` (slice-07),
  `render_confidence` (verbatim), `PeerOrigin`, the `(you)` vs peer annotation pattern
  (slice-06). DESIGN owns the counter ordering + the exact thread markup.
- A counter's CID link reuses the existing `/claims/{counter_cid}` route — no new
  nested-thread render (thread is one level deep; deep recursion is a non-goal).
- Depends on US-CT-001 (the read method). No external dependency.

---

## US-CT-003: An un-countered claim renders cleanly; a countered claim is flagged

> **job_id**: `J-003b`

### Elevator Pitch

- **Before**: There is no signal on the claim detail telling Maria whether a claim is
  disputed; if a counter-thread always rendered (even when empty), every un-countered
  claim would carry pointless "no disagreement" noise.
- **After**: When Maria opens an UN-countered claim (`/claims/bafy...solo`), the detail
  looks EXACTLY as it does today — no empty thread, no noise. When she opens a
  COUNTERED claim, it is clearly FLAGGED as disputed (a "Countered" marker near the
  claim) so she knows at a glance that disagreement exists before she reads it.
- **Decision enabled**: Maria can trust an un-countered claim's clean render AND
  immediately recognize a disputed claim as one to read with both sides in mind —
  without the flag ever changing the claim's confidence or content.

### Problem

Maria reads many claims. Most are not countered. If the viewer rendered an empty
"Counter-claims (0)" section on every claim, it would add noise to the common case and
dilute the signal when disagreement actually exists. Conversely, when a claim IS
countered, Maria wants an at-a-glance FLAG so she doesn't read a disputed claim as if
it were uncontested — without that flag implying the system has picked a winner or
re-scored the claim.

### Who

- P-001 Maria (node operator, counter-claim-reader hat) | scanning and drilling into
  claims | wants no noise on un-countered claims AND a clear, non-judgmental disputed
  flag on countered ones.

### Solution

Render the "Counter-claims" thread ONLY when `query_counter_claims(cid)` is non-empty
(the `CounterThread::None` arm renders nothing extra; the `Countered` arm renders the
flag + thread). When countered, add a presence FLAG near the claim (e.g. a "Countered"
marker) — a neutral presence indicator, NEVER a weight, score, or verdict. The flag
and thread carry through both htmx and no-JS shapes.

### Domain Examples

#### 1: Happy Path — un-countered claim renders as today
Maria opens `/claims/bafy...solo` (her own claim, no counters). The detail shows the
claim fields + evidence exactly as in slice-06/07 — NO "Counter-claims" section, NO
"0 counters" line, NO empty noise.

#### 2: Edge Case — countered claim is flagged
Maria opens `/claims/bafy...n4ka` (countered by `bafy...new`). Near the claim, a
"Countered" flag appears; beneath, the thread (from US-CT-002). The claim's confidence
0.91 is unchanged by the flag.

#### 3: Boundary — claim that does not exist
Maria opens `/claims/bafy...nope` (no such claim). The existing guided 404 not-found
render (slice-07) is unchanged — slice-11 adds NO counter-thread to the not-found path
(there is no claim to counter).

### UAT Scenarios (BDD)

#### Scenario: An un-countered claim shows no counter section and no empty noise
Given Maria's store contains her claim `bafy...solo` and nothing counters it
When Maria requests `GET /claims/bafy...solo` from the `openlore ui` viewer
Then the response renders the claim fields and evidence as in slice-06/07
And the response contains NO "Counter-claims" section
And the response contains no "0 counters" or "no disagreement" empty-state text

#### Scenario: A countered claim is flagged as disputed
Given Rachel's claim `bafy...n4ka` (confidence 0.91) is countered by Maria's
`bafy...new`
When Maria requests `GET /claims/bafy...n4ka` from the viewer
Then the response shows a "Countered" flag near the claim
And the flag is a neutral presence marker — it shows no score, weight, count-verdict,
or "consensus" judgement
And the claim's confidence still renders "0.91" (the flag does not re-weight the claim)

#### Scenario: The no-noise / flag discipline carries through both shapes
Given an un-countered claim `bafy...solo` and a countered claim `bafy...n4ka`
When Maria requests each WITH and WITHOUT `HX-Request`
Then in BOTH shapes `bafy...solo` shows no counter section
And in BOTH shapes `bafy...n4ka` shows the "Countered" flag + the thread

#### Scenario: A non-existent claim is unaffected by slice-11
Given Maria's store contains no claim with CID `bafy...nope`
When Maria requests `GET /claims/bafy...nope` from the viewer
Then the existing guided 404 not-found page/fragment renders unchanged
And no counter-thread or "Countered" flag is added to the not-found render

### Acceptance Criteria

- [ ] When `query_counter_claims(cid)` is empty, `GET /claims/{cid}` renders no
  "Counter-claims" section and no empty-state counter noise (clean as slice-06/07).
- [ ] When non-empty, a neutral "Countered" presence flag renders near the claim — no
  score/weight/count-verdict/consensus judgement.
- [ ] The flag never alters the claim's rendered confidence or fields (shown, never
  applied).
- [ ] The no-noise (empty) and flag (non-empty) behaviors carry through both htmx and
  no-JS shapes.
- [ ] The existing guided not-found (unknown CID) render is unchanged — no counter
  thread/flag on the 404 path.

### Outcome KPIs

- **Who**: P-001 Maria | **Does what**: distinguishes disputed from undisputed claims
  at a glance, with zero noise on the common (un-countered) case | **By how much**:
  100% of un-countered claims render with no counter noise; 100% of countered claims
  carry the neutral flag; 0 claims have confidence altered by the flag | **Measured
  by**: acceptance test over the real route (empty + non-empty + 404 cases) +
  shown-never-applied gold | **Baseline**: n/a (new legibility behavior). Leading
  indicator OF KPI-VIEW-1 (legibility) and KPI-FED-3.

### Technical Notes

- Keys entirely off the empty-vs-non-empty result of `query_counter_claims` (US-CT-001).
  The `CounterThread::None` arm is the no-noise branch; `Countered` is the flag+thread
  branch. Reuses the slice-09/10 guided-empty-state precedent (no error on empty).
- The "Countered" flag wording/placement is DESIGN's; the PRODUCT contract is
  "neutral presence marker, never a verdict, never re-weights the claim".
- Depends on US-CT-001 + US-CT-002. No external dependency.
