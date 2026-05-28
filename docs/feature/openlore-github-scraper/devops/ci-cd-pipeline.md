# CI/CD Pipeline Delta — openlore-github-scraper (slice-02)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex
- **Tool**: GitHub Actions (UNCHANGED from D-D1)
- **Branching**: GitHub Flow (UNCHANGED from D-D7)

This is the slice-02 **delta** to `ci-cd-pipeline.md` (foundation) and the
slice-03 ci-cd delta. Read those first; this document describes only the
additions and the single-line modifications. No YAML is written here. DELIVER
lands the YAML into the EXISTING `ci.yml` and `nightly.yml` workflow files — no
new workflow file is created.

## 1. Workflow files (no new files)

| File | Triggers (UNCHANGED) | Slice-02 additions |
|---|---|---|
| `.github/workflows/ci.yml` | `pull_request`, `push: [main]` | Five new acceptance-stage jobs (§3.2–3.5); one new `contract-pact-github` sub-job (§3.6) |
| `.github/workflows/nightly.yml` | `schedule: cron 08:00 UTC daily`, `workflow_dispatch` | Mutation `--package` list grows to include `scraper-domain` (§4) |
| `.github/workflows/release.yml` | `push: tags: ['v*']` (lands when first tag cut) | Re-runs the new acceptance jobs as part of the existing acceptance re-run; `contract-pact-github` real/recorded-GitHub variant once at release (§5) |

> **Current state**: as of slice-01 step 06-08, `release.yml` is not yet
> committed (it lands when the first `vX.Y.Z` tag is cut). The §5 release-delta
> below is the spec for that file when it lands; it is forward-compatible with
> the slice-03 release-delta.

## 2. Commit-stage (UNCHANGED in command; rule sets expand)

`fmt`, `clippy`, `deny` (cargo-deny), `check-arch`, `check-probes`,
`test` (nextest workspace) all run unchanged in their CI job shape. Two existing
jobs gain expanded scope WITHOUT a CI-config change:

- **`check-arch`**: the `xtask check-arch` rule set extends per WD-65 — adds
  `scraper-domain` to the pure-core set and whitelists its pure YAML-parse
  dependency alongside `serde`; registers `adapter-github` as an effect adapter
  wired only by `cli`. Same command (`cargo run -p xtask -- check-arch`);
  expanded rule set. DELIVER lands the rule code in the `xtask` crate (and its
  positive/negative classifier tests, run by the existing
  `cargo test -p xtask` step that already precedes the check). The CI job
  invocation is unchanged.
- **`check-probes`**: covers the new `GithubPort` probe automatically — it's an
  AST walker over every `impl <Port> for <Adapter>`, so the new
  `impl GithubPort for AdapterGithub` block (with its non-stub five-step
  `probe()` body) is in scope by construction. No CI change needed.
- **`test`**: the new `mapping_matches_ssot` build-time/unit test (WD-67 —
  asserts the embedded signal→predicate snapshot matches the `jobs.yaml` SSOT)
  runs as an ordinary test within the existing nextest workspace run. No new job.

`deny` (cargo-deny) runs unchanged; the one new production dependency (the pure
YAML parser for `scraper-domain`) is MIT/Apache-2.0 and covered by the existing
allowlist (technology-stack.md §License compliance) — `adapter-github` reuses
the workspace `reqwest` and adds zero new transport surface (I-11).

## 3. Acceptance-stage additions

All five new ATs + the contract sub-job run in parallel within the existing
acceptance stage (the `test` job's lane in `ci.yml`), after the commit-stage
gates pass. Each is **blocking on PR** and **gates release**.

> Note on harness: the existing `ci.yml` `test` job runs
> `cargo nextest run --workspace --test-threads=1` (the `OPENLORE_TEST_NOW`
> serialization workaround). The new scrape ATs join the same workspace nextest
> run; DELIVER may keep them in that single job or split them into named jobs for
> clearer status checks. The §3.x entries below are the LOGICAL gates regardless
> of physical job packaging.

### 3.1 PR/nightly run UNAUTHENTICATED against FakeGithub

All PR and nightly scrape ATs run against the `wiremock`-driven FakeGithub
fixture set (`tests/fixtures/github/`) with NO `GITHUB_TOKEN` (anonymous path).
This is hermetic (no network), zero-rate-budget, and zero-flake. The
PAT-bearing path is exercised ONLY by the probe step-3 fixture (token-accepted
stub) and at release (§5). Rationale: D-D24 (FakeGithub for PR/nightly; recorded
real-GitHub for release).

### 3.2 `at-scraper-never-persists-unsigned` + `at-candidate-confidence-no-autoinflate` (KPI-SCR-2 guardrails)
- **Commands**: `cargo nextest run --test scraper_never_persists_unsigned` and `--test candidate_confidence_no_autoinflate`
- **What `scraper_never_persists_unsigned` does**: runs `scrape github <fixture-target>` WITHOUT `--sign` against FakeGithub; asserts `SELECT COUNT(*) FROM author_claims` is unchanged (zero new rows); asserts ZERO PDS writes occurred (mock PDS sees no `createRecord`); asserts ZERO files written to the claim store; asserts the candidate list WAS rendered to stdout (the in-memory ADTs materialized for display but not for persistence).
- **What `candidate_confidence_no_autoinflate` does**: derives candidates from the fixture signals; asserts every `CandidateClaim.confidence == 0.25` (numeric, WD-52); asserts NO candidate is proposed above 0.3; asserts that only a human edit raises the value (the derivation never emits >0.25).
- **Maps to**: KPI-SCR-2 (human-gate: zero unsigned persistence / auto-publish); WD-49, WD-52, WD-55
- **Type**: blocking GUARDRAIL
- **Wall-clock target**: < 20 s each

### 3.3 `at-candidate-names-source-signal` (KPI-SCR-3)
- **Command**: `cargo nextest run --test candidate_names_source_signal`
- **What it does**: derives candidates from a FakeGithub fixture whose signals include a case that collapses two signals into one candidate (US-SCR-002 Example 4); asserts every `CandidateClaim.source_signals` is non-empty; asserts the collapsed candidate lists ALL contributing signals (no truncation); asserts the derivation is deterministic over the SSOT mapping (same signals → same candidates across runs); asserts a structured-log `scrape.candidates.derived{count, source_signal_coverage}` event was emitted.
- **Maps to**: KPI-SCR-3 (auditability — every candidate names its source signal); WD-53 (SSOT mapping); the architecture-design §8 "Auditability" truncation risk
- **Type**: blocking
- **Wall-clock target**: < 20 s

### 3.4 `at-scraper-only-reads-public-data` (KPI-SCR-4 guardrail)
- **Command**: `cargo nextest run --test scraper_only_reads_public_data`
- **What it does**: configures FakeGithub with the `private_refusal` stub (returns 404 for a "private" path) and the `public_repo` stub; runs `scrape github <private-target>`; asserts `resolve_target` returns `GithubError::NotPublic` (NOT a silent empty harvest, NOT a generic 404); asserts the CLI surfaces "scraper only reads public data" and exits non-zero with ZERO candidates rendered; asserts that across a public-target harvest, the wiremock request log shows ONLY public-allowlist endpoints were hit (no authenticated-private path); asserts a `health.startup.refused` or `scrape.refused{reason: NotPublic}` event was emitted.
- **Maps to**: KPI-SCR-4 (public-data-only: zero private endpoint calls; private/non-existent refused); WD-51; probe step 2 (architecture-design §6.3)
- **Type**: blocking GUARDRAIL (trust)
- **Wall-clock target**: < 30 s
- **Fixture**: `tests/fixtures/github/private_refusal` (D-D25)

### 3.5 `at-scraper-reuses-slice01-publish-path` (single-publish-path)
- **Command**: `cargo nextest run --test scraper_reuses_slice01_publish_path`
- **What it does**: runs `scrape github <fixture-target> --sign 1` against FakeGithub (harvest) + mock PDS (publish); uses the build-time non-interactive sign guard (reused from slice-03's D-D20 pattern) to drive the human-signing gesture in CI; asserts the candidate was pre-filled into the EXISTING `VerbClaimAdd` compose preview (the literal "not as truth" framing appears — I-7); asserts the publish went through the SAME `VerbClaimPublish` internals (no parallel publish code path — WD-66); asserts the published claim's signed payload is byte-identical in shape to a hand-authored claim (display-only provenance adds no field — WD-62); asserts the publish-success message mentions the retract command (I-8); asserts a `claim.signed.from_scraper` event was emitted.
- **Maps to**: WD-66 (single publish path; `CandidatePrefill` is the only bridge); WD-49 (human-gate); WD-62 (display-only provenance — byte-identical payload); inherits I-7, I-8
- **Type**: blocking
- **Wall-clock target**: < 30 s

### 3.6 `contract-pact-github` (new sub-job under existing `contract-pact-pds`)
- **Command**: `cargo nextest run --test pact_github`
- **What it does**: extends the foundation + slice-03 Pact suite with consumer-driven contracts for the GitHub public read paths:
  - `GET /repos/{owner}/{repo}` (repository metadata)
  - `GET /repos/{owner}/{repo}/contents/{path}` (manifest/README presence)
  - tags/releases, languages
  - `GET /users/{user}`, `GET /users/{user}/repos` (the contributor-target paths)
  - (OR the GraphQL public equivalents if DELIVER picks GraphQL — Q-DELIVER-2; the allowlist assertion adapts to whichever transport)
  - Each pact covers happy path + the rate-limit (403) + token-rejected (401) response shapes (to exercise the remediation paths, US-SCR-004, and the no-token-leak assertion).
- **Public-endpoint allowlist assertion (KPI-SCR-4 release-gate)**: the test asserts that across ALL scrape operations exercised, `adapter-github` calls ONLY endpoints on the public allowlist. ANY observed off-allowlist (authenticated-private) endpoint call fails CI. This is THE KPI-SCR-4 release-gate at the contract layer (complementing the probe-layer gate in §3.4). Mechanism: the wiremock/recorded provider records every requested path; the assertion is `requested_paths ⊆ public_allowlist`. Zero private-endpoint calls.
- **No-token-leak assertion**: when the token-bearing variant runs (step-3 fixture / release), the test asserts the `GITHUB_TOKEN` value never appears in any captured request log echo, structured event, or stdout/stderr line.
- **Provider**: `FakeGithub` (wiremock) for PR/nightly read paths; a RECORDED real-GitHub response fixture (committed by DEVOPS to `tests/contracts/pact/github/`) replayed in PR for drift baseline; the real public GitHub API for the release-tag variant (§5).
- **Maps to**: KPI-SCR-4 (public-only); architecture-design §6.4; D-D22
- **Type**: blocking GUARDRAIL
- **Wall-clock target**: < 30 s (mocked/recorded); ~2 min for the real-GitHub variant (release-tag only)
- **Real-GitHub variant**: gated by `PACT_REAL_GITHUB=1` env var; runs in the release workflow only (per D-D24, mirrors D-D12). The new public-read endpoints are exercised against the real public GitHub API once per release — same manual-approval gate as the foundation/slice-03 real-PDS Pact.

### 3.7 Acceptance-stage summary (delta)

Net additions to foundation §4.6 + slice-03 §3.7:

| Stage | Wall-clock target | Type | Conditional? |
|---|---|---|---|
| at-scraper-never-persists-unsigned | < 20 s | blocking GUARDRAIL | no |
| at-candidate-confidence-no-autoinflate | < 20 s | blocking GUARDRAIL | no |
| at-candidate-names-source-signal | < 20 s | blocking | no |
| at-scraper-only-reads-public-data | < 30 s | blocking GUARDRAIL | no |
| at-scraper-reuses-slice01-publish-path | < 30 s | blocking | no |
| contract-pact-github (FakeGithub + recorded) | < 30 s | blocking GUARDRAIL | no |
| contract-pact-github (real GitHub) | ~2 min | manual approval at release | release-tag only |

Aggregate added wall-clock: **< 3 min per PR** (jobs parallelize within the
acceptance stage); release-tag overhead **~2 min**. Foundation's target
(< 30 min acceptance) is comfortably preserved.

## 4. Mutation testing (delta)

Per Apex Core Principle 9 + D-D8: nightly-only, scoped to pure-core. **This is
the one genuine scope change in slice-02 (D-D23).**

- The nightly `cargo mutants` invocation widens its `--package` list from
  `claim-domain` to `{claim-domain, scraper-domain}`. Concretely, the
  `cargo mutants -p claim-domain --no-shuffle --timeout 60` step in
  `nightly.yml` gains a second package: `cargo mutants -p claim-domain -p scraper-domain --no-shuffle --timeout 60` (or a second invocation — DELIVER's call).
- Kill-rate target for `scraper-domain`: **≥95%** (matches `claim-domain` per
  ADR-006 Earned Trust — the signal→predicate derivation is the load-bearing
  pure-core trust primitive of slice-02).
- `adapter-github` is NOT added to the mutation scope (effect shell; covered by
  the five-step probe gold-tests + the FakeGithub integration tests, per the
  D-D8 pure-core-only policy).
- Release-tag mutation re-run inherits the gate from D-D8 (blocking on
  regression).
- DELIVER updates the nightly workflow's `--package` list. No new gate
  semantics; just a wider scope.

Unlike slice-03 (which kept peer-claim verification inside `claim-domain`, so
its mutation scope was unchanged), slice-02 adds a GENUINELY NEW pure-core crate
(`scraper-domain`, WD-57/59) — so the scope MUST widen. This is the first
mutation-scope widening since slice-01.

## 5. Release workflow (delta)

Per `ci-cd-pipeline.md` (foundation) §7 + slice-03 §5. Slice-02 inserts:

- 5.1 The five new acceptance-stage jobs (§3.2–3.5) are re-run on the tagged ref as part of the existing acceptance re-run. No new step needed; they're already in the workflow.
- 5.2 `contract-pact-github` with `PACT_REAL_GITHUB=1` runs against the real public GitHub API once at release-tag time, gated by the same manual-approval environment used for the foundation/slice-03 real-PDS Pact (per D-D24/D-D12). Solo dev clicks Approve once; the GitHub read-path contracts run against real public GitHub. The public-endpoint allowlist assertion runs in BOTH the mocked and the real variant.
- 5.3 The nightly mutation re-run for `scraper-domain` (§4) is re-run at release-tag time under the same D-D8 blocking-on-regression rule.
- 5.4 Substrate matrix: NO new cells; each cell now also exercises the `scrape github` happy path against the FakeGithub fixture (per `platform-design.md` delta §6). Same job, expanded body.

Estimated release wall-clock (delta): **+3 to +5 min** (the five scrape ATs plus
the real-GitHub contract variant + the `scraper-domain` mutation re-run).
Slice-03 estimate was 18–35 min; new estimate 20–38 min. Acceptable.

## 6. Quality-gate enforcement summary (delta rows only)

Insert these rows into the foundation table at §9 (after the slice-03 rows):

| Gate | Pre-PR (local) | PR | Nightly | Release-tag |
|---|---|---|---|---|
| at-scraper-never-persists-unsigned | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-candidate-confidence-no-autoinflate | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-candidate-names-source-signal | – | ✓ blocking | – | ✓ blocking |
| at-scraper-only-reads-public-data | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-scraper-reuses-slice01-publish-path | – | ✓ blocking | – | ✓ blocking |
| contract-pact-github (FakeGithub + recorded) | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| contract-pact-github (real GitHub) | – | – | – | ✓ manual approval |
| mutation testing (scraper-domain) | – | – | ✓ advisory | ✓ blocking on regression |
| mapping_matches_ssot | (runs in pre-push unit suite) | ✓ (in `test` job) | – | ✓ |

The "Pre-PR (local)" column is intentionally empty for the new acceptance gates
(too slow for pre-push; foundation pre-push runs only unit + property + arch).
The `mapping_matches_ssot` test IS in the unit suite, so it runs at pre-push as
part of the foundation pre-push hook (unchanged hook config; new test in scope).
The pre-commit and pre-push hook designs from foundation §5 are otherwise
unchanged.

## 7. FakeGithub fixture + maintenance helper

The FakeGithub fixture set is the load-bearing CI infrastructure for KPI-SCR-4
(and the probe gold-tests). Per `platform-design.md` delta §7 + D-D25:

- **Location**: `tests/fixtures/github/`
- **Contents** (5 wiremock stubs, mapping 1:1 to the five probe steps +
  failure modes — architecture-design §6.2/§6.3):
  - `public_repo` — happy-path public repo response (`rust-lang/cargo`-shaped); probe step 1 + harvest happy path.
  - `public_user` — public user + repos response; the contributor-target path (US-SCR-001 Example 2).
  - `private_refusal` — 404 for a "private" path; probe step 2 + the `scraper_only_reads_public_data` AT (THE KPI-SCR-4 release-gate at the probe layer).
  - `rate_limited` — 403 + rate-limit headers; the `RateLimited` remediation path (US-SCR-004; "set GITHUB_TOKEN for higher limits"; no partial candidate list).
  - `token_rejected` — 401; the stale-PAT fast-fail path (probe step 3 + the no-token-leak assertion).
- **Recorded-real provider**: `tests/contracts/pact/github/` holds the once-captured real-GitHub response fixture (DEVOPS captures; DELIVER consumes) for the contract drift baseline.
- **Maintenance (OPTIONAL — DELIVER-may-defer)**:
  `cargo xtask regenerate-github-fixtures` refreshes the stub bodies from a
  captured real-GitHub response when the API shape evolves; mirrors slice-03's
  `cargo xtask regenerate-peer-fixtures` (D-D15). If DELIVER's scope is tight,
  defer the regenerator — the fixtures work without it; they just risk drift.
  The `mapping_matches_ssot` drift guard (WD-67) is UNRELATED to this and is NOT
  optional.

## 8. PAT handling in CI

Per WD-54/WD-63 (env-only `GITHUB_TOKEN`) + D-D24:

- **PR / nightly**: NO `GITHUB_TOKEN`. Scrape ATs run UNAUTHENTICATED against
  FakeGithub. The token-bearing path is exercised only by the `token_rejected`
  (401) and a token-accepted probe step-3 fixture — both stubbed, no real token.
- **Release-tag**: the real-GitHub contract variant (§5.2) uses the repo's
  `GITHUB_TOKEN` secret (the GitHub Actions auto-provided token, or a
  least-privilege fixture PAT) ONLY for the higher rate budget against real
  public GitHub. It is used for the recorded-fixture-vs-real drift check; it is
  NEVER written into a committed fixture, a claim, or a log line. The
  no-token-leak contract assertion (§3.6) runs in this context.
- **Secrets management**: the token is referenced as `${{ secrets.GITHUB_TOKEN }}`
  (or a dedicated repo secret), never hardcoded; never echoed; passed only as a
  request header in `adapter-github`. This matches the foundation secrets policy
  (never commit secrets; the PAT is an ephemeral effect-shell credential, NOT in
  the OS keychain — that is for the signing key, ADR-002).

## 9. Branch protection rules (UNCHANGED)

Foundation §10 + slice-03 §8 rules carry forward unchanged. The new acceptance
jobs (and `contract-pact-github`) are added to the "required status checks" list
at the same level as the existing acceptance jobs.

## 10. `deny.toml` (UNCHANGED in policy; one dependency to vet)

Foundation §11 + slice-03 §9 content unchanged. Slice-02 adds ONE new
production dependency (the pure YAML parser for `scraper-domain` — `serde_yaml`
or a maintained fork). It is MIT/Apache-2.0 and covered by the existing
allowlist; DELIVER MUST run `cargo deny check` after adding it and confirm no
new advisory/source surface (ADR-019 acceptance criterion). `adapter-github`
reuses the workspace `reqwest` and adds zero new transport surface.

## 11. References

- `platform-design.md` (sibling, this dir) — gate-inventory delta + risk register
- `observability.md` (sibling, this dir) — the new scrape events these tests assert on
- `kpi-instrumentation.md` (sibling, this dir) — KPI-SCR gate mapping
- `wave-decisions.md` (sibling, this dir) — D-D22..D-D29
- Foundation `ci-cd-pipeline.md` + slice-03 `ci-cd-pipeline.md` delta — the base to extend
- `docs/feature/openlore-github-scraper/design/architecture-design.md` §6.2/§6.3/§6.4 (endpoints, probe, contract-test recommendation)
- `docs/feature/openlore-github-scraper/discuss/outcome-kpis.md` (KPI-SCR-1..5)
