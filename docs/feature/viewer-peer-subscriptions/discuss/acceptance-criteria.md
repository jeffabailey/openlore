# Acceptance Criteria (BDD): viewer-peer-subscriptions (slice-15)

> Job: **J-003c** · Driving surface: the real `openlore ui` subprocess, route `GET /peers`.
> Every scenario is port-to-port (no direct call to the read method or `viewer-domain`).
> Confidence/data use REAL names (Maria, Rachel, Tobias) and REAL DIDs.
> Derived from `user-stories.md` UAT + the brief's required AC themes.

## Theme 1 — The subscription list renders DID + per-peer claim count + render-only revoke command (US-PS-002)

```gherkin
Scenario: The peers list shows each followed peer's DID, local claim count, and render-only revoke command
  Given Maria actively follows did:plc:rachel-test (5 cached claims) and did:plc:tobias-test (3 cached claims)
  When she opens GET /peers in the openlore ui viewer
  Then she sees two rows, one per peer
  And Rachel's row shows the DID did:plc:rachel-test, a local claim count of 5, and the render-only command "openlore peer remove did:plc:rachel-test"
  And Tobias's row shows the DID did:plc:tobias-test, a local claim count of 3, and the render-only command "openlore peer remove did:plc:tobias-test"
  And neither command is an executable control (no button, no form, no mutating link)

Scenario: A followed peer with zero cached claims still appears with its revoke command
  Given Maria subscribed to did:plc:newpeer-test but has never run openlore peer pull
  When she opens GET /peers
  Then did:plc:newpeer-test appears with a local claim count of 0
  And its render-only command "openlore peer remove did:plc:newpeer-test" is shown
```

## Theme 2 — Active-only: a removed peer is absent (residue made visible) (US-PS-002 / C-2)

```gherkin
Scenario: A peer removed via the CLI is absent from the peers list
  Given Maria actively follows did:plc:rachel-test and it appears on GET /peers
  When she runs "openlore peer remove did:plc:rachel-test" in her terminal and reopens GET /peers
  Then did:plc:rachel-test is absent from the list
  And its absence holds even though its cached peer claims remain on disk (no --purge)

Scenario: A soft-removed peer is excluded even though its cached claims exist
  Given Maria's only subscription to did:plc:rachel-test is soft-removed (removed_at set) with 5 cached claims retained
  And Maria still actively follows did:plc:tobias-test
  When she opens GET /peers
  Then the page lists only did:plc:tobias-test
  And did:plc:rachel-test is absent (active-only filter, removed_at IS NULL)
```

## Theme 3 — The read-only / no-write invariant (US-PS-002 / C-1, CARDINAL)

```gherkin
Scenario: The peers view exposes no write, subscribe, or unsubscribe control
  Given Maria actively follows one peer
  When she opens GET /peers
  Then the rendered page contains no form, no button, and no mutating link to subscribe, unsubscribe, remove, or purge
  And the only revocation affordance is the render-only command text "openlore peer remove <did>"
  And the viewer process holds no signing key

@property
Scenario: The viewer cannot mutate the subscription set through GET /peers
  Given the viewer serves GET /peers over a StoreReadPort that declares no mutation method
  Then no request to GET /peers can add, remove, soft-remove, or purge a subscription
  And the active subscription set is identical before and after any number of GET /peers requests
```

## Theme 4 — The guided empty state (US-PS-003)

```gherkin
Scenario: No active subscriptions shows the guided empty state
  Given Maria has no active peer subscriptions
  When she opens GET /peers
  Then she sees the message "You are not subscribed to any peers."
  And she sees the render-only starting command "openlore peer add <did>"
  And the page is not blank and is not an error

Scenario: A store with only soft-removed peers still shows the empty state
  Given Maria's only peer_subscriptions row is soft-removed (removed_at set) and she follows no one else
  When she opens GET /peers
  Then she sees the guided empty state (the soft-removed row is residue, not an active subscription)
```

## Theme 5 — LOCAL / offline (US-PS-001/002/003 / C-4)

```gherkin
Scenario: The peers view renders fully with the network down
  Given Maria actively follows did:plc:rachel-test and did:plc:tobias-test
  And the network is unavailable
  When she opens GET /peers
  Then the page renders the full peer list with per-peer counts and render-only commands
  And no outbound network request is made by the route (no PDS fetch, no DID re-resolution, no peer pull)
  And the page references only the vendored local /static/htmx.min.js (no CDN)

@property
Scenario: The peers read is a single aggregate query invariant to peer count
  Given Maria actively follows N peers for any N
  When she opens GET /peers
  Then the active subscription set and all per-peer counts are resolved in exactly ONE aggregate query
  And the query count does not grow with N (no N+1)
```

## Theme 6 — htmx-vs-no-JS parity (US-PS-002 / US-PS-003 / C-5)

```gherkin
Scenario: The peers list renders identically under htmx and no-JS
  Given Maria actively follows one peer
  When she requests GET /peers WITH HX-Request and again WITHOUT it
  Then the htmx response is the peers view-panel fragment with the peer row + its render-only revoke command
  And the no-JS response is the full page = chrome + the SAME fragment, rendered identically

Scenario: The empty state renders identically under htmx and no-JS
  Given Maria has no active peer subscriptions
  When she requests GET /peers WITH HX-Request and again WITHOUT it
  Then the htmx response is the peers fragment containing the empty state
  And the no-JS response is the full page = chrome + the SAME fragment, rendered identically
```

## Theme 7 — Anti-merging / per-peer counts (US-PS-002 / C-3, J-003a)

```gherkin
Scenario: The per-peer count is never a merged total
  Given Maria follows did:plc:rachel-test (5 cached claims) and did:plc:tobias-test (3 cached claims)
  When she opens GET /peers
  Then Rachel's row shows 5 and Tobias's row shows 3 (per-peer)
  And no row shows a combined total of 8 and there is no merged "all peers" row
  And each peer is its own attributed row keyed by its DID
```

## Theme 8 — US-PS-001 read capability (infrastructure)

```gherkin
Scenario: The peers read returns active subscriptions with per-peer counts in one aggregate query
  Given Maria's store has active subscriptions to did:plc:rachel-test (5 cached claims) and did:plc:tobias-test (3 cached claims)
  When she opens GET /peers in the openlore ui viewer
  Then the page lists exactly two peers
  And Rachel's row shows a local claim count of 5 and Tobias's row shows 3
  And the subscription set + counts are resolved in exactly ONE aggregate query

Scenario: No active subscriptions resolves to an empty result without error
  Given Maria has no active peer subscriptions
  When she opens GET /peers
  Then the peers read returns an empty result (not an error)
  And the viewer renders the guided empty state
```

## AC → Story → Theme traceability

| AC theme | Stories | Cardinal/guardrail |
|---|---|---|
| 1 list (DID + count + revoke command) | US-PS-002 | render-only command (C-7) |
| 2 active-only / residue made visible | US-PS-002 | C-2 (CARDINAL) |
| 3 read-only / no-write | US-PS-002 | C-1 (CARDINAL) |
| 4 guided empty state | US-PS-003 | — |
| 5 LOCAL / offline + single-query | US-PS-001/002/003 | C-4, C-8 |
| 6 htmx-vs-no-JS parity | US-PS-002/003 | C-5 |
| 7 anti-merging / per-peer | US-PS-002 | C-3 (J-003a) |
| 8 read capability | US-PS-001 | infra |
