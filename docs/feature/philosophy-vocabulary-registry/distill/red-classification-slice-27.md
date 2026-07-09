<!-- markdownlint-disable MD013 -->
# RED Classification — slice-27 (viewer-philosophies-surface)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). Every slice-27 acceptance scenario was run
> once against the CURRENT (unimplemented) production code and classified.
> DELIVER reads this file at the RED-phase entry gate (ADR-025 D2) to confirm RED
> is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-07-09 · Rust / cucumber-free
> subprocess + HTTP acceptance shape (mirrors slice-21 `viewer_persistent_left_nav.rs`
> + slice-15 `viewer_peer_subscriptions.rs`).
> Scope: US-PV-006 (AC-006.1..2, job_id J-002; ADR-059 §5 row 27) — the read-only
> `GET /philosophies` VIEWER VOCABULARY surface. The LAST slice of
> `philosophy-vocabulary-registry` — it closes the feature. Slices 22 (seed+list),
> 23 (show), 24 (mint), 25 (compose advisory), 26 (alias triangulation), 28 (scraper)
> are SHIPPED. MINTED-philosophy records (the slice-24 `philosophies` table) in the
> viewer are a documented FOLLOW-UP — OUT of scope; the surface lists the SEED
> vocabulary only (offline, no store read), which is what AC-006.1/.2 are about.

## Wave-decision reconciliation

The feature uses the single `docs/feature/philosophy-vocabulary-registry/feature-delta.md`
SSOT — there are no separate `discuss/`, `design/`, `devops/` `wave-decisions.md`
files to cross-check. US-PV-006 AC-006.1/.2 and the DESIGN row 27 ("viewer-domain +
adapter-http-viewer (`/philosophies`); slice-21 `LANDING_HUB_SURFACES` | Read-only,
no authoring control, no key in web process (D5/I-VIEW-1/3)") agree with each other
and with the slice brief. **Reconciliation passed — 0 contradictions.**

## How the run was performed

```
cargo build --bin openlore                                                        # build-before-run (the AT spawns the real bin)
cargo test -p cli --test viewer_philosophies --test viewer_philosophies_invariants --no-run   # COMPILE gate (BROKEN check)
cargo test -p cli --test viewer_philosophies -- --test-threads=1
cargo test -p cli --test viewer_philosophies_invariants -- --test-threads=1
```

Both acceptance targets COMPILE green (`--no-run` → `Finished`; the 15 warnings
per binary are ALL from the shared `support` harness — unused imports / unreachable
`FederatedGraphFixture` match arms — NONE from `viewer_philosophies.rs` or
`viewer_philosophies_invariants.rs`). Each spawns the REAL `openlore ui` viewer via
the existing frozen `ViewerServer` support harness and imports only that harness
(`mod support; use support::*`) plus `lexicon::philosophy::{seeds, object_id}`.
`lexicon` already ships (a `cli` dependency, `crates/cli/Cargo.toml:37`) and its
`seeds()`/`object_id()` are SHIPPED (slices 22–26) — so the import resolves and
`seeds()` is a pure, offline ORACLE for the expected vocabulary (VP-4 completeness),
NEVER the system under test. Therefore every acceptance failure is a RUNTIME
assertion against the observable HTTP surface (status + rendered body), not a
compile / import error → RED, never BROKEN.

Single-threaded (`--test-threads=1`) per the known in-process viewer parallel-load
flake (a `get_htmx .send()` transport error under heavy parallelism — an environment
flake, NOT a logic issue). These ATs use `get` (full page) ONLY — never `get_htmx` —
and the frozen `support/mod.rs` is UNMODIFIED (no new harness, no retries).

Two `[[test]]` targets (`viewer_philosophies`, `viewer_philosophies_invariants`) were
added to `crates/cli/Cargo.toml` (mirroring the `viewer_persistent_left_nav` /
`viewer_persistent_left_nav_invariants` pair) so the workspace-root
`tests/acceptance/viewer_philosophies*.rs` are discoverable — the only build-config
change. No new crate; the workspace stays at 21 members.

## What is missing today (the RED cause)

- **No `GET /philosophies` route exists.** The `adapter-http-viewer` route table
  (`crates/adapter-http-viewer/src/lib.rs:335–440`) has arms for `/`, `/claims`,
  `/peer-claims`, `/search`, `/score`, `/project`, `/philosophy` (the slice-10
  traversal surface — DISTINCT from the new vocabulary list), `/peers`, `/scrape`,
  and `/claims/{cid}`. A `GET /philosophies` falls through to `_ => not_found()`
  (line 438) — the terse `404 Not Found` route-miss. So VP-1 / VP-3 / VP-4 /
  VP-INV-NoControl / VP-INV-Offline all fail at their FIRST assertion (`status == 200`,
  observed left = `404`) → MISSING_FUNCTIONALITY. There is no read-only vocabulary
  page to scan, so the content/description/traversal-href/no-control assertions never
  run against a real body (the status gate fires first — correct RED discipline, never
  a silent pass on an empty 404 body).
- **`LANDING_HUB_SURFACES` carries no `Philosophies` entry.** The slice-21 nav SSOT
  (`crates/viewer-domain/src/common.rs:158`) holds exactly 8 `(label, url)` pairs
  (My Claims, Peer Claims, Project Survey, Philosophy Survey, Contributor Score,
  Network Search, Live Scrape, Peer Subscriptions) — none is `/philosophies`. Note
  "Philosophy Survey" → `/philosophy` (the slice-10 TRAVERSAL surface) is present, but
  the NEW vocabulary list `/philosophies` is not. The persistent nav derives its item
  set SOLELY from this table (`render_viewer_nav_links`, common.rs:209), so NO route's
  nav links `href="/philosophies"` today. VP-2 (on `/claims`), VP-5 (on `/claims`),
  and VP-INV-SingleSource (on every one of the 8 shipped routes) fail on the absent
  nav link → MISSING_FUNCTIONALITY.

## Classification key

- **RED (MISSING_FUNCTIONALITY, assertion)** ✅ — the assertion fires because the
  `/philosophies` route + the `Philosophies` nav SSOT entry are unimplemented.
  Correct RED. **All 8 slice-27 scenarios are this category.**
- **GREEN-today (no-regression / invariant guardrail)** 🟢 — none this slice (the
  surface is wholly new; there is no pre-existing behaviour to pin green). VP-5's
  no-regression FLOOR (`assert_landing_links_all_surfaces` — the prior 8 survive)
  passes today, but VP-5's load-bearing assertion (the ADDED `/philosophies` link) is
  RED, so the scenario as a whole is RED.
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain.**

## Tally

| File | Scenario | AC | Panic line | Classification | Why |
|---|---|---|---|---|---|
| `viewer_philosophies.rs` | VP-1 `the_philosophies_surface_lists_the_vocabulary_with_traversal_links` (WS) | AC-006.1 | 165 | RED ✅ | `GET /philosophies` → 404 (no route); `status == 200` fails (left=404) — the vocabulary page + memory-safety entry (name/description/traversal href) do not exist |
| | VP-2 `the_persistent_nav_links_philosophies_on_every_page_and_marks_it_active` | AC-006.2 | 227 | RED ✅ | the nav on `/claims` (a 200 surface today) carries no `href="/philosophies"` — the SSOT has no Philosophies entry |
| | VP-3 `the_philosophies_surface_exposes_no_authoring_control` | AC-006.1 (I-VIEW-1/3) | 275 | RED ✅ | `status == 200` fails (left=404) — no read-only surface exists to scan for the absence of authoring controls |
| | VP-4 `the_philosophies_surface_lists_the_complete_seed_vocabulary` | AC-006.1 (offline) | 315 | RED ✅ | `status == 200` fails (left=404) — the full `seeds()` vocabulary (12 entries, count == `seeds().len()`) is not listed |
| | VP-5 `adding_the_philosophies_nav_entry_drops_no_existing_surface` | AC-006.2 | 389 | RED ✅ | the prior-8 no-regression floor passes, but the ADDED `href="/philosophies"` nav link is absent (SSOT has no entry) |
| `viewer_philosophies_invariants.rs` | VP-INV-NoControl `the_philosophies_surface_adds_no_executable_control` (CARDINAL) | AC-006.1 / I-VIEW-1/3 | 103 | RED ✅ | `status == 200` fails (left=404) — no read-only surface to scan for `<form>`/`<button>`/mutating `hx-*` absence |
| | VP-INV-Offline `the_philosophies_surface_stays_offline_with_no_external_asset` | AC-006.1 / I-VIEW-3 | 144 | RED ✅ | `status == 200` fails (left=404) — no surface to prove references no external CDN host |
| | VP-INV-SingleSource `the_philosophies_nav_link_holds_on_every_route_from_one_ssot` | AC-006.2 | 190 | RED ✅ | no route's persistent nav links `/philosophies` (the SSOT has no entry) — asserted over all 8 shipped routes, fails on the first (`/`) |

### Numeric summary (slice-27 scenarios only; excludes the 2 pre-existing `support::state_delta` framework self-tests bundled in EACH acceptance binary)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion; route + nav SSOT entry unimplemented) | 8 |
| GREEN-today (no-regression / invariant guardrail) | 0 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-27 tests** | **8** |

RED total = **8**, all assertion-RED. Zero GREEN-today, zero BROKEN. Observed runner
output: `viewer_philosophies` → `test result: FAILED. 2 passed; 5 failed` (the 2
passes are `support::state_delta::tests::*` framework self-tests, NOT slice-27
scenarios — the 5 VP-* appear in the `failures:` list at lines 165/227/275/315/389);
`viewer_philosophies_invariants` → `test result: FAILED. 2 passed; 3 failed` (same 2
framework self-tests; the 3 VP-INV-* fail at lines 103/144/190).

## Gate verdict

**PASS.** Every failing test fails for the RIGHT reason (MISSING_FUNCTIONALITY — the
read-only `GET /philosophies` vocabulary route and the `("Philosophies",
PHILOSOPHIES_URL)` `LANDING_HUB_SURFACES` entry do not exist yet; a GET today hits the
terse 404 route-miss and the nav SSOT holds only the prior 8 surfaces). Zero tests are
in category 2 (IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE — both binaries compile
`Finished`, spawn the real bin, and the only non-`support` import is the SHIPPED
`lexicon::philosophy` oracle) or category 3 (WRONG_ASSERTION / internal-struct coupling
— every assertion scans the OBSERVABLE HTTP status + rendered body, never a
`viewer-domain`/`lexicon` struct field; `seeds()`/`object_id()` are read only to
COMPUTE expected surface strings). Handoff to DELIVER is UNBLOCKED for slice-27.

## Error/edge ratio note

8 scenarios: VP-1 (WS happy vocabulary listing) + VP-2 (happy nav reach + active) = 2
pure-happy; VP-3 (read-only no-control boundary) + VP-4 (offline completeness boundary)
+ VP-5 (no-regression / single-source boundary) + VP-INV-NoControl (CARDINAL read-only
invariant) + VP-INV-Offline (offline invariant) + VP-INV-SingleSource (single-source
nav invariant) = 6 non-pure-happy = **75%** (≥40% target). The load-bearing invariants
of a read-only viewer surface (I-VIEW-1/3 no authoring control; offline/no-CDN; the
slice-21 single-source nav) are covered explicitly, example-based per Mandate 11 (no
PBT at layer 3 — the pure `seeds()`→HTML projection is property-tested at layer 1/2 in
`crates/viewer-domain` by DELIVER: every seed renders its name + description +
`/philosophy?object=<object-id>` traversal href).

## Outcomes-registry note

Skipped — `docs/product/outcomes/registry.yaml` does not exist and the prior philosophy
slices (22–26) registered no OUT-N rows. Following that precedent, no outcome is
registered for the viewer vocabulary surface. If the registry is later adopted for this
feature, register the `/philosophies` render as a `kind: operation` (a read-only
driving-port surface over the embedded vocabulary) at that time.

## DELIVER pointers (from the observed RED)

1. **Mint `PHILOSOPHIES_URL` + a pure vocabulary renderer in `viewer-domain`.** Add a
   `pub const PHILOSOPHIES_URL: &str = "/philosophies";` (held in ONE place, like the
   other surface route consts in `common.rs` / `peers.rs` / `traversal.rs`) and a new
   `crates/viewer-domain/src/philosophies.rs` module mirroring the READ-ONLY list-surface
   shape of `peers.rs` (slice-15): a pure `render_philosophies_page(...) -> String`
   routed through `page_shell("OpenLore — Philosophies", PHILOSOPHIES_URL, body)` so it
   inherits the persistent nav + `#viewer-main` chrome and its own active marker. The
   body renders one entry per `lexicon::philosophy::seeds()` record: the `name`, the
   `description`, and a traversal link built via the EXISTING
   `common::href_philosophy(object_id(&seed.name))` (= `/philosophy?object=<object-id>` —
   object-ids are all-unreserved, so the encoded href is byte-exact). Render NO
   `<form>`/`<button>`/mutating `hx-*` (read-only, I-VIEW-1/3).
2. **(a) Register the route arm in `adapter-http-viewer`.** In the `route` fn's `match
   path` (`crates/adapter-http-viewer/src/lib.rs`, currently ~line 414–424, alongside the
   `PHILOSOPHY_URL => philosophy_page(...)` and `PEERS_URL => peers_page(...)` arms), add
   `PHILOSOPHIES_URL => philosophies_page(...)`. The handler is store-INDEPENDENT (a pure
   projection of the embedded `seeds()` — no `StoreReadPort` read, no `.await`, no signing
   key): the simplest form is `html_ok(render_philosophies_page(shape))` (mirror the
   `"/scrape" => html_ok(render_scrape_page(&ScrapeState::Form))` arm at line 432 for a
   store-free surface). Fork by `Shape` only if a `#philosophies` htmx fragment is wanted;
   the ACs need only the full page, so a full-page-only render is sufficient.
3. **(b) Add the nav SSOT entry.** Append `("Philosophies", PHILOSOPHIES_URL)` to
   `LANDING_HUB_SURFACES` (`crates/viewer-domain/src/common.rs:158`). This is the SINGLE
   change that satisfies AC-006.2 for VP-2 / VP-5 / VP-INV-SingleSource: the persistent
   nav (and the slice-17 landing hub, which reads the SAME table) then links
   `/philosophies` on EVERY page, and `page_shell(..., active = PHILOSOPHIES_URL, ...)`
   marks it `aria-current="page"` on `/philosophies` (the `render_viewer_nav_links`
   active-marker mechanism, common.rs:209, is unchanged — it keys off the `active` arg
   equalling the item's url, so NO separate OOB-nav / `render_viewer_nav` edit is needed
   beyond the SSOT entry + passing the const as `active`). NOTE: the frozen support helper
   `assert_landing_links_all_surfaces` iterates a support-side `LANDING_TOP_LEVEL_SURFACES`
   const, which pins the PRIOR set — VP-5 relies on that helper staying the frozen 8-set
   (it checks the 8 survive; the 9th is a separate assertion), so DELIVER need NOT touch
   the support harness. The count-of-nav-items unit/property tests in `viewer-domain`
   WILL need their expected count bumped 8→9 (DELIVER, layer 1/2).

## Upstream gaps for DELIVER to resolve

- **(d) `viewer-domain` does NOT yet depend on `lexicon` (FLAG — new dependency edge
  required).** `crates/viewer-domain/Cargo.toml` `[dependencies]` are `maud`, `ports`,
  `appview-domain`, `scoring`, `claim-domain` — `lexicon` is ABSENT. The pure
  `render_philosophies_page` must read `lexicon::philosophy::{seeds, object_id}`, so
  DELIVER MUST add `lexicon = { path = "../lexicon" }` to `viewer-domain/Cargo.toml`.
  This is a PURE → PURE edge (lexicon's `philosophy` module is offline — embedded
  `include_str!` seeds, no I/O), so it must be ALLOWLISTED in the `xtask check-arch`
  pure-core arm exactly as the existing `viewer-domain → appview-domain` / `scoring` /
  `claim-domain` pure-→pure edges are (see the rationale comments at
  `viewer-domain/Cargo.toml:30–52`). Confirm `cargo xtask check-arch` stays 21 members /
  no new crate after the edge is added. (The slice-27 ATs themselves do NOT need this
  edge — `cli` already depends on `lexicon`; the edge is a PRODUCTION requirement for the
  renderer.)
- **(b) Route-registration site is confirmed (see pointer 2).** The exact insertion is
  the `route` fn `match path` in `crates/adapter-http-viewer/src/lib.rs` (~line 414–424),
  between the `PHILOSOPHY_URL` and `PEERS_URL` arms — mirror the store-free `"/scrape"`
  arm (line 432) since `/philosophies` reads no store.
- **(c) Active-marker mechanism is confirmed (see pointer 3).** `/philosophies` needs NO
  bespoke `render_viewer_nav` / OOB-nav edit: adding it to `LANDING_HUB_SURFACES` +
  passing `PHILOSOPHIES_URL` as the `page_shell(active=…)` argument is sufficient for the
  `aria-current="page"` marker (the existing `render_viewer_nav_links` keys the marker off
  `*url == active`). If a boosted `/philosophies` navigation is later wanted, the existing
  `append_oob_nav_items_if_boosted` (lib.rs:444) already handles the OOB active-marker
  update generically off the path — no per-surface work.
- **Scope confirmation (SEED vocabulary only).** The ACs (AC-006.1/.2) and DESIGN row 27
  are about the VOCABULARY listing; the embedded `seeds()` ARE the vocabulary (the same
  set the SHIPPED CLI `philosophy list` renders, slice-22). MINTED-philosophy records (the
  slice-24 `philosophies` table) in the viewer are a documented FOLLOW-UP — OUT of scope,
  keeping the surface pure/offline (no `StoreReadPort` read, no signing key in the web
  process; I-VIEW-1/3). DELIVER should NOT add a store read to `/philosophies` this slice.
