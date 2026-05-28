# Evolution: openlore-federated-read (slice-03 federated read)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/openlore-federated-read/`
> (feature-delta.md + the five wave dirs) and ADR-013..ADR-016 under
> `docs/adrs/`; this file is the post-mortem summary.

## Summary

`openlore-federated-read` is the slice-03 walking skeleton of the OpenLore
umbrella (job **J-003**: federated read). It turns the single-author slice-01
CLI into a **federated reader** without re-architecting it — an EXTENSION ONLY
slice that adds zero new crates (WD-26) and instead extends slice-01's eight
production crates + test-support + xtask in place. It proves four pillars:

1. **Subscribe** to another developer's signed claims (`openlore peer add`).
2. **Pull + verify** those claims locally — every record's signature is verified
   and its CID recomputed before it lands in `peer_claims` (`openlore peer pull`).
3. **Federated query with anti-merging** — `openlore graph query --federated`
   surfaces own + peer claims grouped by author DID, with zero merged-consensus
   rows; attribution is preserved field-for-field (`--federated` flag).
4. **Counter-claim authoring** — disagreement is one verb away
   (`openlore claim counter --reason ...`), reusing slice-01's single
   sign+publish pipeline (WD-33).

### Wave timeline

| Wave    | Date       | Owner                              |
|---------|------------|------------------------------------|
| DISCUSS | 2026-05-27 | Luna (nw-product-owner)            |
| DESIGN  | 2026-05-27 | Morgan (nw-solution-architect)     |
| DEVOPS  | 2026-05-27 | Apex (nw-platform-architect)       |
| DISTILL | 2026-05-27 | Quinn (nw-acceptance-designer)     |
| DELIVER | 2026-05-28 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **40/40 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **35/35 slice-03 acceptance scenarios** GREEN (8 `peer_subscribe` PS-1..8 +
  8 `peer_pull` PP-1..8 + 6 `counter_claim` CC-1..6 + 8 `federated_query`
  FQ-1..8 + 5 `lexicon_counter_claim` LCC-1..5).
- **Zero regression** on slice-01 suites (walking_skeleton 19,
  lexicon_conformance 10, federation_roundtrip 6).
- **100% mutation kill rate** on the declared slice-03 pure-core scope (14 + 3
  mutants caught; meets the per-feature >=80% gate per DV-2 + ADR-011).
- **4 ADRs** (ADR-013..ADR-016) all Accepted.
- **ZERO new crates** (WD-26); one new production dependency
  (`unicode-normalization`, a pure dep in `claim-domain`).
- DES integrity: `des-verify-integrity` reports "All 40 steps have complete DES
  traces."
- Adversarial review: **APPROVED** with zero blockers.

## Wave-by-wave changelog

### DISCUSS (2026-05-27)

Defined the J-003 federated-read objective: a subscribed reader walks away from
each session with a defensible view of WHO claimed WHAT with WHAT evidence,
never inheriting peer conclusions silently, with disagreement one verb away.
Authored six outcome KPIs (KPI-FED-1..6) with **KPI-FED-3** (>=30% of dogfood
cohort publishes >=1 counter-claim within 30 days) as the north star and three
guardrails: KPI-FED-2 (zero merged rows), KPI-FED-4 (zero purge residue),
KPI-FED-6 (zero invalid signatures stored). Locked WD-14..WD-25 covering the
four-pillar scope, the soft-vs-hard `peer remove` distinction, the counter-claim
verb shape, the `reason` field requirement, and the carpaccio sequencing
(federation before scoring graph). Inherited slice-01 KPI-4 (zero silent
normalization — now extended to peer-sourced claims) and KPI-5 (local-first —
federated queries on cached claims work offline; only `peer add`/`peer pull`
require network).

### DESIGN (2026-05-27)

Locked WD-26..WD-45 and authored four ADRs. The architectural thesis (WD-26):
slice-03 is a **port-and-table extension**, not a re-architecture — no new
crates. Port decisions: `PeerStoragePort` is a NEW port (WD-27, distinct
Earned-Trust contract, sharing the DuckDB connection pool) while peer-PDS reads
extend `PdsPort` (WD-28) and peer DID resolution extends `IdentityPort` (WD-29).
The anti-merging invariant I-FED-1 is enforced at **three orthogonal layers**
(WD-30): type (`FederatedRow.author_did` is non-`Option`), structural (`xtask
check-arch` `no_cross_table_join_elides_author` SQL rule), and behavioral
(FQ-1/FQ-2/FQ-8 acceptance). Storage uses 4 new DuckDB tables plus a
per-peer-DID filesystem partition so hard-purge is a directory removal (WD-31,
ADR-014). The `reason` field is a top-level optional property preserving CID
stability for slice-01 claims (WD-32, ADR-015); it is NFC-normalized in pure
core (WD-35) and self-counter rejection happens in `claim-domain` (WD-34).
Counter-claim authoring reuses `VerbClaimPublish` internals — no parallel
publish path (WD-33). Pull is sequential, fail-soft, with pull-time-only
verification (WD-37, WD-38, ADR-016); peer records claiming the user's own DID
(WD-40 SelfAttribution) or a third-party DID (WD-41 CrossAttribution) are
rejected at write time. Three once-per-user orientation messages (WD-39/42/43)
bridge habit; counter-claims emit no peer notification (WD-44). The four ADRs:
ADR-013 (verb-contract amendment), ADR-014 (peer-storage schema + anti-merging
invariant), ADR-015 (counter-claim lexicon `reason` extension), ADR-016
(pull-on-demand semantics). DEVOPS (parallel) extended the Pact suite contract
ownership (D-D12/D-D15 adversarial fixtures), set the autoconfirm build-time
guard (D-D20), and kept mutation nightly-scheduled (D-D8) while noting the
per-user vs cohort KPI instrumentation split (D-D17).

### DISTILL (2026-05-27)

Quinn authored the 35-scenario executable acceptance corpus across five files:
`peer_subscribe.rs` (PS-1..8), `peer_pull.rs` (PP-1..8), `counter_claim.rs`
(CC-1..6), `federated_query.rs` (FQ-1..8), and `lexicon_counter_claim.rs`
(LCC-1..5). Built the `FakePeerPds` wiremock HTTP server (hosting
`com.atproto.repo.listRecords` + `getRecord` + `identity.resolveDid`) plus five
adversarial constructors (tampered-signature, cid-mismatch, self-attribution,
cross-attribution, unreachable) and the recorded adversarial fixtures under
`tests/fixtures/peer-adversarial/` (D-D15). LCC-3 + LCC-4 carry the `@property`
tag and use proptest for the NFC-normalization properties. Resolved every DISTILL
`# confirm` flag against the locked DESIGN decisions (cross-attribution reject,
no query-time re-verification, no auto-notification, inline counter template by
default).

### DELIVER (2026-05-28)

Executed 40 roadmap steps across 5 phases via DES-monitored crafter dispatches,
each commit carrying a `Step-ID: NN-NN` trailer:

- **Phase 01 — Bootstrap (01-01..01-07):** extended ports + adapter-duckdb
  (migration v3) + adapter-atproto-{did,pds} + cli + test-support + xtask +
  lexicon. Established the DISTILL fail-for-right-reason gate — all 35 ATs
  compile and classify RED (panic at `todo!()`), not BROKEN.
- **Phase 02 — Lexicon + pure core (02-01..02-05):** LCC-1..5 +
  `normalize_reason` + `validate_counter_claim`. Layer-2 in-memory scenarios
  that unblock counter-claim validation in Phase 05.
- **Phase 03 — peer_subscribe (03-01..03-07):** PS-1..8 (US-FED-001 subscribe +
  US-FED-005 remove). Includes the load-bearing PS-6 (hard-purge atomic
  transaction; KPI-FED-4 release-gate) and PS-8 (`--no-tty --purge` refusal,
  WD-36).
- **Phase 04 — peer_pull (04-01..04-08):** PP-1..8 (US-FED-002). PP-1 is the
  J-003 walking-skeleton beat; PP-3 (tampered signature, KPI-FED-6 release-gate),
  PP-4 (CID mismatch), PP-5 (SelfAttribution, WD-40), PP-6 (CrossAttribution,
  WD-41) are the security gates; PP-7 proves per-peer fault isolation.
- **Phase 05 — counter_claim + federated_query (05-01..05-13):** CC-1..6
  (US-FED-004) + FQ-1..8 (US-FED-003). CC-1 is the KPI-FED-3 north-star beat;
  FQ-1 (KPI-FED-1 grouped attribution), FQ-2 + FQ-8 (KPI-FED-2 zero-merge
  release-gates), FQ-5 (bidirectional counter annotation, Pillar 2 cross-file
  narrative).

Phase 4 L1-L6 refactor (commit 38fa240): RPP L1-L4 applied; production clippy
warnings cleared (~10 -> 0); shared `bare_did` + `render_resolve_header` helpers
extracted. Phase 5 adversarial review (@nw-software-crafter-reviewer):
**APPROVED**, zero blockers — all 15 "no-production-change-needed" scenarios
confirmed genuine (zero Testing Theater); three-layer anti-merging enforcement
verified real; all 6 security/trust guards live and load-bearing; single-publish
path (I-FED-5) confirmed. Phase 6 mutation testing: 100% kill rate on the
declared slice-03 pure-core scope; DES integrity PASS.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-1 | DES stop-hook key-mismatch defect (hook reads `project_id`; `des-init-log` writes `feature_id`) resolved by adding a `project_id` header key to `execution-log.json`; `des-log-phase` preserves it on append. | Unblocked every step's stop-hook without touching the append-only event trail. Future slices: add the `project_id` header right after `des-init-log`. |
| DV-2 | Mutation strategy = per-feature >=80% (Phase 6), matching slice-01 precedent, despite DEVOPS D-D8's nightly-only CI scheduling. | Per-feature gate at deliver-time + nightly delta sweep as backstop. |
| DV-3 | Workspace rustfmt normalization committed as housekeeping (commit ca0ba95) mid-run. | Each crafter staged only its own files, leaving fmt churn uncommitted; a single chore commit prevented accumulation across 40 single-file-staging steps and kept the CI fmt gate green. |
| DV-4 | Test-only peer-pubkey seam `OPENLORE_PEER_PUBKEY_HEX_<did>` added in adapter-atproto-did (mirrors the endpoint seam); production multibase (`z6Mk...`) key decode is a documented TODO. | `FakePeerPds`'s `resolveDid` DID-doc carries a placeholder key; the seam keeps acceptance hermetic. Real PLC key decode lands when production PLC resolution ships (slice-04+). |

## Quality gates — final report

- **Acceptance**: 35/35 slice-03 scenarios GREEN; slice-01 suites zero
  regression. Full suite GREEN single-threaded (2026-05-28).
- **`cargo xtask check-arch`**: OK (10 workspace members) — anti-merging SQL rule
  + autoconfirm-release-build guard active.
- **`cargo xtask check-probes`**: OK (one bootstrap-allowlisted stub warning for
  the not-yet-live `PeerStoragePort` gauntlet probe; exit unaffected).
- **`cargo deny check`**: clean (`unicode-normalization` MIT/Apache-2.0 covered
  by the existing allowlist).
- **Adversarial review**: APPROVED, zero blockers (see DELIVER changelog above).
- **DES integrity**: PASS — all 40 steps have complete DES traces.

## Mutation testing — final report

**Scope**: slice-03 pure-core additions only (per D-D8 + the roadmap mutation
note). Run with cargo-mutants 25.3.1.

| Target | Mutants | Caught | Missed | Kill rate |
|--------|--------:|-------:|-------:|-----------|
| `claim-domain::normalize_reason` + `validate_counter_claim` | 14 | 14 | 0 | **100%** |
| `claim-domain::canonicalize` (reason folding)               | 3  | 3  | 0 | **100%** |
| `lexicon::claim` (slice-03 reason Gate 4)                   | (slice-03 mutants caught) | — | — | gate met |

Slice-03 per-feature gate SATISFIED (>=80%; actual 100% on the declared scope).

**Slice-01 finding (out of slice-03 scope, logged for D-D8 nightly backstop):**
whole-file mutation of `lexicon/src/claim.rs` surfaced **2 surviving mutants** in
pre-existing slice-01 reference validation — `delete !` at line 207
(`!entry_obj.contains_key`) and line 220 (`!ALLOWED_REFERENCE_TYPES.contains`).
These are ADR-008 reference-field-presence / allowed-type checks that predate
slice-03's `reason` gate; no slice-03 test exercises a reference missing a
required field or carrying a disallowed type at the lexicon layer. Logged for
slice-01 reference-validation test hardening via the nightly mutation sweep;
NOT a slice-03 deliverable.

## Lessons learned / issues

- **DES stop-hook key mismatch (DV-1)**: the stop-hook read `project_id` while
  `des-init-log` wrote only `feature_id`, blocking every step's hook. Fixed by
  adding a `project_id` header to `execution-log.json` and preserving it on
  append. Institutional fix for future slices: write the `project_id` header
  immediately after `des-init-log`.
- **Workspace fmt drift accumulation (DV-3)**: because each step staged only its
  own files (single-file-staging discipline), rustfmt churn on shared files
  accumulated uncommitted across the 40-step run. Resolved by the single
  housekeeping commit ca0ba95. Future single-file-staging runs should expect a
  mid-run fmt-normalization chore commit.
- **Test-only peer-pubkey seam (DV-4)**: peer signature verification in slice-03
  rides on a test-only `OPENLORE_PEER_PUBKEY_HEX_<did>` seam against
  `FakePeerPds`; **production multibase (`z6Mk...`) public-key decode from a real
  PLC DID document is deferred to slice-04+** (along with real PLC resolution).
  This is the single most important "shipped != production-complete" caveat.
- **Known adapter-system-clock parallel-run flake**: the slice-01
  `adapter-system-clock` test `now_utc_honors_openlore_test_now_env_var`
  intermittently fails under full-workspace PARALLEL runs because two sibling
  tests race on the process-global `OPENLORE_TEST_NOW` env var. Passes
  deterministically single-threaded / in isolation. The clock crate is untouched
  by slice-03; tracked for a future test-isolation fix.
- **Non-blocking review notes** (test-optimizer candidates, not slice-03
  deliverables): FQ-1 docstring says "Maria" but the test uses the harness
  identity (cosmetic); dead test-support exports (`fixture_adversarial_*`,
  `ADVERSARIAL_RKEY`).

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | Roadmap sized 40 steps for 35 ATs; phase boundaries fixed at DESIGN. | Step **05-12 (FQ-7)** hit a cross-scenario scaffold conflict: the locked WD-42 inline-counter template makes the peer CID appear a 2nd time in stdout, breaking FQ-2's raw-substring `matches(cid).count()==1` assertion (which was outside the FQ-7 editable boundary). Resolved with authorization by changing FQ-2 to count `cid:` **field-lines** (excluding the FQ-7 template mention). | Recorded. The fix is a tightening, not a weakening: FQ-2's zero-merge assertion remains genuine (reviewer-confirmed it counts `cid:` field-lines == 1 per row). |
| 2 | DESIGN assumed peer pubkey arrives via production PLC DID-document multibase decode. | Slice-03 ships a **test-only** `OPENLORE_PEER_PUBKEY_HEX_<did>` seam (DV-4); production `z6Mk...` decode is a documented TODO. | Deferred to slice-04+ when real PLC resolution lands. Acceptance stays hermetic via `FakePeerPds`. |
| 3 | DEVOPS D-D8 scheduled mutation nightly-only; per-user KPI instrumentation split from cohort aggregation (D-D17). | DELIVER ran mutation per-feature at deliver-time (DV-2) in addition to the nightly backstop. KPI-FED-3 / KPI-FED-5 cohort aggregation remains **YELLOW** pending the telemetry endpoint (D-D17); per-user instrumentation is GREEN. | Recorded as KPI status in `docs/product/kpi-contracts.yaml`. |
| 4 | DESIGN did not anticipate the slice-01 lexicon reference-validation mutation gap. | Whole-file mutation of `lexicon/src/claim.rs` surfaced 2 surviving slice-01 mutants (ADR-008 reference checks). | Logged for the D-D8 nightly sweep + slice-01 reference-validation test hardening; NOT a slice-03 deliverable (out of declared mutation scope). |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/openlore-federated-read/` (feature-delta.md + discuss/ design/
  distill/ devops/ deliver/)
- **Slice-03 ADRs**: `docs/adrs/ADR-013-verb-contract-amendment-federated-read.md`,
  `docs/adrs/ADR-014-peer-storage-schema-anti-merging-invariant.md`,
  `docs/adrs/ADR-015-counter-claim-lexicon-extension-reason-field.md`,
  `docs/adrs/ADR-016-pull-on-demand-semantics.md`
- **Architecture design** (C4 + Mermaid, kept in the feature workspace):
  `docs/feature/openlore-federated-read/design/architecture-design.md`
- **Component boundaries / data models / tech stack**:
  `docs/feature/openlore-federated-read/design/component-boundaries.md`,
  `docs/feature/openlore-federated-read/design/data-models.md`,
  `docs/feature/openlore-federated-read/design/technology-stack.md`
- **Wave decisions**: DISCUSS WD-14..25 + DESIGN WD-26..45 in
  `docs/feature/openlore-federated-read/design/wave-decisions.md`; DELIVER
  DV-1..4 in `docs/feature/openlore-federated-read/deliver/wave-decisions.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/openlore-federated-read/deliver/execution-log.json`,
  `docs/feature/openlore-federated-read/deliver/roadmap.json`
- **Outcome KPIs (slice-03 rationale)**:
  `docs/feature/openlore-federated-read/discuss/outcome-kpis.md`
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Slice-01 evolution** (precedent): `docs/evolution/openlore-foundation-evolution.md`
- **CI / nightly mutation**: `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`
- **Supply-chain policy**: `deny.toml`
</content>
</invoke>
