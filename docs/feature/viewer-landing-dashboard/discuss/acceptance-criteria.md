# Acceptance Criteria (BDD): viewer-landing-dashboard (slice-17)

> Job: **J-002** (orientation facet) · Driving surface: the real `openlore ui` subprocess,
> route `GET /`.
> Every scenario is port-to-port (no direct call to a read method or `viewer-domain`).
> Data uses REAL names (Maria) and REAL DIDs (`did:plc:rachel-test`, `did:plc:tobias-test`).
> Derived from `user-stories.md` UAT + the brief's required AC themes.

## Theme 1 — The landing shows the LOCAL store summary (own claims, peer claims, active peers) (US-LD-001)

```gherkin
Scenario: The front door shows the LOCAL store summary
  Given Maria's store has 12 own claims, 7 peer claims, and 2 active peer subscriptions (did:plc:rachel-test, did:plc:tobias-test)
  When she opens GET / in the openlore ui viewer
  Then she sees a store summary showing 12 own claims, 7 peer claims, and 2 active peers
  And she sees the read-only notice telling her nothing here can change her store

Scenario: A fresh empty store shows honest zero counts
  Given Maria has a fresh store with 0 own claims, 0 peer claims, and 0 active subscriptions
  When she opens GET /
  Then the summary shows 0 own claims, 0 peer claims, and 0 active peers
  And each zero is a successful read of zero, not a missing-number state
```

## Theme 2 — The landing links to all the surfaces (discoverability) (US-LD-001 / C-3)

```gherkin
Scenario: The front door links to every shipped surface
  Given Maria opens GET /
  When she looks at the navigation hub
  Then she sees a link to My Claims (/claims), Peer Claims (/peer-claims), Project Survey (/project), Philosophy Survey (/philosophy), Contributor Score (/score), Network Search (/search), Live Scrape (/scrape), and Peer Subscriptions (/peers)
  And each link navigates to that surface with no JavaScript required (a plain link)
  And no deep or parameterized route (/claims/{cid}, /score?contributor, /project?subject) is a top-level link

Scenario: Each surface link uses the route's single-source-of-truth URL constant
  Given the viewer renders the navigation hub on GET /
  Then each link's href equals the route's URL constant from viewer-domain (MY_CLAIMS_URL, PEER_CLAIMS_URL, PROJECT_URL, PHILOSOPHY_URL, SCORE_URL, SEARCH_URL, PEERS_URL, the /scrape path)
  And no link is a hardcoded path literal that could drift from its route
```

## Theme 3 — The read-only / no-write invariant (US-LD-001 / C-1, CARDINAL)

```gherkin
Scenario: The front door exposes no write, compose, sign, subscribe, or follow control
  Given Maria opens GET /
  When she inspects the rendered page
  Then it contains no form, no button, and no control to compose, sign, subscribe, or follow
  And every navigation affordance is a plain link, not a mutating control
  And the viewer process holds no signing key

@property
Scenario: The viewer cannot mutate the store through GET /
  Given the viewer serves GET / over a StoreReadPort that declares no mutation method
  Then no request to GET / can compose, sign, subscribe, unsubscribe, or otherwise change the store
  And the store contents are identical before and after any number of GET / requests
```

## Theme 4 — Graceful degrade if a count read fails (US-LD-000/001 / C-2, C-7)

```gherkin
Scenario: A failed count read degrades to a missing-number state without a 5xx
  Given Maria's peer-claims count read fails transiently while the own-claims and active-peer reads succeed
  When she opens GET /
  Then the navigation hub renders in full
  And the own-claims count shows 12 and the active-peer count shows 2
  And the peer-claims number renders as a missing-number state (e.g. "—"), not a fabricated 0
  And the page is a normal 200, never a 5xx and never a raw stack trace

Scenario: A missing-number state is distinct from a successful zero
  Given Maria's store has 0 own claims (a successful read) and a peer-claims read that fails
  When she opens GET /
  Then the own-claims count shows 0 (an honest empty store)
  And the peer-claims count shows the missing-number state (couldn't read), not 0
```

## Theme 5 — LOCAL / offline (US-LD-000/001 / C-2)

```gherkin
Scenario: The front door renders fully with the network down
  Given Maria's store has claims and peers and the network is unavailable
  When she opens GET /
  Then the store summary and the full navigation hub render
  And no outbound network request is made by the route (no PDS fetch, no DID re-resolution, no peer pull, no network search)
  And the page references only the vendored local /static/htmx.min.js (no CDN)

@property
Scenario: The landing summary is a fixed set of aggregate reads invariant to store size
  Given Maria's store has N own claims, M peer claims, and K active peers for any N, M, K
  When she opens GET /
  Then the three counts are resolved in a FIXED set of aggregate reads
  And the read count does not grow with N, M, or K (no N+1, no per-claim or per-peer loop)
```

## Theme 6 — htmx-vs-no-JS parity (US-LD-001 / C-5)

```gherkin
Scenario: The landing renders identically under htmx and no-JS (if the shape is forked)
  Given Maria's store has claims and peers
  When she requests GET / WITH HX-Request and again WITHOUT it
  Then the no-JS response is the full page including the store summary + the navigation hub
  And the htmx response (if GET / forks by Shape) is the same summary + hub region, rendered identically
  And if GET / is full-page-only, both requests return the same full page

# Note: DESIGN confirms whether GET / forks by Shape (the landing is typically full-page).
# The PRODUCT contract is parity — the summary + hub never differ between the shapes.
```

## Theme 7 — Counts are aggregates, never merges (US-LD-001 / C-7, BR-LD-1)

```gherkin
Scenario: The store summary shows aggregate counts, never a merged "consensus" record
  Given Maria's store has peer claims from did:plc:rachel-test and did:plc:tobias-test
  When she opens GET /
  Then the summary shows a single peer-claims count (how many), an aggregate
  And it does NOT render any per-author content, score, or merged "consensus" claim on the front door
  And reading who-said-what is reached by navigating to the attributed surfaces (/peer-claims, /score)
```

## Theme 8 — US-LD-000 read wiring (infrastructure)

```gherkin
Scenario: The front door resolves three LOCAL aggregate counts using existing reads
  Given Maria's store has 12 own claims, 7 peer claims, and 2 active peer subscriptions
  When she opens GET / in the openlore ui viewer
  Then the landing summary shows 12 own claims, 7 peer claims, and 2 active peers
  And the own-claims count comes from count_claims, the peer-claims count from count_peer_claims
  And the active-peer count is the count of the active-only list_active_peer_subscriptions read (or a count-only variant)
  And no new orientation read method is invented

Scenario: A soft-removed peer is not counted in the active-peer summary
  Given Maria subscribed to did:plc:rachel-test then ran openlore peer remove did:plc:rachel-test (no --purge)
  And she still actively follows did:plc:tobias-test
  When she opens GET /
  Then the active-peer count is 1 (only the active subscription)
  And the soft-removed did:plc:rachel-test is not counted
```

## AC → Story → Theme traceability

| AC theme | Stories | Cardinal/guardrail |
|---|---|---|
| 1 LOCAL store summary (3 counts) | US-LD-001 | KPI-VIEW-1 front door |
| 2 links to all surfaces (discoverability) | US-LD-001 | C-3 navigation completeness |
| 3 read-only / no-write | US-LD-001 | C-1 (CARDINAL) |
| 4 graceful degrade on count failure | US-LD-000/001 | C-2 (CARDINAL), C-7, BR-LD-3 |
| 5 LOCAL / offline + fixed-reads | US-LD-000/001 | C-2 (CARDINAL), C-4 |
| 6 htmx-vs-no-JS parity | US-LD-001 | C-5 |
| 7 counts are aggregates, never merges | US-LD-001 | C-7, BR-LD-1 |
| 8 read wiring (existing reads, active-only) | US-LD-000 | infra, BR-LD-2 |
