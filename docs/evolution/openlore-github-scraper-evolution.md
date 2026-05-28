# Evolution: openlore-github-scraper (slice-02 GitHub scrape -> propose -> sign)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/openlore-github-scraper/`
> (feature-delta.md + the five wave dirs) and ADR-017..ADR-019 under
> `docs/adrs/`; this file is the post-mortem summary.

## Summary

`openlore-github-scraper` is the slice-02 walking skeleton of the OpenLore
umbrella (job **J-004**: evaluate a contributor's body of work through a
philosophy lens). It harvests a target's PUBLIC GitHub signals, derives small,
auditable, conservative-confidence **candidate claims** from those signals via
the `jobs.yaml` signal->predicate SSOT mapping, and routes them through the
slice-01 single sign+publish pipeline — so the human is always the sole signer.
The thesis is the **scrape -> propose -> sign human-gate**: the tool proposes,
the human signs; nothing is ever asserted on the user's behalf, and only ever
from public data.

Unlike slice-03 (an EXTENSION ONLY slice), slice-02 is the first **two-crate
additive extension** since slice-01 (WD-59): it adds the PURE `scraper-domain`
core + the EFFECT `adapter-github` shell and extends `ports` + `cli` + `xtask`
in place — production crate count **10 -> 12**. It proves four pillars:

1. **Harvest** a target's public GitHub signals (`openlore scrape github
   <target>`) — public-data-only, with a reassurance banner before any network
   beat, and ZERO persistence without a sign.
2. **Derive** auditable candidate claims in pure core — every candidate names
   its source signal(s), carries the conservative 0.25 default confidence
   (never auto-inflated), and is byte-deterministic.
3. **Review + edit + sign** via the slice-01 pipeline (`--sign N[,N,...]`) —
   `CandidatePrefill` is the ONLY bridge from a candidate to a signed claim; no
   parallel publish path.
4. **Optional PAT** (`GITHUB_TOKEN` env var) — raises the rate budget without
   the token ever leaking to stdout/stderr/the signed payload/the PDS.

### Sequencing note (planned vs shipped)

Per the umbrella sequence (WD-13: federation -> scrapers -> scoring -> appview),
**slice-02 (scrapers) shipped AFTER slice-03 (federation)** on 2026-05-28. The
brief records slice-02 as shipped alongside slice-03; slices 04 (scoring-graph)
and 05 (appview-search) remain planned.

### Wave timeline

| Wave    | Date       | Owner                              |
|---------|------------|------------------------------------|
| DISCUSS | 2026-05-28 | Luna (nw-product-owner)            |
| DESIGN  | 2026-05-28 | Morgan (nw-solution-architect)     |
| DEVOPS  | 2026-05-28 | Apex (nw-platform-architect)       |
| DISTILL | 2026-05-28 | Quinn (nw-acceptance-designer)     |
| DELIVER | 2026-05-28 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **39/39 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **34/34 slice-02 acceptance scenarios** GREEN (9 `scrape_github` SG-1..9 +
  5 `scrape_candidates` SC-1..5 + 9 `scrape_sign` SS-1..9 + 5 `scrape_auth`
  SA-1..5 + 6 `scraper_domain` SD-1..6).
- **Zero regression** on slice-01 + slice-03 suites.
- **92.3% mutation kill rate** on the declared slice-02 pure-core scope
  (`scraper-domain`; 12/13 viable mutants caught; meets the per-feature >=80%
  gate per DV-2 + ADR-011).
- **3 ADRs** (ADR-017..ADR-019) all Accepted/shipped.
- **TWO new crates** (WD-59): PURE `scraper-domain` + EFFECT `adapter-github`;
  one new production dependency (`serde_yaml_ng`, pure, in `scraper-domain`);
  `adapter-github` reuses the workspace `reqwest` (no new transport).
- DES integrity: `des-verify-integrity` reports "All 39 steps have complete DES
  traces."
- Adversarial review: **APPROVED** with zero blockers.

## Wave-by-wave changelog

### DISCUSS (2026-05-28)

Defined the J-004 contributor-evaluation objective: produce a well-evidenced,
signed claim about a target through a philosophy lens in a fraction of
hand-authoring time, while always remaining the sole signer and only ever
touching public data. Authored five outcome KPIs (KPI-SCR-1..5) with
**KPI-SCR-1** (cost-to-first-signed-claim < 2 min INCLUDING vocabulary
discovery) as the north star and two guardrails: **KPI-SCR-2** (human-gate:
zero unsigned persistence / auto-publish) and **KPI-SCR-4** (public-data-only).
Locked WD-46..WD-58 covering the harvest->propose->sign scope, the human-gate
(WD-49), public-data-only (WD-51), the conservative 0.25-numeric-never-inflated
confidence (WD-52), the mapping SSOT discipline (WD-53), nothing-persisted-
unsigned (WD-55), the pure/effect split (WD-56), and the two-new-crates shape
(WD-57). Opened OD-SCR-1..4 (verb shape, PAT config surface, derived-from
storage, contributor depth) for DESIGN. Inherited slice-01 KPI-4 (zero silent
normalization — a signed-from-scraper claim is byte-identical to a hand-authored
one) and KPI-5 (local-first — `scrape` REQUIRES network for harvest, but
review/edit/sign follow the local-first rule).

### DESIGN (2026-05-28)

Locked WD-59..WD-68 and authored three ADRs. The architectural thesis (WD-59):
slice-02 is a **two-crate additive extension**, not a re-architecture — PURE
`scraper-domain` + EFFECT `adapter-github`, count 8 -> 10 (slice-03's count was
10 by then; the umbrella production count after slice-02 is 12). Resolved the
four DISCUSS open decisions: OD-SCR-1 -> the sugar verb `scrape github <target>
[--sign N[,N,...]]` (WD-60, ADR-017, symmetric with slice-03's sugar verbs);
OD-SCR-2 -> `GITHUB_TOKEN` env-var only, config-file deferred (WD-63, ADR-019);
OD-SCR-3 -> `derived-from` provenance is DISPLAY-ONLY, NO Lexicon change, CID
byte-identical to a hand-authored claim (WD-62, ADR-018, I-SCR-7); OD-SCR-4 ->
contributor (user) targets harvest a BOUNDED cross-repo aggregate, deep
triangulation deferred to slice-04 (WD-64). Port decision (WD-61, ADR-019):
`GithubPort` is a **NEW** port, NOT a `PdsPort` extension — GitHub shares no
method shape, auth model, rate-limit semantics, or failure surface with ATProto,
so folding it into `PdsPort` would conflate two unrelated trust boundaries
(contrast slice-03's WD-28, where peer reads genuinely WERE ATProto XRPC). The
signal->predicate mapping is EMBEDDED from `jobs.yaml` J-004 at build time via
`include_str!` + a pure parse, guarded by a `mapping_matches_ssot` drift test —
runtime filesystem reads from `scraper-domain` are forbidden (WD-67, keeps I-2
pure). The candidate->sign bridge reuses `VerbClaimAdd` + `VerbClaimPublish`
internals via `CandidatePrefill`; NO parallel publish path (WD-66, mirrors
slice-03's WD-33). `scraper-domain` joins the `xtask check-arch` pure-core
allowlist (its YAML-parse dep whitelisted, WD-65). The three ADRs: ADR-017
(verb-contract amendment), ADR-018 (candidate-claim model + signal->predicate
mapping + display-only provenance), ADR-019 (GitHub adapter: new `GithubPort`,
`reqwest` reuse, rate-limit + optional-PAT policy, public-data-only probe).
DEVOPS (parallel) owned the GitHub public-endpoint allowlist Pact contract
(D-D22/D-D24, KPI-SCR-4 release-gate), the recorded-vs-live fixture split
(D-D24), the wiremock stub fixtures (D-D25), added `scraper-domain` to the
nightly mutation sweep (D-D23), and noted the per-user vs cohort KPI
instrumentation split for KPI-SCR-1/5 (D-D26).

### DISTILL (2026-05-28)

Quinn authored the 34-scenario executable acceptance corpus across five files:
`scrape_github.rs` (SG-1..9), `scrape_candidates.rs` (SC-1..5),
`scrape_sign.rs` (SS-1..9), `scrape_auth.rs` (SA-1..5), and `scraper_domain.rs`
(SD-1..6, of which SD-1/SD-2/SD-3 carry the `@property` tag and use proptest for
the auditability / no-auto-inflation / determinism invariants). Built the
`FakeGithub` wiremock HTTP double (constructor-pinned postures: public repo,
public user, not-found, private target, offline, rate-limited-anon,
rejected-token; observable via `saw_token` + `seen_paths` but never echoing the
token value) + the 5 recorded `tests/fixtures/github/` wiremock stubs (D-D25).
Resolved every DISTILL `# confirm` flag against the locked DESIGN decisions
(sugar verb WD-60, display-only provenance WD-62, env-only PAT WD-63, skip
gesture Q-DELIVER-5 asserted as behavior not keystroke).

### DELIVER (2026-05-28)

Executed 39 roadmap steps across 5 phases via DES-monitored crafter dispatches,
each commit carrying a `Step-ID: NN-NN` trailer:

- **Phase 01 — Bootstrap (01-01..01-05):** extended `ports` with `GithubPort` +
  `TargetKind` + `GithubError` + slice-02 `ProbeRefusalReason` variants; created
  the PURE `scraper-domain` crate (ADTs + `derive_candidates` + `load_mapping` +
  the embedded `jobs.yaml` snapshot) and the EFFECT `adapter-github` crate
  (`GithubPort` impl + probe + env seams); wired the `cli scrape github
  [--sign]` dispatch + `CandidatePrefill` + `SelectionParser`; materialized
  `FakeGithub` + fixtures + the 5 `tests/fixtures/github/` stubs; extended
  `xtask`; and registered all 5 `[[test]]` targets AT ONCE (DD-SCR-14, not
  deferred per-file like slice-03) so the RED gate covers all 34. All 34 ATs
  classify RED (panic at `todo!()`), not BROKEN.
- **Phase 02 — scraper-domain pure core (02-01..02-06):** SD-1..6. The
  LOAD-BEARING layer-2 `@property` invariants — SD-1 (every candidate names
  >=1 source signal, auditability / KPI-SCR-3), SD-2 (every candidate confidence
  == 0.25, no auto-inflation / KPI-SCR-2), SD-3 (determinism) — plus SD-4
  (multi-signal collapse), SD-5 (empty-on-no-match), and SD-6
  (`mapping_matches_ssot` drift gate against `jobs.yaml` J-004).
- **Phase 03 — scrape_github harvest (03-01..03-09):** SG-1..9. SG-1 is the
  slice-02 walking skeleton (scrape->propose half; the sign half is SS-1); SG-2
  (public-data banner ordering), SG-3 (bounded user aggregate, WD-64), SG-4
  (404), SG-5 (`scraper_only_reads_public_data` release-gate, KPI-SCR-4), SG-6
  (offline), SG-7 (no-match exit 0), SG-8/SG-9 (persist-nothing reinforcements
  driving `scraper_never_persists_unsigned`, KPI-SCR-2).
- **Phase 04 — candidates + auth (04-01..04-09):** SC-1..5 (SC-1
  `candidate_names_source_signal` release-gate KPI-SCR-3; SC-3 confidence
  display-bucket proposal half; SC-4 collapse rendering; SC-5 disagreed-yet-
  auditable) + SA-1..4 (optional PAT budget + no-leak, unauth small, rate-limit
  -> suggest-token, rejected-token 401, WD-63).
- **Phase 05 — sign + token-no-leak (05-01..05-10):** SS-1..9 (review->edit->
  sign via the slice-01 pipeline; batch `--sign N,N,...`) + SA-5. SS-1 is the
  walking-skeleton sign half + `scraper_reuses_slice01_publish_path` release-gate
  (I-SCR-6); SS-2 (byte-for-byte no-edit accept, KPI-SCR-2 sign half); SS-3
  (provenance display-only / CID stability, WD-62/I-SCR-7); SS-6 (decline
  publish, local-persist via slice-01); SS-7/8/9 (batch walk, skip-continues,
  invalid-list rejected); SA-5 (token-no-leak on sign).

Phase 4 L1-L6 refactor (commit 6d45612): honest "already clean" RPP assessment;
the `adapter-github` auth-report side channel was contained (process-global
`OnceLock<Mutex>` -> thread-local `Cell`, documented as accepted slice-02
tech-debt because `GithubPort` cannot carry `AuthReport` without a contract
change); `--sign` batch orchestration extracted out of `run()`; clippy +
check-arch + check-probes clean. Phase 5 adversarial review
(@nw-software-crafter-reviewer): **APPROVED**, zero blockers — all scrutinized
"no-production-change, just-unskip" clusters (SG-2/4/6/7/8/9, SC-1..5,
SS-2/3/4/5, SA-2/3/4/5) confirmed GENUINE load-bearing port-to-port (a
deletion-test would red them), zero Testing Theater; human-gate (no-write-
without-sign + single-publish-path), public-data-only, and double-observable
token-no-leak (`saw_token` + `assert_token_value_absent`) all PASS; all 9 TDD
gates satisfied. Phase 6 mutation testing: 92.3% kill rate on the declared
slice-02 pure-core scope (one low-value `MappingError::Display` miss logged);
DES integrity PASS.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-1 | DES `project_id` header added to `execution-log.json` immediately after `des-init-log` (same hook-defect workaround as slice-03 DV-1: stop-hook reads `project_id`; `des-init-log` writes `feature_id`). | Unblocked every step's stop-hook without touching the append-only event trail. The institutional fix carried from slice-03. |
| DV-2 | Mutation strategy = per-feature >=80% on the new pure-core `scraper-domain` (Phase 6), matching slice-03 DV-2, despite DEVOPS D-D23 scheduling it nightly-only in CI. | Per-feature gate at deliver-time + nightly delta sweep as backstop. The signal->predicate derivation is the load-bearing pure-core trust primitive. |
| DV-3 | Workspace rustfmt normalization committed as housekeeping mid/end-of-run. | Each per-file-staging crafter accumulates fmt drift on shared files; a chore commit keeps the CI fmt gate green across the 39-step single-file-staging run. Mirrors slice-03 DV-3. |
| DV-4 | A transient source-write-guard race blocked step 05-01's FIRST GREEN attempt (the `PreToolUse(Task)` hook did not set `.nwave/des/des-task-active` after the 04-04 rate-limit interruption). Resolved by **re-dispatching the crafter normally** (the hook set the marker on the fresh dispatch) — **NOT** by forging the guard marker. | The session guard (`session_guard_policy.py`) correctly blocks orchestrator source writes; a legitimately re-dispatched crafter is the ONLY honest path. The execution log records the blocked first attempt + the clean re-dispatch. |
| DV-5 | `serde_yaml_ng` chosen as the PURE YAML parser for `scraper-domain`'s embedded mapping (Q-DELIVER-1); `reqwest` reused for `adapter-github` (no octocrab). | Minimal new dependency surface; license-clean (MIT/Apache-2.0); a maintained fork of the archived `serde_yaml`. No new transport crate. |

## Quality gates — final report

- **Acceptance**: 34/34 slice-02 scenarios GREEN (34 scenarios + support
  self-tests = 42 test fns); slice-01 + slice-03 suites zero regression. Full
  suite GREEN single-threaded (2026-05-28).
- **`cargo xtask check-arch`**: OK (12 workspace members) — `scraper-domain`
  pure-core allowlist (with the `serde_yaml_ng` whitelist) + GitHub public-only
  enforcement active.
- **`cargo xtask check-probes`**: OK — the `GithubAdapter` probe is real and
  non-stub; the single allowlisted-stub warning is the **pre-existing slice-03
  peer-storage probe** (knowingly accepted at the slice-03 review, OUT of
  slice-02 scope).
- **Adversarial review**: APPROVED, zero blockers (see DELIVER changelog above).
- **DES integrity**: PASS — all 39 steps have complete DES traces.

## Mutation testing — final report

**Scope**: slice-02 pure-core additions only (`scraper-domain`; per D-D23 + the
roadmap mutation note). Run with cargo-mutants 25.3.1.

| Target | Mutants | Caught | Missed | Unviable | Kill rate |
|--------|--------:|-------:|-------:|---------:|-----------|
| `scraper-domain` (derive + mapping) | 20 | 12 | 1 | 7 | **92.3%** (12/13 viable) |

Slice-02 per-feature gate SATISFIED (>=80%; actual 92.3% on the declared scope).

**The 1 surviving mutant** replaces `MappingError`'s `Display::fmt` with a
default — i.e. the error-message TEXT for a malformed mapping is not assertion-
covered. Low-value: `MappingError` cannot fire in practice because the embedded
mapping is build-time SSOT-validated by `mapping_matches_ssot`. Logged for a
future test-optimizer pass; NOT a slice-02 deliverable. `adapter-github` is NOT
mutated by design (effect shell; covered by the probe gold-tests + the DEVOPS
contract test).

## Lessons learned / issues

- **DES `project_id` hook workaround (DV-1)**: the stop-hook read `project_id`
  while `des-init-log` wrote only `feature_id`. Fixed (as in slice-03) by adding
  the `project_id` header to `execution-log.json` right after init.
- **Transient source-write-guard race at 05-01 (DV-4)**: the FIRST 05-01 GREEN
  attempt was blocked because the `PreToolUse(Task)` hook had not set
  `.nwave/des/des-task-active` after the 04-04 rate-limit interruption. It was
  resolved by a **normal crafter re-dispatch** (the fresh dispatch set the
  marker) — explicitly NOT by forging the guard marker. This is the single most
  important process caveat: the guard worked as designed; the honest path is
  re-dispatch, and the execution log preserves both the blocked attempt and the
  clean retry.
- **fmt drift cleanup (DV-3)**: per-file-staging accumulated rustfmt churn on
  shared files; resolved by a housekeeping chore commit. Future single-file-
  staging runs should expect this.
- **auth-report thread-local (accepted tech-debt)**: the `adapter-github`
  auth-report side channel is a thread-local `Cell` because `GithubPort` cannot
  carry `AuthReport` without a contract change. Safe for the single-threaded CLI;
  revisit when `GithubPort` widens.
- **`parse_selection` dup-detection gap (fixed 05-09)**: `parse_selection`
  lacked the duplicate-index detection its docstrings claimed; SS-9 surfaced it
  and the fix (dup + range rejection BEFORE compose) landed at step 05-09.
- **slice-03 peer-storage probe stub (pre-existing, out of scope)**: the
  bootstrap-allowlisted `PeerStoragePort` gauntlet probe stub remains (it was
  knowingly accepted at slice-03's review); it is NOT a slice-02 deliverable and
  did not affect `check-probes` exit.
- **Known adapter-system-clock parallel-run flake (pre-existing)**: the slice-01
  `adapter-system-clock` `now_utc_*` test intermittently fails under
  full-workspace PARALLEL runs (two sibling tests race on the process-global
  `OPENLORE_TEST_NOW` env var); passes single-threaded / in isolation. Untouched
  by slice-02.

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | Umbrella sequence WD-13: federation -> scrapers -> scoring -> appview. | Slice-02 (scrapers) **shipped AFTER** slice-03 (federation), both on 2026-05-28. | Recorded. Sequence preserved; slice-02 is recorded in the brief as shipped alongside slice-03 (just as slice-03 was recorded ahead of slice-02). Slices 04/05 remain planned. |
| 2 | DEVOPS D-D23 scheduled mutation nightly-only; per-user KPI instrumentation split from cohort aggregation (D-D26). | DELIVER ran mutation per-feature at deliver-time (DV-2) in addition to the nightly backstop. KPI-SCR-1 / KPI-SCR-5 cohort aggregation remains **YELLOW** pending the telemetry endpoint (D-D26); per-user instrumentation is GREEN. | Recorded as KPI status in `docs/product/kpi-contracts.yaml` (KPI-SCR-1/5). |
| 3 | `parse_selection` docstrings claimed duplicate-index detection. | The detection was missing; SS-9 surfaced the gap and the fix landed at step 05-09 (dup + range rejection before compose). | Recorded. The fix is a tightening that brings behavior in line with the documented contract; reviewer-confirmed correct. |
| 4 | One pure-core mutation gate at >=80%. | 92.3% achieved; 1 surviving low-value `MappingError::Display` mutant (cannot fire in practice — guarded by `mapping_matches_ssot`). | Logged for a future test-optimizer pass; gate met, NOT a slice-02 deliverable. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/openlore-github-scraper/` (feature-delta.md + discuss/ design/
  devops/ distill/ deliver/)
- **Slice-02 ADRs**:
  `docs/adrs/ADR-017-verb-contract-amendment-github-scraper.md`,
  `docs/adrs/ADR-018-candidate-claim-model-signal-predicate-mapping.md`,
  `docs/adrs/ADR-019-github-adapter-rate-limit-pat-policy.md`
- **Architecture design / component boundaries / data models / tech stack**
  (kept in the feature workspace):
  `docs/feature/openlore-github-scraper/design/`
- **Wave decisions**: DISCUSS WD-46..58 in
  `docs/feature/openlore-github-scraper/feature-delta.md`; DESIGN WD-59..68 in
  `docs/feature/openlore-github-scraper/design/wave-decisions.md`; DELIVER
  DV-1..5 in `docs/feature/openlore-github-scraper/deliver/wave-decisions.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/openlore-github-scraper/deliver/execution-log.json`,
  `docs/feature/openlore-github-scraper/deliver/roadmap.json`
- **Outcome KPIs (slice-02 rationale)**:
  `docs/feature/openlore-github-scraper/discuss/outcome-kpis.md`
- **Job (JTBD) J-004 + signal->predicate mapping SSOT**: `docs/product/jobs.yaml`
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Slice-01 evolution** (precedent): `docs/evolution/openlore-foundation-evolution.md`
- **Slice-03 evolution** (precedent): `docs/evolution/openlore-federated-read-evolution.md`
- **CI / nightly mutation**: `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`
- **Supply-chain policy**: `deny.toml`
