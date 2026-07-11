//! Slice-27 acceptance — the read-only `GET /philosophies` VIEWER VOCABULARY surface
//! (US-PV-006, job_id J-002; ADR-059 §5 row 27). The LAST slice of
//! `philosophy-vocabulary-registry` — it closes the feature by surfacing the shared
//! philosophy vocabulary in the read-only viewer, mirroring the CLI `philosophy list`
//! (slice-22, SHIPPED) as an HTTP surface.
//!
//! What slice-27 adds (per ADR-059 §5 row 27):
//!   • A NEW read-only route `GET /philosophies` in `adapter-http-viewer` that renders
//!     the embedded philosophy vocabulary — each philosophy's NAME + DESCRIPTION + a
//!     link to the EXISTING `/philosophy?object=<object-id>` traversal surface
//!     (slice-10, `PHILOSOPHY_URL`). The renderer is a PURE function over
//!     `lexicon::philosophy::seeds()` (offline — no store read, no network; I-VIEW-3).
//!   • A NEW `("Philosophies", PHILOSOPHIES_URL)` entry appended to the slice-21
//!     `LANDING_HUB_SURFACES` SSOT (`viewer-domain::common`), so the persistent left
//!     nav (slice-21, SHIPPED) renders a link to `/philosophies` on EVERY viewer page
//!     and marks it `aria-current="page"` when the operator is on it (AC-006.2).
//!
//! Read-only / offline / no-key (I-VIEW-1/3): the surface renders NO authoring control
//! (no mint/edit/compose `<form>`, no `<button>`, no mutating `hx-*`) — the viewer holds
//! no signing key in the web process. Minting a philosophy stays the slice-24 `openlore
//! philosophy add` CLI action. The `/philosophies` surface lists the SEED vocabulary
//! only; MINTED-philosophy records (the slice-24 `philosophies` table) in the viewer are
//! a documented FOLLOW-UP — OUT of scope this slice (keeps the surface pure/offline, no
//! store read). AC-006.1/.2 are about the VOCABULARY listing, which the embedded seeds
//! are.
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET — full page (`get`). NO scenario calls
//! the `viewer-domain` render fns or `lexicon` resolvers directly to PRODUCE the surface
//! (those are unit-level, DELIVER); `lexicon::philosophy::seeds()` is read in-test ONLY
//! as the ORACLE for the expected vocabulary (the completeness assertion), never as the
//! system under test. The LOCAL DuckDB store is REAL (own claims via the real slice-06
//! `claim add` verb) so the OTHER surfaces used for the nav-reach assertions render
//! genuine full pages (Pillar 3) — the `/philosophies` render itself is store-independent.
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every scenario
//! is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only (Mandate 9/11). The
//! surface is a render-only projection of a fixed 12-seed vocabulary over a static nav
//! SSOT; there is no ≥3-scenario chained journey over a domain-rich state machine, so
//! Tier B (state-machine PBT) is NOT warranted (Mandate 10 skip criteria). The pure
//! `seeds()` → HTML projection is property-tested at layer 1/2 in `viewer-domain` by
//! DELIVER (every seed renders its name + description + traversal href).
//!
//! Build-before-run note (mirrors slice-08/16/21): `cargo test` does NOT rebuild a
//! spawned binary automatically — the run MUST `cargo build --bin openlore` before
//! running these ATs so `ViewerServer::start` spawns the CURRENT viewer.
//!
//! Mandate 7 RED scaffolds: the ATs import nothing unbuilt at the Rust level (they spawn
//! the bin + HTTP, and read `lexicon::philosophy::seeds()` which already ships), so they
//! COMPILE now. The RED is the PRODUCTION surface: no `/philosophies` route exists (a GET
//! today falls through to the terse `not_found()` → 404), and `LANDING_HUB_SURFACES`
//! carries no `Philosophies` entry — so every status/content/nav-link/active-marker
//! assertion FAILS for the RIGHT reason (MISSING_FUNCTIONALITY), NOT a setup/import
//! error. They stay RED until DELIVER's per-scenario RED→GREEN→COMMIT cycles (ADR-025).
//!
//! Covers (US-PV-006):
//! - VP-1 (WS, AC-006.1): `GET /philosophies` → 200 read-only vocabulary listing (the
//!   memory-safety entry: name + verbatim description fragment + `/philosophy?object=…`
//!   traversal link).
//! - VP-2 (AC-006.2): `/philosophies` is reachable from the persistent nav (a
//!   `LANDING_HUB_SURFACES` entry) on every page, and marked active on `/philosophies`.
//! - VP-3 (AC-006.1 read-only): the surface carries NO authoring control (no
//!   mint/edit/compose form — I-VIEW-1/3).
//! - VP-4 (AC-006.1 offline/completeness): the FULL seed vocabulary is listed (every
//!   `seeds()` name + its traversal href; count == `seeds().len()`) — proves it reads
//!   the vocabulary, not a hardcoded subset, served with no network dependency.
//! - VP-5 (no-regression): adding the Philosophies nav entry drops NO existing surface
//!   (the prior 8 nav links survive) — guards the slice-21 single-source invariant.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// The embedded philosophy vocabulary is the ORACLE for the completeness assertion
// (VP-4). `lexicon` already ships (a `cli` dependency); reading `seeds()` here is a
// pure, offline expectation-source, NOT the system under test (which is the real
// `openlore ui` HTTP surface).
use lexicon::philosophy::{object_id, seeds};

// -----------------------------------------------------------------------------
// Observable production tokens (SSOT for the OBSERVABLE rendered surface slice-27
// introduces; all ABSENT today → the assertions that scan for them are RED for the
// RIGHT reason). DELIVER mints the `PHILOSOPHIES_URL` route const + the
// `("Philosophies", PHILOSOPHIES_URL)` `LANDING_HUB_SURFACES` entry that render these.
// -----------------------------------------------------------------------------

/// The NEW read-only viewer vocabulary route (ADR-059 §5 row 27 — `GET /philosophies`).
/// DELIVER mints `PHILOSOPHIES_URL` in `viewer-domain::common` (held in ONE place, like
/// the other surface route consts) and adds the route arm in `adapter-http-viewer`.
const PHILOSOPHIES_URL: &str = "/philosophies";
/// The persistent-nav link to `/philosophies` — present on EVERY page once the surface
/// is a `LANDING_HUB_SURFACES` entry (AC-006.2). ABSENT today (the SSOT has 8 entries,
/// none is `/philosophies`).
const PHILOSOPHIES_NAV_LINK: &str = "href=\"/philosophies\"";
/// The neutral, semantic current-surface marker the persistent nav applies to the item
/// whose url equals the active surface (slice-21 / ADR-058 D2 — AC-006.2 active state).
const ARIA_CURRENT: &str = "aria-current=\"page\"";

/// The first embedded seed's NAME — a hard-pinned vocabulary anchor (seeds.json). The
/// vocabulary listing must render it verbatim.
const MEMORY_SAFETY_NAME: &str = "memory-safety";
/// A VERBATIM fragment of the `memory-safety` seed's description (seeds.json) — a stable
/// substring carrying no HTML-escapable characters, so it renders byte-verbatim through
/// maud's text escape. Proves the DESCRIPTION (not just the name) is surfaced.
const MEMORY_SAFETY_DESC_FRAGMENT: &str = "no use-after-free, no dangling pointers";
/// The traversal `href` the `memory-safety` entry must link to — the EXISTING slice-10
/// `/philosophy?object=<object-id>` surface (ADR-044). The object-id
/// `org.openlore.philosophy.memory-safety` is all-unreserved, so `encode_query_component`
/// is a no-op and the href is byte-exact.
const MEMORY_SAFETY_TRAVERSAL_HREF: &str =
    "href=\"/philosophy?object=org.openlore.philosophy.memory-safety\"";
/// The `memory-safety` seed's aliases line (slice-32 — viewer parity with the slice-31
/// CLI `philosophy list`): the entry surfaces the shorthand strings that triangulation
/// resolves (`aliases: mem-safety, memory-safe`), as bare TEXT — never a link or an
/// NSID object-id, so the traversal href above is untouched. The unambiguous `mem-safety`
/// alias is the discoverability anchor (seeds.json).
const MEMORY_SAFETY_ALIASES_LINE: &str = "aliases: mem-safety, memory-safe";
/// The per-entry traversal-link marker (`?object=org.openlore.philosophy.<segment>`). One
/// occurrence per listed philosophy — the persistent nav's `Philosophy Survey` link is
/// bare `/philosophy` (no `?object=`), so this counts LISTED entries only, never the nav.
const PHILOSOPHY_OBJECT_LINK: &str = "?object=org.openlore.philosophy.";

/// An EXISTING 200 surface that renders the persistent nav today (slice-21 SHIPPED) — the
/// vantage for the nav-reach assertions (VP-2 / VP-5) that must see the NEW Philosophies
/// link appear alongside the prior 8.
const AN_EXISTING_NAV_SURFACE: &str = "/claims";

/// Build the traversal `href` attribute a listed philosophy entry must carry for `name`
/// — the EXISTING `/philosophy?object=<object-id>` surface (mirrors the production
/// `href_philosophy(object_id(name))`; object-ids are all-unreserved so no encoding).
fn traversal_href_for(name: &str) -> String {
    format!("href=\"/philosophy?object={}\"", object_id(name))
}

// =============================================================================
// US-PV-006 — the read-only philosophy vocabulary surface. (VP-1 walking skeleton ·
// VP-2 nav reach + active · VP-3 read-only · VP-4 completeness · VP-5 no-regression)
// =============================================================================

/// VP-1 / WALKING SKELETON (US-PV-006; AC-006.1 — the thinnest complete thread the slice
/// can demo end-to-end): the operator opens `GET /philosophies` and sees the shared
/// philosophy vocabulary rendered READ-ONLY — each philosophy's NAME + DESCRIPTION + a
/// link to the existing `/philosophy?object=<object-id>` traversal surface. The
/// load-bearing user outcome: "browse the shared vocabulary in the viewer, and click
/// through to any philosophy's survey."
///
/// Given the viewer is running;
/// When she opens `/philosophies` (full page);
/// Then it renders a 200 vocabulary listing containing the `memory-safety` entry — its
///   name, a verbatim description fragment, and a `/philosophy?object=…memory-safety`
///   traversal link.
///
/// @us-pv-006 @j-002 @walking_skeleton @driving_port @driving_adapter @real-io @happy
#[test]
fn the_philosophies_surface_lists_the_vocabulary_with_traversal_links() {
    // GIVEN a REAL store with genuine content (own claims via the production `claim add`
    // verb — Pillar 3) and the REAL `openlore ui` viewer over it. The `/philosophies`
    // render is store-independent (pure over `seeds()`), but seeding keeps the harness
    // identical to the sibling viewer suites.
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 3);
    let viewer = ViewerServer::start(&env);

    // WHEN she opens the vocabulary surface as a full page.
    let response = viewer.get(PHILOSOPHIES_URL);

    // THEN it is a 200 full page (RED today — no `/philosophies` route → terse 404).
    assert_eq!(
        response.status, 200,
        "VP-1 (AC-006.1): GET {PHILOSOPHIES_URL} must render a 200 read-only vocabulary \
         page; body:\n{}",
        response.body
    );
    assert!(
        response.is_full_page(),
        "VP-1 (AC-006.1): GET {PHILOSOPHIES_URL} (no-JS) must be a COMPLETE full page \
         carrying the viewer chrome; body:\n{}",
        response.body
    );
    // …listing the `memory-safety` philosophy by NAME (RED today).
    assert!(
        response.body.contains(MEMORY_SAFETY_NAME),
        "VP-1 (AC-006.1): the vocabulary listing must render the {MEMORY_SAFETY_NAME:?} \
         philosophy's NAME; body:\n{}",
        response.body
    );
    // …with its DESCRIPTION surfaced (a verbatim fragment — RED today).
    assert!(
        response.body.contains(MEMORY_SAFETY_DESC_FRAGMENT),
        "VP-1 (AC-006.1): the {MEMORY_SAFETY_NAME:?} entry must render its DESCRIPTION \
         (fragment {MEMORY_SAFETY_DESC_FRAGMENT:?}); body:\n{}",
        response.body
    );
    // …and a LINK to the existing `/philosophy?object=<object-id>` traversal surface
    // (RED today — the entry, and its link, do not exist yet).
    assert!(
        response.body.contains(MEMORY_SAFETY_TRAVERSAL_HREF),
        "VP-1 (AC-006.1): the {MEMORY_SAFETY_NAME:?} entry must link to its traversal \
         surface ({MEMORY_SAFETY_TRAVERSAL_HREF}); body:\n{}",
        response.body
    );
}

/// slice-32 (viewer parity with the slice-31 CLI `philosophy list`): the read-only
/// `/philosophies` surface SURFACES each seed's aliases, so the shorthand strings that
/// power triangulation (D4) are discoverable in the browser where the reader is already
/// browsing the vocabulary (D5). Aliases render as bare TEXT (`aliases: mem-safety,
/// memory-safe`), never as links nor NSID object-ids — the traversal href stays the
/// name-only link, so the slice-27 object-link contract is untouched.
///
/// Given the viewer is running;
/// When she opens `/philosophies`;
/// Then the `memory-safety` entry carries an `aliases:` label listing the unambiguous
///   `mem-safety` alias.
///
/// @us-pv-006 @j-002 @j-004 @driving_port @driving_adapter @real-io @aliases @happy
#[test]
fn the_philosophies_surface_surfaces_seed_aliases() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 3);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PHILOSOPHIES_URL);

    assert_eq!(
        response.status, 200,
        "GET {PHILOSOPHIES_URL} must render a 200 read-only vocabulary page; body:\n{}",
        response.body
    );
    // The vocabulary browse surface must surface the alias strings (RED until the entry
    // renderer appends the `aliases:` line — the slice-31 CLI line ported to the viewer).
    assert!(
        response.body.contains(MEMORY_SAFETY_ALIASES_LINE),
        "slice-32: the {MEMORY_SAFETY_NAME:?} entry must surface its aliases line \
         ({MEMORY_SAFETY_ALIASES_LINE:?}) so the shorthand is discoverable in the browser; \
         body:\n{}",
        response.body
    );
    // …and it stays TEXT, never a link/object-id: the aliases must NOT be wrapped in an
    // `?object=…mem-safety` traversal link (the traversal href is the name-only link, so
    // the slice-27 one-link-per-seed contract is preserved).
    assert!(
        !response
            .body
            .contains("?object=org.openlore.philosophy.mem-safety"),
        "slice-32: aliases must render as bare TEXT, never an object-id traversal link \
         (found a mem-safety `?object=` link, which would break the slice-27 \
         one-link-per-seed contract); body:\n{}",
        response.body
    );
}

/// VP-2 (US-PV-006; AC-006.2 — the persistent-nav reachability guarantee): `/philosophies`
/// is reachable AS A SURFACE from the persistent nav (slice-21) — it is a
/// `LANDING_HUB_SURFACES` entry, so the nav renders a link to it on EVERY viewer page; and
/// when the operator is ON `/philosophies` the nav marks that item `aria-current="page"`.
///
/// Given the viewer is running;
/// When she loads an existing surface (`/claims`) and then `/philosophies`;
/// Then the nav on `/claims` links `/philosophies` (reachable from every page), and the
///   `/philosophies` page marks the current surface active.
///
/// @us-pv-006 @j-002 @driving_port @real-io @nav-reach @nav-active @happy
#[test]
fn the_persistent_nav_links_philosophies_on_every_page_and_marks_it_active() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    // WHEN she is on an EXISTING surface — the persistent nav (rendered on every page,
    // slice-21) must now carry a link to the NEW `/philosophies` surface (AC-006.2).
    let existing = viewer.get(AN_EXISTING_NAV_SURFACE);
    assert_eq!(
        existing.status, 200,
        "VP-2: GET {AN_EXISTING_NAV_SURFACE} must be 200; body:\n{}",
        existing.body
    );
    // THEN the nav links `/philosophies` (RED today — the SSOT has no Philosophies entry).
    assert!(
        existing.body.contains(PHILOSOPHIES_NAV_LINK),
        "VP-2 (AC-006.2): the persistent nav on {AN_EXISTING_NAV_SURFACE} must link the \
         Philosophies surface ({PHILOSOPHIES_NAV_LINK}) — a LANDING_HUB_SURFACES entry \
         reachable from every page; body:\n{}",
        existing.body
    );

    // WHEN she is ON `/philosophies`, the nav marks it the current surface.
    let current = viewer.get(PHILOSOPHIES_URL);
    assert_eq!(
        current.status, 200,
        "VP-2 (AC-006.2): GET {PHILOSOPHIES_URL} must be a 200 surface so the nav can \
         mark it active; body:\n{}",
        current.body
    );
    // THEN the philosophies surface renders its own nav link AND an active marker
    // (RED today — the surface is a 404, and no such nav item exists).
    assert!(
        current.body.contains(PHILOSOPHIES_NAV_LINK) && current.body.contains(ARIA_CURRENT),
        "VP-2 (AC-006.2): on {PHILOSOPHIES_URL} the nav must mark the current surface \
         active ({ARIA_CURRENT}) on the Philosophies item ({PHILOSOPHIES_NAV_LINK}); \
         body:\n{}",
        current.body
    );
}

/// VP-3 (US-PV-006; AC-006.1 — READ-ONLY, no authoring control, I-VIEW-1/3): the
/// `/philosophies` surface renders NO authoring affordance — no mint/edit/compose
/// `<form>`, no `<button>`, no mutating `hx-post`/`hx-put`/`hx-delete`. The viewer holds
/// no signing key in the web process; minting a philosophy stays the slice-24 `openlore
/// philosophy add` CLI action. The surface is a read-only projection.
///
/// Given the operator opens the philosophies surface;
/// When the page renders;
/// Then it carries no executable/mutating control (a read-only listing, not a compose UI).
///
/// @us-pv-006 @driving_port @real-io @read-only @no-control @boundary
#[test]
fn the_philosophies_surface_exposes_no_authoring_control() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PHILOSOPHIES_URL);
    // The surface must exist as a 200 read-only page (RED today — 404) before the
    // no-control scan runs over REAL surface content (never a silent pass on an empty
    // 404 body).
    assert_eq!(
        response.status, 200,
        "VP-3 (AC-006.1): GET {PHILOSOPHIES_URL} must render the read-only vocabulary \
         page (200) so the no-control scan runs over real content; body:\n{}",
        response.body
    );
    // THEN the page carries NO authoring / mutating control (I-VIEW-1/3). The
    // `/philosophies` surface has no content-region form of its own (unlike `/search` /
    // `/scrape`), and the persistent nav is plain links — so a whole-page scan is clean.
    let lowered = response.body.to_ascii_lowercase();
    for banned in ["<form", "<button", "hx-post", "hx-put", "hx-delete"] {
        assert!(
            !lowered.contains(banned),
            "VP-3 (AC-006.1 / I-VIEW-1/3): the read-only philosophies surface must carry \
             NO executable/mutating authoring control — found {banned:?}; body:\n{}",
            response.body
        );
    }
}

/// VP-4 (US-PV-006; AC-006.1 — OFFLINE completeness): the surface lists the FULL embedded
/// philosophy vocabulary, not a hardcoded subset — every `lexicon::philosophy::seeds()`
/// entry appears by NAME and links its `/philosophy?object=<object-id>` traversal surface,
/// and the count of traversal links equals `seeds().len()`. This proves the renderer READS
/// the shared vocabulary (offline — pure over the embedded seeds, no store/network), so a
/// later-added seed surfaces automatically.
///
/// Given the viewer is running;
/// When she opens `/philosophies`;
/// Then every seed philosophy is listed (name + traversal href) and the entry count equals
///   the embedded vocabulary size.
///
/// @us-pv-006 @j-002 @driving_port @real-io @offline @completeness @boundary
#[test]
fn the_philosophies_surface_lists_the_complete_seed_vocabulary() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PHILOSOPHIES_URL);
    assert_eq!(
        response.status, 200,
        "VP-4 (AC-006.1): GET {PHILOSOPHIES_URL} must be a 200 vocabulary page; body:\n{}",
        response.body
    );

    // THEN EVERY embedded seed is listed — name + its traversal href (RED today: no
    // entry exists). `seeds()` is the offline ORACLE (the same embedded vocabulary the
    // production renderer must read), not the SUT.
    let vocabulary = seeds();
    assert!(
        vocabulary.len() >= 12,
        "VP-4 sanity: the embedded vocabulary must carry the full seed set (>= 12); got {}",
        vocabulary.len()
    );
    for seed in &vocabulary {
        assert!(
            response.body.contains(&seed.name),
            "VP-4 (AC-006.1): the listing must render the {:?} philosophy by name; body:\n{}",
            seed.name, response.body
        );
        let href = traversal_href_for(&seed.name);
        assert!(
            response.body.contains(&href),
            "VP-4 (AC-006.1): the {:?} entry must link its traversal surface ({href}); \
             body:\n{}",
            seed.name, response.body
        );
    }
    // …and the surface lists EXACTLY the vocabulary — the count of per-entry traversal
    // links equals `seeds().len()`, so it is neither a subset nor padded with extras
    // (the nav's bare `/philosophy` link carries no `?object=`, so it is not counted).
    let listed = response.body.matches(PHILOSOPHY_OBJECT_LINK).count();
    assert_eq!(
        listed,
        vocabulary.len(),
        "VP-4 (AC-006.1): the surface must list EXACTLY the {} embedded philosophies \
         (one `{PHILOSOPHY_OBJECT_LINK}` traversal link each), got {listed}; body:\n{}",
        vocabulary.len(),
        response.body
    );
}

/// VP-5 (US-PV-006; AC-006.2 — no-regression / single-source guard): appending the
/// `Philosophies` entry to the slice-21 `LANDING_HUB_SURFACES` SSOT ADDS the new nav link
/// WITHOUT dropping any existing surface — the prior 8 nav links still render on every
/// page, now alongside `/philosophies`. Guards the slice-21 single-source invariant (the
/// nav item set is derived SOLELY from `LANDING_HUB_SURFACES`; adding a 9th surface must
/// not perturb the other 8).
///
/// Given the viewer serves an existing surface;
/// When the persistent nav renders;
/// Then all prior surface links survive AND the new Philosophies link is additionally
///   present.
///
/// @us-pv-006 @driving_port @real-io @no-regression @single-source @boundary
#[test]
fn adding_the_philosophies_nav_entry_drops_no_existing_surface() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(AN_EXISTING_NAV_SURFACE);
    assert_eq!(
        response.status, 200,
        "VP-5: GET {AN_EXISTING_NAV_SURFACE} must be 200; body:\n{}",
        response.body
    );
    // The prior surface links all survive (GREEN-shaped no-regression anchor: the
    // pre-slice-27 nav surfaces are still linked — the shared `assert_landing_links_all_
    // surfaces` helper pins the established set).
    assert_landing_links_all_surfaces(&response.body);
    // …AND the new Philosophies link is now additionally present (RED today — not yet in
    // the SSOT). The feature ONLY adds a surface; it removes none.
    assert!(
        response.body.contains(PHILOSOPHIES_NAV_LINK),
        "VP-5 (AC-006.2): the persistent nav must ADD the Philosophies link \
         ({PHILOSOPHIES_NAV_LINK}) without dropping any prior surface (single-source \
         LANDING_HUB_SURFACES); body:\n{}",
        response.body
    );
}
