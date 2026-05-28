# Wave Decisions — DEVOPS — openlore-github-scraper (slice-02)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-github-scraper (sibling slice-02; walking-skeleton-for-scraper)
- **Inherits**: openlore-foundation DEVOPS D-D1..D-D13 (all LOCKED, carry forward unchanged), ADR-010..ADR-012 (all in force unchanged); openlore-federated-read DEVOPS D-D14..D-D21 (all LOCKED, carry forward unchanged)

This file is the DEVOPS-wave decision log for slice-02. Decisions are
numbered **D-D22 onward** to continue the sequence after slice-03's D-D21.
None of the foundation (D-D1..D-D13) or slice-03 (D-D14..D-D21) decisions are
re-opened or amended.

> **Sequencing note**: slice-02 is numbered before slice-03 in the carpaccio
> split (WD-13) but ships AFTER it; the DEVOPS decision numbers follow ship
> order, so slice-02's decisions are D-D22+ (after slice-03's D-D14..D-D21),
> not interleaved with slice-03's.

## Inheritance

All foundation DEVOPS decisions (D-D1..D-D13) AND all slice-03 DEVOPS
decisions (D-D14..D-D21) carry forward verbatim:

**Foundation (D-D1..D-D13):**

- D-D1 (GitHub Actions + GitHub Flow + `v*` tag releases) → unchanged
- D-D2 (tracing + tracing-subscriber + tracing-appender + JSON Lines local) → unchanged; slice-02 emits additional events into the SAME pipeline (no new endpoint)
- D-D3 (distributed tracing skipped) → unchanged; slice-02 is still single-binary; `scrape github` is one HTTP harvest from the user's machine — nothing distributed
- D-D4 (telemetry opt-in OFF by default, no endpoint) → unchanged
- D-D5 (`openlore stats` verb is DELIVER's call; jq fallback) → unchanged; slice-02 adds a `--scraper` flag to the verb-or-fallback design
- D-D6 (no capacity/perf/stress/chaos) → unchanged; `scrape github` is a single bounded harvest; no >2s single-op KPI breach reported; same revisit trigger
- D-D7 (GitHub Flow branching) → unchanged
- D-D8 (mutation testing nightly-only, release-tag blocking) → unchanged in policy; SCOPE widens to add `scraper-domain` (the new pure-core crate) — see D-D23
- D-D9 (4-cell PR substrate, 8-cell release substrate, +tmpfs/overlayfs nightly) → unchanged in shape; per-cell body extended with a `scrape-github` happy-path against the FakeGithub fixture
- D-D10 (cargo install primary + 4-platform binaries; Homebrew/AUR/nix deferred; Windows out) → unchanged
- D-D11 (cosign + CycloneDX SBOM + SLSA L2 minimum / L3 target) → unchanged
- D-D12 (Pact mocked in PR/nightly; real bsky.social in release with manual approval) → unchanged in POLICY; slice-02's GitHub contract suite inherits the SAME gating shape (FakeGithub fixture in PR/nightly; recorded real-GitHub fixture in release) — see D-D24
- D-D13 (no KPI marked RED; KPI-3 and KPI-6 are YELLOW for cohort) → carried forward as the SAME policy applied to KPI-SCR-1..5; see D-D26

**Slice-03 (D-D14..D-D21):**

- D-D14 (peer-DID resolution at startup probe = user's OWN DID only) → unchanged; orthogonal to slice-02 (no peer DID involved in GitHub harvest)
- D-D15 (adversarial-peer fixture in `tests/fixtures/peer-adversarial/`, `xtask`-regenerated) → unchanged; slice-02 adds an ANALOGOUS but distinct GitHub fixture set — see D-D25
- D-D16 (KPI-FED-5 post-hoc jq aggregation, no state file) → unchanged; informs the slice-02 KPI-SCR-1 duration-measurement default — see D-D27
- D-D17 (KPI-FED feasibility GREEN/YELLOW policy) → the SAME policy framework applied to KPI-SCR-1..5 — see D-D26
- D-D18 (one-shot Likert survey after first event, file-presence pattern) → unchanged; reused unmodified for the KPI-SCR-1/KPI-SCR-5 day-30 think-aloud prompt — see D-D27
- D-D19 (renderer-review checklist at release time) → unchanged; slice-02 adds ONE line to the same checklist (candidate renderer must list ALL source signals — KPI-SCR-3) — see D-D28
- D-D20 (CI test-only escape hatch via build-time guard) → unchanged; slice-02 reuses the SAME pattern for the `--sign` non-interactive test path — see D-D25
- D-D21 (no new ADR at DEVOPS layer) → the SAME outcome holds for slice-02 — see D-D29

## Locked slice-02 decisions

| # | Decision | Rationale | Status | ADR / Document |
|---|----------|-----------|--------|----------------|
| D-D22 | **GitHub public-API consumer-driven contract test** lands as a new acceptance sub-job `contract-pact-github` under the existing `contract-pact-pds` umbrella, extending the slice-01+slice-03 Pact-style suite (NOT a separate suite). It carries a **public-endpoint allowlist assertion** as the KPI-SCR-4 release-gate: the test fails CI if `adapter-github` is observed calling ANY endpoint not on the public allowlist (`GET /repos/{owner}/{repo}`, `/contents/{path}`, tags/releases, languages, `GET /users/{user}`, `/users/{user}/repos`, or the GraphQL public equivalents). NO authenticated-private endpoint is ever reachable. | KPI-SCR-4 (public-data-only) is a release-blocking guardrail. The contract test is the ONLY mechanism that asserts the END-TO-END harvest path touches only public endpoints — code review of `adapter-github` is insufficient because a future endpoint addition could silently touch a private path. Mirrors slice-03's D-D12 Pact gating policy + the adversarial-fixture-as-guardrail pattern (D-D15). | LOCKED | `ci-cd-pipeline.md` delta §3.1, §3.6; `kpi-instrumentation.md` delta §6 (KPI-SCR-4) |
| D-D23 | **Mutation scope widens: `scraper-domain` is added to the nightly `cargo mutants --package` list.** Kill-rate target **≥95%** (matches `claim-domain` per ADR-006 Earned Trust — the signal→predicate derivation is the load-bearing pure-core trust primitive of slice-02). Release-tag mutation re-run inherits the D-D8 blocking-on-regression gate. `adapter-github` is NOT mutated (effect shell; covered by probe gold-tests + integration tests, per D-D8 pure-core-only policy). | D-D8 scopes mutation to pure-core. Unlike slice-03 (which kept peer-verify inside `claim-domain`, so no scope change), slice-02 adds a GENUINELY NEW pure-core crate (`scraper-domain`, WD-57/WD-59) — it MUST enter the mutation `--package` list or its derivation correctness is unguarded by mutation. This is the first mutation-scope widening since slice-01. | LOCKED | `ci-cd-pipeline.md` delta §4; `platform-design.md` delta §6 |
| D-D24 | **FakeGithub fixture for PR/nightly; recorded real-GitHub fixture for release.** PR and nightly runs exercise the harvest + probe + contract paths UNAUTHENTICATED against a `wiremock`-driven `FakeGithub` stub (no network, no PAT). The release workflow additionally replays the GitHub contract against a RECORDED real-GitHub response fixture (captured once by DEVOPS, committed); a `PACT_REAL_GITHUB=1`-gated variant MAY hit the real public GitHub API at release-tag time under the same manual-approval environment as the slice-03 real-bsky Pact (D-D12). | Mirrors D-D12 exactly: mocked in PR/nightly for speed + hermeticity; real provider at release to catch contract drift before users do. The FakeGithub stub IS the slice-02 analogue of slice-03's wiremock peer fixture. Recorded-real fixture is the default release gate (zero-flake, zero-rate-budget); the live-real variant is an opt-in manual escalation. | LOCKED | `ci-cd-pipeline.md` delta §3.1, §3.6, §5; `platform-design.md` delta §7 |
| D-D25 | **GitHub fixture set lives in `tests/fixtures/github/`** as wiremock-driven stubs: (a) `public_repo` (happy-path public `rust-lang/cargo`-shaped response), (b) `public_user` (public user + repos), (c) `private_refusal` (404 for a "private" path — the KPI-SCR-4 probe-layer gate), (d) `rate_limited` (403 + rate-limit headers), (e) `token_rejected` (401). DELIVER's `--sign` acceptance tests reuse the slice-03 build-time non-interactive guard pattern (D-D20) for the human-signing gesture. | KPI-SCR-4 is release-blocking; the `private_refusal` fixture is the only end-to-end exercise of the refuse-private path (probe step 2 + the `scraper_only_reads_public_data` AT). The five fixtures map 1:1 to the five `adapter-github` probe steps (architecture-design §6.3) and the failure modes (§6.2). The `--sign` tests need a non-interactive sign path; the D-D20 build-time guard already exists and forbids `--yes` in release builds. | LOCKED with DELIVER-may-defer escape hatch on the live-real-GitHub variant | `ci-cd-pipeline.md` delta §3.2–3.5, §7; `platform-design.md` delta §7 |
| D-D26 | **KPI-SCR feasibility: no KPI-SCR marked RED.** KPI-SCR-2 (human-gate), KPI-SCR-3 (auditability), KPI-SCR-4 (public-data-only) = **GREEN** (each is a CI gate). KPI-SCR-1 (cost-to-first-claim) = **GREEN per-user / YELLOW cohort-percentile** (per-user duration readable from the local log; cohort percentiles need the future telemetry endpoint). KPI-SCR-5 (edit-rate ≥50%) = **GREEN per-user / YELLOW cohort** (per-user diff captured locally; cohort rate needs the endpoint OR PO day-30 outreach). | All 5 KPI-SCR have designed capture in slice-02. The two YELLOW items (KPI-SCR-1 cohort percentile, KPI-SCR-5 cohort rate) mirror the slice-03 D-D17 / foundation KPI-3/KPI-6 deferral — a deferred-endpoint constraint, NOT a capture gap. The two GUARDRAILS (KPI-SCR-2, KPI-SCR-4) are both fully GREEN and release-blocking. | LOCKED | `kpi-instrumentation.md` delta §1, §9 |
| D-D27 | **KPI-SCR-1 (scrape→sign duration) measurement default = POST-HOC jq aggregation** over `scrape.harvest.completed`, `scrape.candidates.derived`, and `claim.signed.from_scraper` events joined by a per-invocation `scrape_id` within a session window. NO state file (mirrors D-D16). The 30-day think-aloud uses the SAME one-shot Likert survey mechanism as D-D18 (delivered after the first `claim.signed.from_scraper`). | Keeps state out of the binary; post-hoc aggregation is exact (events carry timestamps). Identical reasoning to slice-03's D-D16 for KPI-FED-5. DESIGN already resolved that the candidate→compose bridge reuses `VerbClaimAdd`/`VerbClaimPublish` (WD-66), so the sign timestamp is the existing `sign.success`/`publish.*` boundary — no new sign-side instrumentation needed beyond the `from_scraper` tag. | LOCKED with DESIGN-already-answered note | `observability.md` delta §2.5; `kpi-instrumentation.md` delta §2 (KPI-SCR-1) |
| D-D28 | **KPI-SCR-3 auditability gets BOTH a CI gate AND a runtime guardrail counter.** CI: AT `candidate_names_source_signal` asserts every derived `CandidateClaim.source_signals` is non-empty AND a collapsed-multi-signal candidate lists ALL contributing signals. Runtime: counter `scraper_candidate_missing_source_total` (target = 0 forever; non-zero is a P0 bug). The renderer-review checklist (D-D19) gains one line: "candidate renderer lists ALL source signals (no truncation)". | KPI-SCR-3 is a leading-indicator (precondition for KPI-SCR-1 trust). The collapsed-signal truncation risk (architecture-design §8 "Auditability" row) is a renderer hazard that the AT alone covers for current renderers but the checklist backstops for future renderers — exactly the D-D19 reasoning for KPI-FED-2. | LOCKED | `ci-cd-pipeline.md` delta §3.3; `observability.md` delta §4; `kpi-instrumentation.md` delta §4 |
| D-D29 | **No new ADR at the DEVOPS layer.** ADR-010, ADR-011, ADR-012 carry forward unchanged. Slice-02's DESIGN wave already raised ADR-017/018/019 (verb contract, candidate model, GitHub adapter) — those are DESIGN ADRs. Slice-02's DEVOPS decisions (new contract sub-job, mutation scope widening, FakeGithub fixtures, KPI-SCR instrumentation events) are CI/observability tactical extensions of existing decisions; none crosses the DEVOPS-ADR threshold. | ADR convention: cross-slice or cross-component architectural decisions. The GitHub adapter trust-boundary decision IS an ADR — but it's ADR-019 (DESIGN), not a DEVOPS ADR. The mutation-scope widening and the new contract sub-job are tactical applications of D-D8 and D-D12, not new axes. Same outcome as slice-03's D-D21. | LOCKED | `platform-design.md` delta §9 |

## Proposed (awaiting user confirmation)

None. All slice-02 DEVOPS decisions are LOCKED. (In auto-mode the recommended
verdicts are taken per the auto-mode product-defaults instruction; the user may
override any D-D22..D-D29 on review.)

## Open questions (handed to DESIGN — already answered; recorded for traceability)

DESIGN ran in parallel and has already resolved every cross-wave question that
DEVOPS would otherwise hand back. Recorded here so the trace is complete:

1. **Verb shape** (`scrape github` sugar verb) — RESOLVED by WD-60 / ADR-017. The contract test + instrumentation target this verb.
2. **`GithubPort` as a new port vs `PdsPort` extension** — RESOLVED by WD-61 / ADR-019 (NEW port). The probe + contract test target `adapter-github`'s own boundary, not `PdsPort`.
3. **`derived-from` provenance display-only vs signed-payload field** — RESOLVED by WD-62 / ADR-018 (display-only). DEVOPS instruments the display-only line; NO Lexicon-conformance contract test needed (the signed payload is byte-identical to a hand-authored claim, so the existing slice-01 `kpi-4-roundtrip` gate covers it unchanged).
4. **PAT surface** (`GITHUB_TOKEN` env-only) — RESOLVED by WD-63 / ADR-019. DEVOPS handles the token in CI via env (release fixture only); PR/nightly run unauthenticated (D-D24).
5. **`harvest_user` page-walk cap** (bounded aggregate) — RESOLVED by WD-64 (bounded; cap is Q-DELIVER-4). DEVOPS instruments per-target-type bucket (small repo / large repo / user) regardless of the exact cap.
6. **Pure YAML parser placement + drift guard** — RESOLVED by WD-65 + WD-67 (`scraper-domain` whitelisted in `check-arch`; `mapping_matches_ssot` build-time test). DEVOPS adds `scraper-domain` to mutation scope (D-D23); the `mapping_matches_ssot` test runs in the existing test stage (no new CI job).

## Open questions (handed to DELIVER)

These are deliberately deferred to DELIVER. DEVOPS has defaults; DELIVER decides.

1. **FakeGithub mock library + version pin** (`wiremock` or equivalent) and the live-vs-recorded GitHub fixture split — coordinate with this DEVOPS doc; recorded fixtures for CI, real public GitHub for the release variant (D-D24, Q-DELIVER-6 in DESIGN).
2. **`scrape_id` correlation field** for the KPI-SCR-1 post-hoc join (D-D27) — DELIVER threads a per-invocation UUID through the `scrape.*` and the reused `sign.success`/`publish.*` events, OR DEVOPS's jq does a timestamp-window join with no correlation field. Default: thread a `scrape_id` (cheap, removes window ambiguity).
3. **`scripts/kpi-scr-{1,3,4,5}.jq` snippets** — DELIVER lands these alongside the foundation `kpi-{1,2,4,5}.jq` and slice-03 `kpi-fed-*.jq` snippets.
4. **Recorded real-GitHub fixture for the contract suite** — DEVOPS captures this manually once (one-time setup of public `rust-lang/cargo` + a public user response) and commits to `tests/contracts/pact/github/`. DELIVER does NOT regenerate; consumes the recording.
5. **`openlore stats --scraper` flag implementation** — concrete only if D-D5 verb landed; otherwise the `scripts/kpi-scr-*.jq` snippets are the fallback.
6. **GitHub fixture regeneration helper** (`cargo xtask regenerate-github-fixtures`) — OPTIONAL, mirrors slice-03's `regenerate-peer-fixtures` (D-D15); DELIVER may defer (the fixtures work without it; they just risk drift as the GitHub API shape evolves). The `mapping_matches_ssot` drift guard is unrelated and is NOT optional.

## Out of scope for DEVOPS slice-02 (explicit deferrals)

All foundation + slice-03 deferrals (SLOs/SLAs, runbooks, dashboards, telemetry
endpoint, auto-updater, multi-tenancy, DR, capacity, chaos, Windows, push-based
federation) carry forward unchanged. Slice-02 adds these explicit deferrals:

- **Multi-source scrapers** (Mastodon, blogs, etc.): NOT designed. Slice-02 is GitHub-only (story-map "What is NOT in scope"). The contract test + fixtures are GitHub-specific by design.
- **PAT in OS keychain / config-file token**: NOT designed (WD-63 env-only). The token is an effect-shell env-var credential; keychain is for the signing key (ADR-002).
- **Deep cross-repo contributor triangulation instrumentation**: NOT designed (WD-64 bounded aggregate; deep triangulation is slice-04). Per-target-type buckets are coarse (small repo / large repo / user) and sufficient for KPI-SCR-1.
- **Cohort dashboards for KPI-SCR-1 / KPI-SCR-5**: NOT designed (no central aggregation; per ADR-010). Per-user `openlore stats --scraper` + jq fallback is the slice-02 surface. Cohort is a future-telemetry-endpoint problem.
- **GitHub rate-budget alerting / surveillance-pattern detection**: NOT designed. The system reads only public data (KPI-SCR-4) and refuses private targets; there is no aggregation surface that could become a surveillance affordance. Re-evaluate at slice-04/05 when scoring widens the surface.
- **Live-real-GitHub contract in nightly**: NOT designed. The real-GitHub variant runs at release-tag time only (D-D24), under manual approval, to preserve the anon rate budget and avoid flake. Nightly uses FakeGithub.

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (software-crafter — functional, per ADR-007) | every slice-02 DEVOPS doc + every slice-02 DESIGN doc (WD-59..68, ADR-017..019) + slice-01/03 carryover + the Q-DELIVER-1..7 list | Additions to `.github/workflows/ci.yml` (new acceptance jobs §3 + the `contract-pact-github` sub-job) and `.github/workflows/nightly.yml` (mutation `--package` += `scraper-domain`); `tests/fixtures/github/` (5 wiremock stubs: public_repo, public_user, private_refusal, rate_limited, token_rejected); `tests/contracts/pact/github/` (consumed from the DEVOPS one-time recorded fixture); `scripts/kpi-scr-*.jq` snippets; `tracing` event emission code at the scrape boundaries (`scrape.harvest.completed`, `scrape.candidates.derived`, `claim.signed.from_scraper`, etc.); the `scrape_id` correlation field; one line added to the renderer-review checklist; optional `cargo xtask regenerate-github-fixtures`. |
| DISTILL (nw-acceptance-designer) | the five integration gates (`scraper_never_persists_unsigned`, `candidate_names_source_signal`, `scraper_only_reads_public_data`, `candidate_confidence_no_autoinflate`, `scraper_reuses_slice01_publish_path`) + the FakeGithub fixture set (DEVOPS provides) + the contract-test allowlist | Executable acceptance tests consuming the DEVOPS-owned FakeGithub fixtures and the public-endpoint allowlist assertion |
| Operations team (POST-DELIVER) | not applicable — still local-first CLI, no operations team for slice-02 | not applicable |
| Future DEVOPS wave (slice-05 AppView or whichever sibling stands up the telemetry endpoint) | this doc + foundation/slice-03 DEVOPS docs + ADR-010 + `kpi-instrumentation.md` event-shape definitions | cohort aggregation for KPI-SCR-1 (percentiles) and KPI-SCR-5 (edit-rate) — the two slice-02 YELLOWs |

## Changelog

- 2026-05-28 — Apex — initial DEVOPS-wave decisions for slice-02 (openlore-github-scraper). All decisions D-D22..D-D29 LOCKED. No new DEVOPS ADRs proposed (D-D29). Foundation D-D1..D-D13 + ADR-010..012 and slice-03 D-D14..D-D21 carry forward unchanged. Mutation scope widened to add `scraper-domain` (D-D23) — first scope widening since slice-01; `CLAUDE.md` Mutation Testing Strategy section unchanged in POLICY (nightly-only per D-D8), only the `--package` list grows (a workflow-file edit, not a strategy change).
