# DELTA on slice-06 (htmx-scraper-viewer). htmx partial-swaps as a progressive enhancement
# over the SAME read-only routes. The HX-Request header selects fragment vs full page; the
# HTTP surface (URLs, methods) is unchanged. No-JS / offline / curl / bookmark always get the
# slice-06 full page. htmx is served locally (never a CDN) so the dashboard works offline.
#
# These scenarios are driven by SENDING or WITHHOLDING the HX-Request header against the real
# `openlore ui` process — the same harness convention the slice-06 ViewerServer suite uses.

Feature: Operator navigates the viewer without full-page reloads
  As Maria Santos, a node operator browsing her own node at http://127.0.0.1:8788
  I want paging, scraping, opening a claim, and switching tabs to update in place
  So that the page holds still (no reload, no scroll jump, no flash)
  While a no-JS / offline / direct-URL operator still gets the exact slice-06 full page

  Background:
    Given the read-only viewer `openlore ui` is running on 127.0.0.1
    And it holds no signing key and exposes no write or sign route

  # ---- Step 1: Pagination (WALKING SKELETON) ----

  Scenario: Paging the claims list updates only the table, in place
    Given Maria has 312 signed claims rendered 50 per page on the My Claims view
    And she is viewing page 1 with her scroll position partway down
    When her browser requests page 2 as an htmx request with the HX-Request header
    Then the response is just the claims-table fragment showing 51–100 of 312
    And the page chrome, navigation, and scroll position are unchanged
    And no full-page reload occurs

  Scenario: Paging the peer-claims list updates only the table, in place
    Given Maria has 1,840 federated peer claims rendered 50 per page on the Peer Claims view
    When her browser requests the next page as an htmx request with the HX-Request header
    Then the response is just the peer-claims-table fragment with the next 50 rows and their origin
    And the page chrome and navigation are unchanged

  Scenario: The claims page works as a full page without JavaScript
    Given JavaScript is disabled in Maria's browser
    When she clicks Next on the My Claims view as a plain link to /claims?page=2
    Then the server returns the complete /claims?page=2 page
    And that page is byte-equivalent to the slice-06 full page

  Scenario: An over-the-end page clamps to the last page in both shapes
    Given Maria has 312 claims at page size 50 (last page is 301–312 of 312)
    When a request asks for a page beyond the last page
    Then the response shows the last page 301–312 of 312, not a blank result
    And this holds for both the htmx fragment and the full page

  # ---- Step 2: Live scrape form swap ----

  Scenario: Submitting a scrape target swaps in the proposals without reloading
    Given Maria is on the Live Scrape view with network available
    And a live scrape of "tokio-rs/tokio" would propose 7 candidate claims
    When she submits the target as an htmx request with the HX-Request header
    Then only the results region updates to show the 7 candidates with their derived-from
    And the form and its target value remain in place
    And the results state that nothing is signed or saved and direct her to the CLI
    And no candidate is persisted
    And no sign control is rendered

  Scenario: A target that yields no candidates swaps in guidance, no reload
    Given a live scrape of "some-org/empty-repo" derives no candidates
    When Maria submits that target as an htmx request
    Then the results region shows "No candidate claims could be derived" with a suggestion

  Scenario: Network failure swaps in guidance that the store view still works offline
    Given Maria cannot reach GitHub
    When she submits "tokio-rs/tokio" as an htmx request
    Then the results region shows that GitHub could not be reached
    And it states her store view still works offline

  Scenario: Scrape submit works as a full page without JavaScript
    Given JavaScript is disabled
    When Maria submits the scrape form as a plain POST /scrape
    Then the server returns the complete /scrape page with the candidates below the form

  # ---- Step 3: Claim detail inline ----

  Scenario: Opening a claim loads its detail inline without leaving the list
    Given Maria is on the My Claims view and her claim bafyrei...1 has two evidence URLs
    When she opens that claim as an htmx request with the HX-Request header
    Then the detail region updates to show all claim fields and both evidence URLs
    And the claims list remains in place
    And the confidence is shown verbatim as 0.90

  Scenario: Opening a claim works as a full page without JavaScript
    Given JavaScript is disabled
    When Maria clicks a claim row as a plain link to /claims/bafyrei...1
    Then the server returns the complete claim detail page from slice-06

  Scenario: An unknown claim id guides the operator in both shapes
    Given no claim with CID bafyrei...zzz exists in the store
    When Maria opens that claim as an htmx request or as a full page
    Then she sees "No claim with that identifier in your store" with a link back to the list

  # ---- Step 4: Tab switch ----

  Scenario: Switching to Peer Claims swaps the view panel in place
    Given Maria is on the My Claims view with 1,840 federated peer claims from 4 peers
    When she switches to Peer Claims as an htmx request with the HX-Request header
    Then only the view panel updates to the Peer Claims list showing each row's origin
    And the peer rows are separable from her own claims
    And the browser URL reflects /peer-claims so the view is bookmarkable and Back works

  Scenario: The tabs work as full-page navigation without JavaScript
    Given JavaScript is disabled
    When Maria clicks the Peer Claims tab as a plain link to /peer-claims
    Then the server returns the complete /peer-claims page from slice-06

  # ---- Step 5: htmx asset served locally (offline-first) ----

  Scenario: htmx-powered swaps keep working with no network
    Given Maria's machine has no network access
    When she loads any viewer route and triggers a swap (page, open a claim, switch tabs)
    Then the htmx library loads from the viewer process itself, not a CDN
    And every store view and every swap still works fully offline

  @property
  Scenario: No viewer page references an external CDN for htmx
    Given the viewer is serving any route
    Then no served page references an off-host URL to load the htmx library

  # ---- Cross-cutting guardrails (read-only + no-regression) ----

  @property
  Scenario: Every non-htmx response is a complete page byte-equivalent to slice-06
    Given a request without the HX-Request header (curl, bookmark, no-JS, view-source)
    When it hits any viewer route
    Then the response is the complete slice-06 full page for that route

  @property
  Scenario: htmx swaps add no write or sign surface
    Given the viewer serves the htmx-enhanced routes
    Then no swap introduces a write or sign route
    And the web process still holds no signing key

  @property
  Scenario: The slice-06 acceptance suite stays green
    Given the slice-06 viewer acceptance corpus (26 scenarios)
    When the htmx enhancement is layered on
    Then all 26 slice-06 scenarios still pass with zero regression
