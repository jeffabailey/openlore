Feature: Index every author's claims from their own PDS
  So that Maria can discover self-attested claims from authors on any PDS,
  and Jeff can see exactly which repo DIDs were skipped and why.

  Background:
    Given the indexer is configured with the repo DIDs of Priya Raman, Dmitri Volkov and Jeff Bailey
    And Priya's DID document names https://morel.us-east.host.bsky.network
    And Dmitri's DID document names https://pds.volkov.dev

  @walking_skeleton
  Scenario: Maria finds Priya's self-attested claim from bsky.social
    Given Priya published a self-attested claim that github:priyaraman/cargo-pin embodies reproducible-builds
    When one ingest pass runs
    And Maria searches the network for reproducible-builds
    Then Maria sees Priya's claim attributed to did:plc:priyaraman7x2k

  Scenario: One PDS being down does not hide the others
    Given pds.volkov.dev returns 502
    When one ingest pass runs
    Then Priya's and Jeff's claims are indexed
    And Jeff sees did:plc:dvolkov3m9q skipped with reason pds_unreachable

  Scenario: An author who moved PDS is followed to the new one
    Given Priya's DID document now names https://pds.priyaraman.dev
    When the next ingest pass runs
    Then Priya's claims are read from pds.priyaraman.dev and still admitted as self-attested

  Scenario: Self-attested claims read through the fallback stay refused
    Given did:plc:ghost0000 cannot be resolved and a fallback source is configured
    When one ingest pass runs
    Then its app-signed claims are indexed and its self-attested claims are refused

  Scenario: Records of a different repo are never attributed to the requested author
    Given the PDS for did:plc:mallory4k1z answers with records whose repo is did:plc:priyaraman7x2k
    When one ingest pass runs
    Then none of those records are indexed under did:plc:mallory4k1z

  Scenario: A malformed configuration is explained at startup
    Given OPENLORE_INDEXER_REPO_DIDS contains the entry "priya"
    When Jeff starts the indexer
    Then it refuses to start, naming OPENLORE_INDEXER_REPO_DIDS and "priya"

  Scenario: Search labels self-attested claims
    Given Priya's self-attested claim is indexed
    When Maria searches the network for reproducible-builds
    Then Priya's claim is marked [self-attested] next to [verified]
