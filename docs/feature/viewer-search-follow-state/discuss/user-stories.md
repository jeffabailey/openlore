<!-- markdownlint-disable MD024 -->
# User Stories: viewer-search-follow-state (slice-16)

> Combined file (one section per story). Brownfield DELTA on slices 05/06/07/08/15.
> Every non-`@infrastructure` story traces to **J-005c** (`docs/product/jobs.yaml`).
> The follow path stays the slice-03 CLI (`openlore peer add`); the viewer is read-only,
> holds no key. slice-16 RESOLVES each `/search` result author's relationship against the
> operator's LOCAL active subscriptions (REUSING the slice-15
> `list_active_peer_subscriptions` read) and renders a neutral "Following" indicator for a
> followed author instead of re-offering a follow. REUSES the slice-08
> `render_follow_guidance` + `AuthorRelationship` enum (no new variant).

## System Constraints (cross-cutting — apply to every story)

RESTATED as binding commitments (inherited, not re-litigated). Each story's AC inherits
them; they are not repeated per-story.

- **C-1 Read-only / no key (CARDINAL)**: BOTH the "Following" indicator AND the `peer add`
  affordance stay render-only TEXT. The viewer holds `StoreReadPort` + `IndexQueryPort`
  only — no mutation method, no signing key, no follow/unfollow control on the rendered
  surface. The follow stays EXCLUSIVELY the slice-03 CLI. slice-16 changes only WHICH
  render-only affordance a row shows, never adding an executable control. Enforced 3 layers
  (type: no mutation method + xtask check-arch viewer-capability rule + behavioral gold).
  [KPI-VIEW-2, slice-06–15, slice-08 WD-NS-3]
- **C-2 Accuracy — the load-bearing new behavior**: an author the operator ALREADY follows
  (DID ∈ active subscriptions) shows a neutral "Following" indicator and NO `peer add`
  command; a genuinely-unfollowed author KEEPS the slice-08 render-only `openlore peer add
  <did>` affordance. This is the at-a-glance fix to the slice-08 "always NetworkUnfollowed"
  gap. [J-005c, WD-SF-2/5]
- **C-3 LOCAL / offline relationship resolution**: the relationship is resolved against the
  LOCAL active-subscription set (a LOCAL DuckDB read via the slice-15
  `list_active_peer_subscriptions`); NO network is used for resolution. The network index
  stays per-user-neutral (the slice-05/08 boundary — the index never learns who you follow).
  The `/search` route's network seam (the indexer query for the result rows) is UNCHANGED.
  [KPI-5, KPI-AV per-user-neutral boundary, slice-15 WD-PS-4]
- **C-4 ONE batch read of the active set per render (no N+1)**: the active set is read ONCE
  per `/search` render (the slice-15 single-aggregate-query read) into an in-memory bare-DID
  set; each result author is resolved in memory. A per-result subscription query is REJECTED
  (N+1). [WD-SF-3, slice-15 I-PS-3/4]
- **C-5 Attribution + ranking UNCHANGED**: relationship resolution does NOT merge, re-group,
  or re-rank results. Every result stays attributed to its own author (`compose_results`
  per-author grouping + order unchanged, slice-08 I-NS-3); the relationship label is a
  per-row enrichment ONLY. The `[verified]` marker, verbatim confidence, and counter-
  annotation are all unchanged. [J-003a, KPI-AV-2, slice-08 WD-NS-5]
- **C-6 Binary resolution; `You`/`UnsubscribedCache` not resolved on `/search`**: slice-16
  resolves to exactly `SubscribedPeer` (∈ active set) or `NetworkUnfollowed` (otherwise). A
  soft-removed peer is NOT in the active set → `NetworkUnfollowed` (correctly re-offered a
  follow — they are currently unfollowed). `You` (own-DID) is DEFERRED (WD-SF-2 — the
  read-only network-search surface does not cheaply hold the operator's own DID). [WD-SF-2/5]
- **C-7 Graceful degradation**: if the LOCAL active-set read fails, resolution degrades to
  the slice-08 status quo (every author → `NetworkUnfollowed`, the `peer add` affordance
  shown). The `/search` results still render; no crash, no blank region, no leaked error.
  [WD-SF-6, slice-08 I-NS-2, slice-15 `/peers` Err→NoSubscriptions precedent]
- **C-8 Progressive enhancement + parity**: the resolved relationship renders identically
  under the htmx `#search-results` fragment and the no-JS full page (same
  `render_search_results_fragment` both shapes embed; the resolution happens in the shell
  BEFORE the render). A swap is a nicety, never a requirement. [slice-07/08 WD-NS-6]
- **C-9 No new crates / route / variant / persisted type; loopback-only bind**: extend the
  EFFECT `adapter-http-viewer` (thread the active set into resolution) + the PURE
  `viewer-domain` (add the `SubscribedPeer` "Following" render arm). NO `ports` change
  (`AuthorRelationship` + `list_active_peer_subscriptions` already exist), NO
  `adapter-duckdb` change, NO new route, NO new enum variant. Workspace stays 21 members.
  Functional paradigm (ADR-007). [slice-06–15]

---

## US-SF-001: Resolve each `/search` result author's relationship against the operator's LOCAL active subscriptions (`@infrastructure`)

`job_id: infrastructure-only`

### Infrastructure rationale

US-SF-001 replaces the hardcoded `AuthorRelationship::NetworkUnfollowed` in the viewer's
`to_indexed_claim` (`crates/adapter-http-viewer/src/lib.rs` ~line 1021-1033) with a
resolution against the operator's LOCAL active-subscription set. It (a) reads the active set
ONCE per `/search` render via the slice-15 `StoreReadPort::list_active_peer_subscriptions`
(REUSED — no new read method, no new SQL), (b) materializes it into an in-memory set of
bare DIDs, and (c) resolves each result author's `author_did` (fragment-stripped via the
existing `bare_did` SSOT) to `SubscribedPeer` when ∈ the set, else `NetworkUnfollowed`. It
produces no NEW user-visible output on its own — the at-a-glance change (the "Following"
indicator vs the `peer add` affordance) is rendered by US-SF-002 — so it enables a user
decision only THROUGH that story. The slice contains ONE non-infrastructure, user-visible
story (US-SF-002), so the slice has release value (Dimension-0 slice-level check passes).
This is a READ-ONLY capability by construction: it reads two ports (`IndexQueryPort` for the
results, `StoreReadPort` for the active set), neither of which has a mutation method, and
holds no key.

### Problem

The viewer's `to_indexed_claim` hardcodes `AuthorRelationship::NetworkUnfollowed` for EVERY
network-search result author (the comment at ~line 1017 admits the viewer is
"per-user-neutral … always NetworkUnfollowed"). So the render layer (which branches on
`relationship`) cannot tell an already-followed author from an unfollowed one: it offers the
`peer add` follow affordance to everyone, including peers the operator already subscribes to.
The viewer already reads the active-subscription set for `/peers` (slice-15); `/search` does
not yet thread it into relationship resolution.

### Who

- P-001 (the viewer operator, "Maria"), network-discovery hat — indirectly; this story is
  the resolution plumbing the discovery→follow-accuracy story (US-SF-002) consumes.

### Solution

In `resolve_search_state` (the `/search` effect shell), after fetching the indexer result
rows and BEFORE `compose_results`, read the active-subscription set ONCE via
`store.list_active_peer_subscriptions()` (slice-15), materialize the bare `peer_did`s into an
in-memory set, and resolve each result author: `bare_did(author_did) ∈ set → SubscribedPeer`,
else `NetworkUnfollowed`. Thread the resolved relationship into `to_indexed_claim` (which
stops hardcoding `NetworkUnfollowed`). If the active-set read fails, degrade to all-
`NetworkUnfollowed` (the slice-08 status quo). NO per-result query; NO network for
resolution; NO new read method; NO re-grouping or re-ranking.

### Domain Examples

#### 1: Happy path — one followed, one unfollowed, resolved from one batch read

Maria actively follows `did:plc:rachel-test` (a slice-15 active subscription). A search for
object `org.openlore.philosophy.reproducible-builds` returns rows by
`did:plc:rachel-test#org.openlore.application` (followed) and
`did:plc:priya-test#org.openlore.application` (not followed). The shell reads the active set
ONCE (`{did:plc:rachel-test}`), resolves Rachel's row to `SubscribedPeer` and Priya's to
`NetworkUnfollowed` — in memory, from the single batch read.

#### 2: Edge — fragmented result DID vs bare active-set DID

The active set holds the BARE `did:plc:rachel-test`; the result row's `author_did` is
`did:plc:rachel-test#org.openlore.application`. Resolution strips the fragment via the
existing `bare_did` SSOT on both sides before membership, so Rachel resolves to
`SubscribedPeer` despite the `#fragment` — never misclassified as `NetworkUnfollowed`.

#### 3: Boundary — active-set read fails, resolution degrades to the status quo

The LOCAL DuckDB active-subscription read errors mid-request. Resolution degrades to all-
`NetworkUnfollowed` (every result keeps the `peer add` affordance — the pre-slice-16
behavior). The `/search` results still render; no crash, no blank region, no leaked error.

### UAT Scenarios (BDD)

> Each scenario names its DRIVING ROUTE (`GET /search`, port-to-port via the real `openlore
> ui` subprocess). No scenario calls the resolution fn directly.

#### Scenario: A search resolves followed vs unfollowed authors from one batch read of the active set

Given Maria actively follows `did:plc:rachel-test` and a reachable indexer holds claims by `did:plc:rachel-test` and `did:plc:priya-test`
When she opens `GET /search?object=org.openlore.philosophy.reproducible-builds`
Then Rachel's result resolves to the "subscribed peer" relationship and Priya's to the "network unfollowed" relationship
And the operator's active-subscription set is read exactly ONCE for the whole render (invariant to the number of result rows)
And no network call is made to resolve the relationship (the index stays per-user-neutral)

#### Scenario: A followed author is matched despite the signing-key fragment on the result DID

Given Maria actively follows the bare DID `did:plc:rachel-test`
And the search result row's author DID is `did:plc:rachel-test#org.openlore.application`
When she opens `GET /search` and that row appears
Then the row resolves to the "subscribed peer" relationship (the fragment is stripped before the match)

#### Scenario: A failed active-set read degrades to the slice-08 status quo without crashing

Given the operator's LOCAL active-subscription read fails during a search render
When Maria opens `GET /search` with a reachable indexer
Then every result resolves to the "network unfollowed" relationship (the slice-08 status quo)
And the search results still render with no crash, blank region, or leaked error

### Acceptance Criteria

- [ ] `to_indexed_claim` no longer hardcodes `NetworkUnfollowed`; the relationship is resolved against the operator's LOCAL active-subscription set
- [ ] An author whose bare DID ∈ the active set resolves to `SubscribedPeer`; otherwise to `NetworkUnfollowed`
- [ ] The active-subscription set is read exactly ONCE per `/search` render (the slice-15 `list_active_peer_subscriptions`), invariant to the number of result rows (no N+1)
- [ ] The DID comparison strips the `#fragment` on BOTH sides via the existing `bare_did` SSOT before set membership
- [ ] Resolution uses NO network (the index stays per-user-neutral); only the LOCAL active set
- [ ] A soft-removed peer (not in the active set) resolves to `NetworkUnfollowed` (currently-unfollowed); `You`/`UnsubscribedCache` are NOT resolved on `/search` (WD-SF-2/5)
- [ ] A failed active-set read degrades to all-`NetworkUnfollowed` (the slice-08 status quo) — no crash, no blank region, no leaked error
- [ ] Resolution sets the per-row `relationship` ONLY — it does NOT re-group, re-rank, or merge results (`compose_results` grouping + order unchanged)
- [ ] No new read method, no new `AuthorRelationship` variant, no new route, no `adapter-duckdb` change (the read + enum already exist)

### Outcome KPIs

- **Who**: the viewer process serving `GET /search`
- **Does what**: resolves every result author's relationship against the LOCAL active subscription set in one batch read
- **By how much**: exactly 1 active-set read per render, invariant to result count (0 N+1); 100% of followed authors resolved to `SubscribedPeer`
- **Measured by**: behavioral assertion through the real `openlore ui` subprocess (active-set read count invariant to result count; a seeded followed author resolves to `SubscribedPeer`)
- **Baseline**: today every author is hardcoded `NetworkUnfollowed` (0% accurate for followed authors)

### Technical Notes

- Resolution seam: `resolve_search_state` (`crates/adapter-http-viewer/src/lib.rs` ~line 884) reads `store.list_active_peer_subscriptions()` (slice-15, ~line 717 already calls it for `/peers`) ONCE, materializes the bare `peer_did`s into a `HashSet<String>`, and threads it into `to_indexed_claim` (~line 1021) which resolves each `author_did`.
- DID comparison uses the existing `bare_did` SSOT (`crates/viewer-domain/src/lib.rs` ~line 2566; the adapter mirror) — strip `#fragment` on both the result `author_did` and the active-set `peer_did` (the latter is already bare) before membership.
- DESIGN owns whether `to_indexed_claim` takes the set as a param, whether resolution is a small pure fn over `(author_did, &active_set)`, and the exact set type. The PRODUCT contract is the AC.
- Dependencies: slice-15 `list_active_peer_subscriptions` + `PeerSubscriptionSummary` (SHIPPED); slice-08 `/search` + `to_indexed_claim` + `compose_results` (SHIPPED); `AuthorRelationship` enum (SHIPPED, no new variant).
- READ-ONLY: reads `StoreReadPort` (active set) + `IndexQueryPort` (results); neither has a mutation method; no key.

---

## US-SF-002: On `/search`, show "Following" for an author I already follow — and keep the `peer add` affordance only for authors I don't

`job_id: J-005c`

### Problem

Maria discovers claims across the network on `/search` (slice-08). Today EVERY result row —
including authors she already follows — shows the `openlore peer add <did>` follow guidance,
because the viewer hardcodes `NetworkUnfollowed`. So a discovery she has ALREADY acted on
(she follows that developer) is indistinguishable from a fresh one, and she is told to "add"
a peer she already has. She cannot tell, at a glance, which discovered authors are new to her
and worth following.

### Who

- P-001 (the viewer operator, "Maria"), network-discovery hat | scanning network-search
  results in the browser | wants to see, at a glance, which discovered authors she already
  follows (so she does not re-add them) and which are genuinely new (so she knows the next
  step is `openlore peer add` in the CLI).

### Solution

On `/search`, each result row renders an affordance driven by its resolved relationship
(US-SF-001): an author the operator ALREADY follows (`SubscribedPeer`) shows a neutral
"Following" indicator and NO follow command; a genuinely-unfollowed author
(`NetworkUnfollowed`) keeps the slice-08 render-only `openlore peer add <bare-did>` follow
guidance TEXT (via the existing `render_follow_guidance`). Both are render-only TEXT — the
viewer holds no key and executes nothing. Attribution, grouping, ranking, the `[verified]`
marker, and verbatim confidence are all unchanged. The view renders identically under the
htmx fragment + the no-JS full page.

### Elevator Pitch

- **Before**: on `/search` EVERY result row shows the `openlore peer add <did>` follow
  guidance — even for developers Maria already follows — so she cannot tell, at a glance,
  which discovered authors are new and worth following vs ones she has already acted on, and
  she is told to "add" peers she already has.
- **After**: open `http://127.0.0.1:<port>/search`, search a philosophy → a developer she
  already follows shows a neutral "Following" indicator with NO add command, while a developer
  she does not follow shows the render-only `openlore peer add <did>` command she can run in
  the CLI.
- **Decision enabled**: Maria decides WHICH genuinely-new discovered authors to follow next —
  without wasting attention on (or re-adding) developers she already follows — turning
  discovery into the front-door that grows her trusted local graph (J-005c).

### Domain Examples

#### 1: Happy path — a followed author shows "Following", an unfollowed author keeps `peer add`

Maria follows `did:plc:rachel-test`. A search for object
`org.openlore.philosophy.reproducible-builds` returns claims by Rachel (followed) and
`did:plc:priya-test` (not followed). Rachel's row shows a neutral "Following" indicator and
NO `peer add` command; Priya's row shows the render-only `openlore peer add did:plc:priya-test`
guidance. Both are plain TEXT; neither is a button.

#### 2: Edge — all results are already-followed authors

Maria follows both `did:plc:rachel-test` and `did:plc:tobias-test`. A search returns claims
only by those two. Every row shows "Following"; NO `peer add` command appears anywhere on the
results — there is nobody new to add.

#### 3: Boundary — none followed (the slice-08 status quo preserved)

Maria follows nobody who appears in the results. A search for
`org.openlore.philosophy.reproducible-builds` returns claims by `did:plc:priya-test` and
`did:plc:bjorn-test`, neither followed. Both rows show the render-only `openlore peer add
<did>` guidance — exactly the slice-08 behavior, unchanged (no over-correction).

### UAT Scenarios (BDD)

> Driving route: `GET /search` (the real `openlore ui` subprocess), both shapes.

#### Scenario: An already-followed author shows "Following" and is not re-offered a follow

Given Maria actively follows `did:plc:rachel-test`
And a reachable indexer holds a verified claim by `did:plc:rachel-test`
When she opens `GET /search?object=org.openlore.philosophy.reproducible-builds` and Rachel's claim appears
Then Rachel's row shows a neutral "Following" indicator
And Rachel's row shows NO `openlore peer add` command

#### Scenario: A genuinely-unfollowed author keeps the render-only follow affordance

Given Maria does NOT follow `did:plc:priya-test`
And a reachable indexer holds a verified claim by `did:plc:priya-test`
When she opens `GET /search` and Priya's claim appears
Then Priya's row shows the render-only command `openlore peer add did:plc:priya-test`
And the command is plain TEXT — no button, no form, no mutating link

#### Scenario: Following and unfollowed authors render correctly side by side

Given Maria follows `did:plc:rachel-test` but not `did:plc:priya-test`
And a reachable indexer holds verified claims by both
When she opens `GET /search` and both claims appear
Then Rachel's row shows "Following" with no add command
And Priya's row shows `openlore peer add did:plc:priya-test`
And the two rows are still attributed to their own authors with no merged or re-ranked output

#### Scenario: The follow-state renders identically under htmx and no-JS

Given Maria follows `did:plc:rachel-test` and the indexer holds a claim by Rachel
When she requests `GET /search` WITH `HX-Request` and again WITHOUT it
Then the htmx response is the `#search-results` fragment with Rachel's "Following" indicator and no add command
And the no-JS response is the full page = chrome + the SAME fragment, rendered identically

### Acceptance Criteria

- [ ] An author the operator ALREADY follows (resolved `SubscribedPeer`) renders a neutral "Following" indicator and NO `openlore peer add` command
- [ ] A genuinely-unfollowed author (resolved `NetworkUnfollowed`) renders the slice-08 render-only `openlore peer add <bare-did>` follow guidance (via `render_follow_guidance`) as TEXT
- [ ] Both affordances are render-only TEXT — no button, no form, no mutating link, no key (the viewer executes nothing)
- [ ] Results stay attributed per-author with grouping + ranking UNCHANGED vs slice-08 (the relationship is a per-row enrichment only — no merge, no re-rank)
- [ ] The `[verified]` marker and verbatim confidence are unchanged on every row
- [ ] The follow-state renders identically under the htmx `#search-results` fragment and the no-JS full page (parity by construction — same fragment fn)
- [ ] The route is read-only and LOCAL for resolution (renders, and resolves the relationship, offline against the local active set)

### Outcome KPIs

- **Who**: P-001 dogfood operators discovering claims on `/search`
- **Does what**: distinguishes, at a glance, already-followed discovered authors (shown "Following") from genuinely-new ones (shown the `peer add` command), and follows the new ones via the CLI
- **By how much**: leading indicator OF KPI-AV-4 (the discovery→federation funnel) — the `peer add` affordance is shown ONLY where it is actionable (0% re-offered to already-followed authors), sharpening the funnel's accuracy
- **Measured by**: per-feature GREEN (a followed author shows "Following" + no add command; an unfollowed author keeps the add command); cohort via the inherited opt-in telemetry (ADR-010) — search→`peer add` funnel quality
- **Baseline**: today every result shows `peer add` (100% of followed authors are wrongly re-offered a follow)

### Technical Notes

- Render: extend `render_search_results_fragment` (`crates/viewer-domain/src/lib.rs` ~line 1745). Today it branches ONLY `@if matches!(row.relationship, NetworkUnfollowed) → render_follow_guidance`. Add a `@else if matches!(row.relationship, SubscribedPeer)` arm rendering a neutral "Following" indicator (a render-only `<span>`/`<p>` of TEXT — DESIGN owns the exact markup + the indicator copy, e.g. "Following").
- The "Following" indicator is a NEW render-only affordance, the sibling of `render_follow_guidance`; DESIGN owns whether it is a small `render_following_indicator()` fn + a `SEARCH_FOLLOWING_INDICATOR` const (mirroring `SEARCH_FOLLOW_GUIDANCE_PREFIX`). The PRODUCT contract is the AC.
- `render_follow_guidance` + `SEARCH_FOLLOW_GUIDANCE_PREFIX` are REUSED verbatim for the `NetworkUnfollowed` arm (unchanged).
- DESIGN owns whether the two arms are a `match row.relationship { … }` (total over the 4 variants — `You`/`UnsubscribedCache` fall to the same render-nothing or `NetworkUnfollowed`-equivalent default per WD-SF-2/5).
- Dependencies: US-SF-001 (in-slice — the resolution); slice-08 `render_follow_guidance` (SHIPPED).

---

## Out of scope (explicit — restated from feature-delta)

- **Following / unfollowing from the viewer** — no follow/unfollow/add/remove button, form,
  or control. Stays the slice-03 CLI; the follow is render-only `openlore peer add <did>`
  TEXT, the "Following" indicator is a neutral render-only label (C-1).
- **Holding a signing key or any mutation capability in the viewer** (C-1, CARDINAL).
- **`You` (own-DID) resolution on `/search`** — DEFERRED (C-6 / WD-SF-2); the read-only
  network-search surface does not cheaply hold the operator's own DID. An own-author result
  resolves to `NetworkUnfollowed` (re-offered a follow it would never run — acceptable).
- **`UnsubscribedCache` resolution on `/search`** — a soft-removed peer resolves to
  `NetworkUnfollowed` (currently-unfollowed, correctly re-offered a follow) (C-6 / WD-SF-5).
- **Any network seam for resolution** — the relationship is resolved against the LOCAL active
  set only; the index stays per-user-neutral (C-3).
- **Re-grouping, re-ranking, or merging results** — relationship is a per-row enrichment;
  grouping + order unchanged vs slice-08 (C-5).
- **A new route, a new read method, a new `AuthorRelationship` variant, a new crate, or any
  persisted state** (C-9 — workspace stays 21).
- **N+1 (one subscription query per result author)** — ONE batch read of the active set per
  render (C-4).
