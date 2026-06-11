<!-- markdownlint-disable MD024 -->
# User Stories: viewer-search-full-follow-state (slice-20)

> Combined file (one section per story). Brownfield DELTA on slices 05/06/07/08/15/16.
> The one non-`@infrastructure` story traces to **J-005c** (`docs/product/jobs.yaml`).
> The follow path stays the slice-03 CLI (`openlore peer add`); the viewer is read-only,
> holds no key. slice-20 COMPLETES the slice-16 `/search` follow-state ADT to its full
> four-arm resolution: it RESOLVES the two deferred arms — `You` (the operator's OWN claim)
> and `UnsubscribedCache` (a peer the operator soft-removed; cached but not active) — and
> renders a neutral self / residue indicator for each. REUSES the slice-16
> `render_following_indicator` pattern + the existing four-variant `AuthorRelationship`
> enum (no new variant) + the render `@match`'s two ALREADY-EMPTY `You | UnsubscribedCache`
> arms.

## System Constraints (cross-cutting — apply to every story)

RESTATED as binding commitments (inherited, not re-litigated). Each story's AC inherits
them; they are not repeated per-story.

- **C-1 Read-only / no key (CARDINAL)**: ALL FOUR follow-state affordances — the slice-16
  "Following" indicator + `openlore peer add` guidance, and the slice-20 self + residue
  indicators — stay render-only TEXT. The viewer holds `StoreReadPort` + `IndexQueryPort`
  only — no mutation method, no signing key, no follow/unfollow control on the rendered
  surface. The follow stays EXCLUSIVELY the slice-03 CLI. slice-20 changes only WHICH
  render-only affordance a row shows, never adding an executable control. Enforced 3 layers
  (type: no mutation method + xtask check-arch viewer-capability rule + behavioral gold).
  [KPI-VIEW-2, slice-06–19, slice-08 WD-NS-3]
- **C-2 Accuracy / completeness — the load-bearing new behavior**: a `/search` result
  author is resolved to exactly ONE of four honest states — `You` (the result is the
  operator's OWN claim), `SubscribedPeer` (an author the operator follows — slice-16),
  `UnsubscribedCache` (a peer the operator soft-removed; cached but not active), or
  `NetworkUnfollowed` (a genuinely-new author). The render-only `openlore peer add`
  affordance is shown ONLY for `NetworkUnfollowed`. [J-005c, WD-FS-1/2]
- **C-3 LOCAL / offline relationship resolution**: every arm is resolved against LOCAL
  reads (the slice-15 active set REUSED; the operator's OWN-claim author DIDs from the
  `claims` table NEW; the cached-peer author DIDs from the `peer_claims` table NEW). NO
  network is used for resolution. The network index stays per-user-neutral (the slice-05/08
  boundary — the index never learns who you follow, who you are, or whom you removed). The
  `/search` route's network seam (the indexer query for the result rows) is UNCHANGED.
  [KPI-5, KPI-AV per-user-neutral boundary, slice-15 WD-PS-4]
- **C-4 Batch reads per render (no N+1)**: each of the three LOCAL sets — active (slice-16,
  REUSED), own-author DIDs (NEW), cached-peer DIDs (NEW) — is read AT MOST ONCE per `/search`
  render into an in-memory bare-DID set; each result author is resolved in memory. A
  per-result presence query is REJECTED (N+1). [WD-FS-3, slice-16 C-4, slice-15 I-PS-3/4]
- **C-5 Attribution + ranking UNCHANGED**: the four-arm resolution does NOT merge, re-group,
  or re-rank results. Every result stays attributed to its own author (`compose_results`
  per-author grouping + order unchanged, slice-08 I-NS-3); the relationship label is a
  per-row enrichment ONLY. The `[verified]` marker, verbatim confidence, and counter-
  annotation are all unchanged. [J-003a, KPI-AV-2, slice-08 WD-NS-5, slice-16 C-5]
- **C-6 Resolution precedence**: a result author resolves to the FIRST matching arm in
  `You` (bare DID ∈ own-claim author DIDs) > `SubscribedPeer` (∈ active set) >
  `UnsubscribedCache` (∈ cached-peer DIDs AND ∉ active set) > `NetworkUnfollowed`
  (otherwise). `You` is the strongest fact; a currently-active subscription outranks a stale
  cache; cache-without-active is the residue state. Mirrors the LOCAL-graph resolver
  (`adapter-duckdb::attributed_claim_from`), adapted to the network corpus (no
  `source_table`, so presence is resolved by DID-set membership). [WD-FS-2]
- **C-7 Additive / no-regression (CARDINAL)**: the slice-16 `SubscribedPeer` "Following"
  indicator + the `NetworkUnfollowed` `openlore peer add` guidance render BYTE-STABLE; the
  original search ranking, per-author attribution, `[verified]` marker, and verbatim
  confidence are unchanged. The `You` + `UnsubscribedCache` arms only ADD. [slice-16 I-SF-4,
  slice-12/14 byte-identity discipline]
- **C-8 Graceful degradation**: each LOCAL read degrades INDEPENDENTLY to its slice-16
  fallback — a failed own-DID read → no `You` arm (those authors fall through to the next
  precedence step); a failed cached-peer read → no `UnsubscribedCache` arm; a failed
  active-set read → the slice-16 degrade (all `NetworkUnfollowed`). In every case the row
  still resolves to a valid arm and renders; no crash, no blank region, no leaked error. The
  worst case (all three reads fail) is exactly the slice-08 status quo. [WD-FS-4, slice-16
  C-7, slice-17 per-count `.ok()` independence precedent]
- **C-9 Neutral framing**: the self + residue indicators are NEUTRAL descriptive TEXT,
  never pejorative. The `You` indicator reads as a neutral self-attribution ("your own
  claim"-class copy, DESIGN owns exact wording); the `UnsubscribedCache` indicator reads as
  a neutral residue note ("a peer you removed (cached)"-class copy) — never "ex-peer",
  "abandoned", "stale", or any judgement. [slice-11/12/14 anti-misread discipline]
- **C-10 Progressive enhancement + parity**: all four resolved arms render identically
  under the htmx `#search-results` fragment and the no-JS full page (same
  `render_search_results_fragment` both shapes embed; resolution happens in the shell BEFORE
  the render). A swap is a nicety, never a requirement. [slice-07/08 WD-NS-6, slice-16 C-8]
- **C-11 No new crates / route / variant / persisted type; loopback-only bind**: extend the
  EFFECT `adapter-http-viewer` (thread two new sets into resolution) + the PURE
  `viewer-domain` (fill the two empty render arms) + `ports` / `adapter-duckdb` (two new
  read-only presence reads). NO new route, NO new `AuthorRelationship` variant (the enum is
  already four-variant), NO new crate. Workspace stays 21 members. Functional paradigm
  (ADR-007). [slice-06–19]

---

## US-FS-001: Resolve `You` + `UnsubscribedCache` on `/search` against LOCAL own-DID + cached-peer presence (`@infrastructure`)

`job_id: infrastructure-only`

### Infrastructure rationale

US-FS-001 extends the slice-16 binary `/search` resolution (`SubscribedPeer` vs
`NetworkUnfollowed`) to the FULL four-arm `AuthorRelationship` by (a) adding two read-only
LOCAL presence reads to `StoreReadPort` — the operator's distinct OWN-claim author DIDs (from
the `claims` table, for `You`) and the distinct cached-peer author DIDs (from the `peer_claims`
table INCLUDING soft-removed peers, for `UnsubscribedCache`); (b) reading each set AT MOST ONCE
per `/search` render into an in-memory bare-DID set (alongside the slice-16 active set); and (c)
resolving each result author by precedence (`You` > `SubscribedPeer` > `UnsubscribedCache` >
`NetworkUnfollowed`). It produces no NEW user-visible output on its own — the two new indicators
are rendered by US-FS-002 — so it enables a user decision only THROUGH that story. The slice
contains ONE non-infrastructure, user-visible story (US-FS-002), so the slice has release value
(Dimension-0 slice-level check passes). This is a READ-ONLY capability by construction: the two
new reads are read-only `StoreReadPort` methods (no mutation method on the port); it reads three
LOCAL sets + the `IndexQueryPort` results; it holds no key.

### Problem

The slice-16 `/search` resolution (`crates/adapter-http-viewer/src/lib.rs::to_indexed_claim`
~line 1305) resolves ONLY the binary `SubscribedPeer` (author ∈ active set) vs `NetworkUnfollowed`
(otherwise). So TWO local situations both misclassify as `NetworkUnfollowed`: (1) a result that
is the operator's OWN claim (own DID), and (2) a result from a peer the operator soft-removed
(present in the LOCAL `peer_claims` cache but not in the active set — the slice-15 PS-4 residue).
The `AuthorRelationship` enum already has the `You` and `UnsubscribedCache` variants
(`crates/ports/src/federated_row.rs` ~line 67); the LOCAL-graph resolver already resolves all four
(`crates/adapter-duckdb/src/graph_query.rs::attributed_claim_from` ~line 186); the render `@match`
already has the two arms wired empty (`viewer-domain` ~line 1927). What's missing is the `/search`
EFFECT-shell resolution producing the two arms — which needs two LOCAL presence reads the
`StoreReadPort` does not yet expose.

### Who

- P-001 (the viewer operator, "Maria"), network-discovery hat — indirectly; this story is the
  resolution plumbing the four-arm completeness story (US-FS-002) consumes.

### Solution

In `resolve_search_state` (the `/search` effect shell), after fetching the indexer result rows
and BEFORE `compose_results`, read three LOCAL sets — the active set (slice-16, REUSED), the
operator's distinct own-claim author DIDs (NEW read), and the distinct cached-peer author DIDs
incl. soft-removed (NEW read) — each AT MOST ONCE into an in-memory bare-DID set. Resolve each
result author by precedence: `bare_did(author_did) ∈ own → You`; else `∈ active → SubscribedPeer`;
else `∈ cached → UnsubscribedCache`; else `NetworkUnfollowed`. Thread the resolved relationship
into `to_indexed_claim` (which stops resolving only the binary). Each read degrades INDEPENDENTLY
to its fallback (a failed own read → no `You`; a failed cached read → no `UnsubscribedCache`; a
failed active read → the slice-16 all-`NetworkUnfollowed`). NO per-result query; NO network for
resolution; NO new `AuthorRelationship` variant; NO re-grouping or re-ranking.

### Domain Examples

#### 1: Happy path — four authors, four arms, resolved from three batch reads

Maria has published her own claim (author `did:plc:maria-test`), actively follows
`did:plc:rachel-test`, soft-removed `did:plc:tobias-test` (his cached claims retained), and has
never heard of `did:plc:priya-test`. A search for object
`org.openlore.philosophy.reproducible-builds` returns rows by all four. The shell reads the three
sets ONCE each (own `{maria}`, active `{rachel}`, cached `{rachel, tobias}`) and resolves —
Maria's row → `You`, Rachel's → `SubscribedPeer`, Tobias's → `UnsubscribedCache` (cached but not
active), Priya's → `NetworkUnfollowed` — all in memory.

#### 2: Edge — precedence: an active peer who is also cached resolves `SubscribedPeer`

Rachel (`did:plc:rachel-test`) is BOTH in the active set AND in the cached-peer set (the operator
has her cached claims). Precedence (`SubscribedPeer` > `UnsubscribedCache`) resolves her row to
`SubscribedPeer` ("Following"), never `UnsubscribedCache` — you follow her NOW, the cache is
incidental.

#### 3: Boundary — a soft-removed-then-fragmented author still resolves `UnsubscribedCache`

The cached-peer set holds the BARE `did:plc:tobias-test`; the result row's `author_did` is
`did:plc:tobias-test#org.openlore.application`. Resolution strips the fragment via the `bare_did`
SSOT on both sides before membership, so Tobias resolves to `UnsubscribedCache` despite the
`#fragment` — never misclassified as `NetworkUnfollowed`.

#### 4: Boundary — the cached-peer read fails, that arm degrades independently

The LOCAL `peer_claims` cached-peer read errors mid-request, but the own and active reads
succeed. Tobias (soft-removed, cached) falls through to `NetworkUnfollowed` (the slice-16
fallback for that arm); Maria still resolves `You`, Rachel still `SubscribedPeer`. The `/search`
results still render; no crash, no blank region, no leaked error.

### UAT Scenarios (BDD)

> Each scenario names its DRIVING ROUTE (`GET /search`, port-to-port via the real `openlore ui`
> subprocess). No scenario calls the resolution fn directly.

#### Scenario: A search resolves all four follow-states from three batch reads of the local store

Given Maria has her own claim by `did:plc:maria-test`, follows `did:plc:rachel-test`, soft-removed `did:plc:tobias-test` (cached), and does not know `did:plc:priya-test`
And a reachable indexer holds verified claims by all four authors
When she opens `GET /search?object=org.openlore.philosophy.reproducible-builds`
Then Maria's result resolves to the "you" relationship, Rachel's to "subscribed peer", Tobias's to "unsubscribed cache", and Priya's to "network unfollowed"
And the operator's own-claim, active-subscription, and cached-peer sets are each read AT MOST ONCE for the whole render (invariant to the number of result rows)
And no network call is made to resolve any relationship (the index stays per-user-neutral)

#### Scenario: An active peer who is also cached resolves to "subscribed peer" by precedence

Given Maria actively follows `did:plc:rachel-test` and also holds Rachel's cached claims
When she opens `GET /search` and Rachel's claim appears
Then Rachel's row resolves to the "subscribed peer" relationship (active outranks cached)
And NOT to the "unsubscribed cache" relationship

#### Scenario: A soft-removed author is matched despite the signing-key fragment on the result DID

Given Maria soft-removed `did:plc:tobias-test` (his cached claims retained, subscription inactive)
And the search result row's author DID is `did:plc:tobias-test#org.openlore.application`
When she opens `GET /search` and that row appears
Then the row resolves to the "unsubscribed cache" relationship (the fragment is stripped before the match)

#### Scenario: A failed cached-peer read degrades only that arm without crashing

Given the operator's LOCAL cached-peer read fails during a search render (the own and active reads succeed)
When Maria opens `GET /search` with a reachable indexer holding a claim by her soft-removed peer Tobias
Then Tobias's row resolves to the "network unfollowed" relationship (that arm's slice-16 fallback)
And her own claim still resolves to "you" and her followed peer still to "subscribed peer"
And the search results still render with no crash, blank region, or leaked error

### Acceptance Criteria

- [ ] `to_indexed_claim` resolves the FULL four-arm `AuthorRelationship` (not only the binary): `You` / `SubscribedPeer` / `UnsubscribedCache` / `NetworkUnfollowed`
- [ ] An author whose bare DID ∈ the operator's own-claim author DIDs resolves to `You`
- [ ] An author whose bare DID ∈ the cached-peer DIDs AND ∉ the active set resolves to `UnsubscribedCache`
- [ ] Resolution precedence is `You` > `SubscribedPeer` > `UnsubscribedCache` > `NetworkUnfollowed` (an active-and-cached peer resolves `SubscribedPeer`)
- [ ] Each of the three LOCAL sets (active REUSED, own-author NEW, cached-peer NEW) is read AT MOST ONCE per `/search` render, invariant to the number of result rows (no N+1)
- [ ] The DID comparison strips the `#fragment` on ALL sides via the existing `bare_did` SSOT before set membership
- [ ] Resolution uses NO network (the index stays per-user-neutral); only the three LOCAL sets
- [ ] Each LOCAL read degrades INDEPENDENTLY: a failed own read drops the `You` arm, a failed cached read drops the `UnsubscribedCache` arm, a failed active read keeps the slice-16 all-`NetworkUnfollowed` degrade — no crash, no blank region, no leaked error
- [ ] Resolution sets the per-row `relationship` ONLY — it does NOT re-group, re-rank, or merge results (`compose_results` grouping + order unchanged)
- [ ] No new `AuthorRelationship` variant, no new route, no new crate (the enum is already four-variant); the two new presence reads are read-only `StoreReadPort` methods (no mutation method)

### Outcome KPIs

- **Who**: the viewer process serving `GET /search`
- **Does what**: resolves every result author's relationship to the full four-arm `AuthorRelationship` against the three LOCAL sets, in batch reads
- **By how much**: each of the three LOCAL sets read AT MOST once per render, invariant to result count (0 N+1); 100% of own-author results resolved `You` and 100% of cached-but-inactive results resolved `UnsubscribedCache`
- **Measured by**: behavioral assertion through the real `openlore ui` subprocess (each read fires at most once, invariant to result count; a seeded own claim resolves `You`; a seeded soft-removed peer's cached claim resolves `UnsubscribedCache`)
- **Baseline**: today the operator's own claims and her soft-removed peers' cached claims BOTH resolve `NetworkUnfollowed` (0% accurate for these two states)

### Technical Notes

- Resolution seam: `resolve_search_state` (`crates/adapter-http-viewer/src/lib.rs` ~line 1095) reads the three LOCAL sets (active via the slice-16 `read_local_active_set` ~line 1247, REUSED; the two NEW reads), materializes each into a `HashSet<String>` of bare DIDs, and threads them into `to_indexed_claim` (~line 1305) which resolves each `author_did` by precedence.
- The four-arm precedence MIRRORS `crates/adapter-duckdb/src/graph_query.rs::attributed_claim_from` (~line 186), adapted to the network corpus: an index row carries no `source_table`, so `You`/`UnsubscribedCache` are resolved by DID-set membership (own / cached) rather than the `'Own'`/`'Peer'` column.
- Two NEW read-only `StoreReadPort` presence reads: distinct own-claim author DIDs (from `claims`); distinct cached-peer author DIDs (from `peer_claims`, NO `removed_at` filter — INCLUDING soft-removed, since the residue cache is the point). DESIGN owns whether these are two aggregates or one; the PRODUCT contract is batch-once + read-only.
- DID comparison uses the existing `bare_did` SSOT (`crates/viewer-domain/src/lib.rs`; the adapter mirror) — strip `#fragment` on the result `author_did` AND each LOCAL set before membership.
- DESIGN owns whether `to_indexed_claim` takes the three sets as params, whether resolution is a small pure fn over `(author_did, &own, &active, &cached)`, and the set types. The PRODUCT contract is the AC.
- Dependencies: slice-16 binary resolution + `read_local_active_set` + the four-variant `AuthorRelationship` enum (SHIPPED); slice-15 `list_active_peer_subscriptions` (SHIPPED); slice-08 `/search` + `to_indexed_claim` + `compose_results` (SHIPPED). NEW: two read-only `StoreReadPort` presence reads + their `adapter-duckdb` impls.
- READ-ONLY: reads `StoreReadPort` (three LOCAL sets) + `IndexQueryPort` (results); neither has a mutation method; no key.

---

## US-FS-002: On `/search`, my own claim shows a neutral self indicator and a peer I removed shows a neutral residue indicator — and the slice-16 states are unchanged

`job_id: J-005c`

### Problem

Maria discovers claims across the network on `/search` (slice-08/16). Since slice-16, an author
she follows shows "Following" and an unfollowed author shows the `openlore peer add` guidance —
but her OWN claims and her SOFT-REMOVED peers' cached claims BOTH still show the `peer add`
guidance (they resolve `NetworkUnfollowed`). So she is told to "add" a peer for HER OWN claim (she
can't follow herself), and a cached claim from a developer she DELIBERATELY removed looks
identical to a fresh network discovery — erasing the fact that she once followed them and chose to
stop. She cannot tell, at a glance, the four honest situations apart.

### Who

- P-001 (the viewer operator, "Maria"), network-discovery hat | scanning network-search results
  in the browser | wants to see, at a glance, the FOUR honest states of each discovered author:
  her own claim (so she ignores the add prompt), a peer she follows (so she ignores it),
  a peer she removed and cached (so she knows it's residue, not a fresh find), and a genuinely-new
  author (so she knows the next step is `openlore peer add` in the CLI).

### Solution

On `/search`, each result row renders an affordance driven by its resolved four-arm relationship
(US-FS-001): the operator's OWN claim (`You`) shows a neutral self-attribution indicator and NO
follow command; a peer she soft-removed (`UnsubscribedCache`) shows a neutral residue indicator
("a peer you removed (cached)"-class) and NO follow command; an author she follows
(`SubscribedPeer`) shows the slice-16 "Following" indicator (UNCHANGED); a genuinely-new author
(`NetworkUnfollowed`) keeps the slice-16 render-only `openlore peer add <bare-did>` follow
guidance (UNCHANGED). All four are render-only TEXT — the viewer holds no key and executes
nothing. Attribution, grouping, ranking, the `[verified]` marker, and verbatim confidence are all
unchanged. The view renders identically under the htmx fragment + the no-JS full page.

### Elevator Pitch

- **Before**: on `/search`, since slice-16, a followed author shows "Following" and an unfollowed
  one shows `openlore peer add` — but Maria's OWN claims and her SOFT-REMOVED peers' cached claims
  BOTH still show the `peer add` guidance, so she's told to "add" herself, and a cached claim from
  a peer she deliberately removed is indistinguishable from a fresh network find.
- **After**: open `http://127.0.0.1:<port>/search`, search a philosophy → a row that is her OWN
  claim shows a neutral self indicator (no add command), a row from a peer she removed shows a
  neutral "removed (cached)" indicator (no add command), a followed author still shows "Following",
  and only a genuinely-NEW author shows the render-only `openlore peer add <did>` command she can
  run in the CLI.
- **Decision enabled**: Maria decides WHICH genuinely-new discovered authors to follow next —
  cleanly skipping her own claims and her removed-peer residue — so the only `peer add` she ever
  sees is one she could meaningfully run, keeping discovery a clean front-door that grows her
  trusted local graph (J-005c).

### Domain Examples

#### 1: Happy path — own claim → self indicator, removed peer → residue indicator, others unchanged

Maria's own DID is `did:plc:maria-test`; she follows `did:plc:rachel-test`, soft-removed
`did:plc:tobias-test` (cached), and doesn't know `did:plc:priya-test`. A search for object
`org.openlore.philosophy.reproducible-builds` returns claims by all four. Maria's row shows a
neutral self indicator and NO add command; Tobias's row shows a neutral residue indicator
("removed (cached)"-class) and NO add command; Rachel's row shows the slice-16 "Following"
indicator; Priya's row shows the render-only `openlore peer add did:plc:priya-test` guidance. All
four are plain TEXT; none is a button.

#### 2: Edge — no-regression: a search with only a followed + an unfollowed author is byte-stable

Maria follows `did:plc:rachel-test` and doesn't know `did:plc:priya-test`; neither her own claim
nor a removed peer appears. The search renders EXACTLY as slice-16: Rachel's row shows "Following"
+ no add; Priya's row shows `openlore peer add did:plc:priya-test`. The `You`/`UnsubscribedCache`
arms add nothing where they don't apply (no regression to the slice-16 rendering).

#### 3: Boundary — a removed peer's cached claim is neutral, not pejorative

Maria soft-removed `did:plc:tobias-test`. His cached claim appears in a search. His row shows a
NEUTRAL residue indicator describing the cache/removal state factually — never "ex-peer",
"abandoned", "stale", or any judgement; and NO `openlore peer add` command (he is not a fresh
network find). She reads it as "this is residue from a peer I removed", not as a verdict.

### UAT Scenarios (BDD)

> Driving route: `GET /search` (the real `openlore ui` subprocess), both shapes.

#### Scenario: The operator's own claim shows a neutral self indicator and is not offered a follow

Given Maria's own DID is `did:plc:maria-test` and she has published a claim
And a reachable indexer holds her verified claim
When she opens `GET /search?object=org.openlore.philosophy.reproducible-builds` and her own claim appears
Then her row shows a neutral self-attribution indicator
And her row shows NO `openlore peer add` command (you cannot follow yourself)

#### Scenario: A soft-removed peer's cached claim shows a neutral residue indicator, not a follow

Given Maria soft-removed `did:plc:tobias-test` (his cached claims retained, subscription inactive)
And a reachable indexer holds a verified claim by Tobias
When she opens `GET /search` and Tobias's claim appears
Then his row shows a neutral residue indicator (a peer you removed, cached)
And his row shows NO `openlore peer add` command
And the indicator copy is neutral, never pejorative (no "ex-peer", "abandoned", "stale", or judgement)

#### Scenario: The slice-16 followed and unfollowed states are unchanged (no regression)

Given Maria follows `did:plc:rachel-test` but not `did:plc:priya-test`, and neither her own claim nor a removed peer appears
And a reachable indexer holds verified claims by both
When she opens `GET /search` and both claims appear
Then Rachel's row shows the slice-16 "Following" indicator with no add command
And Priya's row shows the slice-16 render-only `openlore peer add did:plc:priya-test` guidance
And the rendering of these two rows is byte-stable versus slice-16 (the two new arms add nothing here)

#### Scenario: All four follow-states render correctly side by side, attributed and unranked-by-relationship

Given Maria's own claim, a followed peer (Rachel), a soft-removed peer (Tobias, cached), and a new author (Priya) all appear in one search
When she opens `GET /search` and all four claims appear
Then her own row shows the self indicator, Rachel's shows "Following", Tobias's shows the residue indicator, and Priya's shows `openlore peer add did:plc:priya-test`
And each affordance is plain TEXT — no button, no form, no mutating link
And the four rows are still attributed to their own authors with grouping and ranking unchanged versus slice-16

#### Scenario: The four follow-states render identically under htmx and no-JS

Given Maria's own claim and a soft-removed peer's cached claim both appear in a search
When she requests `GET /search` WITH `HX-Request` and again WITHOUT it
Then the htmx response is the `#search-results` fragment with the self indicator and the residue indicator
And the no-JS response is the full page = chrome + the SAME fragment, rendered identically

### Acceptance Criteria

- [ ] A result that is the operator's OWN claim (resolved `You`) renders a neutral self-attribution indicator and NO `openlore peer add` command
- [ ] A result from a peer the operator SOFT-REMOVED (resolved `UnsubscribedCache`) renders a neutral residue indicator and NO `openlore peer add` command
- [ ] The `You` + `UnsubscribedCache` indicator copy is NEUTRAL — never pejorative (no "ex-peer", "abandoned", "stale", or judgement)
- [ ] A followed author (resolved `SubscribedPeer`) still renders the slice-16 "Following" indicator, and a genuinely-new author (resolved `NetworkUnfollowed`) still renders the slice-16 render-only `openlore peer add <bare-did>` guidance — BYTE-STABLE versus slice-16 (no regression)
- [ ] All four affordances are render-only TEXT — no button, no form, no mutating link, no key (the viewer executes nothing)
- [ ] Results stay attributed per-author with grouping + ranking UNCHANGED versus slice-16 (each arm is a per-row enrichment only — no merge, no re-rank)
- [ ] The `[verified]` marker and verbatim confidence are unchanged on every row
- [ ] All four states render identically under the htmx `#search-results` fragment and the no-JS full page (parity by construction — same fragment fn)
- [ ] The route is read-only and LOCAL for resolution (renders, and resolves all four arms, offline against the local store)

### Outcome KPIs

- **Who**: P-001 dogfood operators discovering claims on `/search`
- **Does what**: distinguishes, at a glance, the four honest states — own claim (self), followed peer (Following), removed-and-cached peer (residue), and genuinely-new author (`peer add`) — and follows only the genuinely-new ones via the CLI
- **By how much**: leading indicator OF KPI-AV-4 (the discovery→federation funnel) — the `peer add` affordance is shown ONLY where it is actionable (0% re-offered to own claims OR soft-removed peers' cached claims, on top of slice-16's 0% to followed peers), so 100% of shown `peer add` affordances are genuinely-new authors
- **Measured by**: per-feature GREEN (own claim → self indicator + no add; removed peer's cached claim → residue indicator + no add; followed → "Following" unchanged; new → add command unchanged); cohort via the inherited opt-in telemetry (ADR-010) — search→`peer add` funnel quality
- **Baseline**: today (post-slice-16) own claims and soft-removed peers' cached claims both show `peer add` (100% of these two states are wrongly re-offered a follow)

### Technical Notes

- Render: fill the existing empty `You | UnsubscribedCache => {}` arm of `render_search_results_fragment`'s `@match` (`crates/viewer-domain/src/lib.rs` ~line 1924) with `You → render_self_indicator()` and `UnsubscribedCache → render_cached_unsubscribed_indicator()` — neutral render-only TEXT (a `<p>`/`<span>`, DESIGN owns markup + copy), each a sibling of `render_following_indicator` (~line 1957).
- Each new indicator gets its own SSOT constant (mirroring `SEARCH_FOLLOWING_INDICATOR` ~line 1684), e.g. `SEARCH_SELF_INDICATOR` and `SEARCH_REMOVED_CACHED_INDICATOR` — held in ONE place so the copy is a single source of truth. DESIGN owns the exact neutral wording.
- The `SubscribedPeer → render_following_indicator()` and `NetworkUnfollowed → render_follow_guidance(...)` arms are REUSED VERBATIM (UNCHANGED, byte-stable — C-7).
- The render becomes a TOTAL `match` over all four `AuthorRelationship` variants (no empty arm remains).
- Dependencies: US-FS-001 (in-slice — the four-arm resolution); slice-16 `render_following_indicator` + `render_follow_guidance` (SHIPPED, reused verbatim).

---

## Out of scope (explicit — restated from feature-delta)

- **Following / unfollowing / removing from the viewer** — no follow/unfollow/add/remove button,
  form, or control. Stays the slice-03 CLI; all four follow-state affordances are render-only TEXT
  (C-1).
- **Holding a signing key or any mutation capability in the viewer** (C-1, CARDINAL).
- **An own-identity surface beyond reading the operator's own-claim author DIDs** — `You` is
  resolved purely from the presence of the result author's bare DID in the operator's own `claims`
  table; no `/me` page, no identity store, no key.
- **Resolving `You` / `UnsubscribedCache` on any surface other than `/search`** — the LOCAL graph
  already resolves all four arms on `/project`, `/philosophy`, etc. (WD-FS-5).
- **Any network seam for resolution** — every arm is resolved against LOCAL sets only; the index
  stays per-user-neutral (C-3).
- **Re-grouping, re-ranking, or merging results** — each arm is a per-row enrichment; grouping +
  order unchanged versus slice-16 (C-5).
- **Regressing the slice-16 `SubscribedPeer` / `NetworkUnfollowed` rendering** — byte-stable
  (C-7, CARDINAL).
- **A new route, a new `AuthorRelationship` variant, a new crate, or any persisted state** (C-11 —
  workspace stays 21).
- **N+1 (one presence query per result author)** — each LOCAL set is read AT MOST once per render
  (C-4).
