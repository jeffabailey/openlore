# Platform: web (htmx, read-only operator dashboard on localhost)
# Slice: slice-06 — DELTA on slices 01/02/03
# Key heuristics: visibility of status, match real world, error prevention, help with errors
# Inherited invariants: I-SCR-1 (human-gate signing in CLI), slice-01 KPI-5 (local-first)
# Note: scenario titles describe operator outcomes, never implementation.

Feature: Node operator inspects their store and browses scrape proposals in a browser
  As an OpenLore node operator on localhost
  I want a read-only browser view of my persisted claims and live scrape proposals
  So I can see what my node holds and what I could add — without writing SQL and without
  any risk of writing, signing, or exposing my signing key

  Background:
    Given Maria Santos runs an OpenLore node on her own machine
    And her local store is at "~/.openlore/store.duckdb"
    And the viewer is strictly read-only and signing happens only in the CLI

  # ---------- Journey A: inspect my store (PRIMARY, Job 1, offline) ----------

  Scenario: Operator starts the read-only viewer on localhost
    When Maria runs "openlore viewer"
    Then the viewer reports it is listening on a localhost address
    And it states that the view is read-only
    And no signing key is loaded into the viewer process

  Scenario: Operator reads their persisted signed claims with zero SQL
    Given Maria has signed a claim ("rust-lang/rust", "is-maintained-by", "The Rust Project") with confidence 0.90
    And she has signed 312 claims in total
    When she opens the My Claims view in her browser
    Then she sees that claim rendered as a row showing subject, predicate, object, confidence 0.90, and its CID
    And the total count of 312 claims is shown
    And she did not write any SQL to see it

  Scenario: Operator inspects the full evidence behind one claim
    Given Maria's claim with CID "bafyrei...1" has two evidence URLs
    When she opens the detail view for CID "bafyrei...1"
    Then she sees the claim's subject, predicate, object, confidence, author, composed time, and CID
    And she sees both evidence URLs

  Scenario: Operator distinguishes federated peer claims from their own
    Given Maria has federated 1,840 peer claims from 4 peers
    When she opens the Peer Claims view
    Then she sees federated peer claims with their peer origin shown
    And they are presented separately from her own signed claims

  # ---------- Journey A: empty/error states (offline-capable) ----------

  Scenario: First-run operator with an empty store is guided, not dead-ended
    Given Maria has not signed any claims yet
    When she opens the My Claims view
    Then she sees a message explaining that signed claims will appear here
    And she is told that claims are signed from the CLI

  Scenario: Operator sees a helpful message when the store cannot be opened
    Given another process is holding "~/.openlore/store.duckdb"
    When Maria opens the My Claims view
    Then she sees a message that the store could not be opened, naming the path
    And she is asked whether another process is using it
    And she is not shown a raw stack trace

  @property
  Scenario: The store view works fully offline
    Given Maria's machine has no network connection
    When she opens the My Claims view and the Peer Claims view
    Then both views render her persisted claims from the local store
    And no network access is required to view her store

  @property
  Scenario: No web route can write to the store or sign a claim
    Given the viewer is running
    When any of its routes are requested
    Then none of them write to the store
    And none of them trigger the signing pipeline
    And the viewer process never holds the signing key

  # ---------- Journey B: browse scrape proposals (SECONDARY, Job 2, network) ----------

  Scenario: Operator opens the read-only live-scrape form
    Given the viewer is running
    When Maria opens the Live Scrape view
    Then she sees a target input and a Propose action
    And the page states that nothing is signed or saved
    And there is no sign control on the page

  Scenario: Operator browses live scrape proposals without signing anything
    Given a live scrape of "tokio-rs/tokio" would propose 7 candidate claims
    When Maria submits the target "tokio-rs/tokio"
    Then she sees 7 candidate claims, each showing subject, predicate, object, confidence, and derived-from provenance
    And the page states that none are signed or saved
    And no candidate is persisted to her store
    And she is directed to the CLI to sign any candidate

  Scenario: Provenance is shown only for live proposals, never for persisted claims
    Given Maria is viewing live scrape proposals for "tokio-rs/tokio"
    Then each proposed candidate shows its derived-from provenance
    When she then opens the My Claims view of her persisted claims
    Then no persisted claim shows a derived-from provenance
    # (derived-from is display-only and never persisted — WD-62)

  Scenario: Target that yields no candidates guides the operator
    Given a live scrape of "some-org/empty-repo" derives no candidate claims
    When Maria submits the target "some-org/empty-repo"
    Then she sees a message that no candidate claims could be derived
    And she is suggested an alternative such as checking license or manifest data

  Scenario: Network failure on live scrape clarifies that the store view still works offline
    Given Maria's machine cannot reach GitHub
    When she submits the target "tokio-rs/tokio" on the Live Scrape view
    Then she sees a message that GitHub could not be reached
    And she is told that her store view still works offline
