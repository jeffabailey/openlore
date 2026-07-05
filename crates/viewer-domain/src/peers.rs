//! `/peers` — the peer-subscription management (read-only) surface.

use super::*;

/// The HTML `id` of the `/peers` swap-target region (slice-15; the sibling of
/// slice-10's [`TRAVERSAL_RESULTS_ID`] + slice-08's [`SEARCH_RESULTS_ID`]). htmx
/// swaps the element whose id matches; the no-JS full page EMBEDS the SAME
/// `<div id="peers">` so the fragment and the full page's peers region are
/// byte-identical by construction (I-PS-5 parity). Held in ONE place.
pub const PEERS_REGION_ID: &str = "peers";

/// The real route the Peer Subscriptions view is served at (`/peers`) — the no-JS
/// `href`, any htmx `hx-get`, AND the nav link all reference this one path
/// (ADR-052: one source of truth for the peers route). Held in ONE place so the
/// references can never drift apart.
pub const PEERS_URL: &str = "/peers";

/// The render-only REVOCATION guidance prefix an ACTIVE peer row carries (slice-15
/// / DD-PS-6 / I-PS-1; the sibling of slice-08's [`SEARCH_FOLLOW_GUIDANCE_PREFIX`]):
/// the viewer surfaces the slice-03 `openlore peer remove <bare-did>` command as
/// TEXT so the operator can revoke the subscription FROM THE CLI. It is GUIDANCE
/// ONLY — there is NO executable remove/unsubscribe control and NO mutating swap;
/// unsubscribing stays a deliberate CLI action and the read-only viewer holds no
/// key. Held in ONE place (the SAME slice-03 verb the CLI emits) so the guidance is
/// a single source of truth + a single mutation site. The bare DID (the slice-03
/// `peer remove` verb's accepted form) is appended by [`render_remove_guidance`].
pub const PEER_REMOVE_GUIDANCE_PREFIX: &str =
    "Revoke this subscription from the CLI: openlore peer remove";

/// The render-only STARTING guidance prefix the guided [`PeersView::NoSubscriptions`]
/// empty state carries (slice-15 / US-PS-003): the viewer surfaces the slice-03
/// `openlore peer add <did>` command as TEXT so the operator learns how to start
/// subscribing — in-context, never a dead end. GUIDANCE ONLY (no executable
/// control). Held in ONE place (the SAME `openlore peer add` verb the slice-08
/// follow-guidance emits) so the empty-state command is a single source of truth.
pub const PEER_ADD_GUIDANCE_PREFIX: &str = "Subscribe to a peer from the CLI: openlore peer add";

/// The guided plain-language notice the [`PeersView::NoSubscriptions`] arm renders
/// when the operator has no active subscriptions (US-PS-003). Held in ONE place +
/// emitted as a fixed constant so emptiness is recognized as emptiness — never
/// blank, never an error.
pub const PEERS_NO_SUBSCRIPTIONS_NOTICE: &str = "You are not subscribed to any peers.";

/// The pure render input for the `/peers` Peer Subscriptions view (slice-15 /
/// DD-PS-5). An ADT so the renderer matches TOTALLY (nw-fp-domain-modeling §1): a
/// non-empty active set is `Subscriptions`; an empty one (or a store read failure
/// degraded in the shell) is the guided `NoSubscriptions`. The effect shell builds
/// this from the LOCAL active-subscription read via [`peers_view`]; the renderer is
/// a pure total function over it. The render is a TOTAL function of the flat
/// [`PeerSubscriptionSummary`] DTO — NO new pure-core dependency edge (I-PS-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeersView {
    /// ≥1 ACTIVE subscription: one attributed row per peer (DID verbatim +
    /// per-peer count + render-only remove command). NEVER a merged "all peers"
    /// row — each peer is its own row keyed by its DID (anti-merging, I-PS-3).
    Subscriptions {
        /// The active subscriptions, each carrying its DID + per-peer count. Order
        /// follows the adapter's `ORDER BY subscribed_at, peer_did` (deterministic).
        peers: Vec<PeerSubscriptionSummary>,
    },
    /// Zero active subscriptions (an empty active set, OR a store whose only
    /// subscription was soft-removed — residue, I-PS-2): the guided empty state
    /// naming "no peers" + the render-only `openlore peer add <did>` command.
    NoSubscriptions,
}

/// Map the flat active-subscription read into a [`PeersView`] (PURE total function,
/// slice-15 / DD-PS-5). An EMPTY `peers` slice → [`PeersView::NoSubscriptions`] (the
/// guided empty state — a soft-removed-only store reads empty here, so its residue
/// maps to the SAME empty state; I-PS-2 / US-PS-003). A non-empty slice →
/// [`PeersView::Subscriptions`] (one attributed row per peer). NO I/O, no network.
pub fn peers_view(peers: Vec<PeerSubscriptionSummary>) -> PeersView {
    if peers.is_empty() {
        PeersView::NoSubscriptions
    } else {
        PeersView::Subscriptions { peers }
    }
}

/// Render the Peer Subscriptions swap-target FRAGMENT (slice-15; ADR-052): the
/// `<div id="peers">` wrapping one attributed row per ACTIVE peer (or the guided
/// empty state) for the given [`PeersView`]. PURE: a total function from the
/// view-model to a `Markup` — NO full-page chrome (no `<!DOCTYPE>`, no
/// `<html>`/`<head>`), so an `HX-Request` response carries ONLY this region
/// (I-PS-5). Renders NO write/subscribe/unsubscribe control — the only revocation
/// affordance is the render-only `openlore peer remove <did>` command TEXT (I-PS-1).
/// [`render_peers_page`] EMBEDS this SAME fn, so the fragment and the full page's
/// peers region are byte-identical by construction (I-PS-5 parity).
pub fn render_peers_fragment(view: &PeersView) -> Markup {
    html! {
        div id=(PEERS_REGION_ID) {
            (render_peers_region(view))
        }
    }
}

/// Render the Peer Subscriptions page (`GET /peers`, US-PS-002) as a complete HTML
/// document (maud). PURE: a total function from the [`PeersView`] to an HTML string
/// — no I/O, no network. Renders the page chrome (incl. the single local
/// offline-first htmx `<script src>` + a nav link back to the other views) THEN the
/// `#peers` region.
///
/// COMPOSITION (slice-15; ADR-052): the peers region is chrome + nav wrapped AROUND
/// [`render_peers_fragment`] — the EXACT same fragment fn the htmx shape returns
/// alone. Because the region is the SAME fn in both shapes, fragment/full-page
/// parity is structural, not asserted by duplicating render logic (I-PS-5). The
/// `<head>` emits exactly ONE local `<script src="/static/htmx.min.js">`
/// (offline-first, never a CDN).
pub fn render_peers_page(view: &PeersView) -> String {
    // slice-21 (ADR-058 D6): composed through `page_shell` (persistent left nav +
    // `<main id="viewer-main">`); `active = PEERS_URL` marks the Peer Subscriptions nav
    // item current. The `render_*_fragment` fn is UNCHANGED (it rides `Shape::Fragment`
    // for the #peers swap).
    let body = html! {
        h1 { "Peer Subscriptions" }
        nav {
            a href=(MY_CLAIMS_URL) { "My Claims" }
        }
        (render_peers_fragment(view))
    };
    page_shell("OpenLore — Peer Subscriptions", PEERS_URL, body)
}

/// Render the inner `#peers` region for the given [`PeersView`]. PURE total match
/// over the ADT: a `Subscriptions` view renders one attributed row per peer; a
/// `NoSubscriptions` view renders the guided empty state + the render-only
/// `openlore peer add <did>` starting command (no fabricated peer, no error).
fn render_peers_region(view: &PeersView) -> Markup {
    html! {
        @match view {
            PeersView::Subscriptions { peers } => {
                h2 { "Peers you follow" }
                @for peer in peers {
                    (render_peer_row(peer))
                }
            }
            // No active subscriptions (US-PS-003 / I-PS-2): the guided plain-language
            // empty state + the render-only starting command — never blank, never an
            // error. A soft-removed-only store reads empty, so its residue lands here.
            PeersView::NoSubscriptions => {
                p { (PEERS_NO_SUBSCRIPTIONS_NOTICE) }
                p { (PEER_ADD_GUIDANCE_PREFIX) }
            }
        }
    }
}

/// Render ONE attributed peer row: the peer's DID VERBATIM (attribution, I-PS-3 —
/// never elided, never merged into an "all peers" row), its PER-PEER local claim
/// count, and the render-only `openlore peer remove <bare-did>` revocation command
/// (via [`render_remove_guidance`]). PURE total function. NEVER a merged total —
/// the count rendered is THIS peer's own `local_claim_count` (J-003a).
fn render_peer_row(peer: &PeerSubscriptionSummary) -> Markup {
    html! {
        section {
            p { (peer.peer_did) }
            p { (peer.local_claim_count) " cached claims" }
            (render_remove_guidance(&peer.peer_did))
        }
    }
}

/// Render the render-only `openlore peer remove <bare-did>` REVOCATION command for a
/// peer row (slice-15 / DD-PS-6 / I-PS-1) — the EXACT mirror of slice-08's
/// [`render_follow_guidance`]: the [`PEER_REMOVE_GUIDANCE_PREFIX`] TEXT + the BARE
/// DID. It is GUIDANCE ONLY: NO `<button>`/`<form>`/`hx-*` control, NO executable
/// unsubscribe — the read-only viewer holds no key, so revoking stays a deliberate
/// CLI action. The BARE DID (the slice-03 `peer remove` verb's accepted form) is
/// derived by stripping any app-identity `#…` fragment (via the shared [`bare_did`]
/// SSOT, the SAME strip the slice-08 follow guidance + `/score` cross-link use),
/// mirroring the CLI affordance. PURE total function.
pub(crate) fn render_remove_guidance(peer_did: &str) -> Markup {
    html! {
        p { (PEER_REMOVE_GUIDANCE_PREFIX) " " (bare_did(peer_did)) }
    }
}
