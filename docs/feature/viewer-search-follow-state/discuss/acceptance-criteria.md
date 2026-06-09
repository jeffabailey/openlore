# Acceptance Criteria (BDD): viewer-search-follow-state (slice-16)

> Wave: DISCUSS (lean) · Owner: Luna (nw-product-owner) · 2026-06-09 · Job: **J-005c**
> Driving route for every scenario: `GET /search` (port-to-port via the real `openlore ui`
> subprocess). Scenario titles describe WHAT the operator achieves, never HOW the viewer
> resolves it. Grouped by theme; each maps to AC in `user-stories.md`.

## Theme A — Accuracy: followed shown "Following", unfollowed keeps `peer add` (the load-bearing fix)

### Scenario: An already-followed author shows "Following" and is not re-offered a follow

```
Given Maria actively follows did:plc:rachel-test
And a reachable indexer holds a verified claim by did:plc:rachel-test
When she opens GET /search?object=org.openlore.philosophy.reproducible-builds and Rachel's claim appears
Then Rachel's row shows a neutral "Following" indicator
And Rachel's row shows NO "openlore peer add" command
```

### Scenario: A genuinely-unfollowed author keeps the render-only follow affordance

```
Given Maria does NOT follow did:plc:priya-test
And a reachable indexer holds a verified claim by did:plc:priya-test
When she opens GET /search and Priya's claim appears
Then Priya's row shows the render-only command "openlore peer add did:plc:priya-test"
And the command is plain TEXT — no button, no form, no mutating link
```

### Scenario: All-followed results show "Following" everywhere, no add command anywhere

```
Given Maria follows both did:plc:rachel-test and did:plc:tobias-test
And the indexer holds claims only by those two
When she opens GET /search
Then every row shows "Following"
And no "openlore peer add" command appears anywhere in the results
```

### Scenario: None-followed results preserve the slice-08 status quo (no over-correction)

```
Given Maria follows nobody who appears in the results
And the indexer holds claims by did:plc:priya-test and did:plc:bjorn-test
When she opens GET /search
Then both rows show the render-only "openlore peer add <did>" guidance
And this is exactly the slice-08 behavior, unchanged
```

## Theme B — Read-only / no write (CARDINAL)

### Scenario: Neither affordance is an executable control

```
Given any /search result render (followed or unfollowed authors)
When the results render
Then the "Following" indicator is a render-only label (no button, no form, no hx-* control)
And the "openlore peer add <did>" guidance is render-only TEXT (no button, no form, no mutating link)
And the viewer process holds no signing key and exposes no follow/unfollow route
```

## Theme C — LOCAL / offline relationship resolution (per-user-neutral index)

### Scenario: The relationship is resolved against the LOCAL active set, not the network

```
Given Maria actively follows did:plc:rachel-test
When she opens GET /search and a claim by Rachel appears
Then the row resolves to "subscribed peer" using the LOCAL active-subscription set
And no network call is made to resolve the relationship (the index stays per-user-neutral)
```

### Scenario: A followed author is matched despite the signing-key fragment on the result DID

```
Given Maria actively follows the bare DID did:plc:rachel-test
And the search result row's author DID is did:plc:rachel-test#org.openlore.application
When she opens GET /search and that row appears
Then the row shows the "Following" indicator (the fragment is stripped before the match)
```

## Theme D — One batch read, no N+1

### Scenario: The active set is read once per render, invariant to result count

```
Given a reachable indexer returns many result rows for a search
When Maria opens GET /search
Then the operator's active-subscription set is read exactly ONCE for the whole render
And the read count is invariant to the number of result rows (no per-result subscription query)
```

## Theme E — Graceful degradation

### Scenario: A failed active-set read degrades to the slice-08 status quo without crashing

```
Given the operator's LOCAL active-subscription read fails during a search render
When Maria opens GET /search with a reachable indexer
Then every result shows the "openlore peer add <did>" guidance (the slice-08 status quo)
And the search results still render with no crash, blank region, or leaked error
```

## Theme F — htmx vs no-JS parity

### Scenario: The follow-state renders identically under htmx and no-JS

```
Given Maria follows did:plc:rachel-test and the indexer holds a claim by Rachel
When she requests GET /search WITH HX-Request and again WITHOUT it
Then the htmx response is the #search-results fragment with Rachel's "Following" indicator and no add command
And the no-JS response is the full page = chrome + the SAME fragment, rendered identically
```

## Theme G — Attribution + ranking unchanged vs slice-08 (anti-merging)

### Scenario: Following and unfollowed authors render side by side, attribution + order preserved

```
Given Maria follows did:plc:rachel-test but not did:plc:priya-test
And the indexer holds verified claims by both
When she opens GET /search and both claims appear
Then Rachel's row shows "Following" with no add command
And Priya's row shows "openlore peer add did:plc:priya-test"
And the two rows are still attributed to their own authors with no merged or re-ranked output
And each row's [verified] marker and verbatim confidence are unchanged from slice-08
```

---

## Coverage map (scenario → story / AC)

| Theme | Scenario | Story | AC |
|---|---|---|---|
| A | followed → "Following", no add | US-SF-002 | FR-SF-4, C-2 |
| A | unfollowed → keeps `peer add` | US-SF-002 | FR-SF-5, C-2 |
| A | all-followed | US-SF-002 | FR-SF-4 |
| A | none-followed (status quo) | US-SF-002 | FR-SF-5 |
| B | no executable control | US-SF-002 | NFR-SF-1, C-1 |
| C | LOCAL resolution, no network | US-SF-001 | NFR-SF-4, C-3 |
| C | fragment-strip match | US-SF-001 | FR-SF-3, R-SF-5 |
| D | one batch read, no N+1 | US-SF-001 | NFR-SF-3, C-4 |
| E | failed read degrades | US-SF-001 | FR-SF-7, NFR-SF-6, C-7 |
| F | htmx vs no-JS parity | US-SF-002 | FR-SF-6, NFR-SF-7, C-8 |
| G | attribution + ranking unchanged | US-SF-002 | NFR-SF-5, C-5 |
