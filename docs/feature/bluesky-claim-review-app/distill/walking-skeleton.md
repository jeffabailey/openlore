# Walking Skeleton — bluesky-claim-review-app

- **Wave**: DISTILL · **Date**: 2026-10-04 · **Designer**: Quinn (nw-acceptance-designer)
- **Stories**: US-BRA-000..004 (+ the reader half of AC-004.3 / US-BRA-009) · **Job**: J-009
- **Executable SSOT**: `tests/acceptance/review_app_walking_skeleton.rs` (WS-1..WS-4)
- **Status**: all four NOT `#[ignore]`; RED at hand-off (`red-classification.md`); the first scenarios DELIVER turns green, in order.

## The thread (four chained scenarios — Pillar 2)

```gherkin
@walking_skeleton @driving_port @driving_adapter @real-io
Feature: A Bluesky developer publishes one consented, self-attested claim into their own PDS

  Scenario: WS-1 Priya signs in with her Bluesky handle and Bluesky recognises the OpenLore review app
    Given Priya's handle "priyaraman.bsky.social" belongs to did:plc:7x3kq2mzv5rj4w6hbn2tqclp on bsky.social
    When she signs in with Bluesky and authorizes the app at her PDS
    Then she sees "Signed in as @priyaraman.bsky.social"
    And her PDS identified the app by its published client details as "OpenLore review"
    And she was asked only to let the app create claims and posts, and nothing was written

  Scenario: WS-2 Priya proves github.com/priyaraman is hers with her DID in her bio
    Given Priya is signed in                                   # = WS-1 Given + When
    And her GitHub bio reads "Rust, tide models. did:plc:7x3kq2mzv5rj4w6hbn2tqclp"
    When she verifies GitHub username "priyaraman"
    Then she sees "Verified: github.com/priyaraman belongs to @priyaraman.bsky.social"
    And only her public profile was read — none of her repos yet

  Scenario: WS-3 Priya sees private suggestions from her own repos
    Given Priya has verified github.com/priyaraman             # = WS-2 Given + When
    When the scan finishes
    Then she sees her 5 suggestions, incl. "priyaraman/tidepool embodies dependency-pinning" at 0.25 (speculative) with the Cargo.lock evidence link
    And the page states "Private: only you can see these"
    And her ownership was re-checked immediately before any repo was read, and nothing was written

  Scenario: WS-4 Priya approves a suggestion and it lands in her own PDS, accepted by OpenLore as self-attested
    Given Priya has the pending suggestion "priyaraman/tidepool embodies dependency-pinning"   # = WS-3 Given + When
    When she previews it ("not as truth") and confirms "Publish to my repo"
    Then her PDS holds the org.openlore.claim (confidence 2500, bare-DID author, unsigned) whose recomputed CID is its record key
    And exactly one write happened and the card left her queue
    And she sees the record's at:// address and the retract path
    And when Maria adds Priya as a peer and pulls, OpenLore shows the claim as "self-attested", never "unverified"
```

## Litmus (Mandate 5)

1. **User-goal titles** — sign in, prove my account, see my private suggestions, publish one into my own PDS.
2. **Given/When are user actions** — typing a handle, authorizing at her PDS, verifying, scanning, approving.
3. **Then are user observations** — what she sees, what her PDS holds, what a reader's OpenLore shows.
4. **Stakeholder-confirmable** — "a Bluesky developer with no CLI publishes a claim they consented to, into their own repo, and OpenLore accepts it": the J-009 promise end to end.

## End-to-end path (what is real, what is faked)

| Hop | Real | Faked (driven-external) |
|---|---|---|
| Browser → app | the REAL `openlore-review-app serve` (third composition root, ADR-072), real hyper router, real `review-app.duckdb` | — (a JS-less browser submits the real forms) |
| Handle → DID → PDS | real `adapter-atproto-did` resolve-identity | `FakeAtprotoNetwork` directory (PLC + `resolveHandle`) |
| OAuth (PAR, DPoP nonce, PKCE, `private_key_jwt`, token) | real `adapter-atproto-oauth` (atrium-oauth) | `FakeAtprotoNetwork` authorization server (fetches the app's client metadata like a real PDS) |
| Ownership + scan | real `adapter-github`, `review-domain` verdict + reconcile, shipped `scraper-domain` select/derive | `FakeGithubAccounts` (bio with DID, `tidepool` + `quill-docs` facts, a fork) |
| Publish | real Plan-value executor, create-only `UserRepoWritePort`, read-back via `adapter-atproto-ingest`, `claim-domain` CID | `FakeAtprotoNetwork` PDS (`createRecord` with granular scope) |
| Reader | the REAL `openlore` CLI `peer add` / `peer pull` / `graph query` (ADR-071 provenance verdict) | the same PDS double, via the existing peer-resolver seam |

## Why this thread

It carries the feature's three riskiest assumptions at once: (1) ATProto OAuth works for a
confidential client against the user's own PDS (SPIKE-2); (2) a PDS accepts and serves an
`org.openlore.claim` without an app signature, byte-stable so CID == rkey (SPIKE-1); (3) OpenLore's
existing read path can accept that record as **self-attested** rather than rejecting it (D-5,
ADR-071) — today it skips the peer ("no usable verification key"), which is exactly the RED this
thread turns green.

## DELIVER order

WS-1 needs the crate bootstrap (DWD-3), resolve-identity, the OAuth adapter, sessions and the
landing/callback pages. WS-2 adds `/github` + the ownership verdict. WS-3 adds the scan task +
reconcile + the queue page. WS-4 adds the Plan-value publish + read-back AND the ADR-071 reader
change in `adapter-atproto-pds` / `cli peer pull` / render (the AC-004.3 half).
