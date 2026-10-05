# Walking skeleton — indexer-per-did-pds-fetch

> **Status: IMPLEMENTED**. WS-1 and WS-2 GREEN from step 01-01 (`5c39f33`); ignored at the DISTILL hand-off. Copied from `docs/feature/indexer-per-did-pds-fetch/distill/` at finalize; history in `docs/evolution/indexer-per-did-pds-fetch-evolution.md`.

## Thin slice (US-IPF-001)

There are two walking skeletons, both in `tests/acceptance/indexer_per_did_fetch.rs`.

**WS-1 `maria_finds_priyas_self_attested_claim_published_on_her_own_pds`** answers the user's
question: *can Maria find a claim that a bsky.social author approved in the review app?*

```gherkin
@walking_skeleton @driving_port @driving_adapter @real-io @US-IPF-001 @AC-001.1 @AC-001.2 @AC-001.3
Scenario: Maria finds Priya's self-attested claim published on her bsky.social PDS
  Given Priya's DID document names her bsky.social PDS and she published there
    that github:priyaraman/cargo-pin embodies reproducible-builds
  And Dmitri's DID document names pds.volkov.dev, where he published app-signed claims
  When one ingest pass runs
  And Maria searches the network for reproducible-builds
  Then Maria sees Priya's claim on cargo-pin attributed to did:plc:priyaraman7x2k
  And she sees Dmitri's claim on ferrite next to it, attributed to Dmitri
```

**WS-2 `authors_on_different_pdses_are_all_found_in_one_pass`** gives the operator's view of
the same pass:

- all 6 claims are indexed, each attributed to its author;
- Priya's claims are self-attested, and Dmitri's and Jeff's are app-signed;
- each repo was listed on its own PDS and nowhere else;
- the `pass_summary` reads 3 configured / 3 own-PDS / 0 fallback / 0 skipped, and the pass exits 0.

Litmus: the titles name user goals, the Givens describe authors publishing, and the Thens are
what Maria or Jeff see (search output, pass events). A stakeholder can confirm both.

## What is real and what is fake

| Port | Class | In the skeleton |
|---|---|---|
| `openlore-indexer ingest` / `serve` | driving | REAL binary (subprocess, `resolve_workspace_bin`) |
| `openlore search` | driving | REAL binary, Maria's initialized home |
| `index.duckdb` | driven internal | REAL file under the scenario home |
| PLC directory | driven external | `FakeAtprotoNetwork` directory (`GET /<did>`, `resolveHandle`) |
| Priya's, Dmitri's and Jeff's PDSes | driven external | one `FakeAtprotoNetwork` loopback server per host (`morel-us-east-host-bsky-network`, `pds-volkov-dev`, `pds-jeffbailey-us`) |
| App-signed author keys | driven external | the existing `OPENLORE_PEER_PUBKEY_HEX_<did>` seam (debug-only), set per app-signed author by the harness |

The loopback fakes need `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1`, which the harness sets (debug
build; DD-IPF-5).

## Why US-IPF-005 is separate

WS-1 asserts **attribution**, not the `[self-attested]` label. The label (ADR-079, the wire
`provenance` field) is milestone 05 (IPF-31..35). Until that lands, Maria's CLI shows Priya's
row as `[verified]`. ADR-071 §5 accepts this as "absent = app-signed", and WD-IPF-6 calls it a
known misstatement window, closed by US-IPF-005.

## Status at hand-off

Both skeletons are `#[ignore]`d. They were run with `--include-ignored` and fail for the right
reason (see `red-classification.md`): today's indexer refuses to start without a single source
URL, and with one it refuses Priya's own-PDS claim as `unverifiable provenance`. DELIVER
enables WS-1 first (step 01-01).
