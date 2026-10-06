# RCA — indexer follow-ups (from indexer-per-did-pds-fetch)

Source: nw-troubleshooter investigation, 2026-10-05 (read-only). User decisions recorded at the end.

## D2 — OPENLORE_INDEXER_PLC_ENDPOINT not validated at startup (code bug)
- `config.rs:162` reads the PLC endpoint raw: no validation, and a blank value doesn't fall back to the default (it skips the `setting` trim/blank filter).
- Lookups go through the guarded client. `get_json` (`identity_lookup.rs:73-76`) maps a policy-refused URL (plain http, private/loopback IP literal, userinfo) to `Unavailable`, which `plan_listing` treats as fallback-eligible (`ingest_pass.rs:205`).
- With a fallback configured, every did:plc DID is read through the fallback as relay-origin, self-attested claims are refused, and the pass exits 0, all silently. Without a fallback, every did:plc DID is `did_unresolvable` (exit 3 if all are did:plc).
- Root cause: the startup checks are per variable, and the PLC endpoint was treated as "unchanged" (data-models §96) even though it now sits behind the guard. R-IPF-8's "surfaces rather than silently" is wrong when a fallback is set.
- Fix: a `plc_endpoint(url, policy)` validator mirroring `fallback_url`, read through `setting(...)`; refuse naming `PLC_ENDPOINT_VAR` and the value. Hostnames resolving to private addresses stay a runtime refusal (IPF-08).
- Regression: acceptance refusal-table rows in `indexer_per_did_config.rs` (`http://plc.example`, `https://10.0.0.1`, `https://u:p@plc.directory`, blank → default), plus the PLC dimension in the `config_parsing_is_total…` property.

## D3 — store write failure mid-pass (test gap + atomicity bug)
- Each `upsert` is an artifact write, three DELETEs, then INSERTs (`adapter-index-store/src/lib.rs:296-367`), each autocommitted with no transaction. Earlier claims do stay committed (ADR-078 §2 holds), but one claim's upsert is not atomic.
- Hypothesis: `decode_references` doesn't remove duplicates (`claim-domain/src/decode.rs:263-291`), and the references PK is (referencing_cid, referenced_cid, ref_type) (`schema.rs:76`). A remote record listing a reference twice violates the PK after the row is committed, leaving a half-written claim and exit 2 on every pass: a remote-data fault that looks like a local one.
- Fix: one transaction per upsert, and duplicate references removed before insert.
- Regression:
  - Adapter test: re-upsert with duplicate references keeps the claim whole (expected to fail today).
  - Acceptance via the binary: force a store failure for the second author (a regular file at the author's index dir), assert exit 2 and that the first author's claims are still searchable (passes today; guards ADR-078 §2).

## D1 — fallback after a resolution timeout runs on an exhausted deadline (design defect)
- `fetch_repo` makes one deadline (`run.rs:511`), reused by `list_source` (`run.rs:547`). `ResolutionFailure::TimedOut` is fallback-eligible (`ingest_pass.rs:55-60,205`).
- The design specifies this: ADR-078 §4 says "the fallback gets the remaining budget", and data-models §2 says "an immediate expiry gives pds_timeout". When the resolve step expires, the remaining budget is ~0, which breaks ADR-077 NFR-3 ("when PLC is down the fallback reproduces the old outcome").
- When it shows: `LOOKUP_TIMEOUT` is 10 s (`identity_lookup.rs:18`), less than the default 30 s budget, so it appears only with `OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS` ≤ 10.
- Regression: acceptance with a hanging directory, a fallback, and a 2 s budget, asserting the DID is read through the fallback (exit 0, `source_fallback` event). Fails today.

## User decisions (2026-10-05)
- D1: give the fallback its own FRESH per-DID budget. Amend ADR-078 §4 and data-models §2. Worst-case pass time can double.
- D3: remove duplicate references, then index. Wrap each upsert in one transaction.
- One bugfix delivery in order D2 → D3 → D1, each step starting with a regression test that fails today, auto-advancing.
D2 follow-up (user 2026-10-05): a SET-but-blank OPENLORE_INDEXER_PLC_ENDPOINT must be refused at startup by the config check, naming the variable; unset still means https://plc.directory. peer_resolve.rs and its pinned test stay unchanged. Folded into step 01-02.
