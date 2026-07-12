<!-- markdownlint-disable MD013 -->
# Evolution: retraction-aware-search-filter — an opt-in, non-destructive, self-disclosing retraction filter on network search

> A full nWave feature (DISCUSS → DESIGN → DISTILL → DELIVER) delivering the last
> "documented additive future option" from the appview-search umbrella
> (`docs/product/architecture/brief.md`: "a retraction-aware search FILTER, deferred
> OD-AV-7 / I-AV-9"). Paradigm: functional Rust (ADR-007). Companion design: ADR-060.
> Two slices (CLI then viewer parity). **FEATURE COMPLETE.**

## Summary

Network search obeys the standing invariant **I-AV-9** ("counter shown, not applied"):
a soft-retracted verified claim STAYS discoverable and is annotated, NEVER silently
filtered. This feature adds a **user-invoked** view control that HIDES author-self-
retracted claims from the current view only — reconciled with I-AV-9 (not contradicting
it) by three simultaneous constraints locked as the cardinal decision **D-1**: **opt-in**
(default output byte-identical to today), **non-destructive** (view-only; no index
mutation, re-verify, re-rank, or re-weight of survivors), and **honest** (always discloses
"N retracted claim(s) hidden"). A user-invoked, disclosed, reversible filter is not
*silent* filtering — so the invariant is preserved in full (formalized as I-RF-1..3).

The whole feature is one pure decision reused by both surfaces:
`appview_domain::partition_retracted(rows, hide_retracted) -> { survivors, hidden_count }`
— a total function over the RAW attributed rows (`ports::NetworkResultRowRaw`, full
reference graph), invoked by the CLI `openlore search --hide-retracted` and the read-only
viewer `/search?hide_retracted=1`. **No new crate, no new route** (workspace stays 21;
check-arch green throughout).

### The load-bearing design resolution (OD-RF-1 → Branch A)

The one real design risk was whether the shipped index could distinguish an **author
self-retraction** from a **third-party disagreement** without a schema change. DESIGN
resolved it against the real code as **Branch A (sufficient as-is)**: a pure predicate
distinguishes them using only data already on the wire —

> claim C is author-self-retracted **⟺** some result row K has `K.author_did ==
> C.author_did` carrying `{ Retracts, C.cid }`.

Third-party `Counters` and different-author `Retracts` never hide their target (D-3 /
I-RF-4 — **no heckler's veto**, preserving anti-merging I-AV-2). Zero ingest/DTO/schema
change; the CLI slice stayed pure-core.

Two subtleties the real code forced (both pinned by gold tests):
1. **Anti-lossy** — detection runs on the raw rows' full `references` graph, NOT
   `compose_results`' lossy single-slot `counter_annotation` (which could mask a
   self-retraction behind a co-present counter). ADR-060 Earned-Trust #1; RF-4.
2. **Event-count** — a retraction is ONE event = the withdrawn original C + its
   same-author marker K (both dropped); `hidden_count` counts **events**, not raw rows.
   This *refined* the DISCUSS `len − len` note (which double-counts the marker). RF-7.

## Slices (DELIVER, 3 steps, manual-TDD mode)

- **01-01** `a2475aa` — the pure `partition_retracted` predicate + `RetractionPartition`
  in `appview-domain`, replacing the DISTILL scaffold. Four-predicate ladder
  (`is_own_retraction_marker` → `is_self_retracted` → `self_retraction_events` →
  `is_withdrawn`) reading as the spec. Property-based tests (identity-when-off, survivors
  ⊆ input, order-preserving, confidence-verbatim, idempotent, event-count, dominance,
  no-heckler-veto). Pure core, no I/O.
- **01-02** `e65628d` — wire the opt-in `--hide-retracted` flag into `openlore search`
  + the honest disclosure (footer with event-count + re-run guidance; empty-after-filter
  guided buffer; silent when nothing hidden). Greens the 7 CLI feature scenarios; RF-2
  default-unchanged gold guard stays green.
- **02-01** `618ebaa` — viewer parity: parse the read-only `?hide_retracted=1` GET param,
  reuse the same predicate on the raw rows before compose, render survivors + an on-page
  notice / guided empty state. A `SearchState` ADT (`FilteredResults{hidden_count>=1}` /
  `AllRetracted` / `Results`) makes a misleading "0 hidden" notice **unrepresentable by
  construction**. Read-only GET-param checkbox — no mutating markup, no signing key, no
  CDN (I-VIEW). Greens the 5 viewer feature scenarios; RF-V2 gold guard stays green.

## Quality gates (all passed)

- **DISCUSS** DoR 16/16; **consolidated end-of-DISTILL review** (Eclipse + Architect +
  Sentinel-reviewer) all APPROVED, 0 blockers / 0 high.
- **DISTILL** 14 acceptance scenarios (8 CLI + 6 viewer), RED gate 12 genuine RED + 2
  intended-green gold guards + 0 BROKEN; error/edge ratio 87.5% / 83.3%.
- **DELIVER** post-merge integration: all 14 scenarios + shipped `appview_search` (24) /
  `viewer_network_search` (20) green together; Elevator-Pitch demos covered by RF-1/RF-V1.
- **Phase 3** L1–L6 refactor: no changes warranted (code clean). **Phase 4** adversarial
  review: APPROVED, 0 defects / 0 testing theater. **Phase 5** mutation: **100% kill**
  (27/27 viable; retraction.rs 17/17 first pass). **Phase 6** integrity: PASS (manual).
- `cargo xtask check-arch` OK (21 workspace members) throughout; zero new crates/routes.

## Invariants introduced / preserved

- **I-RF-1** opt-in — default path byte-identical (mechanically guarded by RF-2/RF-V2).
- **I-RF-2** non-destructive — survivors keep order + verbatim confidence (RF-5).
- **I-RF-3** honest — always disclose the event count; empty-after-filter is a guided
  state, never a bare empty (RF-6/RF-V4); silent when nothing hidden (RF-8).
- **I-RF-4 / D-3** no heckler's veto — only an author's own retraction of their own claim
  hides it (RF-3/RF-4/RF-V5).
- **I-AV-9 preserved** — a user-invoked, disclosed, reversible filter is not silent
  filtering; the shipped default is unchanged.

## Lessons

- **Reconcile, don't override.** The feature looked like it contradicted a standing
  invariant ("never silently filter"); framing the reconciliation (opt-in + non-destructive
  + honest) as the cardinal decision turned a conflict into three testable constraints.
- **Resolve the load-bearing risk first.** DESIGN spending its first act proving OD-RF-1 =
  Branch A against real code (not assuming) kept the whole feature to zero schema change
  and a pure predicate.
- **Let the code refine the spec.** The event-count semantics and the anti-lossy raw-rows
  requirement both emerged from reading the actual index/compose code, and back-propagated
  cleanly into the DISTILL fixtures and the DISCUSS shared-artifacts table.
- **Tooling degradation is survivable with honesty.** The DES audit CLIs
  (`des-init-log`/`des-verify-integrity`) were broken in this environment; DELIVER ran in
  approved manual-TDD mode with the execution-log maintained by hand and integrity verified
  manually — every substantive gate (TDD, 14 ATs, refactor, review, 100% mutation,
  check-arch) still ran via cargo.

## Links

- Design: `docs/adrs/ADR-060-retraction-aware-search-filter-pure-predicate-existing-reference-graph.md`; `docs/product/architecture/brief.md` (Application Architecture entry; the deferred filter de-listed as shipped). Related: ADR-025 (network-index schema), ADR-027 (search verb), ADR-015 (counter/retraction model), ADR-007 (functional paradigm).
- Requirements: `docs/product/jobs.yaml` J-005 + sub-job J-005d.
- Workspace: `docs/feature/retraction-aware-search-filter/feature-delta.md` (all four waves), `slices/`, `deliver/{roadmap.json, execution-log.json, mutation/mutation-report.md}`.
- Code: `crates/appview-domain/src/retraction.rs` (predicate), `crates/cli/src/{verbs,render}/search.rs` (CLI), `crates/adapter-http-viewer/src/lib.rs` + `crates/viewer-domain/src/search.rs` (viewer).
- Acceptance: `tests/acceptance/{search_hide_retracted.rs, viewer_search_hide_retracted.rs}`.
- Commits: DISCUSS `75dd186`, DESIGN `3ad2cb7`, DISTILL `fad924b`, DELIVER `a2475aa` / `e65628d` / `618ebaa`, mutation `3976875`.
