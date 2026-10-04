# RED classification — bluesky-claim-review-app (pre-DELIVER gate)

Run 2026-10-04 against `main` (feature unimplemented), every suite including the ignored ones:

```
cargo test -p cli --no-fail-fast \
  --test review_app_walking_skeleton --test review_app_sign_in_and_ownership \
  --test review_app_queue_and_publish --test review_app_consent_and_share \
  --test review_app_lifecycle --test review_app_self_attested_readers \
  --test review_app_privacy_invariants --test review_app_core -- --include-ignored
cargo test -p xtask --test review_app_architecture -- --include-ignored
```

All 9 targets **compile** (no BROKEN by construction; `cargo clippy -D warnings` clean on the new
targets and on `openlore-test-support`). The doubles' own contract — 11 self-tests in
`openlore-test-support` (`fake_atproto`, `fake_github_accounts`, incl. a full confidential-client
PAR → consent → PKCE token → granular-scope create → forbidden delete → RFC 7009 revoke-200 round
trip) — is **GREEN**.

In every app-driven scenario the GIVEN world starts first (both doubles bound and serving) and the
failure is the first touch of the missing driving port.

| Scenario(s) | Failing step | Cause | Class |
|---|---|---|---|
| **WS-1..WS-4** (active) | GIVEN "the review app is reachable" (`ReviewApp::start`) | `MISSING_FUNCTIONALITY: the review app is not available — the openlore-review-app composition root (ADR-072) has not been built` | MISSING_FUNCTIONALITY ✅ |
| SI-1..13, OW-1..9, QP-1..18, ED-1..4, DC-1..5, PR-1..4, SH-1..6, RS-1..6, RT-1..5, FG-1..6, PV-1..6, OP-1/2/4..7 (88) | same | same | MISSING_FUNCTIONALITY ✅ |
| OP-3 `probe --self-test` | WHEN the operator runs the subcommand | binary absent → same message | MISSING_FUNCTIONALITY ✅ |
| RD-1, RD-2, RD-5 | WHEN Maria pulls Priya | `peer add` **succeeds**; `peer pull` exits 1: `skipped: no usable verification key in the peer's DID document` | MISSING_FUNCTIONALITY ✅ (ADR-071 reader path absent — the D-5 consequence) |
| RD-3 | THEN mixed pull exits 0 | Rachel (app-signed) fetched + verified as today; Priya skipped as above | MISSING_FUNCTIONALITY ✅ (app-signed half already behaves — the regression guard is live) |
| RD-4 | THEN the refusal names its reason | all three bad records are skipped with the generic key message, not `integrity check failed` / `foreign repo` / `malformed provenance` | MISSING_FUNCTIONALITY ✅ (no provenance verdict yet) |
| RD-6, RD-7 | THEN the index row carries provenance | ingest exits 0; `indexed_claims` has no `provenance` column (`Binder Error: Referenced column "provenance" not found`) | MISSING_FUNCTIONALITY ✅ (ADR-071 additive migration absent). RD-7 additionally requires the reported `unverifiable provenance` refusal, so it cannot pass vacuously |
| CORE-1..10, CORE-12, CORE-1b | the `sut_*` binding | `todo!("SCAFFOLD: bind review_domain …")` panic | MISSING_FUNCTIONALITY ✅ (RED scaffold, Mandate 7) |
| **CORE-11** | — | passes today | GREEN-today regression guard (intended: binds the SHIPPED `select_person_repos`, BR-3) |
| AR-1, AR-5..AR-10 | the first `assert!(… exists)` | crate / deploy file not created yet | MISSING_FUNCTIONALITY ✅ |
| AR-2, AR-3, AR-4 | non-vacuity guard (`members.contains(…)`) | crate not in the workspace yet | MISSING_FUNCTIONALITY ✅ (guards added after a first run showed they would otherwise pass vacuously) |

**Result: 0 BROKEN · 0 WRONG_ASSERTION · 0 OBSERVABLE_NOT_AT_PORT — gate passes.**
Active at hand-off: WS-1..WS-4 only (RED). Everything else `#[ignore]`d for one-at-a-time unskip.

Universe check (Mandate 8): every assertion reads a port-exposed observable — the pages the app
serves, the user's PDS records / write attempts / forbidden attempts (`FakeAtprotoNetwork`), the
GitHub request log (`FakeGithubAccounts`), the app's own log stream + admin listener, and the
`openlore` CLI's stdout / `index.duckdb` rows for the reader path. The app's private
`review-app.duckdb` is never read by a scenario.
