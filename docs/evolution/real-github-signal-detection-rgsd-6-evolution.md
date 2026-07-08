<!-- markdownlint-disable MD013 -->
# Evolution: real-github-signal-detection RGSD-6 (retire the legacy synthetic `signals[]` scaffold) — feature COMPLETE

> Sixth and final slice of the `real-github-signal-detection` feature. Paradigm:
> functional Rust (ADR-007). Design: `docs/feature/real-github-signal-detection/design/architecture-design.md` §4/§5.

## Summary

RGSD-6 is the closing cleanup: it retires the transitional synthetic `signals[]`
scaffold that the slice-02 scraper walking skeleton used before real detection
existed. With all five detectors shipped (RGSD-1..5), the scaffold's production path
was dead for real repos and its remaining role was test-injection. RGSD-6 removes it
entirely and migrates every test to real-detection postures — so REAL GitHub metadata
is now the SOLE signal path, end to end.

**Real detection unchanged:** `openlore scrape github BurntSushi/ripgrep` still yields
the same 5 candidates (memory-safety, dependency-pinning, semantic-versioning,
documentation-first, test-driven) — now with zero synthetic scaffolding underneath.

### What shipped (one paragraph)

PRODUCTION: `harvest_repo` → `Ok(detect_signals(&facts))` (the `union_signals_by_kind`
+ `parse_signals` bridge removed); `harvest_user` → records the auth report then
`Ok(Vec::new())` (user-target aggregation is deferred to slice-04 — and a REAL user
scrape already yielded 0 signals, so this is behavior-preserving). Deleted
`client::{parse_signals, signal_kind_from_wire, parse_one_signal}`,
`union_signals_by_kind`, `bound_user_aggregate`, `USER_AGGREGATE_SIGNAL_CAP`.
TEST-SUPPORT: deleted `FakeSignal`, the `State.signals` body field + its serialization,
the `signals` params on `for_public_repo`/`for_public_user`/`from_state`/`authenticated`,
`with_multi_signal_single_predicate`, and the three `fixtures_github` signal fixtures.
MIGRATION: 8 acceptance files (`scrape_candidates/sign/github/auth`,
`viewer_scrape/htmx/invariants/htmx_invariants`) moved from synthetic `signals[]` to
real-detection postures — a new `for_public_repo_with_all_signals` (all 5 facts → 5
real signals), `with_no_matching_signals` (0), and the single-signal postures. The
`grep`-for-scaffold-symbols check returns **zero hits**.

### Two behaviors that could not migrate 1:1 (handled honestly)

- **The derivation COLLAPSE** (SC-4: three same-kind signals → one candidate listing
  all three, I-SCR-4). Real detection fires once per kind and can never emit two
  same-kind signals, so this is unreachable via the CLI. Its coverage moved to a
  DIRECT pure unit test of `derive_candidates` in `scraper-domain/src/derive.rs`
  (`three_same_predicate_signals_collapse_into_one_candidate_listing_all_three`) —
  stronger than the CLI-level scenario it replaced.
- **The user-target aggregate** (SG-3). The old scenario injected a synthetic 2-signal
  aggregate. `harvest_user` now honestly derives 0 candidates ("aggregation deferred to
  slice-04") — matching what a real user scrape already produced. The auth-report
  banner (from `parse_auth_report`, independent of `signals[]`) still renders.

### Steps (refactor — no new RED; the gate is "all tests green + real detection unchanged")

- **rgsd6-1** `a77de00` — FOUNDATION (additive, scaffold intact): the
  `for_public_repo_with_all_signals` posture + the `derive.rs` collapse unit test.
- **rgsd6-2** `b4b22d1` (Phase A: migrate all 8 acceptance files) + `1dea350`
  (Phase B+C: remove the production + test-support scaffold).

## Quality gates — final report

| Gate | Result |
|---|---|
| Phase-4 adversarial review | **APPROVED — 0 defects.** No weakened assertions (migrated to the real detector values byte-for-byte); collapse coverage preserved (→ stronger unit test); user-defer honest; auth reporting survives; real detection untouched; dead code clean. |
| grep-is-zero | 0 hits for `FakeSignal`/`parse_signals`/`signal_kind_from_wire`/`union_signals_by_kind`/the fixtures |
| check-arch | OK — 21 members |
| Real detection unchanged | live `scrape github BurntSushi/ripgrep` → same 5 candidates |

Green: 13 acceptance suites (scrape_candidates 6, scrape_sign 11, scrape_github 11,
scrape_auth 7, scrape_real_signal_detection 4, scrape_dependency_pinning 4,
scrape_semver_changelog 5, scrape_docs_substantial 5, scrape_test_ci 5, viewer_scrape 5,
viewer_htmx 24, viewer_invariants 6, viewer_htmx_invariants 8) + `adapter-github` 20,
`scraper-domain` 27, `ports` 13, `test-support` 55.

## Feature status — COMPLETE

`real-github-signal-detection` is DONE. The scraper detects all five bounded
`SignalKind`s from REAL public GitHub metadata (RGSD-1 language · RGSD-2 Cargo.lock ·
RGSD-3 semver+CHANGELOG · RGSD-4 README/docs · RGSD-5 CI/tests), and the synthetic
`signals[]` scaffold is fully retired (RGSD-6). The capability the user originally
asked for — point OpenLore at a real GitHub repo and have philosophies inferred (then
human-signed and persisted) — is realized end to end with no scaffolding.

Deferred (out of this feature, noted for the backlog): the "no unsafe blocks",
"doc-comment density", and "test/source ratio > 0.5" precision refinements (each a
code/tree scan); and real user-target aggregation (slice-04).

## Commit trail

`a77de00` (rgsd6-1 foundation), `b4b22d1` (rgsd6-2 Phase A migration), `1dea350`
(rgsd6-2 Phase B+C removal). All on `main` (trunk-based, no PR).
