# Acceptance Criteria: htmx-scraper-viewer (slice-06)

Given-When-Then, all testable, each mapped to a story (`user-stories.md`) and a
requirement (`requirements.md`). `@property` marks ongoing-quality criteria. Scenario
titles describe operator outcomes, never implementation.

---

## US-VIEW-001 — See my store in the browser (Walking Skeleton)

### AC-001.1: Operator sees persisted claims with zero SQL
Given Maria has signed 312 claims including ("rust-lang/rust","is-maintained-by","The Rust Project") at confidence 0.90
When she runs `openlore viewer` and opens the My Claims page
Then she sees that claim as a row with subject, predicate, object, confidence 0.90, and its CID
And she did not write any SQL
> Maps: FR-VIEW-2, FR-VIEW-8, NFR-VIEW-5

### AC-001.2: Viewer starts read-only on localhost with no key
Given Maria has a local store
When she runs `openlore viewer`
Then it reports a loopback listen URL and states the view is read-only
And no signing key is loaded into the process
> Maps: FR-VIEW-1, NFR-VIEW-2, NFR-VIEW-3, I-VIEW-2/3/4

### AC-001.3: Empty store guides a first-run operator
Given Tom has signed no claims
When he opens the My Claims page
Then he sees guidance that signed claims appear here and are created via the CLI
> Maps: FR-VIEW-7, NFR-VIEW-6

### AC-001.4: Unreadable store shows a helpful error
Given the store file is locked by another process
When Maria opens the My Claims page
Then she sees a plain-language message naming the path and asking if another process uses it
And no raw stack trace is shown
> Maps: NFR-VIEW-6

### AC-001.5 @property: Store view renders offline
Given Maria's machine has no network
When she opens the My Claims page
Then it renders her persisted claims with no network access
> Maps: NFR-VIEW-4, I-VIEW-6

### AC-001.6 @property: No route writes or signs
Given the viewer is running
When any route is requested
Then no route writes to the store, triggers signing, or holds the signing key
> Maps: NFR-VIEW-1, NFR-VIEW-2, I-VIEW-1/2/3

---

## US-VIEW-002 — Inspect one claim's full evidence

### AC-002.1: Full evidence on detail page
Given Maria's claim with CID bafyrei...1 has two evidence URLs
When she opens its detail page
Then she sees all claim fields and both evidence URLs
> Maps: FR-VIEW-3

### AC-002.2: No-evidence claim shown clearly
Given Maria has a claim signed without evidence
When she opens its detail page
Then she sees "no evidence attached" rather than a blank section
> Maps: FR-VIEW-3, NFR-VIEW-6

### AC-002.3: Unknown CID guides back
Given no claim with CID bafyrei...zzz exists in the store
When Maria opens that detail page
Then she sees "No claim with that identifier in your store" and a link back to the list
> Maps: FR-VIEW-3, NFR-VIEW-6

### AC-002.4 @property: Detail renders offline
Given no network is available
When Maria opens a claim detail page
Then it renders from the local store
> Maps: NFR-VIEW-4

---

## US-VIEW-003 — Distinguish federated peer claims from my own

### AC-003.1: Peer claims shown with origin, distinct from own
Given Maria has federated 1,840 peer claims from 4 peers
When she opens the Peer Claims view
Then she sees federated claims with peer origin, separate from her own claims
> Maps: FR-VIEW-4, BR-VIEW-5

### AC-003.2: No-peers state is guided
Given Maria has federated no peer claims
When she opens the Peer Claims view
Then she sees "No federated claims yet" guidance
> Maps: FR-VIEW-7

### AC-003.3: Unknown origin still renders
Given a federated peer claim has no recorded origin
When Maria opens the Peer Claims view
Then that claim renders with origin shown as "unknown"
> Maps: FR-VIEW-4, NFR-VIEW-6

### AC-003.4 @property: Peer view renders offline
Given no network is available
When Maria opens the Peer Claims view
Then it renders from the local store
> Maps: NFR-VIEW-4

---

## US-VIEW-004 — Navigate a large store with pagination

### AC-004.1: Page through a large store
Given Maria has 312 signed claims and a page size of 50
When she opens My Claims and clicks Next
Then she sees claims 51–100 of 312 with a position indicator
> Maps: FR-VIEW-6

### AC-004.2: Last page is bounded
Given Maria is on the last page of 312 claims
Then she sees 301–312 of 312 and no further Next action
> Maps: FR-VIEW-6

### AC-004.3: Small store needs no controls
Given Maria has 12 claims and a page size of 50
When she opens My Claims
Then all 12 render and no pagination controls are shown
> Maps: FR-VIEW-6

### AC-004.4 @property: First page renders < 10s at scale
Given a store of any size
When Maria opens My Claims
Then the first page renders within 10 seconds
> Maps: NFR-VIEW-5

---

## US-VIEW-005 — Browse live scrape proposals before signing in the CLI

### AC-005.1: Browse proposals, nothing signed or saved
Given a live scrape of "tokio-rs/tokio" would propose 7 candidate claims
When Maria submits that target on the Live Scrape view
Then she sees 7 candidates with subject, predicate, object, confidence, and derived-from
And the page states none are signed or saved
And no candidate is persisted
And she is directed to the CLI to sign any candidate
> Maps: FR-VIEW-5, BR-VIEW-1, BR-VIEW-2

### AC-005.2: Provenance only on live proposals
Given Maria is viewing live proposals showing derived-from
When she opens her persisted My Claims view
Then no persisted claim shows derived-from
> Maps: BR-VIEW-3, WD-62, I-VIEW-5

### AC-005.3: No candidates guides the operator
Given a live scrape of "some-org/empty-repo" derives no candidates
When Maria submits that target
Then she sees "No candidate claims could be derived" with a suggested alternative
> Maps: FR-VIEW-7, NFR-VIEW-6

### AC-005.4: Network failure clarifies offline store
Given Maria cannot reach GitHub
When she submits "tokio-rs/tokio" on the Live Scrape view
Then she sees that GitHub could not be reached
And she is told her store view still works offline
> Maps: NFR-VIEW-7

### AC-005.5 @property: No sign control or key on the live-scrape surface
Given the Live Scrape view is rendered
Then it presents no sign control
And the process holds no signing key
> Maps: BR-VIEW-1, NFR-VIEW-2, I-VIEW-2/3

---

## Coverage matrix

| Story | ACs | Happy | Edge | Error | @property |
|-------|-----|-------|------|-------|-----------|
| US-VIEW-001 | .1–.6 | .1,.2 | .3 | .4 | .5,.6 |
| US-VIEW-002 | .1–.4 | .1 | .2 | .3 | .4 |
| US-VIEW-003 | .1–.4 | .1 | .3 | .2 | .4 |
| US-VIEW-004 | .1–.4 | .1 | .3 | .2(bound) | .4 |
| US-VIEW-005 | .1–.5 | .1 | .3 | .4 | .2,.5 |

Every story has happy + edge + error coverage and at least one property/guardrail AC.
