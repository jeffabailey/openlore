# OpenLore — Resume Context

## Current Task
Autonomous multi-slice campaign (user: "complete all slices, full autonomy"). SHIPPED: slice-03 (b7aefa1), slice-02 (b5a7be6), slice-04 `openlore-scoring-graph` (a8654c8). slice-05 `openlore-appview-search`: upstream waves DISCUSS+DESIGN+DEVOPS+DISTILL committed (13fcd2b, reviews APPROVED); DELIVER in progress.

## ⛔ BLOCKER (2026-05-28) — environment, needs user action
Machine-wide macOS **dyld `_dyld_start` startup stall**: cargo builds/links + `cargo xtask check-arch` succeed, but NO test binary can RUN to completion (it prints "Running unittests…" then hangs; confirmed across openlore + unrelated projects + unchanged binaries). This halts DELIVER — Outside-In TDD + the integration/mutation/acceptance gates all need to execute tests. **Fix on the user's side** (typical: reboot; or `sudo update_dyld_shared_cache -force`; check for a stuck system-update / security tool intercepting process launch). Resume DELIVER once `cargo test -p scoring --lib` completes within seconds again.

## slice-05 DELIVER progress (resume here)
- Roadmap APPROVED: `docs/feature/openlore-appview-search/deliver/roadmap.json` — 43 steps, 5 phases (01 bootstrap ×5, 02 appview-domain pure core ×9, 03 ingest ×7, 04 search+trust ×7, 05 dimensions/funnel/share ×15). execution-log.json has the `project_id` header; `.nwave/des/deliver-session.json` active.
- DONE: **01-01** (commit 8db835b) — NEW pure `crates/appview-domain` (ingest/compose/suggest ADTs + todo!() entry points) + `claim-domain::decode_ed25519_multibase` skeleton (real z6Mk decode deferred to AV-4/step 03-04) + workspace member + xtask pure-core allowlist. NOTE: its GREEN was verified by build+link+check-arch only (tests could not run due to the dyld blocker) — RE-RUN `cargo test --workspace` once unblocked to formally close 01-01's test gate.
- NEXT: **01-02** (hoist IndexedClaim/RawRecord/SearchDimension/KeyId/CounterRef to `ports` + 4 new ports IndexQueryPort/IngestSourcePort/IndexStorePort/IdentityResolvePort + AuthorRelationship::NetworkUnfollowed), then 01-03/01-04/01-05 bootstrap, then phases 02-05. Walking skeleton = AV-1 (step 03-01 ingest) + AV-8 (step 04-01 search). Cardinal gates: AV-3 verify-before-index (03-03), AV-9 anti-merging (04-02), AV-13 local-first (04-05).

## Key Decisions
- WD-13 sequence: federation (✅) → scrapers (✅) → scoring (✅) → appview (slice-05, in progress).
- slice-05 = `openlore-appview-search`: serves J-001 (make signed claims network-discoverable, not just within your own subscriptions); adds an INDEXER service as a separate binary (the ATProto AppView pattern — aggregates records across the network into a queryable search surface).
- Per-slice pipeline mirrors slice-02/03/04: DISCUSS (nw-product-owner + reviewer hard gate) → DESIGN (nw-solution-architect + reviewer; consider nw-system-designer for the indexer service infra) → DEVOPS + DISTILL (parallel) → DELIVER (roadmap+execute-all+3.5 gate+refactor+review+mutation+finalize). Commit upstream waves together (one commit), then DELIVER.

## Proven mechanics (carry forward)
- DES per-step (5-phase legacy); `des-log-phase --data PASS` exactly; ADD `project_id` header to execution-log right after `des-init-log` (hook defect). Crafters stage only files_to_modify, no `cargo fmt --all`; orchestrator commits a `cargo fmt --all` + clippy cleanup at phase boundaries.
- Source-write guard: Task-dispatched crafters get `.nwave/des/des-task-active` from the hook; on a transient race RE-DISPATCH (NEVER forge the marker). Orchestrator refactor/test-hardening uses DES-MODE orchestrator markers.
- Mutation: cargo-mutants 25.3.1 scopes tests to the mutated crate's OWN package and the duckdb scratch build is flaky — if a pure crate's killers live downstream (cli), measure with a direct empirical harness (apply mutant at line:col, run the real killing suite). A pure crate SHOULD carry its behavior properties in-crate. Per-feature ≥80% gate.
- No git remote (no push); preserve feature workspace at finalize; remove `.nwave/des/` session markers (deliver-session.json + des-task-active*) after finalize.
