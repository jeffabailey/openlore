# Evolution: viewer-counter-flags-score-surface (slice-14 the neutral "Countered" presence flag on the read-only contributor-scoring surface `GET /score?contributor=<did>`)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-counter-flags-score-surface/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL sections,
> plus `discuss/`, `design/`, `deliver/`) and ADR-051 under `docs/adrs/`; this file
> is the post-mortem summary. This slice is a **DELTA on shipped work**: slice-06
> (`htmx-scraper-viewer` — the read-only viewer), slice-07 (`viewer-htmx-swaps` — the htmx
> progressive-enhancement layer), slice-09 (`viewer-contributor-scoring` — the
> `GET /score?contributor=<did>` contributor-scoring surface this slice flags, and the
> byte-identity baseline), slice-11 (`viewer-counter-claim-threads` — the
> `GET /claims/{cid}` thread this slice links to), slice-12
> (`viewer-counter-claim-list-flags` — the `counter_presence_for` batch read + the `/claims`
> list flag this slice REUSES), and slice-13 (`viewer-counter-flags-graph-surfaces` — the
> graph-surface flags + the `survey_counter_presence` flatten pattern + the
> `render_countered_link` render this slice mirrors and REUSES). Read those parent archives
> (`htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
> `viewer-contributor-scoring-evolution.md`, `viewer-counter-claim-threads-evolution.md`,
> `viewer-counter-claim-list-flags-evolution.md`,
> `viewer-counter-flags-graph-surfaces-evolution.md`) for the surfaces, the batch read, and
> the render. slice-12 shipped the at-a-glance flag on `/claims`; slice-13 completed it across
> the graph surfaces (`/peer-claims`, `/project`, `/philosophy`); **slice-14 completes J-003b
> across the LAST remaining viewer surface** — `/score`. There is no deferred remainder: with
> `/score` flagged, J-003b is at-a-glance on EVERY local viewer surface.

## Summary

`viewer-counter-flags-score-surface` extends the neutral **"Countered"** presence flag onto the
read-only **contributor-scoring surface** — `GET /score?contributor=<did>`. Each per-claim
contribution row (each `scoring::WeightedPairing`'s `Contribution`) whose cid has **≥1 counter**
now renders a render-only `<a href="/claims/{cid}">Countered</a>` one-hop link to the slice-11
counter-thread, **beside its verbatim subtotal**; rows whose cid has **no** counter render **no
flag** (no-noise). This **completes the at-a-glance J-003b across the LAST remaining viewer
surface**: slice-12 shipped `/claims`, slice-13 shipped the graph surfaces (`/peer-claims`,
`/project`, `/philosophy`), and **slice-14 ships `/score`** — the disagreement scanner now sees,
at a glance, *which scored contributions have been countered* on the scoring breakdown itself,
and can click straight through to read the disagreement. With `/score` flagged there is **no
deferred surface remaining**.

The load-bearing thesis is what makes slice-14 **distinct from every prior flag slice**:
`/score` carries **SCORING SEMANTICS**, so the flag must be provably **ORTHOGONAL to the score** —
proven by **TWO cardinals**, not one:

1. **Sum-to-weight preserved (CARDINAL):** the displayed per-contribution subtotals still **sum
   to the displayed pairing weight** on a **FLAGGED** breakdown — the countered contribution keeps
   its **FULL original subtotal**. The counter is **SHOWN, never APPLIED**: a flagged contribution
   is not down-weighted, zeroed, filtered, or re-ranked; its subtotal is byte-identical to the
   un-flagged value and still participates in the pairing weight exactly as slice-09 computed it.
2. **Byte-identity (CARDINAL):** with the flag markers **AND** the anti-misread legend elided,
   `/score` is **byte-identical to slice-09** — every weight, confidence, bonus, subtotal, total,
   bucket, ranking, and row order unchanged.

Plus the anti-misread **LEGEND** (`SCORE_COUNTER_LEGEND`, blocklist-clean) rendered **once** on
the Scored arm — a neutral note that the "Countered" flag is a presence indicator that does NOT
affect the score, defending the scoring surface against the misread that a flag re-weights a
contribution.

The slice ships **ZERO new crates**, **ZERO new route**, **ZERO new read method**, **ZERO new
render fn**, **ZERO new SQL**, and **ZERO new xtask edge** (workspace stays at **21 members**). It
**REUSES the slice-12 `counter_presence_for(&[String]) -> HashSet<String>` batch read VERBATIM**
(ADR-049 carry-forward — no new read, no new SQL), **REUSES the slice-13 `render_countered_link`
render VERBATIM**, and **REUSES the slice-11 `COUNTERED_PRESENCE_FLAG` constant VERBATIM**. The
one genuinely-new artifact is the `SCORE_COUNTER_LEGEND` constant. Per **ADR-051**, presence is
threaded as a **`&HashSet<String>` DOWN the render chain** rather than added as an `is_countered`
field (slice-13's approach) — because `/score` rows project `scoring::Contribution` /
`scoring::WeightedPairing`, **foreign-immutable types owned by the pure scoring crate the viewer
must NOT mutate**. This makes the orthogonality **structural**: presence gates only an additive
marker and **never reaches a number**. The render is a **total fn of `(ScoreState, presence)`**.
`score_counter_presence` **flattens EVERY contribution cid across EVERY pairing into ONE
`counter_presence_for` call** (ADR-050 carry-forward N+1 guard, mirroring slice-13's
`survey_counter_presence`). The reads are **LOCAL and offline** (DB-only).

### What shipped (one paragraph)

The read-only contributor-scoring surface `GET /score?contributor=<did>` now renders a neutral
**"Countered"** presence flag wherever a per-claim contribution's cid has **≥1 counter**: each
`scoring::WeightedPairing`'s `Contribution` row whose cid is countered renders a render-only
`<a href="/claims/{cid}">Countered</a>` one-hop link to the slice-11 counter-thread, **beside its
verbatim subtotal**; un-countered contributions render **no flag** (no-noise). The handler
**REUSES the slice-12 `counter_presence_for(&[String]) -> HashSet<String>` batch read VERBATIM**
(ADR-049 carry-forward — no new read method, no new SQL, no adapter change) and **REUSES the
slice-13 `render_countered_link` + the slice-11 `COUNTERED_PRESENCE_FLAG` VERBATIM**. The
distinct work is **orthogonality on a scoring surface**: per **ADR-051**, presence is passed as a
**`&HashSet<String>` threaded DOWN the render chain** (not an `is_countered` field) because the
`/score` rows project `scoring::Contribution` / `scoring::WeightedPairing` — **foreign-immutable
types owned by the pure scoring crate the viewer must NOT mutate** — so presence gates only an
**additive marker** and **never reaches a number**; the render becomes a **total fn of
`(ScoreState, presence)`**. `score_counter_presence` **flattens EVERY contribution cid across
EVERY pairing into ONE `counter_presence_for` call** (ADR-050 carry-forward N+1 guard, mirroring
slice-13's `survey_counter_presence`). The slice ships the one genuinely-new artifact — the
`SCORE_COUNTER_LEGEND` anti-misread constant (rendered once on the Scored arm) — and proves the
flag ORTHOGONAL to the score via TWO cardinals: **sum-to-weight preserved** (the displayed
per-contribution subtotals still sum to the displayed pairing weight on a flagged breakdown; the
countered contribution keeps its FULL original subtotal — shown, never applied) and
**byte-identity** (with the markers AND the legend elided, `/score` is byte-identical to slice-09
— every weight, confidence, bonus, subtotal, total, bucket, ranking, row order). The reads are
**LOCAL and read-only** (DB-only — no artifact read, no network); nothing is persisted. **This
COMPLETES J-003b across the LAST remaining viewer surface** — there is no deferred remainder.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-06-08 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-06-08 | Morgan (nw-solution-architect)                           |
| DISTILL | 2026-06-08 | Quinn (nw-acceptance-designer)                           |
| DELIVER | 2026-06-08 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **11/11 roadmap steps** done (all COMMIT/PASS in `deliver/execution-log.json`).
- **Acceptance scenarios GREEN** across two `[[test]]` targets: **`viewer_counter_flags_score_surface`**
  (the SF-ids — SF-1 WS through SF-7 parity — driven through the `/score?contributor=<did>`
  driving port, incl. the SF-1 walking skeleton: a flagged `/score` contribution row linking to
  the slice-11 thread) + **GOLD invariants** (`viewer_counter_flags_score_surface_invariants` —
  read-only / no-write, offline, the **CARDINAL byte-identity** no-regression vs slice-09, the
  **CARDINAL sum-to-weight** orthogonality, and the N+1) + the `viewer-domain` unit/property tests
  (the presence-threaded render arm + the `SCORE_COUNTER_LEGEND`). The `ViewerServer` harness drives
  the REAL `openlore ui` over HTTP; the store is seeded through the REAL ingest path.
- **Slices 06/07/09/11/12/13 corpora GREEN — zero regression** (the full workspace acceptance
  suite green across all slices; the byte-identity gold proves `/score` unchanged vs slice-09 with
  the markers and legend elided).
- **NO new crate, NO new route, NO new read method, NO new render fn, NO new SQL, NO new xtask
  edge**: extends `viewer-domain` (PURE, the presence-threaded render + the new
  `SCORE_COUNTER_LEGEND` constant) + `adapter-http-viewer` (EFFECT, the `/score` handler) on the
  existing `/score?contributor=<did>` route; REUSES the slice-12 `counter_presence_for` read +
  the slice-13 `render_countered_link` + the slice-11 `COUNTERED_PRESENCE_FLAG` VERBATIM (no
  `adapter-duckdb` change, no `scoring` change). Workspace member count stays **21** (19
  production + 1 test-support + 1 xtask); `cargo xtask check-arch` reports "21 workspace members".
- **NO new production dependency**: `maud`/`hyper`/`duckdb`/`scoring` unchanged; no `deny.toml`
  change; **NO new xtask edge** (the dependency graph was already in place).
- **100% mutation kill rate** on the new + extended pure `viewer-domain` production functions (the
  presence-threaded render arm + the `SCORE_COUNTER_LEGEND` rendering) — **6/6 in-diff viable
  caught, 0 missed** — exceeds the ≥80% per-feature gate.
- **1 ADR** (ADR-051) Accepted/shipped.
- DES integrity: 11/11 steps have complete DES traces.
- Adversarial review: **APPROVED** — zero defects, zero Testing Theater.
- `cargo xtask check-arch`: OK (21 workspace members, no new allowlist edge).
- DISCUSS DoR 9/9; DESIGN gate APPROVED; DISTILL gate APPROVED (9.9/10).

## Wave-by-wave changelog

### DISCUSS (2026-06-08)

Luna framed the slice as a **brownfield DELTA on slices 06/07/09/11/12/13** that **completes the
at-a-glance J-003b across the LAST remaining viewer surface** — slice-12 shipped the `/claims`
list flag, slice-13 the graph surfaces; slice-14 extends the SAME neutral presence flag onto the
contributor-scoring surface `/score?contributor=<did>`. Persona is **P-001 (Maria, the node
operator)** wearing the disagreement-scanner hat on the scoring breakdown. The load-bearing
DISCUSS decision: **on a scoring surface the flag must be ORTHOGONAL — presence-only, additive,
shown but NEVER applied to any number**. Unlike the list/graph surfaces, `/score` carries scoring
semantics, so the orthogonality is a first-class acceptance concern, captured as TWO cardinals
(sum-to-weight preserved on a flagged breakdown + byte-identity vs slice-09 with markers and the
legend elided) PLUS an anti-misread **legend** so a reader cannot mistake the flag for a
re-weighting. slice-14 **REALIZES the existing viewer KPI contracts on the scoring surface**
(read-only / offline guardrails, presence-only attribution, no-regression rendering) plus the new
orthogonality guardrails — rather than minting new KPI IDs. The walking skeleton is the SF-1
flagged `/score` contribution row linking to the slice-11 thread, validating that the
reused-read + reused-render transfer to the scoring surface before tackling the orthogonality
cardinals. DoR: 9/9.

### DESIGN (2026-06-08)

Morgan locked slice-14 as an **additive, orthogonal presence flag on one existing route, not a
re-architecture** — ZERO new crates, ZERO new route, ZERO new read method, ZERO new render fn,
ZERO new SQL, ZERO new persisted type, ZERO new xtask edge. The riskiest design question was
**how to thread presence into a render whose rows project FOREIGN-IMMUTABLE scoring types** —
resolved in one ADR:

- **ADR-051** (thread presence as `&HashSet<String>` DOWN the render chain, do NOT add an
  `is_countered` field; render = total fn of `(ScoreState, presence)`; the anti-misread legend):
  the `/score` rows project `scoring::Contribution` / `scoring::WeightedPairing` — types **owned
  by the pure `scoring` crate** that the viewer must NOT mutate (no field injection like slice-13's
  `EdgeRow.is_countered`). So presence is passed as a **`&HashSet<String>` threaded DOWN the render
  chain**: it gates only an **additive marker** beside the verbatim subtotal and **never reaches a
  number**, making the orthogonality **structural**. The render becomes a **total fn of
  `(ScoreState, presence)`**. `score_counter_presence` **flattens EVERY contribution cid across
  EVERY pairing into ONE `counter_presence_for` call** (ADR-050 carry-forward N+1 guard, mirroring
  slice-13's `survey_counter_presence`). The handler REUSES the slice-12 `counter_presence_for`
  read + the slice-13 `render_countered_link` + the slice-11 `COUNTERED_PRESENCE_FLAG` VERBATIM;
  the one new artifact is the `SCORE_COUNTER_LEGEND` anti-misread constant rendered once on the
  Scored arm.

The read-only contract stays enforced at THREE layers (a `StoreReadPort` with no mutation method
[TYPE], the `xtask check-arch` viewer capability rule [STRUCTURAL], and behavioral GOLD invariants
[BEHAVIORAL]). The orthogonality contract is enforced **structurally** (presence is a
`&HashSet<String>` that gates an additive marker and never touches a `scoring` number — ADR-051)
plus **behaviorally** (the sum-to-weight and byte-identity golds). The C4 views, the
flatten-into-one-presence-call data-flow, and the I-SF-1..7 structural-guarantee table are in the
DESIGN sections of `feature-delta.md` and `design/`. DESIGN gate: APPROVED.

### DISTILL (2026-06-08)

Quinn authored the executable acceptance corpus across two `[[test]]` targets plus the inherited
adapter tests:

- **`viewer_counter_flags_score_surface.rs`** (Tier A — `SF-` ids SF-1..SF-7, 1 WS, driven through
  the `/score?contributor=<did>` port): the SF-1 walking skeleton (a flagged `/score` contribution
  row linking to the slice-11 thread), SF-2 the presence-only flag (an N-author-countered cid → ONE
  neutral marker; un-countered → no flag, no-noise), SF-N1 the N+1 proxy (a multi-pairing scored
  breakdown with a SPREAD countered subset so a per-pairing / per-contribution presence read is
  caught), SF-3 the sum-to-weight orthogonality on a FLAGGED breakdown, SF-4 byte-identity vs
  slice-09, SF-5 the anti-misread legend (rendered once on the Scored arm, blocklist-clean), SF-6
  no-noise, SF-7 the no-JS full page + fragment/page parity. The countered "identical-subtotal twin"
  seed used an **evidence-perturbed quadruple builder** so the twins differ **only in CID, not in
  scorer inputs** — two contributions with identical subtotals where one is countered and one is not,
  proving the flag is gated on cid presence and not on any score value.
- **`viewer_counter_flags_score_surface_invariants.rs`** (gold guardrails): read-only, no-write,
  offline-chrome, offline-data, N+1, and the **TWO CARDINALS** — **sum-to-weight preserved** (the
  displayed per-contribution subtotals still sum to the displayed pairing weight on a flagged
  breakdown; the countered contribution keeps its FULL original subtotal) and **byte-identity** (with
  the markers AND the legend elided, `/score` is byte-identical to slice-09 — every weight,
  confidence, bonus, subtotal, total, bucket, ranking, row order).
- **Inherited `adapter-duckdb` presence + N+1 bound (no adapter change)**: the slice-12
  `counter_presence_for` 1 / N / 5N + empty bound is inherited verbatim (this slice adds no adapter
  read), plus the `viewer-domain` unit/property tests (the presence-threaded render arm + the
  `SCORE_COUNTER_LEGEND`).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); the store is
seeded through the REAL ingest path; multi-author / distinct-peer seeds use a single combined peer
pull (the slice-11/12/13 carry-forward). RED classification: both acceptance targets COMPILE green,
scenarios FAIL via `todo!()` = MISSING_FUNCTIONALITY (correct RED, not BROKEN). DISTILL gate:
APPROVED (9.9/10).

### DELIVER (2026-06-08)

Executed **11 roadmap steps across 3 phases** via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`. The
thick walking skeleton forced all seams into existence first; the rest were confirmatory + genuine
seeds:

- **Phase 01 — walking skeleton + the seams (SF-1 WS, SF-2 presence-only, SF-N1 N+1)**: the SF-1
  thick walking skeleton (01-01) ships the **presence threaded as `&HashSet<String>` down the
  render chain** (ADR-051) + the REUSED `counter_presence_for` wiring + the
  `score_counter_presence` flatten (every contribution cid across every pairing → ONE presence
  call) + the presence-flag render beside the verbatim subtotal — a flagged `/score` contribution
  row linking to the slice-11 thread. **The thick WS forced all seams into existence**; SF-2
  presence-only (01-02) and SF-N1 N+1 (01-03) confirmed off the skeleton.
- **Phase 02 — the orthogonality cardinals + the legend + parity (SF-3..SF-7)**: SF-3
  sum-to-weight on a flagged breakdown (02-01), SF-4 byte-identity vs slice-09 (02-02), SF-5 the
  anti-misread `SCORE_COUNTER_LEGEND` (02-03, the one genuinely-new artifact), SF-6 no-noise
  (02-04), SF-7 parity (02-05) — all confirmatory off the WS render path plus the genuine
  identical-subtotal twin seed (the evidence-perturbed quadruple builder — twins differ only in
  CID, not scorer inputs).
- **Phase 03 — gold** (read-only / no-write + offline + the CARDINAL byte-identity / N+1): the
  GOLD invariants flipped GREEN off the confirmatory render path — read-only / no-write (03-01),
  the two offline golds (03-02), and the CARDINAL byte-identity / N+1 last (03-03).

The render stays a **total fn of `(ScoreState, presence)`** (presence gates only the additive
marker, never a number — ADR-051); no production "disable the flag" seam was added (the
byte-identity gold elides the markers and the legend against the recorded slice-09 baseline).

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-SF-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..13 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-SF-2 | Mutation = per-feature 100% on the new + extended PURE `viewer-domain` production functions (the presence-threaded render arm + the `SCORE_COUNTER_LEGEND` rendering), matching slice-02..13 DV-2; killing properties kept IN-CRATE (the slice-04/05 cross-package lesson). | Per-feature gate at deliver-time + DEVOPS sweep backstop; the measurement reaches the real killing suite locally. 6/6 in-diff viable caught, 0 missed. |
| DV-SF-3 | **The flag is ORTHOGONAL — shown but NEVER applied; sum-to-weight preserved on a FLAGGED breakdown (the countered contribution keeps its FULL original subtotal) AND `/score` byte-identical to slice-09 with the markers + the legend elided** (I-SF-2 + I-SF-3, TWO CARDINALS). | `/score` carries scoring SEMANTICS, so orthogonality is the load-bearing cardinal — TWICE: a flag that down-weighted / zeroed / filtered / re-ranked a contribution would silently change the displayed score, breaking the sum-to-weight identity; and a flag that perturbed any weight/confidence/bonus/subtotal/total/bucket/ranking/row-order would break the no-regression. Guaranteed structurally (presence is a `&HashSet<String>` gating only an additive marker, never a number — ADR-051) + behaviorally (the sum-to-weight gold on a flagged breakdown + the byte-identity gold against the recorded slice-09 baseline, markers + legend elided). |
| DV-SF-4 | **Thread presence as `&HashSet<String>` DOWN the render chain, do NOT add an `is_countered` field (slice-13's approach)** (ADR-051). | `/score` rows project `scoring::Contribution` / `scoring::WeightedPairing` — foreign-immutable types owned by the pure `scoring` crate the viewer must NOT mutate. Field injection (slice-13's `EdgeRow.is_countered`) would require mutating a foreign type; passing presence as a `&HashSet<String>` threaded down the render keeps the scoring types untouched and makes the orthogonality STRUCTURAL — presence gates only the additive marker and never reaches a number. The render = total fn of `(ScoreState, presence)`. |
| DV-SF-5 | **`score_counter_presence` flattens EVERY contribution cid across EVERY pairing into ONE `counter_presence_for` call** (ADR-050 carry-forward, mirroring slice-13's `survey_counter_presence`). | A per-pairing or per-contribution presence read would be an N+1 that scales with pairing / contribution count; flattening every contribution cid across every pairing before the presence read keeps the per-render cost at ONE query regardless. The carry-forward of the slice-13 edge N+1 guard onto the scoring breakdown. |
| DV-SF-6 | **REUSE the slice-12 `counter_presence_for` read + the slice-13 `render_countered_link` render + the slice-11 `COUNTERED_PRESENCE_FLAG` constant VERBATIM; the one genuinely-new artifact is `SCORE_COUNTER_LEGEND`** (ADR-049 carry-forward + L-refactor). | Presence is the same set-membership question on every surface and the flag is the same byte-identical `<a href="/claims/{cid}">Countered</a>` render — so the read, the render, and the flag constant are all reused verbatim, adding zero read surface and zero render duplication. The only new surface-specific artifact the scoring view needs is the anti-misread legend, because `/score` is the only surface where a reader could misread the flag as a re-weighting. |
| DV-SF-7 | **The countered identical-subtotal twin seed uses an evidence-perturbed quadruple builder — twins differ ONLY in CID, not in scorer inputs.** | To prove the flag is gated on cid presence (not on any score value), the seed needs two contributions with IDENTICAL subtotals where one is countered and one is not. The evidence-perturbed quadruple builder produces such twins differing only in CID, so the sum-to-weight + presence-only assertions cannot be satisfied by accidental coupling between the flag and a score value. |

## Cardinal release gates + slice-14 invariants (I-SF-1..7)

The cardinal release gates realized on the scoring surface — all release-blocking:

1. **Read-only / no key (I-SF-1)** — `/score?contributor=<did>` (now flagged) is a READ; no
   write/sign/subscribe route; the web process holds no signing key; the REUSED presence-read seam
   has NO mutation method (type-level). Three-layer: TYPE + STRUCTURAL (`xtask check-arch` viewer
   capability rule) + BEHAVIORAL (gold read-only / no-write).
2. **Sum-to-weight preserved / shown-never-applied (I-SF-2, CARDINAL)** — on a FLAGGED breakdown
   the displayed per-contribution subtotals still SUM to the displayed pairing weight; the
   countered contribution keeps its FULL original subtotal; the flag never down-weights / zeroes /
   filters / re-ranks (DV-SF-3 + the sum-to-weight gold on a flagged breakdown). The orthogonality
   is structural — presence is a `&HashSet<String>` that gates only an additive marker and never
   reaches a number (ADR-051).
3. **No-regression / byte-identity (I-SF-3, CARDINAL)** — with the flag markers AND the
   `SCORE_COUNTER_LEGEND` elided, `/score` is byte-identical to slice-09 (every weight, confidence,
   bonus, subtotal, total, bucket, ranking, row order); the flag never perturbs any number or order
   (DV-SF-3 + the byte-identity gold against the recorded slice-09 baseline, markers + legend
   elided).
4. **Presence-only (I-SF-4)** — an N-author-countered cid renders ONE neutral marker (via the
   `HashSet` / `DISTINCT`), never "disputed by N" (DV-SF-6 + the presence-only scenarios);
   un-countered contributions render no flag (no-noise).
5. **Robust / graceful-degradation (I-SF-5)** — a failed presence read degrades to no flags
   (`unwrap_or_default`, inherited from slice-12), never 5xx; the surface still renders the score.
6. **N+1 guard (I-SF-6)** — exactly ONE `counter_presence_for` call per render, invariant to
   pairing and contribution count (DV-SF-5 + the `score_counter_presence` flatten + the SF-N1 N+1
   proxy on a multi-pairing breakdown with a SPREAD countered subset + the inherited adapter
   1 / N / 5N bound).
7. **Offline / local-only + anti-misread legend (I-SF-7)** — the presence reads hit the LOCAL DB
   ref tables only (no artifact read, no network — fully offline); the page references only the
   vendored local htmx asset (no CDN); loopback-only bind; nothing persisted (the two offline
   golds). The `SCORE_COUNTER_LEGEND` (blocklist-clean) renders once on the Scored arm — the
   anti-misread note defending the scoring surface against the misread that a flag re-weights a
   contribution.

The full slice-14 invariant set (I-SF-1..7; structural-guarantee detail in the DESIGN section of
`feature-delta.md`):

| # | Invariant | Enforcement |
|---|---|---|
| I-SF-1 | Read-only / no key (the flagged `/score` is a READ; no write/sign/subscribe route; no key in the process; the REUSED presence-read seam holds no mutation method). | TYPE (no write method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (gold read-only/no-write). Cardinal. |
| I-SF-2 | Sum-to-weight preserved / shown-never-applied (on a FLAGGED breakdown the per-contribution subtotals SUM to the pairing weight; the countered contribution keeps its FULL original subtotal; the flag never down-weights / zeroes / filters / re-ranks). | STRUCTURAL (presence is a `&HashSet<String>` gating only an additive marker, never a number — ADR-051; the `scoring` types untouched) + BEHAVIORAL (the sum-to-weight gold on a flagged breakdown, DV-SF-3). CARDINAL. |
| I-SF-3 | No-regression / byte-identity (with the markers + the legend elided, `/score` is byte-identical to slice-09 — every weight, confidence, bonus, subtotal, total, bucket, ranking, row order). | STRUCTURAL (the underlying scoring read + the render numbers UNCHANGED; the flag + legend layered on top, DV-SF-3) + BEHAVIORAL (the byte-identity gold against the recorded slice-09 baseline, markers + legend elided). CARDINAL. |
| I-SF-4 | Presence-only (an N-author-countered cid = ONE neutral marker via the `HashSet`/`DISTINCT`, never "disputed by N"; un-countered contributions = no flag). | STRUCTURAL (the `HashSet` membership + slice-12 `SELECT DISTINCT`, DV-SF-6) + BEHAVIORAL (presence-only + no-noise scenarios). Cardinal. |
| I-SF-5 | Robust / graceful-degradation (a failed presence read → no flags via `unwrap_or_default`, never 5xx; the surface still renders the score). | STRUCTURAL (the presence read is additive enrichment over the existing scoring read, inherited from slice-12) + BEHAVIORAL (the degradation path). |
| I-SF-6 | N+1 guard (exactly ONE `counter_presence_for` call per render, invariant to pairing/contribution count). | STRUCTURAL (the `score_counter_presence` flatten — every contribution cid across every pairing → ONE call, ADR-050 carry-forward, DV-SF-5) + BEHAVIORAL (the SF-N1 N+1 proxy on a multi-pairing SPREAD-countered breakdown + the inherited adapter 1/N/5N bound). Cardinal. |
| I-SF-7 | Offline / local-only + anti-misread legend (the presence reads hit the LOCAL DB ref tables only — no artifact read, no network; no-CDN chrome; loopback-only; nothing persisted; the `SCORE_COUNTER_LEGEND` renders once on the Scored arm). | STRUCTURAL (the ref-tables-only DB read inherited from slice-12; the shared `htmx_script` fn + pinned asset; loopback guard unchanged; the blocklist-clean legend constant) + BEHAVIORAL (the two offline golds + read-only row-count delta + the legend-presence assertion). Cardinal. |

All slice-14 invariants INHERIT the slice-06 I-VIEW-1..6 + slice-07 I-HX-1..5 sets (read-only /
no key / human gate / offline + loopback / progressive enhancement / structural fragment/page
parity); confidence stays shown verbatim on every contribution row.

## Quality gates — final report

- **Acceptance / integration**: the `viewer_counter_flags_score_surface` scenarios (SF-1..SF-7
  through the `/score?contributor=<did>` driving port, the SF-1 walking skeleton + the SF-3
  sum-to-weight orthogonality + the SF-4 byte-identity) + the GOLD
  `viewer_counter_flags_score_surface_invariants` GREEN (incl. the TWO CARDINALS — sum-to-weight +
  byte-identity vs slice-09 — and the N+1) + the `viewer-domain` unit/property tests (the
  presence-threaded render arm + the `SCORE_COUNTER_LEGEND`) + the inherited `adapter-duckdb`
  presence/N+1 bound (no adapter change); slices 06/07/09/11/12/13 corpora GREEN — zero regression
  (the byte-identity gold proves `/score` unchanged vs slice-09 with the markers + legend elided).
  The `ViewerServer` harness drives the REAL `openlore ui` over HTTP; the store is seeded through
  the REAL ingest path.
- **`cargo xtask check-arch`**: OK (21 workspace members) — no new crate, no new route, no new
  read method, no new render fn, no new SQL, **no new allowlist edge** (the dependency graph was
  already in place) + the confirmed viewer capability rule (read-only counter-presence reads; no
  signing/identity/PDS, no store-write; no `scoring` mutation).
- **Refactor (L-refactor)**: clippy + check-arch clean; the slice-13 `render_countered_link` + the
  slice-11 `COUNTERED_PRESENCE_FLAG` reused verbatim (no new render fn); `viewer-domain` purity
  intact (no I/O imports; maud + ports only; presence threaded as `&HashSet<String>` keeps the
  render a total fn of `(ScoreState, presence)` and the foreign `scoring` types untouched).
- **Adversarial review**: **APPROVED** — zero defects, zero Testing Theater. The orthogonality
  confirmed load-bearing on TWO cardinals (the sum-to-weight gold on a flagged breakdown + the
  byte-identity gold against the recorded slice-09 baseline, markers + legend elided, DV-SF-3); the
  presence-only confirmed structural (the `HashSet`/`DISTINCT`, DV-SF-6); the N+1 guard confirmed
  (one flattened presence call per render via `score_counter_presence`, the SF-N1 SPREAD proxy,
  DV-SF-5); the identical-subtotal twin seed confirmed genuine (the evidence-perturbed quadruple
  builder — twins differ only in CID, DV-SF-7).
- **DES integrity**: PASS — all 11 steps have complete DES traces (11/11).

## Mutation testing — final report

**Scope**: the new + extended pure `viewer-domain` production functions (the presence-threaded
render arm — gated on `&HashSet<String>` membership beside the verbatim subtotal — and the
`SCORE_COUNTER_LEGEND` rendering on the Scored arm). The slice-04/05 cross-package lesson stays
applied — the `viewer-domain` unit/property tests pin the production functions IN/against the crate,
so the per-feature mutation measurement reaches the real killing suite without a cross-package
detour.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` production logic (the presence-threaded render arm + the `SCORE_COUNTER_LEGEND` rendering, in-diff) | 6 | 6 | 0 | **100%** (6/6 in-diff viable) |

Slice-14 per-feature gate SATISFIED (≥80%; actual 100% on the in-diff production scope, 0 missed).
`adapter-http-viewer` (the `/score` handler + the `score_counter_presence` flatten) +
`adapter-duckdb` (the REUSED slice-12 read, unchanged) + `scoring` (UNCHANGED — foreign-immutable,
never mutated) are NOT mutated by design (effect shell / foreign crate; covered by the GOLD
invariants — incl. the sum-to-weight + byte-identity cardinals — + the inherited adapter
presence/N+1 bound through the real binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **On a scoring surface, make orthogonality STRUCTURAL — thread presence as a `&HashSet<String>`
  that gates only an additive marker, do NOT inject an `is_countered` field into a foreign scoring
  type (ADR-051, DV-SF-4)**: the `/score` rows project `scoring::Contribution` /
  `scoring::WeightedPairing`, types owned by the pure `scoring` crate the viewer must NOT mutate.
  slice-13's `EdgeRow.is_countered` field-injection would require mutating a foreign type AND would
  let presence sit next to a number. Passing presence as a `&HashSet<String>` threaded down the
  render keeps the scoring types untouched and makes the orthogonality structural — presence can
  ONLY gate the additive marker, it can never reach a number. **Lesson: when flagging a render
  whose rows project FOREIGN-IMMUTABLE types (or carry semantics the flag must not perturb), pass
  presence as a separate `&HashSet<String>` threaded down the render (render = total fn of
  `(state, presence)`) rather than injecting a flag field — it keeps the foreign types untouched
  and makes the never-applied guarantee structural rather than merely tested.**
- **A scoring-surface flag needs TWO cardinals, not one — sum-to-weight on a FLAGGED breakdown AND
  byte-identity, because each catches a different way the flag could leak into a number (DV-SF-3)**:
  byte-identity (markers + legend elided) catches the flag perturbing a displayed number or order;
  sum-to-weight on a flagged breakdown catches the flag silently down-weighting / filtering /
  re-ranking the FLAGGED contribution specifically (which a byte-identity test on an un-flagged
  baseline cannot see). **Lesson: when a flag lands on a surface carrying computed semantics, the
  no-regression gold (flag elided vs the prior slice) is necessary but not sufficient — add a
  semantic-invariant gold that holds ON the flagged output (here: the subtotals still sum to the
  weight WITH the flag present, the countered contribution keeping its full subtotal), so a flag
  that quietly altered the flagged rows specifically is caught.**
- **The identical-subtotal twin seed must perturb EVIDENCE, not subtotal — twins differ only in CID
  (DV-SF-7)**: to prove the flag is gated on cid presence and not coupled to any score value, the
  presence-only / sum-to-weight seeds need two contributions with IDENTICAL subtotals where one is
  countered and one is not; the evidence-perturbed quadruple builder produces such twins differing
  only in CID, not in scorer inputs. **Lesson: to prove a flag is gated on identity (cid presence)
  and not on a value, seed twins that are identical in the value dimension and differ only in the
  identity dimension — perturb the upstream evidence that changes the CID while leaving the
  computed value identical, so the flag and the value cannot be accidentally coupled.**
- **REUSING the read + the render + the flag constant verbatim is the win — the only new artifact is
  the anti-misread legend (ADR-049 carry-forward, DV-SF-6)**: presence is the same set-membership
  question and the flag is the same byte-identical link on every surface, so the slice-12
  `counter_presence_for` read, the slice-13 `render_countered_link` render, and the slice-11
  `COUNTERED_PRESENCE_FLAG` constant all reused verbatim with zero new read / render / constant
  surface. The only surface-specific new artifact `/score` needs is the `SCORE_COUNTER_LEGEND`,
  because `/score` is the only surface where a reader could misread the flag as a re-weighting.
  **Lesson (carry-forward): the fifth surface to ask the same data question and render the same flag
  should reuse the read + render + flag constant verbatim and add ONLY what is genuinely
  surface-specific (here: the anti-misread legend on the one surface carrying scoring semantics) —
  keep the read / render surface flat and let the new artifact be exactly the irreducible delta.**
- **The flatten-into-one-presence-call N+1 guard carries from the grouped edge render to the scored
  breakdown unchanged (ADR-050 carry-forward, DV-SF-5)**: `score_counter_presence` flattens every
  contribution cid across every pairing into ONE `counter_presence_for` call, exactly mirroring
  slice-13's `survey_counter_presence` flatten over the edge groups. **Lesson: the
  flatten-every-cid-before-the-batch-read pattern (one presence call per render, invariant to the
  nesting count) is the general N+1 guard for ANY nested render — grouped edges (slice-13) or scored
  pairings (slice-14); the SF-N1 proxy must SPREAD the countered subset across the pairings so a
  per-pairing regression diverges from the one-call baseline.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-051 fixed the thread-presence-as-`&HashSet<String>` contract (do NOT inject an `is_countered` field into the foreign `scoring` types; render = total fn of `(ScoreState, presence)`); field-level shaping left to DELIVER. | Adopted; presence threaded down the render chain + the `score_counter_presence` flatten + the REUSED `counter_presence_for` wiring materialized at DELIVER against the render + the sum-to-weight + byte-identity golds; the `scoring` types untouched; no new read/render fn. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-050 carry-forward fixed the flatten-into-one-presence-call intent (one presence call per render, never per-pairing/per-contribution) via `score_counter_presence`. | The wiring landed; the SF-N1 N+1 proxy (multi-pairing SPREAD-countered breakdown) pinned the one-call-per-render guarantee behaviorally; the inherited adapter 1/N/5N bound backstops it. | Resolved at DELIVER. |
| 3 | The orthogonality planned as TWO cardinals (sum-to-weight on a flagged breakdown + byte-identity vs slice-09 with markers + legend elided). | Both golds GREEN; the countered identical-subtotal twin used the evidence-perturbed quadruple builder (twins differ only in CID), confirming the flag is gated on cid presence not a score value. | Resolved at DELIVER. |
| 4 | The anti-misread `SCORE_COUNTER_LEGEND` planned as the one genuinely-new artifact, rendered once on the Scored arm. | Shipped, blocklist-clean, rendered once on the Scored arm; elided alongside the markers in the byte-identity gold. | Resolved at DELIVER. |
| 5 | `/score` framed as the LAST remaining surface — slice-14 completes J-003b across all local viewer surfaces. | Confirmed — with `/score` flagged there is no deferred surface remaining; J-003b is at-a-glance on every local viewer surface. | Confirmed at DELIVER. |
| 6 | Review expected to pass clean. | Review APPROVED — zero defects, zero Testing Theater (no revision needed). | Confirmed at DELIVER. |
| 7 | DEVOPS scheduled mutation per-feature at deliver-time. | DELIVER ran mutation per-feature (DV-SF-2, 100% in-diff 6/6, 0 missed). | Recorded. |

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-counter-flags-score-surface/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL sections), `discuss/` (wave-decisions, journey), `design/`
  (architecture-design, component-boundaries, data-models, technology-stack), `deliver/`
  (roadmap.json, execution-log.json).
- **Parent slice-06 archive** (the read-only viewer + the htmx chrome):
  `docs/evolution/htmx-scraper-viewer-evolution.md`
- **Parent slice-07 archive** (the htmx PE layer this slice composes):
  `docs/evolution/viewer-htmx-swaps-evolution.md`
- **Parent slice-09 archive** (the `GET /score?contributor=<did>` contributor-scoring surface this
  slice flags; the byte-identity baseline for `/score`):
  `docs/evolution/viewer-contributor-scoring-evolution.md`
- **Parent slice-11 archive** (the `GET /claims/{cid}` counter-thread this slice links to + the
  `COUNTERED_PRESENCE_FLAG` constant): `docs/evolution/viewer-counter-claim-threads-evolution.md`
- **Parent slice-12 archive** (the `counter_presence_for` batch read this slice REUSES):
  `docs/evolution/viewer-counter-claim-list-flags-evolution.md`
- **Parent slice-13 archive** (the graph-surface flags + the `survey_counter_presence` flatten
  pattern + the `render_countered_link` render this slice mirrors and REUSES):
  `docs/evolution/viewer-counter-flags-graph-surfaces-evolution.md`
- **Slice-14 ADR**:
  `docs/adrs/ADR-051-score-presence-projection-threaded-not-recomputed-anti-misread-legend.md`
- **Architecture design / component boundaries / C4 / data-flow**:
  `docs/feature/viewer-counter-flags-score-surface/design/` + the DESIGN sections of
  `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-counter-flags-score-surface/deliver/execution-log.json`,
  `docs/feature/viewer-counter-flags-score-surface/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_counter_flags_score_surface.rs` (the SF-scenarios SF-1..SF-7 through the
  `/score?contributor=<did>` driving port, the SF-1 walking skeleton + the SF-3 sum-to-weight + the
  identical-subtotal twin seed),
  `tests/acceptance/viewer_counter_flags_score_surface_invariants.rs` (the gold invariants, incl.
  the TWO CARDINALS — sum-to-weight + byte-identity vs slice-09 — and the N+1)
- **Reused read + render + flag constant + drill-link target**: the slice-12
  `counter_presence_for(&[String]) -> HashSet<String>` read (verbatim) + the slice-13
  `render_countered_link` render (verbatim) + the slice-11 `COUNTERED_PRESENCE_FLAG` constant
  (verbatim, in `crates/viewer-domain`); the `/claims/{cid}` slice-11 thread as the link terminus
- **New artifact**: the `SCORE_COUNTER_LEGEND` anti-misread constant in `crates/viewer-domain`
- **Extended viewer crates**: `crates/viewer-domain` (the presence-threaded `/score` render arm +
  the `SCORE_COUNTER_LEGEND` constant), `crates/adapter-http-viewer` (the existing `/score` handler
  + the `score_counter_presence` flatten + the presence wiring). `crates/adapter-duckdb` +
  `crates/ports` + `crates/scoring` UNCHANGED (the slice-12 read reused verbatim; the scoring types
  never mutated).
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml`
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-threads-evolution.md`,
  `viewer-counter-claim-list-flags-evolution.md`,
  `viewer-counter-flags-graph-surfaces-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`
