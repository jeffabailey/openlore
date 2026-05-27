# Wave Decisions — DEVOPS — openlore-federated-read (slice-03)

- **Wave**: DEVOPS
- **Date**: 2026-05-27
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-federated-read (sibling slice-03; walking-skeleton-for-federation)
- **Inherits**: openlore-foundation DEVOPS D-D1..D-D13 (all LOCKED, all carry forward unchanged), ADR-010..ADR-012 (all in force unchanged)

This file is the DEVOPS-wave decision log for slice-03. Decisions are
numbered D-D14 onward to continue the foundation sequence. None of the
foundation decisions are re-opened or amended.

## Inheritance

All foundation DEVOPS decisions (D-D1..D-D13) carry forward verbatim:

- D-D1 (GitHub Actions + GitHub Flow + `v*` tag releases) → unchanged
- D-D2 (tracing + tracing-subscriber + tracing-appender + JSON Lines local) → unchanged; slice-03 emits additional events into the same pipeline
- D-D3 (distributed tracing skipped) → unchanged; slice-03 is still single-binary
- D-D4 (telemetry opt-in OFF by default, no endpoint) → unchanged
- D-D5 (`openlore stats` verb is DELIVER's call; jq fallback) → unchanged; slice-03 adds `--federation` and `--rejections` flags to the verb-or-fallback design
- D-D6 (no capacity/perf/stress/chaos in slice-01) → unchanged; same revisit trigger applies to slice-03 (still no daemon; still no >2s single-op KPI breach reported)
- D-D7 (GitHub Flow branching) → unchanged
- D-D8 (mutation testing nightly-only, release-tag blocking) → unchanged; scope MAY widen if DESIGN introduces new pure-core module
- D-D9 (4-cell PR substrate, 8-cell release substrate, +tmpfs/overlayfs nightly) → unchanged in shape; per-cell body extended with peer-pull happy path (per `substrate-matrix.md` delta §1 noted in `platform-design.md` §6)
- D-D10 (cargo install primary + 4-platform binaries; Homebrew/AUR/nix deferred; Windows out) → unchanged
- D-D11 (cosign + CycloneDX SBOM + SLSA L2 minimum / L3 target) → unchanged
- D-D12 (Pact mocked in PR/nightly; real bsky.social in release with manual approval) → unchanged; the slice-03 Pact-peer suite (`com.atproto.repo.listRecords` + optional `getRecord`) inherits the same gating policy
- D-D13 (no KPI marked RED; KPI-3 and KPI-6 are YELLOW for cohort) → carried forward as the SAME policy applied to KPI-FED-1..6; see D-D17

## Locked slice-03 decisions

| # | Decision | Rationale | Status | ADR / Document |
|---|---|---|---|---|
| D-D14 | Peer-DID resolution at startup probe = the USER's OWN DID only (proves the resolver works); per-peer-DID resolution is DEFERRED to first `peer pull` of that peer. | (a) avoids coupling startup time to subscription cardinality; (b) avoids fail-fast on transient resolver outages making the binary unusable; (c) preserves DID-resolver request budget. Per-peer probe added at first-pull time exercises the same code path. | LOCKED | `observability.md` delta §3 |
| D-D15 | Adversarial-peer test fixture lives in `tests/fixtures/peer-adversarial/` as a wiremock-driven sub-fixture; regenerated via `cargo xtask regenerate-peer-fixtures` against the live `org.openlore.claim` Lexicon; `arch-check` CI stage verifies committed bodies are current. | KPI-FED-6 is a release-blocking guardrail; the adversarial fixture is the only end-to-end exercise of the per-claim reject path. Auto-regeneration prevents drift as the Lexicon evolves. DELIVER may defer the regenerator (fixture works without it; just risks drift). | LOCKED with DELIVER-may-defer escape hatch on the regenerator | `ci-cd-pipeline.md` delta §3.3, §7 |
| D-D16 | KPI-FED-5 measurement mechanism default = POST-HOC jq aggregation over `peer.added`, `peer.pull.completed`, `query.executed{kind: federated}` events joined by peer_did within a session window. NO state file. | Keeps state out of the binary; simpler implementation; post-hoc aggregation is exact (the events carry the timestamps). DESIGN may override and pick the alternative single `federation.e2e.timing` event with a `flow_id` state file if join correctness in degenerate cases worries them; both are functionally equivalent. | LOCKED with DESIGN-override window | `observability.md` delta §2.5; `kpi-instrumentation.md` delta §6 |
| D-D17 | KPI-FED feasibility = no KPI-FED marked RED. KPI-FED-1, KPI-FED-2, KPI-FED-4, KPI-FED-6 = GREEN. KPI-FED-3 = YELLOW (per-user GREEN; cohort aggregation requires future telemetry endpoint OR PO out-of-band day-30 outreach). KPI-FED-5 = GREEN per-user / YELLOW cohort-percentile (same constraint). | All 6 KPI-FED have designed capture in slice-03. The two YELLOW items mirror the foundation KPI-3/KPI-6 deferral — not a slice-03 gap; a deferred-endpoint constraint. | LOCKED | `kpi-instrumentation.md` delta §1, §9 |
| D-D18 | Counter-claim qualitative survey (KPI-FED-3) delivered as a 2-question Likert prompt after the FIRST `claim.counter.published` event, using the SAME one-shot file-presence pattern as foundation KPI-3 survey (per foundation `kpi-instrumentation.md` §4). Survey response stored at `$XDG_DATA_HOME/openlore/surveys/post-counter-claim.response.json`. | Reuses the foundation survey-delivery mechanism; no new infrastructure. Same dismiss-on-Enter and `--no-tty` skip semantics. Free-text optional; never telemetry-sent. | LOCKED | `kpi-instrumentation.md` delta §4 |
| D-D19 | Renderer-review checklist (`docs/dev/renderer-review-checklist.md`) is a release-time deliverable enforcing KPI-FED-2 (zero merged consensus). Solo dev = self-review; the checklist exists to prevent forgetting. The release CHANGELOG records "Renderer review: passed YYYY-MM-DD" per release. | KPI-FED-2 is a guardrail; the AT covers code paths but renderer-introduction is an ongoing risk (new renderers added in future slices could regress without test coverage). The checklist is the human-in-the-loop backstop. | LOCKED | `kpi-instrumentation.md` delta §3 |
| D-D20 | CI test-only escape hatch for `peer remove --purge` interactive confirmation = a `--yes-i-am-tested` flag (or env var `OPENLORE_TEST_AUTOCONFIRM=1`) gated by a `#[cfg(test)]` or release-build-rejected guard. WD-21 forbids `--yes` in production; the test hatch must NOT compile into release builds. | The AT `at-peer-remove-purge-zero-residue` cannot prompt interactively in CI. The hatch must be safe-by-construction — flagged for DESIGN as a build-time guard concern. | LOCKED with DESIGN-implementation flag | `ci-cd-pipeline.md` delta §3.5 |
| D-D21 | No new ADR. ADR-010, ADR-011, ADR-012 carry forward unchanged. Slice-03 decisions are CI/observability tactical choices that live in this DEVOPS-wave doc; none crosses the ADR threshold. | ADR convention: cross-slice or cross-component architectural decisions. Slice-03's new event names, new CI jobs, and adversarial-fixture pattern are tactical extensions of existing decisions, not new architectural axes. | LOCKED | `platform-design.md` delta §9 |

## Proposed (awaiting user confirmation)

None. All slice-03 DEVOPS decisions are LOCKED.

## Open questions (handed to DESIGN — answer required before DELIVER lands)

These are deliberately deferred to DESIGN. DEVOPS has defaults; DESIGN may
override.

1. **Persist `flow_id` state file for KPI-FED-5** (D-D16) vs post-hoc jq aggregation. Default: post-hoc. DESIGN's call.
2. **Targeted-fetch by CID** (`com.atproto.repo.getRecord`) for peer pull, in addition to `listRecords`. If yes, include the Pact for `getRecord` in `contract-pact-pds-peer`. Default: include both (cost low; dropping later trivial). DESIGN's call.
3. **Test-only escape hatch shape** for `peer remove --purge` interactive confirmation (D-D20). Build-time `#[cfg(test)]` vs runtime env-var. DEVOPS prefers compile-time exclusion from release builds; DESIGN implements.
4. **New pure-core module** for peer-claim signature/CID verification (e.g., `peer-claim-verify` crate or sub-module inside `claim-domain`). If new, the nightly mutation `--package` list extends; if inside `claim-domain`, no change.
5. **`peer remove --purge` race with concurrent `peer pull`** (per `platform-design.md` delta §7 risk row). DESIGN's call whether to lock per-DID; if not, an additional stress AT is needed.

## Open questions (handed to DELIVER)

1. **Adversarial-fixture regenerator** (`cargo xtask regenerate-peer-fixtures`) — ship in slice-03 OR defer to follow-up. DELIVER's call; spec is in `ci-cd-pipeline.md` delta §7.
2. **`openlore stats --federation` and `--rejections` flag implementation** — concrete only if D-D5 verb landed; otherwise the jq snippets in `scripts/kpi-fed-*.jq` are the fallback.
3. **`scripts/kpi-fed-{1,3,5,6}.jq` snippets** — DELIVER lands these alongside the foundation `kpi-{1,2,4,5}.jq` snippets.
4. **Renderer-review checklist content** (`docs/dev/renderer-review-checklist.md`) — DELIVER drafts; DEVOPS reviews at release-tag time.
5. **`xtask check-arch` rule extension** enforcing WD-19 ("no JOIN between author_claims and peer_claims that elides author_did column"). DELIVER lands the rule; runs as part of existing `arch-check` stage.
6. **Recorded `bsky.social` Pact fixture for `listRecords`** — DEVOPS captures this manually once (one-time setup) and commits to `tests/contracts/pact/`. DELIVER does NOT regenerate; consumes the recording.

## Out of scope for DEVOPS slice-03 (explicit deferrals)

All foundation deferrals (SLOs/SLAs, runbooks, dashboards, telemetry
endpoint, auto-updater, multi-tenancy, DR, capacity, chaos, Windows) carry
forward unchanged. Slice-03 adds these explicit deferrals:

- **Per-peer reputation scoring / trust weighting infrastructure**: NOT designed. WD-19's anti-merging invariant + per-row attribution is the trust surface; opinions on peer trustworthiness are the user's, not the system's.
- **Push-based federation / peer subscription daemon**: NOT designed (per WD-18 pull-on-demand only).
- **Federation-level dashboards** (cohort-level peer-pull metrics across all OpenLore users): NOT designed (no central aggregation; per ADR-010).
- **Brigade / coordinated-inauthentic-behavior detection**: NOT designed. Out-of-scope for slice-03; revisit when slice-05 AppView aggregates across users.
- **Counter-claim threading / "counter-of-counter" rendering**: NOT designed; instrumentation is event-based per published counter-claim regardless of depth. UX-level threading is DESIGN's concern.
- **Peer-DID-revocation propagation** (what if the peer revokes their DID via ATProto's recovery key): UPSTREAM concern. The probe at first-pull-of-peer will detect the revocation; the local cache survives until `peer remove --purge`. Documented; not a guardrail.

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (software-crafter — functional, per ADR-007) | every slice-03 DEVOPS doc + every slice-03 DESIGN doc (when parallel DESIGN completes) + slice-01 carryover + the DESIGN-wave open-questions list | Additions to `.github/workflows/ci.yml` (5 new acceptance jobs + 1 Pact sub-job) and `.github/workflows/nightly.yml` (mutation scope extension if applicable); `tests/fixtures/peer-adversarial/` (3 tampered-record fixtures + wiremock setup); `tests/contracts/pact/listRecords.pact.json` (consumed from DEVOPS one-time recording); `scripts/kpi-fed-*.jq` snippets; `tracing` event emission code at the new boundaries; renderer-review checklist; optional `cargo xtask regenerate-peer-fixtures`. |
| Operations team (POST-DELIVER) | not applicable — still local-first CLI, no operations team for slice-03 | not applicable |
| Future DEVOPS wave (slice-05 AppView or whichever sibling stands up the telemetry endpoint) | this doc + foundation DEVOPS docs + ADR-010 + `kpi-instrumentation.md` event-shape definitions | cohort aggregation for KPI-FED-3 and KPI-FED-5 (and the unresolved YELLOWs from foundation KPI-3, KPI-6) |

## Changelog

- 2026-05-27 — Apex — initial DEVOPS-wave decisions for slice-03 (openlore-federated-read). All decisions D-D14..D-D21 LOCKED. No new ADRs proposed. Foundation D-D1..D-D13 and ADR-010..ADR-012 carry forward unchanged. CLAUDE.md not modified (mutation strategy unchanged from D-D8).
