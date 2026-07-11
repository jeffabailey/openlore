//! `/philosophies` — the read-only philosophy VOCABULARY surface (slice-27;
//! ADR-059 §5 row 27 / US-PV-006). The LAST slice of
//! `philosophy-vocabulary-registry`: it surfaces the shared philosophy vocabulary
//! in the read-only viewer, mirroring the CLI `philosophy list` (slice-22) as an
//! HTTP surface.
//!
//! PURE + OFFLINE (I-VIEW-3): the surface is a total function over the embedded
//! `lexicon::philosophy::seeds()` vocabulary — NO store read, NO network, NO
//! signing key. Each philosophy renders its NAME + DESCRIPTION + a link to the
//! EXISTING `/philosophy?object=<object-id>` traversal survey (slice-10,
//! [`PHILOSOPHY_URL`]). The traversal href is built by REUSING the shared
//! [`href_philosophy`] + `lexicon::philosophy::object_id` — never a hardcoded
//! `/philosophy` path or NSID prefix (one source of truth for both).
//!
//! READ-ONLY / no authoring control (I-VIEW-1/3): the surface renders NO
//! mint/edit/compose `<form>`, NO `<button>`, NO mutating `hx-*` — the viewer
//! holds no signing key in the web process. Minting a philosophy stays the
//! slice-24 `openlore philosophy add` CLI action. This mirrors the read-only
//! shape of `peers.rs` (guidance/links only), and — like `render_scrape_page` —
//! the effect shell serves it STORE-FREE.

use super::*;

/// The real route the read-only philosophy VOCABULARY surface is served at
/// (`/philosophies`) — the no-JS `href`, and the persistent-nav link all reference
/// this one path (ADR-059 §5 row 27: one source of truth for the philosophies
/// route). Held in ONE place, like the other surface route consts
/// ([`PHILOSOPHY_URL`], [`PEERS_URL`], [`SCRAPE_URL`]), so the references can never
/// drift apart. DISTINCT from the slice-10 [`PHILOSOPHY_URL`] (`/philosophy`) — that
/// is the per-object traversal survey; this is the vocabulary INDEX linking into it.
pub const PHILOSOPHIES_URL: &str = "/philosophies";

/// Render the read-only philosophy VOCABULARY page (`GET /philosophies`,
/// US-PV-006 / AC-006.1) as a complete HTML document (maud). PURE + OFFLINE: a
/// total function over the embedded [`seeds`](lexicon::philosophy::seeds)
/// vocabulary — no I/O, no store read, no network. Lists EVERY seed philosophy's
/// NAME + DESCRIPTION + a link to its EXISTING `/philosophy?object=<object-id>`
/// traversal survey; because the listing is derived SOLELY from `seeds()`, a
/// later-added seed surfaces automatically (offline completeness, VP-4).
///
/// Renders NO authoring / mutating control (I-VIEW-1/3): no `<form>`, no
/// `<button>`, no `hx-post`/`hx-put`/`hx-delete` — only plain `<a href>` links.
/// Composed through [`page_shell`] (persistent left nav + `<main id="viewer-main">`);
/// `active = PHILOSOPHIES_URL` marks the Philosophies nav item current (AC-006.2).
pub fn render_philosophies_page() -> String {
    let body = html! {
        h1 { "Philosophies" }
        (render_philosophy_vocabulary())
    };
    page_shell("OpenLore — Philosophies", PHILOSOPHIES_URL, body)
}

/// Render the vocabulary list region — one attributed entry per embedded seed
/// philosophy (PURE total function over [`seeds`](lexicon::philosophy::seeds)). Each
/// entry is [`render_philosophy_entry`]; the list is derived SOLELY from the
/// embedded vocabulary so it is neither a subset nor padded (VP-4 completeness).
fn render_philosophy_vocabulary() -> Markup {
    html! {
        @for seed in lexicon::philosophy::seeds() {
            (render_philosophy_entry(&seed))
        }
    }
}

/// Render ONE philosophy vocabulary entry: its NAME, its DESCRIPTION (verbatim, via
/// maud's text auto-escape), a link to the EXISTING `/philosophy?object=<object-id>`
/// traversal survey (slice-10), and — when the seed carries any — an `aliases:` line
/// of its shorthand strings (slice-32, mirroring the CLI `philosophy list`; bare
/// comma-joined TEXT, never links/object-ids). PURE total
/// function. The traversal href REUSES the shared [`href_philosophy`] over the
/// DERIVED `lexicon::philosophy::object_id(name)` — never a hardcoded `/philosophy`
/// path or NSID prefix (object-ids are all-unreserved, so the percent-encode is a
/// no-op and the href is byte-exact with the slice-10 route). NO authoring control
/// (I-VIEW-1/3) — a plain read-only link into the survey.
fn render_philosophy_entry(seed: &lexicon::philosophy::Philosophy) -> Markup {
    let object_id = lexicon::philosophy::object_id(&seed.name);
    html! {
        section {
            h2 {
                a href=(href_philosophy(&object_id)) { (seed.name) }
            }
            p { (seed.description) }
            // slice-32 (viewer parity with the slice-31 CLI `philosophy list`): surface
            // the shorthand alias strings that triangulation resolves (D4) so they are
            // discoverable in the read-only browse (D5). Rendered ONLY when the seed has
            // aliases (no empty label otherwise) as bare comma-joined TEXT — never an `<a>`
            // link nor an NSID object-id, so the name-only traversal href above (and the
            // slice-27 one-link-per-seed contract) stay untouched.
            @if !seed.aliases.is_empty() {
                p { "aliases: " (seed.aliases.join(", ")) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// slice-32 (viewer parity with the slice-31 CLI `philosophy list`): EVERY seed
    /// philosophy's entry on the read-only `/philosophies` surface surfaces its
    /// `aliases: <joined>` line — the shorthand strings that power triangulation (D4)
    /// are discoverable where the reader is already browsing (D5). Bare comma-joined
    /// alias TEXT (mirroring the CLI `render_seed_block` convention), never `<a>` links
    /// nor NSID object-ids, so the slice-27 name→`/philosophy?object=` traversal href is
    /// untouched. A seed with NO aliases renders NO `aliases:` label (no empty line).
    #[test]
    fn every_seed_renders_its_aliases_on_the_philosophies_surface() {
        for seed in lexicon::philosophy::seeds() {
            let entry = render_philosophy_entry(&seed).into_string();
            if seed.aliases.is_empty() {
                assert!(
                    !entry.contains("aliases:"),
                    "the {:?} entry has no aliases and must render NO `aliases:` label; \
                     got:\n{entry}",
                    seed.name
                );
            } else {
                let expected = format!("aliases: {}", seed.aliases.join(", "));
                assert!(
                    entry.contains(&expected),
                    "the {:?} entry must surface its aliases line ({expected:?}); got:\n{entry}",
                    seed.name
                );
            }
        }
    }

    /// The no-alias branch pinned against a CONSTRUCTED alias-less record (all embedded
    /// seeds currently carry aliases, so this guards the empty-label guarantee directly):
    /// an aliasless philosophy renders its name + description but NO `aliases:` label.
    #[test]
    fn an_aliasless_philosophy_entry_renders_no_alias_label() {
        let bare = lexicon::philosophy::Philosophy {
            name: "no-alias-example".to_string(),
            description: "A philosophy that carries no alias strings.".to_string(),
            aliases: Vec::new(),
            see_also: Vec::new(),
        };
        let entry = render_philosophy_entry(&bare).into_string();
        assert!(
            entry.contains("no-alias-example") && !entry.contains("aliases:"),
            "an aliasless entry must render its name but NO empty `aliases:` label; got:\n{entry}"
        );
    }
}
