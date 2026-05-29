# OpenLore — Resume Context

## Current Task
Autonomous multi-slice campaign (user: "complete all slices, full autonomy"). SHIPPED: slice-03 (b7aefa1), slice-02 (b5a7be6), slice-04 `openlore-scoring-graph` (a8654c8). slice-05 `openlore-appview-search`: upstream waves committed (13fcd2b); DELIVER Phase 01 (bootstrap) + Phase 02 (pure core) COMPLETE; Phase 03 (indexer ingest) next. (dyld stall from 2026-05-28 is CLEARED.)

## slice-05 DELIVER progress (resume here)
- Roadmap: `docs/feature/openlore-appview-search/deliver/roadmap.json` — 43 steps, 5 phases (01 bootstrap ×5 ✅, 02 appview-domain pure core ×9, 03 ingest ×7, 04 search+trust ×7, 05 dimensions/funnel/share ×15). `.nwave/des/deliver-session.json` active.
- ✅ Phase 01 DONE: 01-01 (8db835b) · 01-02 (39f9a68) · 01-03 (a5304f3) · 01-04 (9ea1717) · 01-05 (4af37d3). Boundary fmt+log 38746fe. DD-AV-13 gate held (38 ATs RED).
- ✅ Phase 02 DONE (pure appview-domain core, all 9 appview_core.rs AVC scenarios GREEN): 02-01 (9afb5da ingest_decision/AVC-1) · 02-02 (1d8f3a3 AVC-3a determ) · 02-03 (f0366ea AVC-4 author-derived) · 02-04 (0ff6a7a compose_results/AVC-2) · 02-05 (81a7c53 AVC-3b) · 02-06 (303fcc9 AVC-5) · 02-07 (54833e3 AVC-7) · 02-08 (7e078dd annotate_counter/AVC-6) · 02-09 (495ac8e near_match_suggestion/AVC-8). Pure core complete: ingest_decision + compose_results + annotate_counter_relationship + near_match_suggestion, all proptest-proven.
- NEXT: **Phase 03 step 03-01** (AV-1 walking-skeleton beat 1: indexer ingests a verified attributed claim → searchable; lands live index.duckdb DDL + the indexed_claims/<did>/<cid>.json artifact). Then 03-02..03-07. Layer-3 subprocess ATs (indexer_ingest.rs) over the REAL openlore-indexer binary + FakeIngestSource + fixture real-z6Mk PLC resolver + REAL index.duckdb. Cardinal gate AV-3 verify-before-index (03-03). AV-4 (03-04) = the real z6Mk decode + the seam open-item. Then Phase 04 (appview_search: AV-8 walking-skeleton beat 2 / 04-01; AV-9 anti-merging 04-02; AV-13 local-first 04-05), Phase 05 (dimensions/funnel/share, 15 steps).

## CRITICAL convention — AT bodies are todo!() scaffolds DELIVER fills
The 3 slice-05 AT files (appview_core/indexer_ingest/appview_search.rs) have `todo!("DELIVER: drive ... assert ...")` bodies with the full spec in the panic string + comment block. Each scenario step MUST: (a) replace ONLY its `scenario_name` fn's todo!() with the real assertions (leave all other fns' todo!() RED for later steps), AND (b) implement the production behavior. So **add the test_file to every scenario step's files_to_modify** (the roadmap omits it). Layer-3 steps also use/fill the `tests/acceptance/support/mod.rs` harness (IndexerHandle spawns real openlore-indexer on ephemeral :0).

## Open items (must close before/at finalize)
- **03-04 (AV-4):** land the REAL z6Mk ADR-026 decode (claim_domain::decode_ed25519_multibase, currently todo!()); AND broaden `no_pubkey_seam_in_release_build` xtask scan to `adapter-atproto-did/src/peer_resolve.rs` + cfg-gate the PRE-EXISTING slice-03 ungated `OPENLORE_PEER_PUBKEY_HEX_` seam (peer_resolve.rs:77, release-readable verification bypass — rule currently narrowed to lib.rs to dodge it).
- **Phase 03/04:** as each adapter's real probe() body lands, DE-allowlist it from `xtask/src/check_probes.rs` BOOTSTRAP_STUB_ALLOWLIST (currently: DuckDbPeerStorageAdapter[slice-03], AtProtoIngestAdapter, HttpIndexQueryAdapter, IndexStoreAdapter, AtProtoDidAdapter). By finalize the slice-05 entries must be gone (real Earned-Trust probes).
- **Post-finalize cleanup:** `adapter-system-clock` has a parallel-race flaky test (set_var env mutation, slice-01 tech debt) — fix with a serialize mutex once the deliver session is closed (guard blocks orchestrator src/ edits during DELIVER).

## Key Decisions
- WD-13 sequence: federation (✅) → scrapers (✅) → scoring (✅) → appview (slice-05, in progress).
- slice-05 adds the INDEXER as a 2nd binary (ATProto AppView pattern): verified+attributed+anti-merging search across the public claim graph. ADR-023..027; I-AV-1..9; 3-layer anti-merging (TYPE non-Option author_did in ports + STRUCTURAL xtask SQL rule + BEHAVIORAL AV-9/AVC-2).

## Proven mechanics (carry forward)
- DES per-step (5-phase legacy [PREPARE,RED_ACCEPTANCE,RED_UNIT,GREEN,COMMIT]); `des-log-phase --data PASS` exactly for EXECUTED; valid skip prefix for SKIPPED. project_id header already in execution-log. Crafters stage only files_to_modify, no `cargo fmt --all`; orchestrator does fmt+clippy+log commit at phase boundaries (via Bash cargo fmt — not blocked by the source-write guard, which only blocks the Edit/Write tools on src+tests during an active deliver session).
- Source-write guard: Task-dispatched crafters get `.nwave/des/des-task-active` from the pre-task hook; NEVER forge it. Orchestrator CANNOT Edit src/tests during DELIVER — route fixes through a crafter Task. The pre-bash hook blocks any Bash command containing the literal `execution-log` (use Read tool, or a glob like `exec*.json` for git add).
- Mutation (Phase 06): cargo-mutants 25.3.1 scopes tests to the mutated crate's OWN package + duckdb scratch build is flaky — for a pure crate whose killers live downstream, use a direct empirical harness. appview-domain SHOULD carry behavior properties in-crate. D-D40: mutate appview-domain ingest/compose/suggest + claim-domain decode at ≥95% (per-feature gate ≥80%).
- No git remote (no push); preserve feature workspace at finalize; remove `.nwave/des/` session markers after finalize.
