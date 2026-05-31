# Evolution: htmx-scraper-viewer (slice-06 read-only localhost htmx store viewer)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/htmx-scraper-viewer/`
> (discuss/ design/ distill/ deliver/) and ADR-028..ADR-030 under `docs/adrs/`;
> this file is the post-mortem summary.

## Summary

`htmx-scraper-viewer` is the slice-06 read-only browser viewer of the OpenLore
umbrella (job **J-001**: make the node operator's node LEGIBLE — let them *see
what their node holds* and what they could add, in a browser, read-only, without
SQL). It turns an opaque local DuckDB store into a glanceable one while **provably
never writing, signing, or exposing the signing key**. The thesis is **legibility
without authority**: the viewer is a window onto the store, never a hand on it —
the CLI remains the sole authoring + signing path.

It ships a single new read-only verb: `openlore ui [--port <P>]` (default 8788),
binding **127.0.0.1 only, no auth**, serving server-rendered HTML (htmx-ready,
progressive enhancement) over the operator's local store. Six routes prove the
thesis:

1. **Read-only landing** — `GET /` (no write/sign affordance anywhere).
2. **My Claims** — `GET /claims` (paginated, page size 50): the operator's
   persisted slice-01 `claims`, glanceable, zero SQL typed.
3. **Claim detail** — `GET /claims/{cid}` (detail + evidence): the full
   "what did I sign" view including the verbatim confidence (FR-VIEW-8).
4. **Peer Claims** — `GET /peer-claims` (federated, origin = `author_did` +
   `fetched_from_pds`): slice-03 `peer_claims`, each row showing origin and
   separable from the operator's own claims (KPI-VIEW-3).
5. **Live Scrape** — `GET/POST /scrape`: a live, EPHEMERAL GitHub propose view
   that reuses the slice-02 `GithubPort` + `derive_candidates`, rendering
   candidates for browser triage; signing stays in the CLI (no sign control in
   the browser, KPI-VIEW-4).

The CLI + signed claims REMAIN the source of truth. The viewer reads the SAME
shared `Arc<Mutex<Connection>>` the CLI writes — **zero new persisted types, zero
new table, zero new CID path** (ADR-030 / data-models.md). The web surface is
additive and non-load-bearing for authoring (KPI-VIEW-5: store views render with
zero network calls).

### Wave timeline

| Wave    | Date       | Owner                              |
|---------|------------|------------------------------------|
| DISCUSS | 2026-05-30 | Luna (nw-product-owner)            |
| DESIGN  | 2026-05-30 | Morgan (nw-solution-architect)     |
| DEVOPS  | 2026-05-30 | Apex (nw-platform-architect)       |
| DISTILL | 2026-05-30 | Quinn (nw-acceptance-designer)     |
| DELIVER | 2026-05-30..31 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **20/20 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **26/26 slice-06 acceptance scenarios** GREEN: 15 `viewer_store` (V-1..V-13;
  the binary reports 15 incl. support self-tests) + 5 `viewer_scrape`
  (V-S1/S3/S4 + support) + 6 `viewer_invariants` (V-INV-1..4 gold + support) —
  reported by the orchestrator as viewer_store 15/15, viewer_scrape 5/5,
  viewer_invariants 6/6. Plus **40 `viewer-domain` unit/property tests** (pure
  render + view-model + pagination arithmetic). The `ViewerServer` test harness
  spawns the REAL `openlore ui` over HTTP.
- **TWO new crates**: 1 pure (`viewer-domain`) + 1 effect (`adapter-http-viewer`);
  extends `ports` + `adapter-duckdb` + `cli` + `xtask` in place. Workspace member
  count **19 → 21** (19 production + 1 test-support + 1 xtask); `cargo xtask
  check-arch` reports "21 workspace members".
- **ONE new dependency** (`maud`, pure, in `viewer-domain`); the listener is a
  hand-rolled `hyper` 1.x handler (`axum`/`actix` are `deny.toml`-banned).
- **Zero regression** on slice-01/02/03/04/05 suites (full acceptance suite GREEN
  across all slices).
- **100% mutation kill rate** on the new pure `viewer-domain` production functions
  (62/62 viable caught, 5 unviable, 0 missed) — exceeds the ≥80% per-feature gate.
- **3 ADRs** (ADR-028..ADR-030) all Accepted/shipped.
- DES integrity: `des-verify-integrity` reports "All 20 steps have complete DES
  traces" (exit 0).
- Adversarial review: **APPROVED**, zero blockers, zero Testing Theater.
- `cargo xtask check-arch`: OK (21 workspace members). L1-L4 refactor done
  (commit `a5ddb22`).

## Wave-by-wave changelog

### DISCUSS (2026-05-30)

Defined the J-001 legibility objective: make the node operator's node legible —
let them *see what their node holds* (and what they could add) in a browser,
read-only, without SQL — turning an opaque store into a glanceable one, while
provably never writing, signing, or exposing the key. Authored five outcome KPIs
(KPI-VIEW-1..5) with **KPI-VIEW-1** (north star: view persisted claims in a
browser in < 10 s from cold viewer start, with ZERO SQL typed) as the heart of the
feature, and two cardinal guardrails: **KPI-VIEW-2** (read-only — zero write/sign
code paths reachable from any route; zero key reads in the process; a single
reachable write/sign path is a release blocker) and **KPI-VIEW-5** (local-first —
store views render with zero network calls, inheriting slice-01 KPI-5). Two
leading indicators: KPI-VIEW-3 (federated peer rows show origin + are separable
from own claims) and KPI-VIEW-4 (scrape proposals triaged in a scannable browser
list, sign-in-CLI). Inherited the slice-02 human-gate I-SCR-1 (only the human
signs) as I-VIEW-1/2/3.

### DESIGN (2026-05-30)

Morgan locked the slice-06 invariants I-VIEW-1..6 and authored three ADRs. The
headline decision: slice-06 is a **two-crate additive extension** — a NEW pure
`viewer-domain` (maud render + view-model ADTs + pure pagination arithmetic, deps
maud + ports only) + a NEW effect `adapter-http-viewer` (hand-rolled hyper 1.x).
The three ADRs: **ADR-028** (viewer architecture — the `openlore ui` verb, the
pure/effect split, read-only, loopback + no-auth; the verb is routed BEFORE
`Wiring::production` as a read-only composition root); **ADR-029** (maud as the
templating engine — a pure `view-model → HTML string` transformation with NO
runtime I/O, joining the `xtask check-arch` pure-core allowlist alongside the
slice-02 `serde_yaml_ng` precedent); **ADR-030** (the read-only DuckDB store-read
port — `StoreReadPort` with NO mutation method, the `ClaimRow` / `ClaimDetail` /
`PeerClaimRow` / `PageRequest` / `Page` / `StoreReadError` ADTs, the column →
displayed-field mapping, and offset/limit pagination at size 50 over the SAME
shared `Arc<Mutex<Connection>>`). The read-only guarantee is enforced at THREE
structural layers: the type system (`StoreReadPort` exposes no write; no signing
key in the web process), the `xtask` viewer capability rule, and the behavioral
gold tests (V-INV-1..4). DEVOPS (parallel) added `viewer-domain` to the mutation
sweep, designed the KPI-VIEW-1 cold-start-to-first-paint timing, the KPI-VIEW-2
route-inventory + key-access audit (blocking), and the KPI-VIEW-5 offline-store
baseline.

### DISTILL (2026-05-30)

Quinn authored the 26-scenario executable acceptance corpus across three targets:
`viewer_store` (V-1..V-13 — the read-only landing, paginated My Claims at size 50,
claim detail + evidence + verbatim confidence, the federated Peer Claims origin
view, pagination clamp behavior), `viewer_scrape` (V-S1/S3/S4 — the live ephemeral
GitHub propose view reusing the slice-02 `GithubPort`, candidate rendering, the
`NetworkDown` render, no-sign-control-in-browser), and `viewer_invariants`
(V-INV-1..4 — the GOLD read-only behavioral guarantees: no write/sign route, no
signing key in the process, derived-from honesty, loopback-only bind). Built the
`ViewerServer` test harness that spawns the REAL `openlore ui` over HTTP and
probes the live routes.

### DELIVER (2026-05-30..31)

Executed 20 roadmap steps via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Walking skeleton `47984fa` → final step
`1c4e4ff`; L1-L4 refactor `a5ddb22`. Key per-step SHAs are in
`deliver/execution-log.json`.

- **Bootstrap**: extended `ports` with `StoreReadPort` (no mutation method) +
  the `ClaimRow` / `ClaimDetail` / `PeerClaimRow` / `PageRequest` / `Page` /
  `StoreReadError` ADTs; created the PURE `viewer-domain` crate (maud render +
  view-model ADTs + pure pagination arithmetic) + the EFFECT `adapter-http-viewer`
  crate (hand-rolled hyper 1.x); added the read-only `StoreReadPort` impl to
  `adapter-duckdb` over the SAME shared `Arc<Mutex<Connection>>`; wired the
  `openlore ui` verb as a read-only composition root routed BEFORE
  `Wiring::production`; extended `xtask` (the maud pure-core allowlist + the viewer
  capability rule + the pure-core arm); materialized the `ViewerServer` harness +
  registered the 3 test targets. Fail-for-right-reason RED gate.
- **viewer-domain pure core**: the view-model render functions + the pure
  pagination arithmetic (offset/limit, clamp), pinned by the 40 in-crate
  unit/property tests.
- **Store-view walking skeleton + routes**: V-1..V-13 — the read-only landing,
  the paginated My Claims (size 50), claim detail + evidence + verbatim confidence,
  the federated Peer Claims origin view (`author_did` + `fetched_from_pds`), and
  the pagination clamp.
- **Live scrape + gold invariants**: V-S1/S3/S4 (the live ephemeral GitHub propose
  view reusing the slice-02 `GithubPort` + `derive_candidates`; the `NetworkDown`
  render; no sign control in the browser) + V-INV-1..4 (the gold read-only /
  no-key / derived-from-honesty / loopback-only behavioral guarantees).

Refactor / review / mutation / integrity outcomes are in the Quality Gates +
Mutation sections below.

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02/03/04/05 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-2 | Mutation = per-feature 100% on the new PURE `viewer-domain` production functions, matching slice-02/03/04/05 DV-2. The killing properties are kept IN-CRATE (the 40 `viewer-domain` unit/property tests) per the slice-04/05 cross-package lesson. | Per-feature gate at deliver-time + DEVOPS sweep backstop; the per-feature measurement reaches the real killing suite locally (no cross-package cargo-mutants scope detour). |
| DV-3 | **`hyper` 1.x, NOT `axum`/`actix`, for the viewer listener** (`adapter-http-viewer`). | `axum`/`actix` are `deny.toml`-banned (transitive supply-chain narrowing); a hand-rolled `hyper` handler serves the six read-only routes with no new banned dependency — mirroring the slice-05 DV-3 `hyper`-over-`axum` resolution for the XRPC server. |
| DV-4 | The `/scrape` `NetworkDown` render resolved as a **unit ADT variant that discards the raw error** — structurally cannot leak transport internals. | Verified at review: a unit variant carries no payload, so no `reqwest`/transport detail can reach the rendered page. Read-only + no-leak by type, not by string-scrubbing. |
| DV-5 | Fixed a REAL pagination **clamp gap**: `?page` beyond the last page previously overshot to an empty page; now clamps to the LAST page. | A direct over-the-end `?page` URL would have shown a blank list instead of the last claims — a legibility bug for the north-star (KPI-VIEW-1) view. Fixed in the pure pagination arithmetic + pinned by an in-crate property. |
| DV-6 | Closed an **xtask capability-rule coverage gap**: the `adapter-atproto-pds` exclusion was not independently unit-pinned. | The viewer capability rule's exclusion set was correct but under-tested; an independent unit pin prevents a future edit from silently widening the web-process capability surface. |
| DV-7 | Added a roadmap-format `criteria` field per step to satisfy the DES integrity validator. | The integrity validator (`des-verify-integrity`) requires a per-step `criteria` field; adding it brought the slice-06 roadmap into spec without altering the executed step semantics. |

## Cardinal release gates + slice-06 invariants

The cardinal release gate is **read-only** (KPI-VIEW-2 / I-VIEW-1/2/3): no route
writes or signs, and the web process holds no signing key. It is enforced at THREE
structural layers and is unshippable on any violation:

1. **Type system** — `StoreReadPort` exposes NO mutation method; the viewer
   composition root holds no `SigningPort` / keychain handle (no key in the web
   process).
2. **`xtask` capability rule** — the viewer crates may not link signing / mutation
   capabilities; the rule's exclusion set is independently unit-pinned (DV-6).
3. **Behavioral gold tests** — V-INV-1..4 drive the REAL `openlore ui` over HTTP
   and assert no write/sign route, no key in the process, derived-from honesty, and
   loopback-only bind.

The full slice-06 invariant set (I-VIEW-1..6; detail in
`docs/feature/htmx-scraper-viewer/design/component-boundaries.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-VIEW-1 | Read-only (no route writes or signs; signing stays exclusively in the CLI; inherits I-SCR-1). | TYPE (`StoreReadPort` no-mutation) / STRUCTURAL (xtask capability rule) / BEHAVIORAL (V-INV-1). Cardinal (KPI-VIEW-2). |
| I-VIEW-2 | No signing key in the web process (the viewer never loads or holds the signing key). | TYPE (no `SigningPort` in the viewer root) / BEHAVIORAL (V-INV-2). Cardinal (KPI-VIEW-2). |
| I-VIEW-3 | Human gate preserved (only the human, in the CLI, signs; the `/scrape` view has no sign control). | Inherits I-SCR-1; BEHAVIORAL (V-S*, V-INV). |
| I-VIEW-4 | Derived-from honesty (only `CandidateRowView` carries `derived_from`; persisted view-models have NO such slot, WD-62). | TYPE (the persisted view-models structurally lack a `derived_from` field) / BEHAVIORAL (V-INV-3). |
| I-VIEW-5 | Same-store (zero new persisted types, zero new table, zero new CID path; reads the SAME shared `Arc<Mutex<Connection>>`). | STRUCTURAL (data-models.md / ADR-030; no schema change). |
| I-VIEW-6 | Offline / local-first store views (My Claims, Peer Claims, detail render with zero network calls; KPI-VIEW-5) + loopback-only bind. | BEHAVIORAL (offline harness + V-INV-4 loopback-only). Guardrail (KPI-VIEW-5). |

Confidence is shown **verbatim** (FR-VIEW-8) — no bucket-midpoint rounding,
inheriting the KPI-4 zero-silent-normalization discipline into the rendered view.

## Quality gates — final report

- **Acceptance / integration**: 26/26 slice-06 scenarios GREEN (viewer_store
  15/15, viewer_scrape 5/5, viewer_invariants 6/6); slice-01/02/03/04/05 suites
  zero regression. Full workspace acceptance suite GREEN across all slices. Plus
  40 `viewer-domain` unit/property tests. The `ViewerServer` harness spawns the
  REAL `openlore ui` over HTTP.
- **`cargo xtask check-arch`**: OK (21 workspace members) — `viewer-domain`
  pure-core allowlist (with the `maud` whitelist) + the viewer capability rule
  (the web process may not link signing / mutation; exclusion set independently
  unit-pinned, DV-6) + the pure-core arm active.
- **Refactor (L1-L4)**: commit `a5ddb22` — clippy + check-arch + check-probes
  clean; `viewer-domain` purity intact (no I/O imports; maud + ports only; ADTs
  make illegal states unrepresentable — `StoreReadError` choice type, no `Option`
  smuggling of write capability).
- **Adversarial review**: APPROVED, zero blockers, zero Testing Theater. The
  cardinal read-only gate verified load-bearing across all three layers; the
  `/scrape` `NetworkDown` no-leak confirmed structural (a unit variant, DV-4); the
  pagination clamp fix (DV-5) confirmed a real bug-fix, not theatre.
- **DES integrity**: PASS — "All 20 steps have complete DES traces" (exit 0).

## Mutation testing — final report

**Scope**: the new pure `viewer-domain` production functions (the view-model
render functions + the pure pagination arithmetic). The slice-04/05 cross-package
lesson was applied from the start — the 40 `viewer-domain` properties pin the
production functions IN/against the `viewer-domain` crate, so the per-feature
mutation measurement reaches the real killing suite without a cross-package detour.

| Mutant category | Viable | Caught | Missed | Unviable | Kill rate |
|---|---:|---:|---:|---:|---|
| `viewer-domain` production logic (render + pagination arithmetic, incl. the DV-5 pagination-clamp boundary) | 62 | 62 | 0 | 5 | **100%** (62/62 viable) |

Slice-06 per-feature gate SATISFIED (≥80%; actual 100% on the production scope,
0 missed). The DV-5 pagination-clamp fix added the boundary property that closed
the clamp survivor. `adapter-http-viewer` is NOT mutated by design (effect shell;
covered by the V-INV gold tests through the real binary). DEVOPS sweep is the
ongoing backstop.

## Lessons learned / issues

- **A unit ADT variant is a no-leak proof (DV-4)**: the `/scrape` `NetworkDown`
  render is a unit variant that carries no payload, so no transport/`reqwest`
  internal can reach the rendered page — read-only + no-leak BY TYPE, not by
  fragile string-scrubbing. Institutional lesson: when an output must not leak an
  error's internals, model the rendered error as a payload-free choice variant and
  the leak becomes structurally impossible.
- **A new read path is a free regression test for old write paths (DV-5)**: the
  pagination clamp gap (over-the-end `?page` showing a blank list) surfaced only
  when a browser URL could request an arbitrary page directly — the CLI never
  exercised that input shape. The viewer's new read surface earned its keep by
  surfacing it; the fix landed in the pure arithmetic + an in-crate property.
- **Capability-rule exclusions need independent pins (DV-6)**: the viewer
  capability rule's `adapter-atproto-pds` exclusion was correct but not
  independently unit-pinned; a future edit could have silently widened the
  web-process capability surface. The independent pin closes that gap. Lesson: an
  allowlist/exclusion in an architecture rule is only as safe as its own dedicated
  test.
- **Supply-chain policy picks the listener again (DV-3)**: `axum`/`actix` are
  `deny.toml`-banned, so the viewer listener is a hand-rolled `hyper` 1.x handler —
  the same resolution slice-05 reached for its XRPC server. For a six-route
  read-only surface this is a net simplification, not a cost.
- **Roadmap schema drift (DV-7)**: the DES integrity validator required a per-step
  `criteria` field the slice-06 roadmap initially lacked; adding it brought the
  roadmap into spec without changing executed step semantics. Future roadmaps
  should carry `criteria` from authoring.

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | OD-VIEW-1 left the HTML-rendering approach open. | `maud` chosen (pure `view-model → HTML`, no runtime I/O; ADR-029); joins the pure-core allowlist. | Resolved at DESIGN; the only new dependency. |
| 2 | The HTTP listener framework. | Hand-rolled `hyper` 1.x handler (`axum`/`actix` `deny.toml`-banned). | Decided at deliver-time; recorded as DV-3 (mirrors slice-05 DV-3). |
| 3 | The `/scrape` `NetworkDown` error render was unspecified at the type level. | Resolved as a payload-free unit ADT variant (structurally cannot leak transport internals). | Recorded as DV-4; strengthens I-VIEW (no-leak). |
| 4 | DESIGN assumed the pagination arithmetic clamped over-the-end pages. | Found + fixed a real clamp gap (over-the-end `?page` now clamps to the last page, not an empty overshoot). | Resolved within DELIVER; recorded as DV-5; pinned by an in-crate property + 100% mutation on the boundary. |
| 5 | The viewer capability xtask rule's exclusion set was assumed adequately covered. | The `adapter-atproto-pds` exclusion was not independently unit-pinned; pin added. | Resolved within DELIVER; recorded as DV-6. |
| 6 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-2, 100% on production functions, 0 missed). | Recorded. |

## KPI status at GA (slice-06)

| KPI | Type | Status at GA | Note |
|---|---|---|---|
| KPI-VIEW-1 (north star: <10 s, zero SQL) | leading | per-feature GREEN; cohort/timing instrumentation YELLOW | the store-view path is GREEN (V-1/V-2 render My Claims with zero SQL typed); the cold-start-to-first-paint timing histogram is pending the DEVOPS timing hook. |
| KPI-VIEW-2 (read-only / no key) | guardrail | MET (release-blocking) | three-layer enforced (type / xtask rule / V-INV-1/2 gold); zero write/sign route, zero key in the web process. |
| KPI-VIEW-3 (peer origin distinguishable) | leading | MET | V-* Peer Claims rows show `author_did` + `fetched_from_pds`, separable from own claims. |
| KPI-VIEW-4 (browser proposal triage) | leading (secondary) | per-feature GREEN | the `/scrape` view renders candidates with NO sign control (sign-in-CLI directive present); cohort behavior pending dogfood. |
| KPI-VIEW-5 (offline store views) | guardrail | MET | store views render with zero network calls (I-VIEW-6 offline harness). |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/htmx-scraper-viewer/` (discuss/ design/ distill/ deliver/)
- **Slice-06 ADRs**:
  `docs/adrs/ADR-028-htmx-viewer-architecture.md`,
  `docs/adrs/ADR-029-maud-templating-pure-core.md`,
  `docs/adrs/ADR-030-read-only-store-read-port.md`
- **Architecture design / component boundaries / data models / tech stack**
  (kept in the feature workspace): `docs/feature/htmx-scraper-viewer/design/`
- **DELIVER wave decisions**:
  `docs/feature/htmx-scraper-viewer/deliver/wave-decisions.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/htmx-scraper-viewer/deliver/execution-log.json`,
  `docs/feature/htmx-scraper-viewer/deliver/roadmap.json`
- **Outcome KPIs (slice-06 rationale)**:
  `docs/feature/htmx-scraper-viewer/discuss/outcome-kpis.md`
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
</content>
</invoke>
