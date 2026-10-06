Feature: Live public network search on the OpenLore PDS host
  As Maria, I want openlore search to reach a live public index,
  and as Jeff, I want to run that index cheaply and safely next to my PDS.

  Background:
    Given the indexer is deployed on the PDS host at https://index.openlore.jeffbailey.us
    And the DID list holds did:plc:priyaraman7x2k, did:plc:dvolkov3m9q and did:plc:jbailey5q8n

  Scenario: Maria finds claims from authors on different PDSes through the public index
    Given the public index holds Priya's claim on github:priyaraman/cargo-pin and Dmitri's claim on github:dvolkov/nix-lockcheck
    And Maria has set OPENLORE_INDEXER_URL to https://index.openlore.jeffbailey.us
    When Maria runs openlore search --object org.openlore.philosophy.reproducible-builds
    Then Maria sees both claims, each attributed to its own author DID

  Scenario: The public index offers search only
    When anyone sends a write request or requests any path other than the search method
    Then the request is refused
    And the index contents are unchanged

  Scenario: A newly approved claim becomes searchable within half an hour
    Given Priya approved a claim on github:priyaraman/cargo-pin at 14:02
    When the scheduled passes run
    Then Maria finds that claim with openlore search no later than 14:32

  Scenario: Search keeps answering while a pass runs
    Given a pass is writing to the index
    When Maria searches
    Then she gets results within 1 second and no error

  Scenario: Passes never overlap
    Given a pass is still running when the next one is due
    When the timer fires
    Then no second concurrent pass starts

  Scenario: A DID added to the list is indexed on the next pass without a redeploy
    Given Jeff adds did:plc:therrera2v6w to the DID list parameter at 15:05
    When the 15:15 pass runs
    Then the pass reports 4 configured DIDs
    And no deploy or restart happened

  Scenario: A malformed list refuses the pass and keeps search serving
    Given Jeff saves the list "did:plc:therrera2v6w,tomas"
    When the next pass runs
    Then the pass exits 2 with a message naming OPENLORE_INDEXER_REPO_DIDS and "tomas"
    And Jeff receives an alarm email
    And search keeps returning the claims indexed before the edit

  Scenario: Two consecutive total outages alert the operator once
    Given every DID is unreachable
    When two consecutive passes exit 3
    Then Jeff receives one alarm email

  Scenario: Partial skips never alert
    Given pds.volkov.dev is down all day
    When every pass exits 0 with one DID skipped
    Then no alarm email is sent

  Scenario: The operator rolls back on command
    Given digest B is running
    When Jeff runs the rollback command
    Then digest A is running and search answers, with no data restore
