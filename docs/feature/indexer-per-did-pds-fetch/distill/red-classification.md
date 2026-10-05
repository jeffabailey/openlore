# RED classification — indexer-per-did-pds-fetch (pre-DELIVER gate)

This was run on 2026-10-05 against `main` (feature unimplemented), with every new scenario
included:

```
cargo test -p cli --test indexer_per_did_fetch      -- --include-ignored
cargo test -p cli --test indexer_per_did_resilience -- --include-ignored
cargo test -p cli --test indexer_per_did_config     -- --include-ignored
cargo test -p cli --test search_self_attested_label -- --include-ignored
cargo test -p cli --test indexer_pass_core          -- --include-ignored
cargo test -p xtask --test indexer_per_did_architecture -- --include-ignored
```

Build and lint status:

- All 6 targets **compile**, so nothing is BROKEN by construction.
- `cargo clippy -D warnings` is clean on the 5 cli targets, on `openlore-test-support
  --all-targets` and on the xtask test. New files are rustfmt-formatted.
- The fake's own contract is **GREEN**: 10/10 `fake_atproto` self-tests, including the 3 new
  ones for multi-host DID documents and moves, DID-document postures, listing postures, the
  request log and in-flight peak.
- Every subprocess run finished in under a second, so no scenario hangs today.

In every subprocess scenario the GIVEN world is fully up before the indexer runs: the fake
network is bound, records are seeded and Maria's home is initialized. The failure is the
indexer's (or CLI's) observable behaviour.

| Scenario(s) | Failing step | Observed cause | Class |
|---|---|---|---|
| **WS-1** | THEN the pass exits 0 | exit 2: `health.startup.refused{adapter:"ingest_source", detail:"ingest source URL is empty — cannot PULL listRecords"}`. Today the indexer needs ONE source URL and cannot read authors from their own PDSes | MISSING_FUNCTIONALITY ✅ (DD-IPF-8 probe + ADR-077 per-DID listing absent) |
| **WS-2**, IPF-01, 02, 03, 10, 12, 14..19, 25, 26 | THEN `pass_summary` | same startup refusal (no fallback configured); no `pass_summary` | MISSING_FUNCTIONALITY ✅ |
| IPF-05 | THEN `pass_summary` | ingest runs against the single source and exits 0; Priya's own claim is refused as relay origin | MISSING_FUNCTIONALITY ✅ |
| IPF-07 | THEN `pass_summary` | exit 2: `listing did:plc:priyaraman7x2k failed: … listRecords returned HTTP 503`. One failing source aborts the whole pass (the WD-IPF-3 brownfield gap) | MISSING_FUNCTIONALITY ✅ |
| IPF-08 | THEN `pass_summary` | exit 2: `listing did:plc:ghost0000 failed: … transport error … https://localhost:<port>`. Today's indexer **dials** the loopback-only fallback (there is no SSRF guard) | MISSING_FUNCTIONALITY ✅ |
| **IPF-04** (thin-slice evidence) | THEN `pass_summary` | Ingest **runs** (fallback = Jeff's PDS acts as today's single source). Ghost's 2 app-signed claims are verified and indexed, so the fake and the pubkey seam are proven. Stderr shows `refused at://did:plc:priyaraman7x2k/…: unverifiable provenance (not fetched from the author's own PDS)`: **the exact brownfield bug** this feature fixes | MISSING_FUNCTIONALITY ✅ |
| IPF-06, 09, 27, 28 | THEN `pass_summary` | ingest runs against the single source and exits 0 with only `verified`/`rejected`; no `pass_summary` / `source_fallback` | MISSING_FUNCTIONALITY ✅ |
| IPF-11, IPF-13 | first table row, THEN exit 0 | exit 2, the same startup refusal | MISSING_FUNCTIONALITY ✅ |
| IPF-20 | THEN `pass_summary` | same startup refusal | MISSING_FUNCTIONALITY ✅ |
| IPF-21, IPF-23 | THEN refusal names `config` | refusal names `adapter:"ingest_source"`, not `config` / `IndexerConfigInvalid` | MISSING_FUNCTIONALITY ✅ |
| IPF-22 | THEN `health.startup.refused` | `pds.jeffbailey.us` is accepted. The pass starts and fails with `listing … failed: PDS URL is not a URL` (exit 2, no refusal event) | MISSING_FUNCTIONALITY ✅ |
| IPF-24 | THEN `pass_summary` / `config.loaded` | startup refusal; no `indexer.config.loaded` | MISSING_FUNCTIONALITY ✅ |
| IPF-29 | THEN exit 2 | exit 0: a plain-http loopback fallback is accepted without the seam | MISSING_FUNCTIONALITY ✅ |
| IPF-30 (CI release guard) | THEN refusal names the seam | the existing `target/release/openlore-indexer` refuses on `ingest_source`, and `structured.variable` is null | MISSING_FUNCTIONALITY ✅ (run locally against the pre-existing release binary) |
| IPF-31, 32, 33 | GIVEN pass (chained) | the chained Given pass fails at `pass_summary` (startup refusal) | MISSING_FUNCTIONALITY ✅ |
| IPF-35 | THEN only the known-provenance row | the CLI shows the `peer-vouched` row as `[verified]`, so an unknown provenance is misstated | MISSING_FUNCTIONALITY ✅ |
| **IPF-34** | — | **passes today** | GREEN-today regression guard (intended: an old server's output must stay byte-identical after ADR-079; the CLI already ignores an unknown `provenance` key) |
| CORE-1..14 | the `sut_*` binding | `todo!("SCAFFOLD: bind …")` panic | MISSING_FUNCTIONALITY ✅ (RED scaffold, Mandate 7) |
| XA-1 | THEN no `RecordOrigin::of` | `run.rs:539 RecordOrigin::of(fetched_from, &identity.pds_endpoint)` | MISSING_FUNCTIONALITY ✅ |
| XA-2 | THEN no unguarded constructors | `run.rs:160,171 AtProtoIngestAdapter::new`, `run.rs:172 IdentityLookup::new` | MISSING_FUNCTIONALITY ✅ |
| XA-3 | THEN rules registered | `check_arch.rs` has neither rule name | MISSING_FUNCTIONALITY ✅ |
| **XA-4** | — | passes today | GREEN-today non-vacuity guard for XA-1/XA-2 (intended) |

**Result: 0 BROKEN · 0 WRONG_ASSERTION · 0 OBSERVABLE_NOT_AT_PORT. The gate passes.**
At hand-off **every** new scenario is `#[ignore]`d. The default run shows 0 executed and 55
ignored, plus each suite's 2 pre-existing `state_delta` self-tests, so `main` stays GREEN.

## Universe check (Mandate 8)

Every assertion reads a port-exposed observable:

- the indexer's exit code and its stdout/stderr JSON events (`source_skipped`,
  `source_fallback`, `pass_summary`, `config.loaded`, `verified`, `rejected.by_reason`,
  `health.startup.refused`);
- `index.duckdb` `indexed_claims` rows (author, cid, subject, object, provenance);
- `openlore search` output;
- the fake network's request log and in-flight peak;
- tripwire connection counts.

No indexer internal is read. Refusal scenarios also assert the *unchanged* universe: no network
request and no index file.

## Existing suites re-run (no harness regression)

Changes to existing code: `fake_atproto` gained postures, with additive defaults. `support/mod.rs`
made `raw_record_to_list_records_view` public and adds the inert seam variable at 4 indexer
spawn sites, and `review_app_self_attested_readers.rs` adds it at 1 site.

| Suite | Result |
|---|---|
| `indexer_ingest` | ok, 9 passed |
| `appview_core` | ok, 9 passed |
| `appview_search` | ok, 24 passed |
| `search_hide_retracted` | ok, 10 passed |
| `review_app_self_attested_readers` | ok, 12 passed (2 re-runs). The first run had 1 failure in the **viewer** test `the_viewer_labels_self_attested_claims_never_unverified` (`GET /peer-claims: error sending request`): an `openlore ui` startup flake under parallel load that never touches the indexer and did not reproduce |
| `openlore-test-support` (lib + doc) | ok, 72 + 2 + 2 passed |
