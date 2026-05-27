# Evolution: openlore-foundation (slice-01 walking skeleton)

> Feature archive. Authored at finalize (Phase 7). Source of truth for all detail
> remains `docs/feature/openlore-foundation/feature-delta.md` and the 12 ADRs
> under `docs/adrs/`; this file is the post-mortem summary.

## Summary

`openlore-foundation` is the slice-01 walking skeleton of the OpenLore umbrella:
a single-binary Rust CLI that lets a senior engineer **compose -> sign -> persist
locally -> publish to PDS -> read back** a structured philosophical claim. It
proves the end-to-end thesis (Lexicon + signing + DuckDB + ATProto + local query)
in one minimal vertical slice and births sibling features (slices 02-05) into
their own future DISCUSS waves.

### Wave timeline

| Wave    | Start      | End        | Owner                              |
|---------|------------|------------|------------------------------------|
| DISCUSS | 2026-05-25 | 2026-05-25 | Luna (nw-product-owner)            |
| DESIGN  | 2026-05-25 | 2026-05-26 | Morgan (nw-solution-architect)     |
| DEVOPS  | 2026-05-25 | 2026-05-26 | Apex (nw-platform-architect)       |
| DISTILL | 2026-05-26 | 2026-05-26 | Quinn (nw-acceptance-designer)     |
| DELIVER | 2026-05-26 | 2026-05-27 | Crafter + Apex (orchestration)     |

### Shipping metrics

- **44/44 roadmap steps** done.
- **29/29 acceptance scenarios** GREEN (17 `walking_skeleton` + 8
  `lexicon_conformance` + 4 `federation_roundtrip`).
- **150 tests passing** workspace-wide.
- **80% mutation kill rate** on `claim-domain` (16 caught / 4 missed / 5 unviable
  out of 25 viable mutants; meets >=80% gate per ADR-011).
- **12 ADRs** all Accepted.
- **50 git commits** since DELIVER start.

## Wave-by-wave changelog

### DISCUSS (2026-05-25)

Locked the carpaccio split (umbrella -> 5 sibling features), the persona
priority (P-001 senior engineer solo builder as primary), and the J-001 job
("author a signed philosophical claim") as the walking-skeleton job. Resolved
five blocking open decisions (OD-1..OD-5) covering carpaccio approval,
confidence semantics (numeric `[0.0, 1.0]` only — no persisted buckets),
retraction model (counter-claim, no hard-delete), signing identity (reuse
user's ATProto DID with per-app derived key), and sibling-slice sequence
(federation before scrapers). Authored 4 user-visible stories plus 1
infrastructure story; all passed DoR. Two ask-intelligent expansions fired and
were accepted: `alternatives-considered.md` (DuckDB vs Kùzu vs SurrealDB; reuse
DID vs mint fresh; sign+publish as one verb vs two) and
`gherkin-scenarios-expanded.md` (anxiety-path and habit-path scenarios for
J-001). Defined 6 outcome KPIs with KPI-6 as north star and KPI-4 (zero silent
normalization) + KPI-5 (local-first invariant) as hard guardrails.

### DESIGN (2026-05-25 -> 2026-05-26)

Selected **hexagonal modular monolith** in single Rust binary (ADR-009),
**functional-leaning Rust** (pure core + effect shell, ADR-007). Cut the
codebase into 8 crates: `claim-domain` (pure canonicalization, CID, signing,
reference rules, confidence), `lexicon` (pure schemas + validation), `ports`
(pure traits — `StoragePort`, `IdentityPort`, `PdsPort`, `ClockPort`,
`ProbeOutcome` ADT), `adapter-duckdb`, `adapter-atproto-did`,
`adapter-atproto-pds`, `adapter-system-clock`, and `cli` (driver / composition
root). Authored 12 ADRs covering DuckDB single-file local store (ADR-001),
ATProto DID with per-app derived Ed25519 key (ADR-002), CLI noun-verb contract
(ADR-003), Tokio async runtime (ADR-004), `org.openlore.*` Lexicon namespace
(ADR-005), CIDv1 dag-cbor sha2-256 addressing (ADR-006), functional Rust
paradigm (ADR-007), retraction as counter-claim with no hard-delete (ADR-008),
hexagonal-modular-monolith style (ADR-009), opt-in telemetry policy (ADR-010),
release matrix and channels (ADR-011), and supply chain policy (ADR-012).
DEVOPS (parallel) designed the CI/CD pipeline, the nightly mutation job, the
`xtask check-arch` + `xtask check-probes` invariants, and the cargo-deny
supply-chain policy.

### DISTILL (2026-05-26)

Quinn authored the executable acceptance test corpus: 17 scenarios in
`tests/walking_skeleton.rs` covering the WS-1..WS-17 happy/edge/anxiety/habit
paths, 8 in `tests/lexicon_conformance.rs` pinning the Lexicon schema, and 4 in
`tests/federation_roundtrip.rs` proving the on-the-wire claim contract
round-trips byte-stable through canonicalization and CID computation. Built the
`test-support` crate (`FakePds`, `FakeKeychain`, `FakeClock`, `TempXdg`) so
acceptance tests run hermetically. Established the DOR-LOCKED corpus prior to
any DELIVER step touching production code.

### DELIVER (2026-05-26 -> 2026-05-27)

Executed 44 roadmap steps across 7 phases (00 bootstrap, 01 pure domain, 02
ports + lexicon, 03 adapters, 04 cli wiring, 05 acceptance integration, 06
quality gates). All 29 acceptance scenarios reached GREEN. 50 commits with
strict Step-ID trailers and DES-trace integrity verified (Phase 6:
`des-verify-integrity` PASS — all 44 steps have complete traces). Phase 3.5
Elevator Pitch demos: 3 strict pass + 1 structural-fail-with-acceptance-coverage
(US-003 publish demo against a placeholder PDS; success path is acceptance-
tested under `FakePds` in WS-8). Phase 4 adversarial review: **APPROVED** with
zero Testing Theater, zero production-test boundary violations, all critical
invariants verified. Phase 5 mutation testing on `claim-domain`: **80% kill
rate** (exactly meets >=80% gate). Phase 6 integrity verification: **PASS**.

## Mutation testing — final report

**Scope**: `claim-domain` (pure-core canonicalization, CID, signing, reference
rules, confidence helpers).

**Results**:

| Category   | Count |
|------------|-------|
| Caught     | 16    |
| Missed     | 4     |
| Unviable   | 5     |
| **Viable total** | **20** |
| **Kill rate** | **80%** (16/20) |

**Missed mutants** (preserved here as institutional knowledge — these are
candidates for follow-up test coverage in slice-02+):

| # | File:Line | Mutation | Why it survived |
|---|-----------|----------|-----------------|
| 1 | `crates/claim-domain/src/lib.rs:72` | `Confidence::value -> f64 with 0.0` | `Confidence::value()` is a trivial getter; no production caller exercises a discriminating downstream branch on the returned `f64` (consumers stringify or pass-through). |
| 2 | `crates/claim-domain/src/lib.rs:72` | `Confidence::value -> f64 with 1.0` | Same as #1. |
| 3 | `crates/claim-domain/src/lib.rs:72` | `Confidence::value -> f64 with -1.0` | Same as #1 — and `Confidence` constructor already pre-validates the `[0.0, 1.0]` range at construction time, so a downstream "is this in range" assertion would be testing the constructor, not `value()`. |
| 4 | `crates/claim-domain/src/references.rs:128` | `replace == with != in reference_rules_validate` (cycle-detection equality) | The flipped equality lives in a defensive cycle-detection branch that is unreached in the current US-001..US-005 corpus (slice-01 never produces a reference that traverses back to its origin within validation depth). Follow-up: add a property test in slice-02 that constructs an adversarial reference chain. |

**Disposition**: 80% meets the >=80% kill-rate gate per ADR-011. The 4 missed
mutants are tracked here rather than ignored; nightly cargo-mutants in
`.github/workflows/nightly.yml` will re-run advisorily on every push to `main`.

## Known caveats / deferred work

- **Real ATProto PDS resolution stubbed**: `adapter-atproto-pds` issues real
  HTTPS to whatever `OPENLORE_PDS_ENDPOINT` points at, but full bi-directional
  PDS flow (handle resolution, identity service round-trip, federation indexing)
  is deferred to **slice-03**. Slice-01 acceptance tests use `FakePds` from
  `test-support`.
- **WSL2 keychain fallback**: documented in ADR-002 but not exercised on the
  macOS development box; the macOS Keychain code path is the only one covered
  by integration tests in slice-01. Linux Secret Service + WSL2 fallback file
  paths ship but ride on nightly-only acceptance.
- **cargo-mutants is nightly-advisory, not per-PR** (per ADR-011): mutation
  testing runs on `nightly.yml` and surfaces as PR comments, but does NOT gate
  PR merges in slice-01. Promotion to PR-gating depends on the per-feature
  mutation strategy decided in `## Mutation Testing Strategy` of `CLAUDE.md`.
- **KPI-3 (felt-framing survey) + KPI-6 (day-30 north-star survey)**:
  measurement instrumentation requires the CLI `+stats` verb and an opt-in
  telemetry endpoint, both deferred per ADR-010. Survey delivery mechanism
  ships in slice-01 (CLI prompt hook); aggregation is post-slice-05.
- **US-003 publish demo (Phase 3.5)** structurally fails when no real PDS is
  bound; the success path is acceptance-tested by `walking_skeleton.rs::WS-8`
  against `FakePds.serve_http()`. To re-run the success demo, set
  `OPENLORE_PDS_ENDPOINT` to a real PDS or a wiremock instance.

## Pointers

- **Full feature delta** (DISCUSS through DELIVER, all detail):
  `docs/feature/openlore-foundation/feature-delta.md`
- **All 12 ADRs**: `docs/adrs/ADR-001-*.md` through
  `docs/adrs/ADR-012-*.md`
- **Architecture design** (C4 + Mermaid diagrams):
  `docs/feature/openlore-foundation/design/architecture-design.md`
- **Walking-skeleton spec**:
  `docs/feature/openlore-foundation/distill/walking-skeleton.md`
- **Cross-feature architecture brief** (SSOT going forward):
  `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT going forward):
  `docs/product/kpi-contracts.yaml`
- **CI / nightly mutation**: `.github/workflows/ci.yml`,
  `.github/workflows/nightly.yml`
- **Supply-chain policy**: `deny.toml`
