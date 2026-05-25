# Wave Decisions — DEVOPS — openlore-foundation

- **Wave**: DEVOPS
- **Date**: 2026-05-25
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-foundation (slice-01 walking skeleton)
- **Inherits**: WD-1..WD-13 (DISCUSS), D-1..D-13 (DESIGN), ADR-001..ADR-009

This file mirrors Morgan's `design/wave-decisions.md` style. Each row records
a DEVOPS-wave decision (D-D prefix to disambiguate from DESIGN's D series),
its rationale, and its status. Decisions that point at a proposed ADR
(ADR-010..ADR-012) are binding for DELIVER unless explicitly re-opened.

## Locked decisions

| # | Decision | Rationale | Status | ADR / Document |
|---|---|---|---|---|
| D-D1 | CI/CD tool = GitHub Actions; branching = GitHub Flow; release = `v*` tags from `main`. | Solo dev, GitHub-hosted repo, no multi-version support needed; GitHub Actions has every needed action (rust-cache, attest-build-provenance, cosign-installer) and is free at our scale. | LOCKED | `ci-cd-pipeline.md` §1, `platform-design.md` §7 |
| D-D2 | Observability emission = `tracing` + `tracing-subscriber` + `tracing-appender`; JSON Lines to local rolling file; no remote sink by default; no vendor lock-in. | Avoids Datadog/Honeycomb dependencies; the `tracing` ecosystem is already in `technology-stack.md`; adding `opentelemetry` is a clean future extension behind a cargo feature. | LOCKED | `observability.md` §3, ADR-010 |
| D-D3 | Distributed tracing SKIPPED in slice-01; in-process `tracing` spans only. | Single-binary CLI; nothing distributed to trace. Revisit at slice-05 (AppView introduces a separate process). | LOCKED with revisit trigger | `observability.md` §5 |
| D-D4 | Telemetry is OPT-IN, OFF by default, BUFFERED locally pending future endpoint; no endpoint operated in slice-01. | Privacy by default; no PII; the user owns their data; no surprise outbound traffic. | LOCKED | `observability.md` §6, proposed ADR-010 |
| D-D5 | `openlore stats` verb (proposed addition to Morgan's CLI surface) is HANDED TO DELIVER as an open question. If DELIVER cannot land it in slice-01 scope, the `scripts/kpi-*.jq` snippets are the fallback read mechanism. | KPIs must be readable; the verb is the ergonomic answer; the jq snippets are the guaranteed answer. Same handling pattern as Morgan's "Open questions for DELIVER" rows. | LOCKED as an open question | `observability.md` §4.2, `kpi-instrumentation.md` §2-§4 |
| D-D6 | NO capacity/performance/stress/chaos testing stage in slice-01. | Single-binary CLI; capacity is not a quality-attribute driver per Morgan's architecture-design.md §2. Revisit when (a) slice-05 introduces a daemon OR (b) a user reports >2s wall-clock for a single claim op. | LOCKED with revisit trigger | `ci-cd-pipeline.md` §5 |
| D-D7 | Branching = GitHub Flow (NOT GitFlow, NOT release branching). | No multi-version support needed for slice-01. Re-evaluate if a sibling slice introduces multi-version (e.g., slice-04 graph-store swap that needs a long-tail v0.x line). | LOCKED with revisit trigger | `platform-design.md` §7 |
| D-D8 | Mutation testing schedule = NIGHTLY only (not per-PR); kill-rate gate ENFORCED at release-tag time. | Per-PR cargo-mutants is too slow for the loop; nightly catches regressions within 24h; release gate is the hard backstop. Aligned with Apex Core Principle 9 (50k-200k LOC nightly-delta variant; slice-01 is under 50k but the per-PR per-feature variant is impractical for `cargo mutants` overhead). | LOCKED | `ci-cd-pipeline.md` §6 |
| D-D9 | Per-PR substrate gate = 4 cells (C2, C4, C5, C6); release-tag gate = full 8 cells; nightly = 8 cells + dedicated tmpfs/overlayfs jobs. | Balances PR wall-clock (~15 min cap) with substrate coverage at release time. | LOCKED | `substrate-matrix.md` §3, §5 |
| D-D10 | Release distribution = `cargo install openlore` (primary) + GitHub Releases binaries on 4-platform matrix (secondary). Homebrew/AUR/nix DEFERRED. Windows OUT OF SCOPE. | Reaches the persona (P-001 senior engineer) by both their natural install paths; deferred channels avoid maintenance burden without an evidence-based audience. | LOCKED | `distribution.md` §1, proposed ADR-011 |
| D-D11 | Release artifact security: cosign-signed tarballs + CycloneDX SBOM + SLSA build provenance (L2 minimum, L3 target via GitHub OIDC). | Supply-chain hygiene; user-verifiable; the cost is small (3 actions in the release workflow). | LOCKED | proposed ADR-012, `distribution.md` §1.2 |
| D-D12 | Pact contract tests = mocked (`wiremock`) in PR and nightly; against real `bsky.social` ONLY in the release workflow (manual approval gate). | Avoids rate-limiting bsky.social on every PR; still validates the contract against the reference implementation before each release. | LOCKED | `ci-cd-pipeline.md` §4.5, §7.1 |
| D-D13 | KPI feasibility = no KPI marked RED. KPI-3, KPI-6 marked YELLOW (per-user signal captured; cohort aggregation requires the future telemetry endpoint). | All 6 KPIs have a designed local-capture path. The two YELLOW items reflect the deferred cohort-aggregation endpoint, not a capture gap. | LOCKED | `kpi-instrumentation.md` §1, §10 |

## Proposed (awaiting user confirmation)

None. All DEVOPS-wave decisions for slice-01 are locked.

The three proposed ADRs (ADR-010, ADR-011, ADR-012) are PROPOSED-status
ADRs (per the Morgan convention) and become Accepted on user sign-off of
this DEVOPS wave.

## Open questions (handed to DELIVER)

These are deliberately deferred to DELIVER and are tracked as
implementation concerns, not platform decisions:

1. Hook-tool choice: `lefthook` vs `pre-commit` vs raw `git hooks`. Spec is
   in `platform-design.md` §5 (gate inventory) and `ci-cd-pipeline.md` §3.
   DELIVER picks.
2. Whether to ship the `openlore stats` verb in slice-01 OR defer with
   `scripts/kpi-*.jq` snippets as the fallback (per D-D5).
3. Whether to ship the `openlore identity remove-key` verb for clean
   uninstall in slice-01 OR document the manual keychain-removal steps
   (`distribution.md` §7).
4. Hook script implementation language for the `xtask` mutation-cache
   helper (Rust subcommand inside `xtask/` is the canonical approach but
   crafter may have a stronger pattern).
5. Whether to enable `cargo binstall` metadata in slice-01 (zero-cost add;
   DELIVER's call).

## Out of scope for DEVOPS slice-01 (explicit deferrals)

- **SLOs / SLAs / error budgets / alerting tiers**: NOT designed. Slice-01
  is solo-dev; no service contract. Revisit at slice-05 AppView (an
  operated service introduces real SLO territory).
- **Runbooks for paging**: NOT designed. No on-call. The
  `health.startup.refused` event IS the user-facing alert.
- **Dashboards** (Grafana, Datadog, Honeycomb): NOT designed. No central
  aggregation in slice-01.
- **Telemetry endpoint** itself (server-side): NOT designed. Only the
  client-side opt-in mechanism and the local buffer (per `observability.md`
  §6).
- **Auto-updater**: NOT designed. Users update manually (per
  `distribution.md` §6).
- **Multi-tenancy, RBAC, secrets management beyond `keyring`**: NOT
  relevant. Solo dev, single binary, OS keychain only.
- **Disaster recovery / backups**: NOT designed. User's local files are
  their responsibility (they own their PDS records as the authoritative
  copy; the local DB is a derived index per `data-models.md` §Two
  representations).
- **Capacity / load / stress testing**: per D-D6.
- **Chaos engineering**: NOT designed for slice-01. Fault injection happens
  organically in the substrate matrix's tmpfs/overlayfs/DrvFs cases (per
  `substrate-matrix.md` §4.1). Reconsider at slice-05 AppView.
- **Windows support**: per D-D10.

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (software-crafter — functional, per ADR-007 Accepted) | every DEVOPS doc + every DESIGN doc + every ADR + open-questions list | `.github/workflows/{ci,nightly,release}.yml`; `deny.toml`; `rust-toolchain.toml`; `.lefthook.yml` (or chosen hook tool config); `tracing-subscriber` init code; KPI emission points; release-tagging procedure |
| Operations team (POST-DELIVER, not slice-01) | not applicable — there is no operations team for a local-first CLI | not applicable |

## Changelog

- 2026-05-25 — Apex — initial DEVOPS-wave decisions for slice-01. All
  decisions D-D1..D-D13 LOCKED. Three ADRs (ADR-010, ADR-011, ADR-012)
  drafted as PROPOSED. CLAUDE.md not modified (no mutation testing strategy
  question reached the user; per Apex Core Principle 9 the default
  per-feature variant is impractical for `cargo mutants` and D-D8 locks
  nightly without requiring the CLAUDE.md persistence step).
