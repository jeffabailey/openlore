# Wave Decisions — DEVOPS — openlore-appview-search (slice-05)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-appview-search (sibling slice-05; the FINAL umbrella slice; discover-across-the-network)
- **Inherits**: openlore-foundation DEVOPS D-D1..D-D13 (LOCKED, carry forward unchanged), ADR-010..ADR-012 (in force unchanged); openlore-federated-read DEVOPS D-D14..D-D21 (LOCKED, unchanged); openlore-github-scraper DEVOPS D-D22..D-D29 (LOCKED, unchanged); openlore-scoring-graph DEVOPS D-D30..D-D34 (LOCKED, unchanged)
- **Runs in PARALLEL with DISTILL** — depends on the APPROVED DESIGN (ADR-023..027, WD-111..124, I-AV-1..9), NOT on DISTILL outputs.

This file is the DEVOPS-wave decision log for slice-05. Decisions are numbered
**D-D35 onward** to continue the sequence after slice-04's D-D34. None of the
foundation (D-D1..D-D13), slice-03 (D-D14..D-D21), slice-02 (D-D22..D-D29), or
slice-04 (D-D30..D-D34) decisions are re-opened or amended.

> **Sequencing note**: slice numbers reflect the carpaccio split (WD-13); the DEVOPS
> decision numbers follow ship/authoring order. Slice-05 is authored after slice-04
> (D-D30..D-D34), so slice-05's decisions are D-D35+.

## Inheritance

All foundation (D-D1..D-D13), slice-03 (D-D14..D-D21), slice-02 (D-D22..D-D29), and
slice-04 (D-D30..D-D34) DEVOPS decisions carry forward verbatim. The load-bearing ones
for slice-05:

**Foundation (D-D1..D-D13):**
- D-D1 (GitHub Actions + GitHub Flow + `v*` tag releases) → unchanged; the indexer ships in the SAME release matrix (D-D35)
- D-D2 (tracing + tracing-subscriber + tracing-appender + JSON Lines local) → unchanged; the CLI `search.*` events flow into the SAME author-side pipeline; the indexer's `indexer.*` events flow into the INDEXER's OWN log (a separate binary, separate log — `observability.md` §5)
- D-D3 (distributed tracing skipped) → unchanged; slice-05 adds a cross-PROCESS boundary so distributed tracing becomes THEORETICALLY applicable, but a single CLI ↔ a single localhost indexer is per-process span enrichment + a `request_id` echo, not distributed tracing (`observability.md` §1)
- D-D4 (telemetry opt-in OFF by default, no endpoint) → unchanged for the CLI author-side; the indexer-OPERATOR surface is a DISTINCT concern, NOT author-side telemetry (`observability.md` §7; D-D40)
- D-D5 (`openlore stats` verb is DELIVER's call; jq fallback) → unchanged; slice-05 adds a `--discovery` flag (CLI) + an `openlore-indexer stats` (indexer) to the verb-or-fallback design
- D-D6 (no capacity/perf/stress/chaos) → unchanged in POLICY; the indexer is the FIRST request-driven service, so RED-method serve metrics apply (`observability.md` §4.2), but no load/stress/chaos test is designed for the walking-skeleton single self-hosted instance
- D-D7 (GitHub Flow branching) → unchanged
- D-D8 (mutation testing nightly-only, release-tag blocking, pure-core scope) → unchanged in POLICY; SCOPE widens to add `crates/appview-domain` — the THIRD widening (D-D40)
- D-D9 (4-cell PR substrate, 8-cell release substrate, +tmpfs/overlayfs nightly) → unchanged in shape; per-cell body extended with a `search` happy-path + degradation against a localhost fixture indexer AND the `adapter-index-store` fsync-honesty probe (the tmpfs/overlayfs cells exercise the container-substrate-durability lie)
- D-D10 (cargo install primary + 4-platform binaries; Windows out) → EXTENDED: the SECOND binary `openlore-indexer` ships in the SAME 4-platform matrix + `cargo install openlore-indexer` (D-D35); Windows stays deferred for both
- D-D11 (cosign + CycloneDX SBOM + SLSA L2 minimum / L3 target) → EXTENDED to cover BOTH binaries; the SBOM now covers both dependency trees incl. the new `axum`/`bs58` deps (D-D35)
- D-D12 (Pact mocked in PR/nightly; real provider in release with manual approval) → unchanged in POLICY; the two new slice-05 Pact suites inherit it (D-D36/D-D37); the new `plc.directory` real-provider host is added (D-D39)
- D-D13 (no KPI marked RED; YELLOW for cohort) → carried forward as the SAME policy applied to KPI-AV-1..6 (D-D40)

**Slice-03 (D-D14..D-D21):**
- D-D14 (peer-DID resolution at startup probe = user's OWN DID only) → the indexer's `adapter-atproto-did` resolve-only probe decodes a FIXTURE `z6Mk...` at startup (proves the real decode path); per-network-author resolution happens at ingest-time, not startup (the same defer-cardinality reasoning)
- D-D15 (adversarial-peer fixture, `xtask`-regenerated; `arch-check` `--check`) → the DIRECT precedent for the slice-05 ingest adversarial fixtures + the `regenerate-ingest-fixtures` helper (D-D38)
- D-D16 (KPI post-hoc jq aggregation, no state file) → the SAME post-hoc default for KPI-AV-1/4/6 (D-D40; `observability.md` §2.7)
- D-D17 (KPI GREEN/YELLOW policy) → the SAME framework applied to KPI-AV (D-D40)
- D-D18 (one-shot Likert survey after first event, file-presence pattern) → reused for the KPI-AV-5 public-data comprehension prompt (after the first `search`; `observability.md` §8; D-D40)
- D-D19 (renderer-review checklist at release time) → EXTENDED with one slice-05 line (the network search/share renderer; D-D41)
- D-D20 (CI test-only escape hatch via build-time guard) → the DIRECT precedent for `no_pubkey_seam_in_release_build` (the test seam is release-forbidden; the `xtask check-arch` rule, `ci-cd-pipeline.md` §2.1 rule 3)
- D-D21 (no new ADR at DEVOPS layer) → the SAME outcome holds for slice-05 (D-D43)

**Slice-02 (D-D22..D-D29):**
- D-D22 (GitHub contract sub-job with public-endpoint allowlist) → the DIRECT precedent for the slice-05 contract allowlist; the `plc.directory` + `bsky.social` allowlist (D-D39)
- D-D23 (mutation scope widened to add `scraper-domain`; first widening) → with D-D31, the precedent for the THIRD widening (`appview-domain`, D-D40)
- D-D24/D-D25 (FakeGithub fixtures; PR/nightly mock vs release real) → the precedent for the slice-05 hermetic fakes (`FakeIngestSource`/`FakeIndexStore`/`FakeIndexQuery`) + mocked-in-PR / real-in-release (D-D36/D-D37)
- D-D26 (KPI-SCR feasibility GREEN/YELLOW; guardrails GREEN+release-blocking) → the SAME framework; the two KPI-AV guardrails are GREEN + release-blocking (D-D40)
- D-D27 (KPI post-hoc jq, no state file; reuses D-D18 survey) → the SAME post-hoc + survey reuse for KPI-AV (D-D40)
- D-D28 (KPI-SCR-3 auditability: CI gate + runtime counter + checklist line) → the DIRECT precedent for the slice-05 runtime guardrail counter `indexer_query_attribution_missing_total` backing the KPI-AV-2 CI gate (D-D35, D-D41)
- D-D29 (no new DEVOPS ADR) → the SAME outcome (D-D43)

**Slice-04 (D-D30..D-D34):**
- D-D30 (release-blocking GUARDRAIL acceptance jobs + runtime guardrail counters) → the DIRECT precedent for the three slice-05 cardinal guardrail jobs (KPI-AV-2/3 + KPI-5; D-D35)
- D-D31 (mutation scope widened to add `scoring`; second widening; ≥95%) → the DIRECT precedent for the THIRD widening (`appview-domain`, D-D40)
- D-D32 (KPI-GRAPH feasibility; no RED; guardrails GREEN+release-blocking; cohort YELLOW) → the SAME framework applied to KPI-AV (D-D40)
- D-D33 (renderer-review checklist gains a slice-04 line) → EXTENDED with a slice-05 line (D-D41)
- D-D34 (no new DEVOPS ADR; DESIGN raised the architectural ADRs; no external contract for the read-only LOCAL slice) → the SAME no-ADR outcome (D-D43), but slice-05 DOES add external contract surface (WD-123; D-D36/D-D37) — the slice-04 "no external surface" property does NOT carry to slice-05 (the first network service since slice-01/02)

## Locked slice-05 decisions

| # | Decision | Rationale | Status | ADR / Document |
|---|----------|-----------|--------|----------------|
| D-D35 | **The `openlore-indexer` binary ships as the SECOND deployable in the existing ADR-011 release matrix (4 platforms; `cargo install openlore-indexer`); the three cardinal release-blocking GUARDRAIL acceptance jobs land in the existing `ci.yml`: `at-indexer-rejects-unverified-claim` (KPI-AV-3), `at-network-result-preserves-attribution` (KPI-AV-2), `at-local-first-preserved` (KPI-5), plus the seven search-scenario ATs.** The indexer is a self-hostable single binary (`cargo run -p openlore-indexer` serve/ingest for the walking skeleton; a packaged service unit is a future concern). The index store (`index.duckdb`) is RE-BUILDABLE — its backup story is "re-ingest", not a backup target. The runtime guardrail counter `indexer_query_attribution_missing_total` (target 0 forever) backs the KPI-AV-2 gate. Windows stays deferred (the ADR-011 "slice-05 AppView introduces a need we can't avoid" trigger evaluates NO — a Linux/macOS self-hosted service). | KPI-AV-2/3 + KPI-5 are cardinal DISCUSS disprovers (`outcome-kpis.md` §Disprovers — any failure UNSHIPPABLE). The indexer is the FIRST network service (ADR-023); it extends the ADR-011 matrix by one artifact (ADR-011 already cites "slice-05 AppView" as the revisit trigger). The two cardinal guardrails extend the slice-03 KPI-FED-6/KPI-FED-1 to network scale. The runtime counter mirrors D-D28/D-D30. | LOCKED | `platform-design.md` §3, §4; `ci-cd-pipeline.md` §3, §5; `observability.md` §2.6, §4.2 |
| D-D36 | **Contract suite `contract-pact-indexer-query` (boundary B1, CLI↔indexer XRPC) — consumer-driven; the `openlore` CLI is the consumer, the indexer's query server the provider; the contract pins that EVERY wire result carries `author_did` (anti-merging across the transport, I-AV-2).** PR/nightly: MOCKED + in-process provider verify. Release: re-verify vs a real localhost `openlore-indexer serve` (no third party). The DTOs are SSOT in `lexicon` (both ends consume them). | WD-123 / DESIGN §6.4: B1 is a cross-process boundary whose response shape carries the cardinal per-result `author_did` — a provider change dropping it would silently merge authors in production; the contract catches it at build time. Mirrors the slice-03 `contract-pact-pds-peer` placement. The localhost own-binary provider has no third-party-rate-limit concern (unlike B2). | LOCKED | `contract-test-ownership.md` §2; `ci-cd-pipeline.md` §3.5 |
| D-D37 | **Contract suite `contract-pact-pds-network` (boundary B2, indexer→network-PDS/PLC) — consumer-driven; the indexer is the consumer; pins the `com.atproto.repo.listRecords` record shape (incl. the tampered/CID-mismatch/unsigned adversarial set) + the PLC DID-document `publicKeyMultibase` `z6Mk...` shape the verify-gate + ADR-026 decode depend on.** PR/nightly: RECORDED fixtures. Release: re-verify vs real `bsky.social` + real `plc.directory` (manual approval, D-D12). | WD-123 / DESIGN §6.4: B2 is the EXTERNAL adversarial-input boundary; an ATProto/PLC response-shape drift would silently break the ingest verification gate (KPI-AV-3). The release-tag real-`plc.directory` re-verify is the confirmation KPI-AV-3 holds against REAL network data — the cardinal slice-05 concern the slice-03 DV-4 seam left open. Extends the slice-03 `listRecords` Pact + slice-02 `contract-pact-github` discipline. | LOCKED | `contract-test-ownership.md` §3; `ci-cd-pipeline.md` §3.5 |
| D-D38 | **The ingest adversarial fixtures (tampered-sig / CID-mismatch / unsigned) + the real-`z6Mk...` DID-doc fixture are regenerated via `cargo xtask regenerate-ingest-fixtures` (extends the slice-03 `regenerate-peer-fixtures`, D-D15) against the live `org.openlore.claim` Lexicon + the `decode_ed25519_multibase` contract; an `arch-check`-stage `--check` run fails on drift.** DELIVER may DEFER the regenerator (the fixtures work without it; just risks drift). | KPI-AV-3 is a cardinal release-blocking guardrail; the adversarial fixtures are the only end-to-end exercise of the network-scale reject path + the real-decode gold path. Auto-regeneration prevents drift as the Lexicon evolves. Exactly the slice-03 D-D15 reasoning + escape hatch. | LOCKED with DELIVER-may-defer escape hatch | `ci-cd-pipeline.md` §7; `contract-test-ownership.md` §3.3 |
| D-D39 | **The contract-test public-endpoint allowlist (slice-02 D-D22) EXTENDS with `plc.directory` (NEW) alongside the existing `bsky.social` (slice-01/03).** The real-provider contract variant may contact ONLY these hosts; any other host is a contract-test failure. In PR/nightly NO external host is contacted (recorded fixtures only). The indexer's RUNTIME ingest contacts whatever its `config.toml` seeds/relay configure — operator config, distinct from the contract-test allowlist. | The `plc.directory` DID-document resolution (ADR-026) is the NEW external host slice-05 introduces; it is the production trust anchor for KPI-AV-3 against real data. The allowlist discipline (slice-02 D-D22) keeps the real-provider variant's egress auditable. | LOCKED | `contract-test-ownership.md` §3.4; `platform-design.md` §3.3 |
| D-D40 | **Mutation scope widens (THIRD widening): `crates/appview-domain` added to the nightly `cargo mutants --package` list (≥95% kill rate). KPI-AV feasibility: no KPI-AV marked RED.** KPI-AV-2, KPI-AV-3 (+ inherited KPI-5) = GREEN + RELEASE-BLOCKING. KPI-AV-1 (north star), KPI-AV-4, KPI-AV-6 = GREEN per-user / YELLOW cohort (CLI events + post-hoc jq + D-D18 survey; cohort = future endpoint OR PO day-30 outreach). KPI-AV-5 = GREEN per-user prompt / YELLOW cohort (the D-D18 one-shot comprehension prompt). The `claim-domain` decode helper is mutated within the existing `claim-domain` scope. The per-user/cohort split tightens slice-05's telemetry: NO DID in any `search.*` event (a DID stream would reveal the discovery+subscription graph). | D-D8 scopes mutation to pure-core; `appview-domain` is a GENUINELY NEW pure-core crate (the `ingest_decision` verify-gate + `compose_results` anti-merging composition — the two load-bearing trust primitives); un-mutated, a surviving gate/composition mutant means a tampered record could slip past, or an author be merged away, without test failure — the two cardinal disprovers. THIRD widening, mirroring D-D23/D-D31. The KPI-AV GREEN/YELLOW posture is the D-D17/D-D26/D-D32 policy applied to KPI-AV (the YELLOW cohort = the same deferred-endpoint constraint every prior slice carries). | LOCKED | `platform-design.md` §5; `ci-cd-pipeline.md` §4; `kpi-instrumentation.md` §1, §2, §10 |
| D-D41 | **The renderer-review checklist (D-D19/D-D28/D-D33) gains ONE slice-05 line: "the network search/share renderer never collapses authors into a consensus row, always renders `[verified]` + the relationship label + the up-front public-data banner, and `--share` encodes the query (not a snapshot)."** Recorded in the release CHANGELOG ("Renderer review: passed YYYY-MM-DD"). Solo dev = self-review. | KPI-AV-2 (anti-merging) + KPI-AV-5 (public-data framing) + KPI-AV-6 (query-not-snapshot share) are guardrail/outcome concerns whose CI gates cover CURRENT renderers, but a FUTURE renderer could regress without test coverage — exactly the D-D19/D-D28/D-D33 reasoning. The checklist is the human-in-the-loop backstop for the network search/share renderers specifically. | LOCKED | `kpi-instrumentation.md` §9; `ci-cd-pipeline.md` §5.8 |
| D-D42 | **`deny.toml` change (the FIRST since slice-01): narrow the `axum` ban so the indexer's query server may use it; rely on the STRUCTURAL `xtask check-arch` rule `indexer_holds_no_signing_or_local_store` (extended to assert the CLI links no HTTP server) to enforce "the CLI links no HTTP server".** `actix-web` stays banned; `bs58` (MIT) needs no license addition (already allowlisted). If DELIVER picks the hand-rolled `hyper` handler (Q-DELIVER-AV-2) + inlined base58btc (Q-DELIVER-AV-8), NO `deny.toml` edit is needed. The edit's rationale is recorded inline in `deny.toml` (the ADR-012-amendment discipline; not a silent unban). | The slice-01 `axum` ban premise ("OpenLore is a CLI; we never run an HTTP server in-process") is slice-05-obsolete — the indexer IS a network service serving HTTP (ADR-027). The structural arch rule is a STRONGER guarantee than a license-tool ban (the ban was always belt-and-suspenders for a property now enforced at the type/arch layer, I-AV-3/I-AV-5). DESIGN already justified `axum` (`technology-stack.md`). | LOCKED (with the hand-rolled-fallback escape hatch) | `platform-design.md` §10; `ci-cd-pipeline.md` §6 + Upstream Issue 1 |
| D-D43 | **No new ADR at the DEVOPS layer.** ADR-010, ADR-011, ADR-012 carry forward (ADR-011 GAINS the indexer artifact per D-D35; ADR-012's allowlist discipline is APPLIED via the D-D42 `deny.toml` narrowing — neither crosses the ADR threshold because the policies are unchanged). Slice-05's DESIGN raised ADR-023..027 — those are DESIGN ADRs. Slice-05's DEVOPS decisions (the new release artifact, the two contract jobs, the two guardrail gates, the mutation-scope widening, the indexer-operator observability surface, the `deny.toml` narrowing) are tactical extensions of D-D8/D-D11/D-D12/D-D23/D-D31 — none crosses the DEVOPS-ADR threshold. **CAVEAT**: the indexer-OPERATOR surface is the FIRST deviation from the single-user-CLI-no-operator model (slices 01-04); it does NOT meet the threshold for the walking-skeleton single-instance indexer, but a FUTURE hosted/multi-tenant indexer (ADR-023 revisit) WOULD need a DEVOPS ADR (SLOs, auth, rate-limiting, a real operational model). Recorded as a forward note. | ADR convention: cross-slice/cross-component architectural decisions. The WD-117 store choice IS an ADR — but it is ADR-025 (DESIGN), not DEVOPS. The new release artifact + contract jobs + guardrail gates + mutation widening + `deny.toml` narrowing are tactical applications of existing DEVOPS decisions. Same outcome as slice-03 D-D21, slice-02 D-D29, slice-04 D-D34. | LOCKED | `platform-design.md` §9 |

## Proposed (awaiting user confirmation)

None. All slice-05 DEVOPS decisions are LOCKED. (In auto-mode the recommended verdicts
are taken per the auto-mode product-defaults instruction; the user may override any
D-D35..D-D43 on review.)

## Open questions (handed to DESIGN — answered in parallel; recorded for traceability)

DESIGN ran in parallel and resolved every cross-wave question DEVOPS would otherwise
hand back. Recorded so the trace is complete:

1. **Deployment shape (self-hostable vs hosted)** — RESOLVED by WD-112 / ADR-023
   (self-hostable single binary). The DEVOPS consequence: one new release artifact
   (D-D35); no hosting/ops surface; the index store is re-buildable (no DR target).
2. **CLI→indexer transport** — RESOLVED by WD-115 / ADR-027 (HTTP/XRPC to a configured
   URL). The DEVOPS consequence: the B1 contract test (D-D36); localhost-transport
   environment; no third-party host for B1.
3. **Pull vs Firehose** — RESOLVED by WD-114 / ADR-024 (bounded PULL). The DEVOPS
   consequence: the ingest is hermetically testable (the `at-indexer-rejects-unverified-claim`
   adversarial fixtures); no daemon/reconnection observability concern; the ingest-lag
   freshness signal (not a stream-lag).
4. **The production pubkey decode** — RESOLVED by WD-118 / ADR-026 (real PLC `z6Mk...`
   decode; seam release-forbidden). The DEVOPS consequence: the B2 PLC DID-doc contract
   (D-D37); the `plc.directory` allowlist host (D-D39); `no_pubkey_seam_in_release_build`;
   the `claim-domain` decode helper in mutation scope.
5. **Index store choice** — RESOLVED by WD-117 / ADR-025 (separate `index.duckdb`,
   reuse DuckDB). The DEVOPS consequence: no new store engine; the `no_cross_table_join_elides_author`
   rule EXTENDS to the index-store SQL (the anti-merging-substrate reuse); the
   fsync-honesty probe on the indexer's separate store (the container-substrate lie).
6. **The two external boundaries** — RESOLVED by WD-123 / DESIGN §6.4 (CLI↔indexer +
   indexer→PDS/PLC; consumer-driven contracts). The DEVOPS consequence: D-D36/D-D37.

## Open questions (handed to DELIVER)

These are deliberately deferred to DELIVER. DEVOPS has defaults; DELIVER decides.

1. **`scripts/kpi-av-{1,3,4,6}.jq` + `scripts/indexer-coverage.jq` snippets** — DELIVER
   lands these alongside the foundation `kpi-{1,2,4,5}.jq`, slice-03 `kpi-fed-*.jq`,
   slice-02 `kpi-scr-*.jq`, slice-04 `kpi-graph-*.jq` snippets (`observability.md` §4.3).
2. **`openlore stats --discovery` + `openlore-indexer stats` flag/verb implementation**
   — concrete only if D-D5 verb landed; otherwise the jq snippets are the fallback.
3. **`deny.toml` `axum` ban edit (D-D42)** — REQUIRED on the `axum` path; UNNEEDED on
   the hand-rolled `hyper` path (Q-DELIVER-AV-2). Same for `bs58` vs inlined base58btc
   (Q-DELIVER-AV-8). DELIVER applies the edit (with inline rationale) IFF it picks the
   dep.
4. **The bootstrap: the new crates (`appview-domain` + 4 effect adapters) + the
   `openlore-indexer` binary in the workspace `Cargo.toml`** — DELIVER's first step; the
   `--workspace` CI jobs pick them up by construction afterward (`ci-cd-pipeline.md` §2).
   The new `appview-domain` enters the nightly `--package` list (D-D40) + the
   `arch-check` pure-core allowlist + the three new `arch-check` rules.
5. **`cargo xtask regenerate-ingest-fixtures` (D-D38)** — ship in slice-05 OR defer to
   follow-up. DELIVER's call (the slice-03 D-D15 escape hatch).
6. **Recorded `plc.directory` DID-doc fixture + `bsky.social` `listRecords` fixture** —
   DEVOPS captures these manually once (one-time setup) and commits to
   `tests/contracts/pact/`; DELIVER consumes the recordings (the slice-03 D-D15 §6.6
   pattern).
7. **Renderer-review checklist content (D-D41)** — DELIVER drafts the slice-05 line;
   DEVOPS reviews at release-tag time.
8. **The per-test ephemeral-port allocation for the localhost-transport ATs** — DELIVER
   binds to `:0` and reads back the port (NOT a fixed port) to keep the contract +
   search ATs parallel-safe (avoid a NEW bind race alongside the existing
   `OPENLORE_TEST_NOW` `--test-threads=1` workaround; `ci-cd-pipeline.md` §2).
9. **Ingest cadence + freshness budget defaults** (Q-DELIVER-AV-4) — DELIVER tunes
   `ingest_interval` + the ingest-lag freshness-budget alert threshold against the
   discovery freshness need (`observability.md` §10).

## Out of scope for DEVOPS slice-05 (explicit deferrals)

All foundation + slice-02/03/04 deferrals (SLOs/SLAs, runbooks, dashboards, telemetry
endpoint, auto-updater, multi-tenancy, DR, capacity, chaos, Windows, push-based
federation, multi-source scrapers, graph-store swap) carry forward unchanged. Slice-05
adds these explicit deferrals:

- **A hosted / multi-tenant indexer** (auth, rate-limiting, an operational SLA, a real
  operator model): NOT designed (ADR-023; the walking skeleton is self-hostable
  single-binary). The ADR-023 revisit trigger (coverage too sparse from a single
  instance) is the path; it would need a DEVOPS ADR (D-D43 caveat).
- **The author-side cohort telemetry endpoint**: NOT stood up in slice-05 (correcting
  the slice-04 forward-expectation — `platform-design.md` §11 Upstream Issue 2). The
  indexer-operator surface is a SERVICE LOG, not the cohort endpoint. The KPI-AV-1/4/5/6
  cohort + all prior cohort YELLOWs REMAIN deferred.
- **A backup/DR procedure for `index.duckdb`**: NOT designed (the index is re-buildable;
  "backup" is re-ingest; `platform-design.md` §3.4).
- **K8s / cloud / autoscaling / a managed index store**: NOT designed (`platform-design.md`
  §7 Alternative 1; over-engineered for a self-hostable single binary).
- **Distributed tracing across the CLI↔indexer boundary**: NOT designed (`observability.md`
  §1; per-process spans + a `request_id` echo suffice for a single CLI ↔ a single
  localhost indexer; the revisit trigger is a hosted multi-tenant indexer).
- **Load / stress / chaos testing of the indexer serve path**: NOT designed (D-D6
  carries; the walking-skeleton single self-hosted instance; RED-method serve metrics
  are captured but no load test).
- **A packaged service unit (systemd/launchd) for the indexer**: NOT designed (the
  walking skeleton ships `cargo run -p openlore-indexer`; a unit is a future concern,
  ADR-023).
- **Free-text claim-prose search infra** (a search engine / DuckDB FTS): NOT designed
  (ADR-025; the walking-skeleton search is exact dimensional lookup; the DuckDB FTS
  extension is the documented revisit path).

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (software-crafter — functional, per ADR-007) | every slice-05 DEVOPS doc + every slice-05 DESIGN doc (WD-111..124, ADR-023..027, I-AV-1..9) + slice-01/02/03/04 carryover + the Q-DELIVER-AV set + the open-questions lists | The bootstrap (new crates + the `openlore-indexer` binary in the workspace); additions to `.github/workflows/ci.yml` (3 cardinal guardrail ATs + 7 search-scenario ATs + 2 Pact sub-jobs) and `nightly.yml` (mutation `--package` += `appview-domain`); the `release.yml` indexer-binary matrix delta (when `release.yml` is authored); the `deny.toml` `axum`-ban narrowing (IFF the `axum` path, D-D42); the ingest adversarial fixtures + the real-`z6Mk...` DID-doc fixture; the recorded `plc.directory` + `bsky.social` contract fixtures (consumed from DEVOPS one-time recordings); `scripts/kpi-av-*.jq` + `scripts/indexer-coverage.jq`; the CLI `search.*` + indexer `indexer.*` tracing event emission; the runtime guardrail counter; the FOUR new adapter probes + the capability-boundary probe; the three new `arch-check` rules + the extended rule + the pure-core allowlist entry; the slice-05 renderer-review-checklist line; optionally `cargo xtask regenerate-ingest-fixtures`. |
| DISTILL (nw-acceptance-designer; PARALLEL) | the 8 release/acceptance gates + the DEVOPS-defined event shapes + the two contract boundaries' shapes + the hermetic fixtures | Executable acceptance tests for the search scenarios + the three cardinal guardrails (`indexer_rejects_unverified_claim`, `network_result_preserves_attribution`, `local_first_preserved`) + the trust/funnel/share/counter ATs, consuming the DEVOPS event shapes + the local fixtures. (DEVOPS runs PARALLEL with DISTILL; DISTILL reads DESIGN + this DEVOPS doc, not the reverse.) |
| Operations team (POST-DELIVER) | for the CLI: not applicable (still local-first CLI, no operations team). For the INDEXER: the single self-hosted dogfood OPERATOR reads `openlore-indexer stats` (the index-coverage/freshness dashboard) + the indexer log to operate the service — the FIRST operator surface in the product (still a single dogfood operator, no fleet, no on-call). | the operator runs `openlore-indexer serve`/`ingest`, watches the coverage/freshness + the verified/rejected ratio (the KPI-AV-1 sparsity diagnosis + the KPI-AV-3 health). |
| Future DEVOPS wave (a hosted/multi-tenant indexer, or whichever sibling stands up the cohort telemetry endpoint) | this doc + all prior DEVOPS docs + ADR-010 + ADR-023 (the hosted revisit) + the event-shape definitions | cohort aggregation for KPI-AV-1/4/5/6 (and the prior cohort YELLOWs: KPI-3/6, KPI-FED-3/5, KPI-SCR-1/5, KPI-GRAPH-1/5/6); the hosted-indexer operational model (SLOs, auth, rate-limiting — the DEVOPS ADR the D-D43 caveat flags). |

## Changelog

- 2026-05-28 — Apex — initial DEVOPS-wave decisions for slice-05 (openlore-appview-search).
  All decisions D-D35..D-D43 LOCKED. No new DEVOPS ADRs proposed (D-D43). Foundation
  D-D1..D-D13 + ADR-010..012, slice-03 D-D14..D-D21, slice-02 D-D22..D-D29, slice-04
  D-D30..D-D34 carry forward unchanged. The FIRST network service: a SECOND deployable
  (`openlore-indexer`) added to the ADR-011 release matrix (D-D35); TWO consumer-driven
  contract suites for the two external/cross-process boundaries (D-D36/D-D37); the
  `plc.directory` contract allowlist host added (D-D39); mutation scope widened to add
  `crates/appview-domain` (D-D40) — THIRD widening since slice-01 (after `scraper-domain`
  D-D23 + `scoring` D-D31); `CLAUDE.md` Mutation Testing Strategy unchanged in POLICY
  (nightly-only per D-D8), only the `--package` list grows. Three cardinal
  release-blocking GUARDRAIL gates (D-D35): verified-before-index (KPI-AV-3),
  anti-merging-at-network-scale (KPI-AV-2), local-first-preserved (KPI-5). The FIRST
  `deny.toml` change since slice-01 (the `axum` ban narrowed; D-D42 — flagged as an
  Upstream Issue). The FIRST indexer-OPERATOR observability surface (distinct from the
  author-side opt-in telemetry; `observability.md` §7). Two Upstream Issues flagged
  (the `deny.toml` ban premise; the slice-04 telemetry-endpoint forward-expectation
  corrected) — both non-blocking.
