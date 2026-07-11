<!-- markdownlint-disable MD013 -->
# Evolution: philosophy vocabulary discoverability (slices 29–34 — expand + alias/seeAlso surfacing)

> A post-COMPLETE **polish arc** on `philosophy-vocabulary-registry`. The feature was
> declared COMPLETE at slice-27 (`viewer-philosophies-slice-27-evolution.md`); slices
> 29–34 are user-directed increments layered on top. Paradigm: functional Rust
> (ADR-007). Companion design: ADR-059 (embedded seeds, read-time alias resolution).
> This doc finalizes the arc: slice-34 closes it, both discovery surfaces (CLI
> `philosophy list` + viewer `/philosophies`) now show **name + description + aliases
> + seeAlso** for every seed.

## Summary

The arc does two things: (1) **grows the seeded vocabulary** from 12 → 64 common
software philosophies (slice-29), and (2) makes the two richer seed fields —
`aliases` and `seeAlso` — **discoverable and resolvable** across every surface, so a
user who knows a philosophy by a non-canonical name can still find it and see its
reference links. Each capability lands as a matched CLI/viewer pair (parity is the
recurring shape): resolve/render in the pure `lexicon` core, then surface it in the
CLI list and the read-only `/philosophies` browse page. Every slice is a single pure
step over the embedded seeds — no new crate, no new route, no store/network/signing
dependency. Workspace stays 21 crates throughout; check-arch unchanged.

## Slices (each a single 5-phase TDD step over the embedded seeds)

- **slice-29** `9cf0151` — **seed the common software philosophies.** Expanded
  `seeds.json` with 52 process/design/architecture/ops philosophies (12 → 64). Pure
  data growth; every downstream discovery surface inherits the larger vocabulary for
  free.
- **slice-30** `0628b81` — **`philosophy show` resolves aliases.** Added
  name|object|alias resolution to the canonical record in the pure `lexicon` layer
  and drove `philosophy show` on it — a user can now look a philosophy up by any of
  its known aliases, not just its canonical name. (Acceptance: `philosophy_show.rs`.)
- **slice-31** `cd21865` — **`philosophy list` surfaces aliases.** Rendered an
  `aliases:` line in each list block (alias discoverability from the list surface).
  (Acceptance: `philosophy_vocabulary.rs`.)
- **slice-32** `ae0dccf` — **`/philosophies` surfaces aliases (viewer parity with
  31).** Rendered the aliases line in each `/philosophies` vocabulary entry — the
  read-only browse page reaches CLI parity for aliases. (Acceptance:
  `viewer_philosophies.rs`.)
- **slice-33** `6554bf9` — **`philosophy list` surfaces seeAlso links (reference
  discoverability).** Rendered a `seeAlso:` line in each list block (bare-text URLs,
  idiomatic for the CLI). (Acceptance: `philosophy_vocabulary.rs`.)
- **slice-34** `874e715` — **`/philosophies` surfaces seeAlso links (viewer parity
  with 33).** Appended a `seeAlso:` line to each `render_philosophy_entry`, each URL
  as a **read-only external `<a href>`** (idiomatic for HTML, unlike the CLI's bare
  text). Rendered only when a seed carries any seeAlso. Closes the arc. (Acceptance:
  `viewer_philosophies.rs`.)

## Key decisions

- **CLI/viewer parity as the unit of work.** Each new field is surfaced in a pair of
  slices — CLI first, then viewer parity — so the two discovery surfaces never drift.
  By slice-34 both show name + description + aliases + seeAlso.
- **Read-time resolution in the pure core (ADR-059).** Alias resolution
  (slice-30) lives in `lexicon`, not in a surface adapter, so `show`, `list`, and
  `/philosophies` all resolve identically from one total function over the seeds.
- **Read-only external links, not traversal links.** slice-34's seeAlso URLs render
  as plain external `<a href>` (no `?object=`), so the slice-27 one-traversal-link-
  per-seed count and slice-32 alias assertions stay green, and the viewer's offline
  invariant holds (a Wikipedia reference is not a CDN-loaded asset — the
  `references_external_cdn` guard only matches htmx CDN hosts).
- **No structural growth.** Every slice is a pure renderer/resolver over embedded
  seeds — no crate, route, dep, store, network, or signing-key change. check-arch
  stays 21 across the whole arc.

## Invariants preserved throughout

- **I-VIEW-1/3** — `/philosophies` exposes no authoring/mutating control and holds no
  signing key (read-only browse surface).
- **Offline** — pure over the embedded seeds; no external CDN asset loaded (external
  reference `<a href>` links are navigational, not loaded assets).
- **Nav SSOT** — `/philosophies` continues to render through `page_shell` with the
  persistent nav marking the item `aria-current="page"`; nav derived solely from
  `LANDING_HUB_SURFACES`.

## Lessons

- **Polish arcs benefit from an explicit close.** The arc had a natural endpoint
  (both surfaces at full field parity); naming it (slice-34 "closes the arc") avoids
  open-ended incrementalism.
- **The stale execution-log was harmless but misleading.** `deliver/execution-log.json`
  was never advanced past the slice-27 steps — slices 29–34 were committed directly
  with per-slice `roadmap.json`s. Git history + the per-slice roadmaps are the source
  of truth for this arc, not the log. Recorded here so a future reader doesn't trust
  the log's step count.

## Links

- Design: `docs/adrs/ADR-059-philosophy-vocabulary-registry-record-reconciliation-embedded-seeds-signed-mints-read-time-alias-resolution.md`, `docs/adrs/ADR-042-viewer-project-philosophy-survey-reads-two-method-anti-merging.md`
- Prior evolution docs: `viewer-philosophies-slice-27-evolution.md` (feature COMPLETE), `philosophy-alias-triangulation-slice-26-evolution.md`, `philosophy-mint-slice-24-evolution.md`, `philosophy-show-slice-23-evolution.md`, `philosophy-vocabulary-registry-evolution.md`
- Acceptance suites: `tests/acceptance/philosophy_show.rs`, `tests/acceptance/philosophy_vocabulary.rs`, `tests/acceptance/viewer_philosophies.rs`
- Seeds: `crates/lexicon/…/seeds.json`; renderer: `crates/viewer-domain/src/philosophies.rs`
