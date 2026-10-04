# Platform: web (hosted) · Accessibility: WCAG 2.2 AA
# Job: J-009 · Persona: Priya Raman (@priyaraman.bsky.social, did:plc:7x3kq2mzv5rj4w6hbn2tqclp, GitHub priyaraman)
# Solution-neutral: describes observable outcomes; DESIGN chooses mechanisms.

Feature: Review and publish my claims from my Bluesky identity
  As a developer on Bluesky
  I want to approve, edit, or privately decline philosophy suggestions inferred from my GitHub work
  So that only what I consent to is published, self-attested, in my own PDS

  Background:
    Given the OpenLore review app is reachable at its public address

  # ---------- Walking skeleton ----------

  @walking_skeleton @US-BRA-001
  Scenario: Priya signs in with her Bluesky handle
    Given Priya's handle "priyaraman.bsky.social" resolves to did:plc:7x3kq2mzv5rj4w6hbn2tqclp
    When she signs in with Bluesky and authorizes the app at her PDS
    Then she sees "Signed in as @priyaraman.bsky.social"

  @walking_skeleton @US-BRA-002
  Scenario: Priya proves her GitHub account with her DID in the bio
    Given Priya is signed in
    And the bio of github.com/priyaraman contains "did:plc:7x3kq2mzv5rj4w6hbn2tqclp"
    When she verifies GitHub username "priyaraman"
    Then she sees "Verified: github.com/priyaraman belongs to @priyaraman.bsky.social"

  @walking_skeleton @US-BRA-003
  Scenario: Priya sees private suggestions from her own repos
    Given Priya has verified github.com/priyaraman
    And priyaraman/tidepool commits Cargo.lock
    When the scan finishes
    Then she sees a suggestion "priyaraman/tidepool embodies dependency-pinning" at confidence 0.25 with the Cargo.lock evidence link
    And the page states the suggestions are private to her

  @walking_skeleton @US-BRA-004
  Scenario: Priya approves a suggestion and it lands in her own PDS as self-attested
    Given Priya has the pending suggestion "priyaraman/tidepool embodies dependency-pinning"
    When she previews it and confirms "Publish to my repo"
    Then her PDS holds an org.openlore.claim for subject github:priyaraman/tidepool with confidence 2500
    And OpenLore's read/verify path accepts the claim and labels it self-attested

  # ---------- Privacy invariants ----------

  @property @I-BRA-1
  Scenario: Pending suggestions never leave the owner's private view
    Given Priya has 5 pending suggestions
    Then none of them is written to any PDS
    And none of them appears in a share post, on her profile page, in OpenLore search, or in any feed
    And Dmitri, signed in as @dmitri.volkov.dev, cannot see Priya's queue

  @property @I-BRA-2 @US-BRA-006
  Scenario: Declining writes nothing public
    Given Priya has the pending suggestion "priyaraman/quill-docs embodies documentation-first"
    When she chooses "Not me"
    Then no record of any kind is written to her PDS
    And the suggestion is not offered again after a rescan

  # ---------- Error paths ----------

  @US-BRA-002
  Scenario: Verification fails when the bio holds someone else's DID
    Given Sam is signed in as did:plc:q9rt5wz2b8kd3m1xv7pn4ahe
    And the bio of github.com/BurntSushi does not contain Sam's DID
    When Sam verifies GitHub username "BurntSushi"
    Then Sam sees that the bio does not contain his DID and how to fix it
    And no scan starts and no suggestions are created

  @US-BRA-004
  Scenario: A failed publish leaves the suggestion pending
    Given Priya confirms publishing a suggestion
    And her PDS is unreachable
    Then she sees "We couldn't reach your PDS. Nothing was published." with a retry
    And the suggestion is still pending

  # ---------- Release 1 ----------

  @US-BRA-008
  Scenario: Sharing on Bluesky only happens after preview and confirm
    Given Priya has 2 published claims and 3 pending suggestions
    When she opens "Share on Bluesky…"
    Then she sees a post preview that links to her profile and names only her 2 published philosophies
    And nothing is posted unless she presses "Post to Bluesky"
