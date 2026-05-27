# Platform Design Delta — openlore-federated-read (slice-03)

- **Wave**: DEVOPS (design portion; this is the sibling-feature extension of slice-01 DEVOPS)
- **Date**: 2026-05-27
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-federated-read (sibling slice-03; walking-skeleton-for-federation)
- **Inherits**: openlore-foundation DEVOPS (D-D1..D-D13, ADR-010..ADR-012) — all UNCHANGED
- **Paradigm context**: functional Rust (ADR-007, Accepted)

This is the DEVOPS platform-design **delta** for slice-03. The slice-01
platform layer (operating model, branching, gate inventory, distribution,
substrate matrix) is **unchanged**. This document records only the
extensions. Read in conjunction with
`docs/feature/openlore-foundation/devops/platform-design.md`.

## 1. What did NOT change

| Concern | Status | Reference |
|---|---|---|
| Operating model (local-first, solo-dev, no SLOs, no on-call) | UNCHANGED | foundation §1 |
| DORA framing (per-release tag, no fleet) | UNCHANGED | foundation §1 |
| CI tool (GitHub Actions) + branching (GitHub Flow) | UNCHANGED | foundation §7, D-D1 |
| Distribution (`cargo install` + 4-platform binaries; Windows out) | UNCHANGED | foundation §3 row 4, ADR-011 |
| Substrate gold matrix (8 cells for release; 4 for PR) | UNCHANGED in shape; one row extended | foundation §3 row 5; this doc §5 |
| Release artifact security (cosign + SBOM + SLSA) | UNCHANGED | ADR-012 |
| Telemetry opt-in policy (off by default; no endpoint operated) | UNCHANGED | ADR-010 |
| Local quality gates (lefthook/pre-commit + pre-push) | UNCHANGED | foundation §5 |
| Quality-gate inventory taxonomy | UNCHANGED in shape; entries added | foundation §6; this doc §3 |

## 2. What DID change (the delta)

Slice-03 introduces three new platform-layer concerns:

| New concern | Where it lives | Why slice-03 introduces it |
|---|---|---|
| Peer-pull contract test surface (`com.atproto.repo.listRecords`, optional `getRecord`) | `contract-pact-pds-peer` CI job (new) | Federation reads peer records over the PDS XRPC — a new external-contract surface beyond slice-01's write paths |
| Adversarial-peer fixture (deliberately tampered records) | `wiremock`-driven sub-fixture exercised by `peer_tampered_signature_rejected` AT (KPI-FED-6 release-blocking) | KPI-FED-6 is a security guardrail; no fixture = no enforcement |
| Six new KPI-FED instrumentation surfaces (KPI-FED-1..6) | `tracing` events emitted at peer-pull, peer-render, counter-publish boundaries | The slice has its own outcome KPIs (`discuss/outcome-kpis.md`); foundation's KPI-1..6 do not subsume them |

Everything else is additive within existing structures — new CI jobs in
existing workflows, new probe assertions in existing `probe()` extensions,
new `tracing` event names emitted from new code paths the DESIGN wave
designs in parallel.

## 3. Quality-gate inventory delta

Net additions to foundation §6 (no rows removed; no semantics changed):

| Category | Where | Type | What it gates | Origin |
|---|---|---|---|---|
| CI | acceptance-stage job (new sub-job `contract-pact-pds-peer`) | blocking (pipeline) | Pact replay for `com.atproto.repo.listRecords` (+ optional `getRecord`) | this slice §4 |
| CI | acceptance-stage job (new AT `peer_tampered_signature_rejected`) | blocking (GUARDRAIL — KPI-FED-6) | adversarial fixture: at least one tampered record per pull is rejected; no tampered record reaches `peer_claims` table | this slice §4 |
| CI | acceptance-stage job (new AT `federation_attribution_preserved`) | blocking (GUARDRAIL — KPI-FED-1, KPI-FED-2) | every rendered peer claim carries non-null `author_did`; zero "merged consensus" rows | this slice §4 |
| CI | acceptance-stage job (new AT `peer_remove_purge_zero_residue`) | blocking (GUARDRAIL — KPI-FED-4) | post-purge: zero peer_claim rows for the purged DID | this slice §4 |
| CI | acceptance-stage job (new AT `peer_cid_round_trip` + `counter_target_cid_round_trip`) | blocking | CID recomputation at pull-time matches; counter-claim's reference CID matches the target | this slice §4 |
| CI (nightly) | mutation-stage scope expansion | advisory | `cargo mutants` extended to include any new pure-core module(s) DESIGN introduces (e.g., `peer-claim-verify`) | this slice §6 |

All other foundation gates (`fmt`, `lint`, `supply-chain`, `arch-check`,
`probe-check`, `test-unit`, `test-property`, `kpi-4-roundtrip`,
`kpi-5-offline`, `test-integration-pds`, `contract-pact-pds`) remain
unchanged in command and gating semantics.

## 4. Constraint Impact Analysis (delta)

Three new constraints surface in slice-03; one foundation constraint gains
weight:

| Constraint | Source | % delivery affected | Priority | New / changed? |
|---|---|---|---|---|
| Attribution-fidelity invariant (KPI-FED-1, KPI-FED-2 guardrails) | DISCUSS KPI-FED-1, KPI-FED-2; WD-19 (no JOIN elides author_did) | 100% of federated-query paths | HIGH | NEW |
| Signature-verification correctness at pull (KPI-FED-6) | DISCUSS KPI-FED-6, WD-24 (per-claim sig+CID checks) | 100% of pulls | HIGH | NEW |
| Adversarial-peer test infrastructure exists in CI | DISCUSS Risks logged §3 (deliberately bad records); §1 of this doc | every release (gates KPI-FED-6) | HIGH | NEW |
| ATProto PDS contract surface widened (added read paths) | DESIGN (this slice) extends `PdsPort` | every release | MEDIUM (was MEDIUM in foundation; weight unchanged) | CHANGED — new methods to contract-test |
| Pure-core has zero I/O imports (now applies to new peer-verify module if any) | ADR-009, D-11 | every PR (CI gate) | HIGH | UNCHANGED (rule applies to new code too) |

**Decision Rule applied (per platform-engineering-foundations skill)**:
attribution-fidelity and signature-verification both affect 100% of the
slice's user-visible behavior. Both warrant first-class blocking CI gates —
landed as the new AT entries in §3.

**Constraint-Free Baseline (delta)**: nothing about slice-03 introduces an
operational gating ceremony that wasn't already there. The release cadence
is still "ship when green" with the same set of gates plus the five new
acceptance-test gates listed in §3. Wall-clock impact is small (each new AT
is <30s; aggregate <3 min added to acceptance stage).

## 5. Simplest Solution Check (per cicd-and-deployment skill)

Before extending CI/observability/etc. for slice-03, three simpler
alternatives were considered:

### Alternative 1: "Just run the new AT suite in PR; no separate Pact job for listRecords"
- **What**: rely on `test-integration-pds` (already exists) to cover peer-pull via additional wiremock scenarios; do not add a dedicated `contract-pact-pds-peer` Pact job.
- **Expected Impact**: meets ~70% of requirements (functional coverage yes; contract drift detection no — Pact's value is consumer-driven contract enforcement against the reference impl, which `wiremock` does not provide).
- **Why insufficient**: at release time we re-run Pact against real `bsky.social` per D-D12. If the consumer contract for `listRecords` is not in the Pact suite, the release gate cannot detect a breakage in that XRPC. Skipping the Pact job would silently weaken the foundation's contract-drift guarantee for the federation read path.

### Alternative 2: "Skip the adversarial fixture; trust the per-claim sig+CID code path"
- **What**: write the sig+CID verification code, unit-test it in pure-core, do not stand up a wiremock fixture that publishes deliberately tampered records.
- **Expected Impact**: meets ~50% of KPI-FED-6 (the code is correct in isolation; the wiring at the adapter boundary is not adversarially exercised).
- **Why insufficient**: KPI-FED-6 is a release-blocking guardrail. The adversarial fixture is the only mechanism that exercises the END-TO-END pull path with a real bad record. Unit-testing the verify function in isolation does not validate that the adapter actually calls it on every record before insertion to `peer_claims`. This is the same failure mode foundation guards against via the substrate matrix for storage probes.

### Alternative 3: "Add a separate workflow file `federation.yml`"
- **What**: separate workflow trigger for federation tests; keep `ci.yml` foundation-only.
- **Expected Impact**: meets ~100% of requirements but at the cost of duplicating triggers, caches, toolchain setup, and approval ceremony.
- **Why rejected**: the slice ships as part of the same binary; the CI is monorepo; splitting workflows multiplies maintenance for zero isolation benefit. Existing `ci.yml` + `nightly.yml` extend cleanly with the new jobs. DELIVER will add jobs to existing workflow files (see §3 of `ci-cd-pipeline.md` delta).

The chosen shape (extend `ci.yml` and `nightly.yml` with the five new
acceptance jobs and the one Pact-peer job; stand up a wiremock adversarial
fixture under `tests/fixtures/peer-adversarial/`; extend the `PdsPort`
probe to cover peer-read paths with a peer-DID sentinel) is the minimum
that satisfies the KPI-FED-1, KPI-FED-2, KPI-FED-4, KPI-FED-6 guardrails
without duplicating foundation infrastructure.

## 6. Substrate matrix (delta)

Per `substrate-matrix.md` delta in this dir: no new axes, no new cells.
The existing 8-cell release matrix and 4-cell PR subset are extended only
in the "per-cell exercised path" — each cell now also exercises a
`peer-pull` happy-path scenario against the wiremock peer fixture. See
that file §1 for the exact addition.

## 7. Risk register (delta)

New risks introduced by slice-03:

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `listRecords` XRPC behavior drift on `bsky.social` (e.g., pagination/cursor semantics change) | MEDIUM | peer-pull breaks for users on `bsky.social`; recorded contract fixture stale | Pact suite (`contract-pact-pds-peer`) replays against fixture per-PR + against real PDS at release-tag time (per D-D12); release-tag run catches drift before any user hits it |
| Adversarial fixture maintenance debt (tampered-record fixtures drift from the real Lexicon shape as `org.openlore.claim` evolves) | MEDIUM | fixture passes for the wrong reason; KPI-FED-6 guardrail silently weakens | Fixture is regenerated from the live Lexicon JSON via an `xtask` helper (DELIVER lands; this doc proposes); regeneration is itself CI-checked via `arch-check` extension |
| Peer-DID resolution at probe time hits rate limits / outages (PDS or DID resolver) | LOW-MEDIUM | `peer add` startup probe refuses; user blocked | Peer-probe uses a fixture DID at startup (the user's own DID is sufficient — proves the resolver works; per-peer DID resolution is deferred to first `peer pull`). See `observability.md` delta §3.1 |
| Counter-claim publish path regression (a refactor of slice-01's publish path breaks counter-claim) | MEDIUM | KPI-FED-3 collapses; users cannot disagree | WD-22 mandates SAME publish path; an AT `counter_claim_via_slice01_publish_path` asserts the publish path is unchanged in observable behavior (test added to `tests/acceptance/`) |
| `peer remove --purge` race with concurrent `peer pull` of the same DID | LOW | partial purge | DESIGN's decision; if DESIGN selects per-DID lock, no DEVOPS gate needed; if not, add a stress AT (not designed here; flagged as open question to DESIGN) |

All foundation risks (atrium pre-1.0 churn, PDS drift, substrate-lies,
mutation slowness, supply-chain, Windows) remain in force and unchanged
in mitigation.

## 8. Handoff to DELIVER (delta)

Files DELIVER will translate from spec into code/config in addition to the
foundation handoff:

- `ci-cd-pipeline.md` (this dir) → additions to `.github/workflows/ci.yml` and `.github/workflows/nightly.yml`
- `observability.md` (this dir) → new `tracing` event-emission points across the new peer-pull, peer-render, counter-publish code paths (the code paths themselves are DESIGN's deliverable)
- `kpi-instrumentation.md` (this dir) → KPI-FED-1..6 read mechanisms and `openlore stats` extensions
- `tests/fixtures/peer-adversarial/` — wiremock fixture publishing 3+ tampered-record variants (bad sig, bad CID, both)

Files DELIVER does NOT translate (DEVOPS owns post-DELIVER):

- The Lexicon-regeneration `xtask` for the adversarial fixture (proposed below as a maintenance helper; DELIVER may defer if scope tight)
- The recorded `bsky.social` Pact fixture for `listRecords` — DEVOPS captures this manually once and commits the recording; DELIVER does NOT regenerate it (it's a recorded reference)

## 9. Proposed ADRs

No new ADRs. ADR-010..ADR-012 carry forward unchanged. Slice-03 does not
introduce any decision that meets the ADR threshold (per ADR convention:
architectural decisions with cross-slice or cross-component consequences).
The new instrumentation events, the Pact-peer job, and the adversarial
fixture are all CI/observability tactical choices that live in this
DEVOPS-wave doc; they do not warrant ADRs.

If DESIGN's parallel work surfaces an architectural decision (e.g., a new
`PeerPort` rather than extension of `PdsPort` + `StoragePort`), that's a
DESIGN ADR, not a DEVOPS ADR.

## 10. References

- `docs/feature/openlore-federated-read/feature-delta.md` (WD-14..WD-25)
- `docs/feature/openlore-federated-read/discuss/outcome-kpis.md` (KPI-FED-1..6)
- Foundation DEVOPS docs (all `docs/feature/openlore-foundation/devops/*.md`)
- ADR-010 (telemetry-opt-in), ADR-011 (release-matrix), ADR-012 (supply-chain) — still in force
- Sibling files in this dir: `ci-cd-pipeline.md`, `observability.md`, `kpi-instrumentation.md`, `wave-decisions.md`
