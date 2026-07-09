<!-- markdownlint-disable MD013 -->
# Evolution: viewer-philosophies-surface (slice-27 — read-only `/philosophies` vocabulary page) — FEATURE COMPLETE

> The LAST slice of `philosophy-vocabulary-registry` (slices 22–28). Builds on
> slice-22 (seeds), slice-21 (persistent nav / `LANDING_HUB_SURFACES`), and slice-10
> (`/philosophy` traversal). Paradigm: functional Rust (ADR-007). Companion design:
> ADR-059 §5 (slice-27 row). **With this slice the philosophy-vocabulary-registry
> feature is COMPLETE.**

## Summary

slice-27 ships US-PV-006 ("Browse philosophies in the viewer"): a read-only viewer
surface `GET /philosophies` that lists the shared philosophy vocabulary (each
philosophy's name + description), every entry linking to the existing
`/philosophy?object=<object-id>` traversal survey, and reachable + active-marked from
the slice-21 persistent nav. It mirrors the CLI `philosophy list` (slice-22) as an
HTTP surface. Read-only (no authoring control, no signing key — I-VIEW-1/3), offline
(pure over the embedded seeds). No new crate — workspace stays 21.

### What shipped (one paragraph)

A pure `viewer-domain::philosophies` module: `PHILOSOPHIES_URL = "/philosophies"` +
`render_philosophies_page()`, a total function over `lexicon::philosophy::seeds()`
that renders each seed's name + description + an `<a href>` to
`/philosophy?object=<object-id>` — the href built by reusing the shared
`href_philosophy` over the DERIVED `lexicon::object_id(name)` (no hardcoded
`/philosophy` path or NSID prefix), descriptions auto-escaped by maud. It renders
through `page_shell("…", PHILOSOPHIES_URL, body)` so the persistent nav marks the
Philosophies item `aria-current="page"`. `("Philosophies", PHILOSOPHIES_URL)` was
added to the `LANDING_HUB_SURFACES` nav SSOT (nav derived solely from it — no second
list). The viewer HTTP adapter registers `PHILOSOPHIES_URL => html_ok(
render_philosophies_page())` — a STORE-FREE arm mirroring `/scrape` (no store handle,
no signing key). `viewer-domain` gained a new PURE→PURE `lexicon` dep (serde-only;
check-arch deny-list permits it).

### Wave timeline

- DISCUSS / DESIGN — feature-level: `feature-delta.md` US-PV-006 (AC-006.1/2) +
  ADR-059 §5 (slice-27 row). No per-slice DISCUSS/DESIGN.
- DISTILL — 2026-07-09, commit `e0c5490`: RED scaffold
  `tests/acceptance/viewer_philosophies.rs` (VP-1..5) + `_invariants.rs`
  (VP-INV-NoControl/Offline/SingleSource) + `distill/red-classification-slice-27.md`.
  Gate PASS (8/8 genuine RED, 0 BROKEN, 75% error/edge).
- DELIVER — 2026-07-09 (this slice). NOTE: the crafter process crashed after
  reaching GREEN on both steps; the orchestrator recovered the uncommitted 02-01
  route wiring, verified it (8/8 green, check-arch OK), and committed it.

## DELIVER steps (5-phase TDD, functional crafter + orchestrator recovery)

- **01-01** `54179f6` — pure `/philosophies` view-model (`philosophies.rs`:
  `PHILOSOPHIES_URL` + `render_philosophies_page` over `seeds()`, read-only, through
  `page_shell`) + `("Philosophies", PHILOSOPHIES_URL)` in `LANDING_HUB_SURFACES` +
  the new `viewer-domain → lexicon` PURE→PURE dep + the nav-item single-source count
  tests bumped 8→9. Reuses `href_philosophy` + `object_id`.
- **02-01** `9007289` — register the store-free `GET /philosophies` route in
  `adapter-http-viewer` (mirrors the `/scrape` arm; no store, no signing key).
  Greened VP-1 (WS list + links), VP-2/VP-5 (nav reach + active), VP-3 (read-only),
  VP-4 (full offline seed set), and all three VP-INV-* golds. (Committed by the
  orchestrator after the crafter crash — the crafter had reached GREEN.)

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — quality gate PASS (2 steps, VP-1/3/4 + INV owned by 02-01 + the pure renderer/nav owned by 01-01, valid DAG, DISTILL linkage present) |
| DISTILL RED | 8/8 genuine MISSING_FUNCTIONALITY (no route / no SSOT entry), 0 BROKEN — gate PASS; 75% error/edge |
| Phase-3 refactor (L1-L4) | No changes warranted — minimal (one const + one pure fn + one route arm + one nav entry); adversarial review found 0 L1-L2 smells |
| Phase-4 adversarial review | APPROVED — 0 blockers, 0 defects. Independently verified read-only/no-authoring (I-VIEW-1/3 — no form/button/mutating hx, store-free handler), offline/pure (no external asset host), nav single-source (one SSOT, active marker), link correctness (reused `object_id`+`href_philosophy`, encoded), XSS-safety (maud auto-escape), completeness (count == `seeds().len()`), and external validity (wired through the HTTP route). 0 testing theater |
| Phase-5 mutation | Pure `crates/viewer-domain/src/philosophies.rs` = **100% viable kill (4/4, 0 survivors)** — gate ≥80% PASSED. Report: `deliver/slice-27/mutation/mutation-report.md` |
| Full regression | `cargo test --workspace` → all green except ONE parallel-load flake — `viewer_htmx::opening_a_claim_without_htmx_returns_the_full_detail_page` panicked in the harness `get()` HTTP `.send()` (in-process viewer server transport error under contention, `support/mod.rs:7544`), on the claim-detail path UNTOUCHED by slice-27; **isolated re-run of `viewer_htmx` = 24 passed, 0 failed**. Confirmed environment flake per the WS-determinism contract, not a regression. The slice-21 nav suites `viewer_persistent_left_nav` + `_invariants` (which the Philosophies nav entry touches) passed — no regression |
| check-arch | OK — 21 workspace members (no new crate); the new `viewer-domain → lexicon` edge is PURE→PURE (deny-list); viewer capability boundary intact (no signing/store in the handler) |

Tests green: `viewer_philosophies` 7/7 (VP-1..5 + 2 support) + `viewer_philosophies_invariants`
5/5 (VP-INV-* 3 + 2 support); viewer-domain nav tests (8→9); full workspace 0 failed.

## Load-bearing invariants

- **Read-only / no authoring (I-VIEW-1/3, gold VP-INV-NoControl)**: the surface
  renders only `<a href>` read links — no `<form>`, `<button>`, or mutating `hx-*` —
  and the route handler is store-free with no signing key. Minting stays the
  slice-24 `openlore philosophy add` CLI action.
- **Offline / pure (gold VP-INV-Offline)**: `render_philosophies_page` is a total
  function over `include_str!` seeds — no store, no network, no external asset host.
- **Nav single-source (gold VP-INV-SingleSource)**: the nav item set is derived
  SOLELY from `LANDING_HUB_SURFACES`; the Philosophies entry adds a link without a
  second list, and `page_shell(active=…)` marks it current.

## Deviations: planned vs shipped

- One new file — `crates/viewer-domain/src/philosophies.rs` (the surface; listed in
  the design component set). Otherwise edits extend `common.rs`, `lib.rs`,
  `Cargo.toml`, `adapter-http-viewer/lib.rs`. No new crate; 21 members.
- New `viewer-domain → lexicon` PURE→PURE dep (deny-list permits it; check-arch
  green — no explicit allowlist edit needed).
- **Recovery**: the crafter process crashed after reaching GREEN on both steps; the
  orchestrator recovered + verified + committed the uncommitted 02-01 route wiring
  (no work lost).
- **Scope**: lists the SEED vocabulary (offline, mirrors CLI `philosophy list`).
  Minted-philosophy records (slice-24 `philosophies` table) in the viewer are a
  documented FOLLOW-UP.
- `// SCAFFOLD: true` left in the acceptance files — repo convention.
- Outcome registry: skipped — no `registry.yaml`; prior slices registered none.

## KPI / dogfood

`./run.sh` then open `http://127.0.0.1:8788/philosophies` → the vocabulary list
(name + description), each linking to `/philosophy?object=…`; the persistent left
nav shows a "Philosophies" item, marked current on this page. No authoring control
anywhere on the page.

## Feature COMPLETE

`philosophy-vocabulary-registry` is now fully shipped across all 7 stories / slices:
22 (seed + list), 23 (show), 24 (mint), 25 (compose advisory), 26 (alias
triangulation), 27 (viewer surface), 28 (scraper single-source). Philosophy is a
first-class, discoverable, seeded-but-open shared vocabulary — CLI + viewer + graph
+ scraper all speak it.

Documented follow-ups (out of scope, tracked): minted-philosophy alias triangulation
(slice-26 seed-only today); minted philosophies in the viewer `/philosophies` +
`query_philosophy_survey` alias widening.

## Commit trail

`e0c5490` (DISTILL RED), `54179f6` (01-01), `9007289` (02-01). All on `main`
(trunk-based, no PR).
