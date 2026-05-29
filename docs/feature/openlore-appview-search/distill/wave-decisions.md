# Wave Decisions — DISTILL — openlore-appview-search (slice-05)

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Inherits from**: DISCUSS WD-100..110 + OD-AV-1..7; DESIGN WD-111..124 + ADR-023..027 + I-AV-1..9; DEVOPS D-D35..D-D43; slice-01/02/03/04 DD-*
- **Format**: DD-AV-XX entries; one decision per row

## Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions.** The cross-wave matrix (DISCUSS ↔
DESIGN ↔ DEVOPS) is in `acceptance-tests.md §1`. Every DISCUSS product
requirement (WD-103..110) has a consistent DESIGN resolution (WD-111..124) and a
consistent DEVOPS consequence (D-D35..D-D43); the four `# DISTILL: confirm` flags
are RESOLVED (search verb / self-hostable single binary / pull / real PLC decode
— `acceptance-tests.md §2`). The OD-AV-6 numbering ambiguity (share resolver +
pubkey decode share the label) is a DESIGN-recorded NON-BLOCKING upstream
observation; both are resolved (WD-122 + WD-118), so it is not a contradiction.

## DISTILL-wave decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| DD-AV-1 | Rust std `#[test]` + `proptest` for `@property`; same framework as slice-01/02/03/04. Layer-3 subprocess via `assert_cmd::cargo_bin` for BOTH binaries; layer-2 pure-core direct invocation. | Continuity with the established acceptance architecture; no new test framework. `[lang-mode] rust` per the workspace `Cargo.toml`. | LOCKED |
| DD-AV-2 | Hermetic seam: a `FakeIngestSource` (bounded fixture records incl. the adversarial set) + a fixture PLC resolver (carrying a REAL `z6Mk`) + a REAL `openlore-indexer serve` over a localhost EPHEMERAL `:0` port + a REAL separate `index.duckdb`. The discovery→federation funnel reuses the slice-03 `PeerPds` verbatim. | The first network service needs hermetic doubles for the two new external boundaries (network ingest + PLC resolution) per the Architecture of Reference (driven-external → fake); the B1 CLI↔indexer boundary is exercised against the REAL serve (Pillar 3 production composition); the ephemeral port keeps the contract + search ATs parallel-safe (DEVOPS open-q 8). No live network. | LOCKED |
| DD-AV-3 | Layer placement: `indexer_ingest.rs` + `appview_search.rs` at layer 3 (subprocess, example-only per Mandate 11); `appview_core.rs` at layer 2 (pure-core, 5 `@property` per Mandate 9 layer-2 PBT full). Layer 1 (per-arm + decode-boundary + mutation) is DELIVER's inner loop (DD-AV-7). | The two cardinal trust primitives (verify gate + anti-merging compose) are PURE functions — proven generatively at the cheap layer-2 boundary (AVC-1/AVC-2); the example-only layer-3 suite then proves the user-visible render + the real-I/O wiring. Mirrors slice-04 `scoring_core.rs` + `graph_query_explore.rs`. | LOCKED |
| DD-AV-4 | FLAT `tests/acceptance/` layout; THREE new files split by concern (the CLI `search` discovery surface / the `openlore-indexer` ingest infra / the pure `appview-domain` core). The indexer infra is its OWN file because it drives a DIFFERENT binary (the second composition root). | Preserves `cargo test --test <file>` ergonomics; symmetric with slice-02/03/04's per-concern file split. Two distinct driving ports → two layer-3 files. | LOCKED |
| DD-AV-5 | Scenario count: 37 (7 indexer + 22 search + 8 core), above the ~25-35 band for 6 stories. | Justified: slice-05 is the architecturally heaviest slice (first network service + 2 binaries + 8 cardinal release gates + the first adversarial-input boundary). The 8 release-gate scenarios are load-bearing, not padding; the funnel + share each need a chained multi-step journey. | LOCKED (above-band, justified) |
| DD-AV-6 | The eight cardinal release gates are NAMED scenarios load-bearing for the two cardinal disprovers + the inherited guardrail + the trust surface: `indexer_rejects_unverified_claim` (AV-3+AVC-1), `network_result_preserves_attribution` (AV-9+AVC-2), `local_first_preserved` (AV-13), `public_data_banner_shown` (AV-10), `verified_marker_is_universal` (AV-11+AVC-7), `search_succeeds_with_indexer_localhost` (AV-14), + the verify-before-index + anti-merging pure-core properties (AVC-1/AVC-2). | These ARE the slice's unshippable-on-failure surface (KPI-AV-2/3 + KPI-5 disprovers, outcome-kpis.md §Disprovers). Each asserts its invariant at a load-bearing boundary; the two cardinal trust guarantees ALSO get a layer-2 generative property. Names match DEVOPS D-D35's CI gate handles + DESIGN §10. | LOCKED |
| DD-AV-7 | Layer-1 exhaustive coverage (each `RejectReason` arm; the multibase decode boundary cases; mutation testing of `ingest_decision`/`compose_results` + the `claim-domain` decode) is OUT of DISTILL scope — DELIVER's inner TDD loop in the crates' `#[cfg(test)] mod tests`. | Symmetric with slice-02 DD-SCR-7 / slice-03 DD-FED-7 / slice-04 DD-GRAPH-7. The pure-core PROPERTIES (the contract) are DISTILL's at layer 2; the exhaustive arm coverage (the implementation decomposition) is DELIVER's. D-D40 puts `appview-domain` + the decode helper in the nightly mutation scope. | LOCKED |
| DD-AV-8 | The four NEW adapters' `probe()` bodies (the substrate-lie checks: index-store fsync-honesty / ingest network-lies / identity real-decode / query-server author_did-present / index-query unreachable-soft) are DELIVER's adapter-integration deliverable below the driving-port boundary (DESIGN §6.3) — NOT DISTILL acceptance scenarios, EXCEPT the user-visible STARTUP REFUSAL the probes drive (AV-6) and the gold real-decode path (AV-4). | The probes are substrate checks below the port boundary (like slice-04's recursive-CTE probe). The acceptance suite asserts the USER-VISIBLE consequence (the binary refuses to start, exit 2 + health.startup.refused — AV-6; the real decode runs with the seam unset — AV-4), not the probe internals. | LOCKED |
| DD-AV-9 | **Tier B state-machine acceptance is JUSTIFIED but DEFERRED to DELIVER** (Open Item 9). The ingest→store→query lifecycle qualifies (≥3 chained scenarios; domain-rich inputs; a genuine indexer state machine — the surface the slice-04 CM-G forward-note flagged). DELIVER SHOULD add `tests/acceptance/appview_state_machine.rs` over an `InMemoryComposition` (`@rule` ingest / `@invariant` = the two cardinal gates) once the Tier A step-method vocabulary lands. | Mandate 10's shared-vocabulary contract requires Tier B `@rule`s to invoke EXISTING Tier A step-methods (`when_the_indexer_ingests`, `then_every_stored_row_is_verified_and_attributed`) — those land in the bootstrap harness, not before. The verify GATE + anti-merging COMPOSE are pure (not state machines — Hebert ch.11 model-shape test) and ALREADY explored generatively at layer 2 (AVC-1/AVC-2), so the example layer-3 suite is sound without Tier B; Tier B is AMPLIFICATION (ingest/query interleavings), not a gap. Deferring keeps the DISTILL handoff tractable + honors the vocabulary contract. | LOCKED (deferred, documented) |
| DD-AV-10 | State-delta + Universe (Mandate 8 / CM-E): DECLARED per scenario (each docstring's "Universe (port-exposed)" line) but the explicit `assert_state_delta(...)` MIGRATION of the load-bearing scenarios (AV-3, AV-9, AV-13, AV-5, AV-22) is DEFERRED to DELIVER. The Rust `state_delta` port at `tests/common/state_delta.rs` is inherited (slice-01 bootstrap; `[port-mode] inherit`). | Same status as slice-01 DD-3 / slice-03 DD-FED-10 / slice-04 DD-GRAPH-10. The universe is DECLARED now (port-exposed names: CLI stdout fields, exit codes, indexed-row author_did set, ingest counters, the openlore.duckdb byte-unchanged guard, peer_subscriptions before==after); DELIVER migrates to the explicit helper once the helper bodies are real. Universe entries are port-exposed, NEVER internal store/compose struct fields. | LOCKED (declared; migration deferred) |
| DD-AV-11 | The `FakeIngestSource` + the fixture PLC resolver MUST input-validate like the real adapters (nw-tdd-methodology Test Doubles contract): `FakeIngestSource` rejects the same malformed records the real ingest adapter would; the fixture resolver carries a REAL `z6Mk` so the decode it feeds is the production path. | A permissive fake that "verified" anything would hide the AV-3 reject-gate wiring (the exact dogfood bug class nw-tdd-methodology warns about: a too-permissive double makes a release-blocking gate green against a broken seam). The real-`z6Mk` carry is what makes AV-4 the GOLD test (real decode, not the seam). | LOCKED |
| DD-AV-12 | Error-path / guardrail-surface ratio: explicit `@error` = 5/37 = 13.5%; the load-bearing NON-happy surface (8 release gates + anti-merging + capability/public-data/probe-refusal + adversarial + the pure `@error` exits) is ≥18/37 = 49%, above the 40% target. | Same read/discovery-surface logic as slice-03/04: the risk is the GUARDRAIL + adversarial surface, not input-validation sad paths. The 5 pure `@error` exits + the adversarial reject set (AV-3, the cardinal gate) are the validation surface the discovery domain admits. Substrate sad paths live at the adapter probes (DD-AV-8). | LOCKED |
| DD-AV-13 | The Pre-DELIVER fail-for-right-reason gate is DEFERRED until DELIVER's first slice-05 step (the indexer-subsystem bootstrap) lands the `appview-domain` crate + 4 ports + 4 effect crates + the `openlore-indexer` binary + the cli `search` dispatch + the test-support bodies + the 3 `[[test]]` registrations. Until then `cargo build --tests` fails on missing imports (BROKEN, not RED) by construction. | Same logic as slice-01 DD-2 / slice-03 DD-FED-13 / slice-04 DD-GRAPH-13: registering test targets whose production imports do not exist would fail the build for a reason OTHER than the scaffold `todo!()`. Once the bootstrap lands, every `#[test]` reaches its `todo!()` (RED) and `red-classification.md` is written (DELIVER's ADR-025 RED phase entry). | LOCKED (deferred) |
| DD-AV-14 | The three `[[test]]` registrations (`appview_search`, `indexer_ingest`, `appview_core`, all `path = "../../tests/acceptance/<file>.rs"`, `harness = true`) in `crates/cli/Cargo.toml` are DEFERRED to the bootstrap step. | Same as DD-AV-13 — registering before the imports exist breaks `cargo build --tests` (BROKEN). The pattern mirrors the slice-02/03/04 `[[test]]` entries in `crates/cli/Cargo.toml`. (Note: `appview_core` is layer-2 pure-core but co-locates under the cli package's test targets purely for workspace co-location, exactly like slice-04's `scoring_core` + slice-03's `lexicon_counter_claim`.) | LOCKED (deferred) |

## Project Infrastructure Policy

The policy file `docs/architecture/atdd-infrastructure-policy.md` was ABSENT
(slice-01 DD-11 / slice-03 DD-FED-11 / slice-04 DD-GRAPH-11 each deferred it).
Per `nw-distill` write-if-absent, this DISTILL wave BOOTSTRAPS it with the
cumulative slice-01..05 port→mechanism entries (the slice-05 additions are in
`acceptance-tests.md §11`). `--policy=inherit` (default); the file grows by
accretion in future slices.

## Open questions handed to DELIVER

These are deliberately deferred to DELIVER (the Q-DELIVER-AV-1..9 set is DESIGN's;
these are the DISTILL-specific test-infra deferrals).

1. **The ingest harness bodies** (`FakeIngestSource`, the fixture PLC resolver,
   `IndexerHandle` for the localhost `:0` serve, `seed_network_index`,
   `run_openlore_indexer`, the assertion helpers) in `support/mod.rs` + the
   `fixtures_ingest.rs` recipes (the adversarial + valid records + the real-`z6Mk`
   DID-doc keypair). Scaffolded by this wave; bodies `todo!()`.
2. **State-delta universe migration** (DD-AV-10): the port-exposed universe per
   load-bearing scenario:
   - AV-3: `{ count(indexed_claims rows)==1, adversarial-cids ∉ indexed_claims, adversarial-cids ∉ any search result, indexer.ingest.rejected{reason} counts, indexer.ingest.verified==1 }`
   - AV-9: `{ attributed-rows-for(deno,dependency-pinning)==2, author-set=={priya,sven}, NO merged/consensus/mean-confidence row, footer distinct_author_count==2 }`
   - AV-13: `{ claim_add.exit==0, claim_publish.exit==0(offline), graph_query.exit==0, search.exit==non-fatal(soft Unreachable + graph-query pointer), search.hung==false }`
   - AV-5: `{ indexer-help verb-set has no sign/publish/add, openlore.duckdb bytes unchanged, index.duckdb written }`
   - AV-22: `{ peer_subscriptions(priya)==absent, peer_claims/<priya>/ removed, no parallel discovery-subscription record }`
3. **Tier B state-machine file** (DD-AV-9 / Open Item 9): `appview_state_machine.rs`
   over `InMemoryComposition`; `@rule`(ingest) / `@invariant`(verified+attributed;
   distinct_author_count == COUNT DISTINCT); reuse the Tier A step-method vocabulary.
4. **The exact rendered strings** AV-8/AV-9/AV-10/AV-23/AV-26 assert (the footer
   no-merge wording, the banner copy, the `--show` "Signature: VERIFIED against"
   line, the `--share` link grammar) — DELIVER fills within the locked contracts
   (Q-DELIVER-AV-3/7; the banner copy is a PO product default, US-AV-004 Tech Notes).

## Changelog

- 2026-05-28 — Quinn — initial DISTILL-wave decisions for slice-05
  (openlore-appview-search). DD-AV-1..DD-AV-14 LOCKED. 37 scenarios authored RED
  across 3 files (7 indexer-ingest + 22 search + 8 pure-core). 8 cardinal release
  gates named + bound. The four DISCUSS `# DISTILL: confirm` flags resolved per
  DESIGN. Reconciliation passed (0 contradictions). Project Infrastructure Policy
  bootstrapped (was absent). Tier B state-machine acceptance JUSTIFIED but
  DEFERRED to DELIVER (DD-AV-9 — the shared-vocabulary contract requires the Tier A
  harness step-methods first; the slice-04 CM-G forward-note's predicted surface).
