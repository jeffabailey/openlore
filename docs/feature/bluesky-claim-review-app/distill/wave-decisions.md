# Wave Decisions — bluesky-claim-review-app (DISTILL)

- **Wave**: DISTILL · **Date**: 2026-10-04 · **Designer**: Quinn (nw-acceptance-designer), autonomous
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (ADR-007)
- `[lang-mode] rust` · `[policy-mode] inherit` (`docs/architecture/atdd-infrastructure-policy.md`, 4 rows appended) · `[port-mode] inherit` (`tests/common/state_delta.rs`)
- Rigor: agent model inherit · mutation testing per feature (CLAUDE.md) — `review-domain` + the ADR-071 provenance verdict go in DELIVER's mutation scope · phases PREPARE / RED_ACCEPTANCE / RED_UNIT / GREEN / COMMIT

## Inputs read

| Input | Status |
|---|---|
| `discuss/` requirements, user-stories, acceptance-criteria, journey `.feature` + `.yaml`, outcome-kpis, story-map, wave-decisions | ✓ |
| `design/` architecture-design, component-boundaries, data-models, technology-stack, wave-decisions (incl. **Spike results 2026-10-04**) | ✓ |
| ADR-071..076 | ✓ |
| `devops/` wave-decisions, ci-cd-pipeline, infrastructure-integration, observability-design, kpi-instrumentation, monitoring-alerting | ✓ |
| House style: `contributor-philosophy-inference/distill/*`, `serverless-philosophy-federation/distill/*`, `tests/acceptance/README.md`, `support/mod.rs`, `test-support` fakes | ✓ |

## Reconciliation (HARD GATE)

**Reconciliation passed — 0 contradictions.** Checked every DISCUSS decision (D-1..D-12, I-BRA-1..8)
against DESIGN (DD-1..10, ADR-071..076) and DEVOPS (DV-BRA-1..13). Two near-misses were
examined and are NOT contradictions:

- D-11 "share is strictly opt-in" vs DD-3 "post permission is requested at sign-in": asking for
  the permission is not posting. I-BRA-6 is asserted behaviourally (SH-1/SH-2/SH-4).
- D-5 "no app-level signature" vs the existing peer pull, which today skips a peer whose DID
  document has no application key (observed in the RED run): that is the D-5 *consequence*
  DESIGN already scoped (ADR-071), not a conflict.

## Decisions

| ID | Decision | Rationale |
|---|---|---|
| DWD-1 | Rust std `#[test]` suites in flat `tests/acceptance/review_app_*.rs`, Gherkin in doc comments + `// GIVEN/WHEN/THEN` (house convention). 8 cli `[[test]]` targets + 1 `xtask/tests` file. | Same as every prior feature; `cargo test --test <file>` ergonomics. |
| DWD-2 | **RED convention = compile-now, fail-at-runtime** (the 2026-09-27 precedent), not deferred registration. The harness locates `openlore-review-app` next to the test's `deps/` dir (or via `CARGO_BIN_EXE_openlore-review-app`); when absent it panics `MISSING_FUNCTIONALITY: the review app is not available…` *after* every fake started. Only the 4 walking-skeleton scenarios are un-ignored. | Every suite compiles today; RED is classifiable now (see `red-classification.md`), not deferred. |
| DWD-2b | **WS strategy** (Architecture of Reference; legacy label **B — real local, fake external**): driving + driven-internal real (the app binary, `review-app.duckdb`, the `openlore` CLI and its DuckDB, the indexer), driven-external faked (ATProto network, GitHub). WS tags `@walking_skeleton @driving_port @driving_adapter @real-io`. | Recorded for consistency with earlier features (reviewer observation). |
| DWD-3 | Suites registered under `crates/cli/Cargo.toml` (the review-app crate does not exist; WS + reader suites also drive the `openlore` binary). **DELIVER decision at bootstrap**: make sure the app binary is built before the cli acceptance stage — either a CI step `cargo build -p openlore-review-app`, or move the app-only suites into `crates/openlore-review-app/Cargo.toml` (the harness honours `CARGO_BIN_EXE_*`). Do NOT add the app as a cli dev-dependency: `check-arch` reads `cargo metadata` resolve edges (dev included), which would trip the "cli never reaches openlore-review-app" rule. | Keeps the capability boundary honest. |
| DWD-4 | Two new driven-external doubles in `openlore-test-support`: **`FakeAtprotoNetwork`** (PLC + handle resolution + per-host PDS **and** OAuth authorization server: PAR + DPoP nonce dance + client-metadata fetch, consent, PKCE token + refresh, RFC 7009 revoke answering **200**, granular-scope `createRecord`, reads; delete/put/applyWrites refused + recorded) and **`FakeGithubAccounts`** (multi-account, mutable bio/repos/id, per-repo facts, rate-limit window + remaining floor, token rejected/expiring, ordered request log). 11 self-tests green, incl. a full confidential-client handshake. | The existing `FakeGithub` is single-target and constructor-pinned; no OAuth double existed. |
| DWD-5 | **What the doubles cannot model** (covered elsewhere): JWS signatures (DPoP, client assertion, ES256 JWKS) — claims only; the real DAG-CBOR CID a PDS returns from `createRecord` (the app must recompute, never trust it); real token-lifetime timing; PDS lexicon-validation modes; relay/firehose; DNS-TXT and `/.well-known/atproto-did` handle resolution. → DEVOPS nightly `live-contract-smoke` (DV-BRA-12) + SPIKE-1/3 on the OpenLore PDS (R-0). | Earned-trust gap stated, not hidden. |
| DWD-6 | **Driving-port page contract** (DISTILL-proposed; one-table rename in the harness): routes `/`, `/github`, `/review`, `/scan`, `/scan/status`, `/settings`, `/@{handle\|did}`, `/oauth/client-metadata.json`, `/oauth/jwks.json`, `/oauth/callback`, `/plans/{cid}/confirm`, `/share`, `/healthz`, `/readyz`; admin (loopback) `/admin/kpi`, `/admin/purge`. Fields `handle`, `github_login`, `object`, `confidence`, `text`, `csrf`. Labels from the stories ("Sign in with Bluesky", "Verify", "Approve", "Edit", "Not me", "Undo", "Publish to my repo", "Back", "Retry", "Share on Bluesky…", "Post to Bluesky", "Don't post", "Retract", "Disconnect and forget me", "Cancel") plus proposed "Scan my repos", "Scan again", "Resume scan", "Preview", "Confirm retraction", "Yes, forget me", "Sign out", "Copy". One `<article>` per suggestion/claim; `data-scan-status`; `data-triage-keys`. | Progressive enhancement (works without JS) makes the plain form the driving port; semantic HTML doubles as the WCAG check. |
| DWD-7 | **Config contract** (DISTILL-proposed seams; production names from DEVOPS §4.1): `APP_ORIGIN`, `LISTEN_ADDR`, `ADMIN_LISTEN_ADDR`, `REVIEW_DB`, `SECRETS_DIR`, `LOG_FORMAT`, `OAUTH_SCOPES`, `OPENLORE_GITHUB_API_BASE` (existing), `REVIEW_APP_PLC_URL`, `REVIEW_APP_HANDLE_RESOLVER_URL`, `REVIEW_APP_ALLOW_LOOPBACK_HTTP=1` (test builds only — mirror the `test-autoconfirm` discipline; a release binary must refuse it). Secrets files `client-jwk` (from `openlore-review-app gen-client-jwk`), `data-key`, `github-token`, `log-salt`. | Hermetic wiring of the production composition root (Pillar 3). |
| DWD-8 | **Tier B not a separate file.** The one domain-rich state machine (suggestion lifecycle) is pure, so it is a layer-2 model-based property (CORE-8, arbitrary event sequences vs an oracle) — the pure-core analogue of Tier B. The HTTP journey is covered Tier-A style with Pillar-2 chaining. | Hebert ch.11 model-shape test; avoids an `InMemoryComposition` that would duplicate `review-domain`. |
| DWD-9 | **Indexer repo enumeration seam** `OPENLORE_INDEXER_REPO_DIDS` (RD-6/RD-7) is DISTILL-proposed: DESIGN says the ingest adapter "enumerates one repo DID" (listRecords needs `repo`) but not how the indexer learns which. Non-blocking; DELIVER may rename. The PLC seam `OPENLORE_INDEXER_PLC_ENDPOINT` already exists. | Recorded as an upstream note, not a blocker. |
| DWD-10 | **Ownership-token boundary pinned** (ADR-076 "maximal runs of `[A-Za-z0-9._:%-]`"): `xdid:plc:…` and an uppercased DID are NOT tokens; `(did:…).` IS. OW-7 + CORE-1/1b. | Makes OD-BRA-10's "delimited" reading executable. |
| DWD-11 | **Rescan summary semantics pinned**: "N already published / N declined (hidden)" count the *derived* keys in that state (CORE-4, RS-1). | Story copy did not define the denominator. |
| DWD-12 | **Out of acceptance scope (referenced, not automated)**: NFR-BRA-4 latency (an SLO in `observability-design.md`, no timing asserts — F-004); live deploy, rollback drill, IMDS hop-limit, GHCR visibility (runbook R-2..R-6); docker image smoke + trivy (CI image job). OP-3 covers `probe --self-test` at the binary level; AR-8/AR-9 cover the compose mount guard and Caddy HTTPS file shape. | DEVOPS owns live checks. |
| DWD-13 | Mandate 8 universe (port-exposed): `pds.claims`, `pds.posts`, `pds.write_attempts`, `pds.forbidden_writes`, `queue.pending` — used by WS-4, QP-6, QP-9, ED-3, DC-1, DC-4, SH-2, RS-4, RT-2, FG-2. Never the app's private store. | Implicit-unchanged catches stray writes. |

## Mandate-12 evidence (Rust analogue of the four criteria)

- **CM-I-1** domain types: `tests/acceptance/support/review_app/domain.rs` — `Persona`, `PdsHost`, `Philosophy`, `Suggestion`, `ConfidenceEntry`, the canonical `priya::*` / `dmitri::*` suggestions. No DID, handle or object id is spelled inline in a scenario body.
- **CM-I-2** typed steps: every step in `support/review_app/world.rs` takes `Persona` / `Suggestion` / `Philosophy`, not raw strings (raw `&str` only for deliberately invalid input: a mistyped handle, a forged token).
- **CM-I-3** no logic in scenarios: scenario bodies are step calls + assertions; branching lives only in parametrised tables (`for case in cases`).
- **CM-I-4** step-reuse ratio (informational): **215 step invocations / 21 `given_/when_/then_` steps = 10.2×** (natural ceiling for a journey-rich feature).

## AT-completeness audit (Phase 2.5) — 15/15 → **COMPLETE**

| Item | Evidence |
|---|---|
| C1a empty/min | QP-3 (no owned repos), PR-2 (empty profile), SH-4, OW-7 (empty bio), ED-1 (0.00) |
| C1b boundaries | ED-1 (0.00/1.00), ED-2 (1.5, 0.555, -0.1), SH-6 (301 chars), QP-17 (7th scan), CORE-5, CORE-10 |
| C2a state machine documented | `review_app_core.rs` CORE-8 (pending ⇄ declined, pending → published → retracted) |
| C2b illegal event per state | CORE-8 (all events from all states), RS-5, QP-13 |
| C3 0/1/many | queue 0/1/5 (QP-3, QP-14, WS-3), profile 0/1/2 (PR-2, PR-3, PR-1), CORE-4 sizes 0..12 |
| C4a apply twice | QP-12, DC-5, RT-5, CORE-4 (idempotent reconcile) |
| C4b inverse without prerequisite | CORE-8 (Undo/Retract from states that never had the prerequisite) |
| C5a mode combinations | CORE-3 (full provenance Cartesian), QP-10 (failure postures), SI-13 (scope fallback mode) |
| C5b orthogonality | SI-13 (the scope mode changes disclosure + request only, never what is written) |
| C6a malformed input per param | SI-6 (handle), OW-5 (login), ED-2 (confidence), SH-6 (text), SI-10 (callback), PV-6 (csrf), PR-4 (profile id) |
| C6b each declared error | OW-2..5 (AC-002.3 messages), QP-10, RD-4 (3 reject reasons), OP-2 (3 refusal arms) |
| C6c closed error set | OP-5 (closed log-event catalogue), CORE-1/CORE-3 (closed verdict enums) |
| C7a degraded resource | QP-4 (GitHub busy), OW-4 (rate limit), QP-10 / PR-3 / FG-4 (PDS unreachable) |
| C7b interruption | OP-7 (restart mid-scan), QP-16 (restart + sign-out) |
| C7c concurrency | QP-18 (two scans at once), PV-4 (two journeys) |

Gaps: none in delivery scope. **SPECIFICATION_AMBIGUITY: none blocking** (DWD-9, DWD-10, DWD-11 are
pinned clarifications, listed for the reviewers). Telemetry: `(bluesky-claim-review-app, C1..C7, 0, none)`.

## Adapter coverage (Mandate 6)

| Adapter (driven) | Real-I/O scenario | Covered by |
|---|---|---|
| `adapter-review-store` (DuckDB, real file) | YES | every app scenario; restart QP-16/OP-7; purge FG-1/FG-6 |
| `adapter-atproto-oauth` (vs `FakeAtprotoNetwork` over real HTTP) | YES (contract double) | WS-1, SI-*, QP-15, FG-3/FG-5 |
| `adapter-github` (vs `FakeGithubAccounts` over real HTTP) | YES (contract double) | WS-2/3, OW-*, QP-4, RS-* |
| `adapter-atproto-ingest` (profile page reads) | YES | PR-1..4 |
| `adapter-atproto-did` resolve-identity | YES | WS-1, SI-5/SI-6, PR-4 |
| `adapter-atproto-pds` peer read (self-attested parse) | YES | WS-4, RD-1..5 |
| `adapter-index-store` provenance column | YES | RD-6/RD-7 |
| Live third-party systems | `@requires_external` | DEVOPS nightly live smoke (DV-BRA-12) — not acceptance |

## Pre-requisites DELIVER lands first (bootstrap slice)

1. Crates `review-domain`, `adapter-atproto-oauth`, `adapter-review-store`, `openlore-review-app` (+ `serve`, `gen-client-jwk`, `probe --self-test`).
2. The DWD-7 config seams and the DWD-6 page contract.
3. The app binary built before the cli acceptance stage (DWD-3).
4. Replace the `sut_*` bindings in `review_app_core.rs` one property at a time.
