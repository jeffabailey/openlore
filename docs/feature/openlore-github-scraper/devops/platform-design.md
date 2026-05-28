# Platform Design Delta — openlore-github-scraper (slice-02)

- **Wave**: DEVOPS (design portion; sibling-feature extension of slice-01 + slice-03 DEVOPS)
- **Date**: 2026-05-28
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-github-scraper (sibling slice-02; walking-skeleton-for-scraper)
- **Inherits**: openlore-foundation DEVOPS (D-D1..D-D13, ADR-010..012) + openlore-federated-read DEVOPS (D-D14..D-D21) — all UNCHANGED
- **Paradigm context**: functional Rust (ADR-007, Accepted)

This is the DEVOPS platform-design **delta** for slice-02. The slice-01
platform layer (operating model, branching, gate inventory, distribution,
substrate matrix) and the slice-03 extensions are **unchanged**. This
document records only the NEW extensions. Read in conjunction with
`docs/feature/openlore-foundation/devops/platform-design.md` and
`docs/feature/openlore-federated-read/devops/platform-design.md`.

## 1. What did NOT change

| Concern | Status | Reference |
|---|---|---|
| Operating model (local-first, solo-dev, no SLOs, no on-call) | UNCHANGED | foundation §1 |
| DORA framing (per-release tag, no fleet) | UNCHANGED | foundation §1 |
| CI tool (GitHub Actions) + branching (GitHub Flow) | UNCHANGED | foundation §7, D-D1 |
| Distribution (`cargo install` + 4-platform binaries; Windows out) | UNCHANGED | foundation §3 row 4, ADR-011 |
| Substrate gold matrix (8 cells release; 4 PR) | UNCHANGED in shape; one row extended | foundation §3 row 5; this doc §6 |
| Release artifact security (cosign + SBOM + SLSA) | UNCHANGED | ADR-012 |
| Telemetry opt-in policy (off by default; no endpoint operated) | UNCHANGED | ADR-010 |
| Local quality gates (lefthook/pre-commit + pre-push) | UNCHANGED | foundation §5 |
| Quality-gate inventory taxonomy | UNCHANGED in shape; entries added | foundation §6; this doc §3 |
| Mutation testing policy (nightly-only, release-tag blocking) | UNCHANGED in POLICY; scope widens | D-D8; this doc §6 |
| Pact gating policy (mock in PR/nightly; real provider at release) | UNCHANGED in POLICY; applied to GitHub | D-D12; this doc §7 |
| No new operational services (still CLI-first, no daemon) | UNCHANGED | foundation §1; this doc §2 |

## 2. What DID change (the delta)

Slice-02 introduces three new platform-layer concerns:

| New concern | Where it lives | Why slice-02 introduces it |
|---|---|---|
| GitHub public-API contract-test surface (a new external provider beyond ATProto) | `contract-pact-github` CI sub-job (new), with a **public-endpoint allowlist assertion** as the KPI-SCR-4 release-gate | The scraper harvests over a wholly new external system (GitHub) behind a NEW `GithubPort` (WD-61) — a new external-contract surface unrelated to slice-01/03's ATProto write/read paths |
| FakeGithub fixture set (public/private/rate-limit/token-rejected stubs) | `tests/fixtures/github/` (wiremock-driven); exercised by the new scrape acceptance tests + the `adapter-github` probe gold-tests | KPI-SCR-4 (public-only) + the five-step probe contract (architecture-design §6.3) are release-blocking; no fixture = no enforcement. The FakeGithub stub is the slice-02 analogue of slice-03's adversarial-peer fixture (D-D15) |
| Five KPI-SCR instrumentation surfaces (KPI-SCR-1..5) | `tracing` events emitted at harvest, derive, and the reused sign/publish boundaries (`scrape.harvest.completed`, `scrape.candidates.derived`, `claim.signed.from_scraper`) | The slice has its own outcome KPIs (`discuss/outcome-kpis.md`); foundation KPI-1..6 and slice-03 KPI-FED-1..6 do not subsume them |
| Mutation scope grows by one pure-core crate | nightly `cargo mutants --package scraper-domain` | `scraper-domain` is a GENUINELY NEW pure-core crate (WD-57/59) — the first since slice-01; its derivation correctness must be mutation-guarded (D-D23) |

Everything else is additive within existing structures — new CI jobs in
existing workflows, new probe assertions in the new `adapter-github` probe,
new `tracing` event names emitted from new code paths the DESIGN wave
designs in parallel. NO new workflow file. NO new operational service. NO
daemon. The binary is still the same single Rust CLI (architecture-design §7).

## 3. Quality-gate inventory delta

Net additions to foundation §6 + slice-03 §3 (no rows removed; no semantics changed):

| Category | Where | Type | What it gates | Origin |
|---|---|---|---|---|
| CI | acceptance-stage job (new AT `scraper_never_persists_unsigned`) | blocking (GUARDRAIL — KPI-SCR-2) | `scrape github` WITHOUT `--sign` writes zero `author_claims` rows, zero PDS writes, zero files | this slice §3, ci-cd §3.2 |
| CI | acceptance-stage job (new AT `candidate_confidence_no_autoinflate`) | blocking (GUARDRAIL — KPI-SCR-2) | every derived candidate is stamped at numeric 0.25; no candidate proposed above 0.3; only the human raises it | this slice §3, ci-cd §3.2 |
| CI | acceptance-stage job (new AT `candidate_names_source_signal`) | blocking (KPI-SCR-3) | every `CandidateClaim.source_signals` is non-empty; a collapsed-multi-signal candidate lists ALL contributing signals | this slice §3, ci-cd §3.3 |
| CI | acceptance-stage job (new AT `scraper_only_reads_public_data`) | blocking (GUARDRAIL — KPI-SCR-4) | private/non-existent target refused with "scraper only reads public data"; no private endpoint reached | this slice §3, ci-cd §3.4 |
| CI | acceptance-stage job (new AT `scraper_reuses_slice01_publish_path`) | blocking | `--sign` routes a candidate through the EXISTING `VerbClaimAdd`/`VerbClaimPublish` internals (WD-66); no parallel publish path | this slice §3, ci-cd §3.5 |
| CI | acceptance-stage job (new sub-job `contract-pact-github`) | blocking (GUARDRAIL — KPI-SCR-4) | consumer-driven contract for the GitHub public read paths + the **public-endpoint allowlist assertion** (zero off-allowlist calls) | this slice §7, ci-cd §3.6 |
| CI (nightly) | mutation-stage scope expansion | advisory (release-tag: blocking on regression) | `cargo mutants` extended to include `scraper-domain` (kill-rate ≥95%) | this slice §6, ci-cd §4 |

All foundation + slice-03 gates (`fmt`, `lint`, `supply-chain`, `arch-check`,
`probe-check`, `test-unit`, `test-property`, `kpi-4-roundtrip`, `kpi-5-offline`,
`test-integration-pds`, `contract-pact-pds`, `contract-pact-pds-peer`, the five
slice-03 peer ATs) remain unchanged in command and gating semantics.

Note: the `mapping_matches_ssot` build-time test (WD-67 — asserts the embedded
signal→predicate snapshot matches the `jobs.yaml` SSOT) runs in the EXISTING
`test` stage as an ordinary unit test; it does NOT need a new CI job. The
`check-arch` allowlist extension for `scraper-domain` (WD-65) runs in the
EXISTING `check-arch` stage — same command, expanded rule set.

## 4. Constraint Impact Analysis (delta)

Three new constraints surface in slice-02; one inherited constraint gains weight:

| Constraint | Source | % delivery affected | Priority | New / changed? |
|---|---|---|---|---|
| Public-data-only invariant (KPI-SCR-4 guardrail) | DISCUSS KPI-SCR-4, WD-51; DESIGN architecture-design §6.2/§6.3 | 100% of harvest paths | HIGH | NEW |
| Human-gate invariant (KPI-SCR-2 guardrail — nothing persisted/published unsigned) | DISCUSS KPI-SCR-2, WD-49/WD-55; DESIGN WD-66 (single publish path) | 100% of scrape paths | HIGH | NEW |
| GitHub-can-lie-about-access (private 404 vs missing 404; rate-limit vs transport error) | DESIGN architecture-design §6.3 (probe exercises the lies) | every harvest + every probe | HIGH | NEW (the only novel substrate risk in slice-02) |
| New external contract surface (GitHub public API) added to the contract suite | DESIGN architecture-design §6.4 extends `ports` with `GithubPort` | every release | MEDIUM | NEW (new provider to contract-test; analogous to slice-03's MEDIUM `listRecords` addition) |
| Pure-core has zero I/O imports (now applies to `scraper-domain`) | ADR-007, ADR-009, WD-65 | every PR (CI gate) | HIGH | CHANGED — `scraper-domain` is new pure-core; `check-arch` allowlist + mutation scope both extend |

**Decision Rule applied (per platform-engineering-foundations skill)**:
public-data-only and human-gate both affect 100% of the slice's user-visible
behavior and are unshippable-if-violated guardrails. Both warrant first-class
blocking CI gates — landed as the new AT + contract-sub-job entries in §3. The
GitHub-can-lie risk affects 100% of harvests and is addressed at the probe
layer (the five-step probe gold-tests, architecture-design §6.3) plus the
contract allowlist (§7).

**Constraint-Free Baseline (delta)**: nothing about slice-02 introduces an
operational gating ceremony that wasn't already there. The release cadence is
still "ship when green" with the same gate set plus the five new acceptance-test
gates + the one contract sub-job. Wall-clock impact is small (each new AT is
<30s; aggregate <3 min added to the acceptance stage — same envelope slice-03
added).

## 5. Simplest Solution Check (per cicd-and-deployment skill)

Before extending CI/observability for slice-02, three simpler alternatives were
considered:

### Alternative 1: "Just run the new scrape ATs in PR; no separate Pact job for the GitHub endpoints"
- **What**: rely on the wiremock-backed scrape acceptance tests to cover the harvest path; do not add a dedicated `contract-pact-github` Pact sub-job.
- **Expected Impact**: meets ~70% of requirements (functional coverage yes; contract-drift detection + the public-endpoint allowlist assertion NO — wiremock proves the code works against a stub the developer wrote, but does not assert against a recorded REAL GitHub response shape and does not, by itself, enforce that ONLY public endpoints are reachable).
- **Why insufficient**: KPI-SCR-4 is a release-blocking guardrail. The public-endpoint allowlist assertion is the specific mechanism that fails CI if a future endpoint addition touches a private path. A bespoke wiremock stub the developer authored cannot catch "the developer added a call to an off-allowlist endpoint" — the allowlist contract test can, by construction. This is the same reasoning slice-03 used to justify the dedicated Pact-peer job over wiremock-only coverage.

### Alternative 2: "Skip the private-refusal fixture; trust the `resolve_target` refuse-private code path"
- **What**: write the refuse-private logic, unit-test it in isolation, do not stand up a FakeGithub stub that returns 404 for a "private" path.
- **Expected Impact**: meets ~50% of KPI-SCR-4 (the code is correct in isolation; the wiring at the adapter boundary + probe is not adversarially exercised).
- **Why insufficient**: KPI-SCR-4 is release-blocking. The `private_refusal` fixture is the only mechanism that exercises the END-TO-END refuse path (probe step 2 + the `scraper_only_reads_public_data` AT) with a real 404. Unit-testing `resolve_target` in isolation does not validate that the adapter actually refuses BEFORE harvesting, nor that a private 404 is distinguished from a missing 404. This is the exact failure mode slice-03 guards against with its adversarial fixture (the "GitHub can lie about access" risk, architecture-design §6.3).

### Alternative 3: "Add a separate workflow file `scraper.yml`"
- **What**: separate workflow trigger for scraper tests; keep `ci.yml` foundation+slice-03 only.
- **Expected Impact**: meets ~100% of requirements but at the cost of duplicating triggers, caches, toolchain setup, and approval ceremony.
- **Why rejected**: the slice ships as part of the same binary; the CI is monorepo; splitting workflows multiplies maintenance for zero isolation benefit. Existing `ci.yml` + `nightly.yml` extend cleanly with the new jobs (slice-03 set this precedent). DELIVER adds jobs to the existing workflow files (see `ci-cd-pipeline.md` delta §3).

The chosen shape (extend `ci.yml` with the five new scrape acceptance jobs and
the one `contract-pact-github` sub-job; add `scraper-domain` to the nightly
mutation `--package` list; stand up a FakeGithub fixture under
`tests/fixtures/github/`; the new `adapter-github` probe ships the five-step
gold-test) is the minimum that satisfies the KPI-SCR-2, KPI-SCR-3, KPI-SCR-4
guardrails + the cost-lowering KPI-SCR-1 instrumentation without duplicating
foundation/slice-03 infrastructure.

## 6. Substrate matrix + mutation scope (delta)

**Substrate matrix**: no new axes, no new cells. The existing 8-cell release
matrix and 4-cell PR subset are extended only in the "per-cell exercised path" —
each cell now also exercises a `scrape github` happy-path against the FakeGithub
fixture (unauthenticated). This mirrors slice-03's per-cell peer-pull extension.

**Mutation scope (D-D23 — the one genuine scope change)**: the nightly
`cargo mutants` invocation widens from `claim-domain` to
`{claim-domain, scraper-domain}`. This is the FIRST mutation-scope widening
since slice-01 (slice-03 added zero new pure-core crates, so its mutation scope
was unchanged). Rationale:

- `scraper-domain` is PURE (WD-56/WD-59) — the signal→predicate derivation
  (`derive_candidates`) is the load-bearing pure-core trust primitive of the
  slice. Pure-core mutation hardness is the price of the trust contract (per
  ADR-006 Earned Trust + D-D8 pure-core-only policy).
- Kill-rate target: **≥95%** (matches `claim-domain`). The derivation is small
  and deterministic over the SSOT mapping; a high kill rate is achievable.
- `adapter-github` is NOT mutated (effect shell; covered by the five-step probe
  gold-tests + the FakeGithub integration tests, per the D-D8 pure-core-only
  scope).
- Release-tag mutation re-run inherits the D-D8 blocking-on-regression gate.
- DELIVER updates the nightly workflow's `cargo mutants -p claim-domain` line to
  add `-p scraper-domain` (or the multi-package form). No new gate semantics.

**CLAUDE.md note**: the project's Mutation Testing Strategy (nightly-only per
D-D8) is unchanged in POLICY — only the `--package` list grows. This is a
workflow-file edit, not a strategy change; no `CLAUDE.md` edit is warranted.

## 7. Risk register (delta)

New risks introduced by slice-02:

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| GitHub public-API contract drift (REST/GraphQL shape change; rate-limit-header rename; pagination/cursor change) breaks harvest silently | MEDIUM | scrape breaks for all users; recorded fixture stale | `contract-pact-github` replays against the recorded real-GitHub fixture per-PR + against the real public GitHub API at release-tag time (D-D24, mirrors D-D12); release-tag run catches drift before any user hits it |
| A future endpoint addition touches a private/authenticated path (KPI-SCR-4 regression) | MEDIUM | public-only guardrail silently weakens; trust model dented | The **public-endpoint allowlist assertion** in `contract-pact-github` fails CI on ANY off-allowlist endpoint (D-D22); the `private_refusal` FakeGithub fixture + probe step 2 catch the refuse-private path; both release-blocking |
| GitHub-can-lie-about-access: a private repo 404s identically to a missing repo; the scraper treats it as "missing-but-harvestable" and silently empties the harvest instead of refusing | MEDIUM | KPI-SCR-4 violated (silent private read attempt) OR a confusing empty result | probe step 2 (`private_refusal` fixture returns 404; probe MUST return `NotPublic`, not empty); the `scraper_only_reads_public_data` AT asserts refusal, not empty harvest |
| PAT leak: a logging regression echoes `GITHUB_TOKEN` into a structured event, the candidate list, a claim, or stdout | LOW-MEDIUM | credential disclosure (KPI-SCR-4 / WD-54) | probe step 5 (no-token-leak) + a contract-test assertion that the token never appears in any event/log line; CI runs the token-bearing release variant in a least-privilege fixture context; the token is held ONLY in `adapter-github` (pure `scraper-domain` never sees it) |
| FakeGithub fixture drifts from the real GitHub API shape as GitHub evolves; fixture passes for the wrong reason | MEDIUM | a contract/probe test silently weakens | The recorded-real fixture is the release-time provider (D-D24); the optional `cargo xtask regenerate-github-fixtures` (D-D25, DELIVER-may-defer) refreshes the stub bodies; the live-real variant at release-tag catches gross drift |
| Mutation run wall-clock grows as `scraper-domain` joins the `--package` list | LOW | nightly run slower | nightly-only (D-D8); `scraper-domain` is small (one derivation function + ADTs); incremental cost is minutes, not hours; nightly already runs unattended at 08:00 UTC |
| `--sign` continuation regresses the slice-01 publish path (a refactor breaks the reuse) | MEDIUM | KPI-SCR-2 human-gate / single-publish-path collapses | WD-66 mandates the SAME publish path; the `scraper_reuses_slice01_publish_path` AT asserts the reuse in observable behavior; the existing `kpi-4-roundtrip` gate covers the signed-payload byte-identity (display-only provenance, WD-62, adds no new CID path) |

All foundation + slice-03 risks (atrium pre-1.0 churn, PDS drift, substrate-lies,
mutation slowness, supply-chain, Windows, adversarial-peer fixture drift) remain
in force and unchanged in mitigation.

## 8. Handoff to DELIVER (delta)

Files DELIVER will translate from spec into code/config, in addition to the
foundation + slice-03 handoff:

- `ci-cd-pipeline.md` (this dir) → additions to `.github/workflows/ci.yml` (5 new scrape acceptance jobs + `contract-pact-github` sub-job) and `.github/workflows/nightly.yml` (mutation `--package` += `scraper-domain`)
- `observability.md` (this dir) → new `tracing` event-emission points across the new harvest/derive code paths + the `from_scraper` tag on the reused sign/publish events (the code paths themselves are DESIGN's deliverable)
- `kpi-instrumentation.md` (this dir) → KPI-SCR-1..5 read mechanisms and `openlore stats --scraper` extensions
- `tests/fixtures/github/` — wiremock fixture set (public_repo, public_user, private_refusal, rate_limited, token_rejected) — 5 stubs mapping 1:1 to the five probe steps + failure modes
- `tests/contracts/pact/github/` — the recorded real-GitHub response fixture (consumed; DEVOPS captures it once)
- `scripts/kpi-scr-{1,3,4,5}.jq` — per-KPI jq fallback snippets

Files DELIVER does NOT translate (DEVOPS owns post-DELIVER):

- The recorded `rust-lang/cargo` + public-user GitHub response fixture for the contract suite — DEVOPS captures this manually once and commits it; DELIVER consumes the recording (does NOT regenerate).
- The optional `cargo xtask regenerate-github-fixtures` helper (proposed as a maintenance helper, mirroring slice-03's regenerator; DELIVER may defer if scope tight).

## 9. Proposed ADRs

No new ADRs at the DEVOPS layer (D-D29). ADR-010..ADR-012 carry forward
unchanged. Slice-02's DESIGN wave already raised ADR-017 (verb contract),
ADR-018 (candidate model + display-only provenance), ADR-019 (GitHub adapter +
`GithubPort` + rate-limit/PAT policy + public-data-only probe). Those are DESIGN
ADRs — the GitHub trust-boundary decision lives in ADR-019, not in a DEVOPS ADR.

The new `contract-pact-github` sub-job, the mutation-scope widening, the
FakeGithub fixture set, and the KPI-SCR instrumentation events are all
CI/observability tactical choices that live in this DEVOPS-wave doc; they are
tactical applications of D-D8 (mutation) and D-D12 (Pact gating), not new
architectural axes. Same outcome as slice-03's D-D21.

If DELIVER's parallel work surfaces an architectural decision, that's a DESIGN
ADR, not a DEVOPS ADR.

## 10. References

- `docs/feature/openlore-github-scraper/feature-delta.md` (WD-46..WD-58)
- `docs/feature/openlore-github-scraper/discuss/outcome-kpis.md` (KPI-SCR-1..5)
- `docs/feature/openlore-github-scraper/design/*` (WD-59..68; ADR-017..019; architecture-design §6.2/§6.3/§6.4 — GitHub endpoints, probe contract, contract-test recommendation)
- Foundation DEVOPS docs (all `docs/feature/openlore-foundation/devops/*.md`)
- Slice-03 DEVOPS docs (all `docs/feature/openlore-federated-read/devops/*.md`) — the structural template
- ADR-010 (telemetry-opt-in), ADR-011 (release-matrix), ADR-012 (supply-chain) — still in force
- Sibling files in this dir: `ci-cd-pipeline.md`, `observability.md`, `kpi-instrumentation.md`, `wave-decisions.md`
