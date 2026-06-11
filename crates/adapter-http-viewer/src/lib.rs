//! `adapter-http-viewer` — the `openlore ui` viewer's HTTP/HTML surface.
//!
//! EFFECT shell (ADR-028/030): a hand-rolled minimal hyper 1.x server that binds
//! `127.0.0.1` ONLY (I-VIEW-4 — loopback, no remote exposure), reads the
//! operator's OWN store over a READ-ONLY [`ports::StoreReadPort`] (NO write/sign
//! method on that port — I-VIEW-1), calls the PURE
//! [`viewer_domain::render_claims_page`], and returns `200 text/html`.
//!
//! ## HTTP framework: `hyper` (NOT axum)
//!
//! `axum` is banned (`deny.toml`); `hyper` is already a TRANSITIVE dep of
//! `reqwest` (not banned). This is a hand-rolled one-endpoint server over the
//! hyper 1.x API, mirroring `adapter-xrpc-query-server`. The cli composition
//! root owns the tokio runtime and calls [`ViewerServer::serve`].
//!
//! ## Capability boundary (xtask check-arch)
//!
//! This crate holds a `Box<dyn StoreReadPort>` and NOTHING that can sign or
//! publish: it depends on NO signing/identity/pds crate. The signing key never
//! enters the viewer process (I-VIEW-3 structural). `cli` is the ONLY crate that
//! links this adapter.

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use appview_domain::{compose_results, NetworkSearchResult};
use claim_domain::{Cid, Did, KeyId};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use ports::{
    GithubError, GithubPort, IndexQueryError, IndexQueryPort, IndexedClaim,
    NetworkResultRowRaw, PageRequest, SearchDimension, StoreReadError, StoreReadPort, TargetKind,
};
use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};
use tokio::net::TcpListener;
use viewer_domain::{
    group_philosophy, group_project, render_claim_detail, render_claim_detail_fragment,
    render_claim_not_found_fragment, render_claims_page, render_claims_view_panel_fragment,
    render_error, render_landing, render_peer_claims_page, render_peer_claims_view_panel_fragment,
    LandingSummary,
    peers_view, render_peers_fragment, render_peers_page, render_philosophy_fragment,
    render_philosophy_page, render_project_fragment, render_project_page,
    render_score_page, render_score_results_fragment, render_scrape_page,
    render_scrape_results_fragment, render_search_page, render_search_results_fragment,
    resolve_author_relationship,
    CandidateRowView, ClaimDetailView, ClaimRowView, CounterThread, PageView, PeerClaimRowView,
    PeersView, ScoreState,
    ScrapeState, SearchState, TraversalView, HTMX_ASSET_URL, PEERS_URL, PHILOSOPHY_URL, PROJECT_URL,
    SCRAPE_NO_CANDIDATES_NOTICE, SCORE_URL, SEARCH_URL,
};

/// Re-export the PURE read-only launch banner formatter so the `cli` composition
/// root (which links this adapter but NOT `viewer-domain` directly) can print the
/// startup notice without taking a new dependency edge. The formatting itself
/// lives + is unit/property-tested in `viewer-domain` (AC-001.2).
pub use viewer_domain::read_only_launch_banner;

mod probe;

/// Re-export the PURE store-unreadable refusal constructors so the `cli` `ui`
/// verb (the viewer's composition root) renders the SAME plain-language refusal
/// when `DuckDbStorageAdapter::open` fails on a held lock — that failure happens
/// BEFORE the server can be built, so it cannot flow through [`ViewerServer::probe`].
/// Both surfaces share one operator-facing sentence (NFR-VIEW-6).
pub use probe::{viewer_store_unreadable_message, viewer_store_unreadable_refusal};

/// The read-only store the viewer serves, shared across the hyper accept loop's
/// per-connection tasks (`Send + Sync` via the `StoreReadPort` supertrait).
pub type SharedStore = Arc<dyn StoreReadPort>;

/// The GitHub driving port the `/scrape` route reuses for the LIVE propose step
/// (US-VIEW-005), shared across the hyper accept loop's per-connection tasks
/// (`Send + Sync` via the `GithubPort` supertrait). This is the SLICE-02 port —
/// the cli composition root wires a `GithubAdapter` (or, in tests, the seam hits
/// the reused `FakeGithub` via `OPENLORE_GITHUB_API_BASE`); a NEW GitHub double
/// is NOT built. `None` for store-only viewers that never serve `/scrape`.
///
/// CAPABILITY NOTE (I-VIEW-1/I-VIEW-3): a `GithubPort` reads ONLY public GitHub —
/// it holds no signing/identity/PDS/write surface — so adding it to the viewer
/// preserves the read-only, no-signing-key invariant. The viewer never persists
/// anything from `/scrape` (BR-VIEW-2 / I-VIEW-1).
pub type SharedGithub = Arc<dyn GithubPort>;

/// The READ-ONLY network index-query port the `/search` route reuses (slice-08;
/// ADR-036/037). Shared across the hyper accept loop's per-connection tasks
/// (`Send + Sync` via the `IndexQueryPort` supertrait). This is the SLICE-05 port
/// (`adapter-index-query::HttpIndexQueryAdapter`) — the cli composition root
/// resolves the indexer URL from the `OPENLORE_INDEXER_URL` / `[appview]
/// indexer_url` seam and wires it; a NEW transport is NOT built. `None` for viewers
/// that never serve `/search`, AND for an UNCONFIGURED viewer (the env-var seam is
/// unset) — the handler then yields [`SearchState::Unavailable`] WITHOUT any
/// network call (I-NS-2).
///
/// CAPABILITY NOTE (I-NS-1 / I-VIEW-3): an `IndexQueryPort` is READ-ONLY by
/// construction — it holds no signing/identity/PDS/write surface and there is no
/// sign/write method on it — so adding it to the viewer preserves the read-only,
/// no-signing-key invariant. The viewer persists NOTHING from `/search` (WD-NS-7).
pub type SharedIndexQuery = Arc<dyn IndexQueryPort>;

/// The fixed rows-per-page for the My Claims list view (ADR-030). Drives the
/// `?page=N` offset math (`OFFSET (page-1)*size LIMIT size`) in [`claims_page`]
/// and the position-indicator + Next/Prev bounds the pure `viewer-domain`
/// `PageView` projects (FR-VIEW-6).
const DEFAULT_PAGE_SIZE: u64 = 50;

/// The vendored htmx library bytes (htmx 2.0.4, 0BSD), embedded at compile time
/// and served at `GET /static/htmx.min.js` (slice-07; ADR-031). Served LOCALLY by
/// the viewer itself — NEVER a CDN (I-HX-2 offline-first). The embedded bytes are
/// pinned against silent drift by [`HTMX_ASSET_SHA256`] + the integrity unit test.
const HTMX_ASSET: &str = include_str!("../assets/htmx.min.js");

/// The pinned SHA-256 of the vendored htmx asset (htmx 2.0.4). The integrity unit
/// test asserts `sha256(HTMX_ASSET) == HTMX_ASSET_SHA256` so the embedded bytes
/// cannot silently change (a swapped/tampered/upgraded asset fails CI until the
/// pin is deliberately updated alongside it). `#[cfg(test)]`: the pin is consumed
/// ONLY by the integrity unit test — gating it here resolves the dead-code warning
/// honestly (it genuinely has no non-test use) rather than masking it.
#[cfg(test)]
const HTMX_ASSET_SHA256: &str = "e209dda5c8235479f3166defc7750e1dbcd5a5c1808b7792fc2e6733768fb447";

/// The response SHAPE the viewer renders for a request (slice-07; ADR-033). The
/// effect shell reads the `HX-Request` header ONCE in [`route`] and yields this
/// typed choice; the PURE `viewer-domain` core stays header-unaware. `Fragment`
/// returns ONLY the swap-target region (htmx in-place swap, I-HX-1); `FullPage`
/// returns the complete slice-06 document (no-JS / bookmark / direct URL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// An `HX-Request` request — return ONLY the swap-target fragment.
    Fragment,
    /// No `HX-Request` header — return the complete full page (no-JS fallback).
    FullPage,
}

impl Shape {
    /// Read the response shape from the request's headers — the SOLE shape
    /// selector (ADR-033): the PRESENCE of the `HX-Request` header (case-insensitive
    /// name; the value is not load-bearing) selects [`Shape::Fragment`], its
    /// absence [`Shape::FullPage`]. The header is read ONCE here so the pure core
    /// never sees it. PURE total function over the header map.
    fn from_request(req: &Request<Incoming>) -> Self {
        if req.headers().contains_key("HX-Request") {
            Shape::Fragment
        } else {
            Shape::FullPage
        }
    }
}

/// Why the viewer server failed to bind / serve.
#[derive(Debug, thiserror::Error)]
pub enum ViewerServerError {
    /// The configured listen address could not be bound (port in use, etc.).
    #[error("viewer server bind failed: {message}")]
    BindFailed { message: String },
    /// The server loop terminated abnormally while serving.
    #[error("viewer server serve loop failed: {message}")]
    ServeFailed { message: String },
}

/// The `openlore ui` viewer's HTTP server (ADR-028/030). Holds the bound
/// [`TcpListener`], the address it actually bound (so `:0` ephemeral ports can be
/// read back), and the read-only [`SharedStore`] it reads each request from.
pub struct ViewerServer {
    listener: TcpListener,
    local_addr: SocketAddr,
    store: SharedStore,
    /// The slice-02 `GithubPort` the `/scrape` route reuses for the LIVE propose
    /// step (US-VIEW-005). `None` for store-only viewers — `/scrape` then 404s.
    github: Option<SharedGithub>,
    /// The slice-05 READ-ONLY `IndexQueryPort` the `/search` route reuses (slice-08;
    /// ADR-037). `None` for an UNCONFIGURED viewer — `/search` then renders the
    /// fixed [`SearchState::Unavailable`] notice WITHOUT any network call (I-NS-2).
    index_query: Option<SharedIndexQuery>,
}

impl ViewerServer {
    /// Earned-Trust probe — see `probe.rs`. Real (non-stub): the store is
    /// readable (sentinel `count_claims`), the port is read-only by
    /// construction, and the bound address is loopback (127.0.0.1). `store_path`
    /// is the resolved store file the composition root opened — threaded in so a
    /// store-readable refusal can NAME the store (NFR-VIEW-6).
    pub fn probe(&self, store_path: &str) -> ports::ProbeOutcome {
        probe::run_probe(self.store.as_ref(), &self.local_addr, store_path)
    }

    /// Bind the HTTP listener at `addr` over a read-only store ONLY (no `/scrape`
    /// GitHub seam — store-only viewers: `/`, `/claims`, `/claims/{cid}`,
    /// `/peer-claims`). `/scrape` 404s. See [`Self::bind_with_github`] for the
    /// live-scrape-enabled viewer.
    pub fn bind(addr: SocketAddr, store: SharedStore) -> Result<Self, ViewerServerError> {
        Self::bind_inner(addr, store, None, None)
    }

    /// Bind the HTTP listener at `addr` over a read-only store AND the slice-02
    /// `GithubPort`, enabling the LIVE `/scrape` route (US-VIEW-005). The viewer
    /// reuses the supplied `GithubPort` for resolve+harvest; it persists nothing
    /// (BR-VIEW-2 / I-VIEW-1) and still holds NO signing key (a `GithubPort`
    /// reads only public GitHub). The cli composition root wires the adapter.
    pub fn bind_with_github(
        addr: SocketAddr,
        store: SharedStore,
        github: SharedGithub,
    ) -> Result<Self, ViewerServerError> {
        Self::bind_inner(addr, store, Some(github), None)
    }

    /// Bind the HTTP listener at `addr` over a read-only store, the slice-02
    /// `GithubPort` (for `/scrape`), AND the slice-05 READ-ONLY `IndexQueryPort`,
    /// enabling the `GET /search` route (US-NS-001..004; slice-08; ADR-037). The
    /// viewer reuses the supplied `IndexQueryPort` (resolve+query); it persists
    /// NOTHING (WD-NS-7) and holds NO signing key (an `IndexQueryPort` is read-only
    /// by construction — I-NS-1). The cli composition root resolves the indexer URL
    /// from the `OPENLORE_INDEXER_URL` / `[appview] indexer_url` seam and wires the
    /// adapter; an UNCONFIGURED viewer passes `index_query = None` and `/search`
    /// renders the fixed `Unavailable` notice WITHOUT a network call (I-NS-2).
    pub fn bind_with_index_query(
        addr: SocketAddr,
        store: SharedStore,
        github: Option<SharedGithub>,
        index_query: Option<SharedIndexQuery>,
    ) -> Result<Self, ViewerServerError> {
        Self::bind_inner(addr, store, github, index_query)
    }

    /// Bind the HTTP listener at `addr` (use `:0` for an OS-assigned ephemeral
    /// port, read back via [`Self::local_addr`]). REFUSES any non-loopback
    /// address — the viewer is localhost-only (I-VIEW-4). Must be called inside a
    /// tokio runtime (the cli composition root provides one).
    fn bind_inner(
        addr: SocketAddr,
        store: SharedStore,
        github: Option<SharedGithub>,
        index_query: Option<SharedIndexQuery>,
    ) -> Result<Self, ViewerServerError> {
        if !addr.ip().is_loopback() {
            return Err(ViewerServerError::BindFailed {
                message: format!(
                    "refusing to bind non-loopback address {addr}; the viewer is \
                     localhost-only (I-VIEW-4)"
                ),
            });
        }
        let listener =
            std::net::TcpListener::bind(addr).map_err(|err| ViewerServerError::BindFailed {
                message: format!("bind {addr}: {err}"),
            })?;
        listener
            .set_nonblocking(true)
            .map_err(|err| ViewerServerError::BindFailed {
                message: format!("set_nonblocking: {err}"),
            })?;
        let local_addr = listener
            .local_addr()
            .map_err(|err| ViewerServerError::BindFailed {
                message: format!("local_addr: {err}"),
            })?;
        let listener =
            TcpListener::from_std(listener).map_err(|err| ViewerServerError::BindFailed {
                message: format!("tokio from_std: {err}"),
            })?;
        Ok(Self {
            listener,
            local_addr,
            store,
            github,
            index_query,
        })
    }

    /// The address the listener actually bound (the ephemeral port resolved when
    /// `:0` was requested). The cli prints this as `viewer.serve.listening` so
    /// the test harness can point HTTP at it.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Serve the My Claims view until the process is killed. Runs the hyper
    /// accept loop: each connection is handled by [`route`], which reads the
    /// read-only store and renders the pure HTML. Must be called inside a tokio
    /// runtime.
    pub async fn serve(self) -> Result<(), ViewerServerError> {
        loop {
            let (stream, _peer) =
                self.listener
                    .accept()
                    .await
                    .map_err(|err| ViewerServerError::ServeFailed {
                        message: format!("accept: {err}"),
                    })?;
            let io = TokioIo::new(stream);
            let store = Arc::clone(&self.store);
            let github = self.github.clone();
            let index_query = self.index_query.clone();
            tokio::task::spawn(async move {
                let service = service_fn(move |req| {
                    route(req, Arc::clone(&store), github.clone(), index_query.clone())
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, service)
                    .await;
            });
        }
    }
}

/// Route one HTTP request. Serves the read-only GET surfaces (`/`, `/claims`,
/// `/claims/{cid}`, `/peer-claims`) plus the Live Scrape view: the `GET /scrape`
/// form and the `POST /scrape` live propose (US-VIEW-005). `POST /scrape` is the
/// one non-GET route and the one route that touches the network; everything else
/// is a 404.
async fn route(
    req: Request<Incoming>,
    store: SharedStore,
    github: Option<SharedGithub>,
    index_query: Option<SharedIndexQuery>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    // Read the response SHAPE once (ADR-033): `HX-Request` present -> Fragment,
    // absent -> FullPage. The sole shape selector; the pure core stays
    // header-unaware. NO new data route keys on it — only the render fork.
    let shape = Shape::from_request(&req);

    // `POST /scrape` — the LIVE propose step (US-VIEW-005). The ONLY non-GET
    // route + the ONLY route that reaches the network. Reads the form body, runs
    // resolve+harvest+derive via the reused `GithubPort`, and renders the
    // proposals. Persists NOTHING (BR-VIEW-2 / I-VIEW-1).
    if method == Method::POST && path == "/scrape" {
        return Ok(scrape_post(req, github.as_deref(), shape).await);
    }
    if method != Method::GET {
        return Ok(not_found());
    }
    // `GET /search` — the slice-08 network-search route (US-NS-001..004; ADR-037).
    // The ONLY GET route that reaches the network (the read-only `IndexQueryPort`).
    // Async (it `.await`s the index query), so it forks here before the synchronous
    // store-read match. Reuses the SAME `Shape` fork (fragment vs full page) every
    // other enhanced route uses (ADR-033). Reads ONLY public signed claims; persists
    // NOTHING (WD-NS-7); holds NO signing key (I-NS-1).
    if path == SEARCH_URL {
        return Ok(search_page(
            index_query.as_deref(),
            store.as_ref(),
            query.as_deref(),
            shape,
        )
        .await);
    }
    match path.as_str() {
        "/" => Ok(landing_page(store.as_ref())),
        // `GET /static/htmx.min.js` — serve the vendored htmx asset locally (no
        // CDN; I-HX-2 offline-first). GET-only, loopback, no write surface. The
        // route path is the SAME `HTMX_ASSET_URL` const the pure chrome references
        // in its `<script src>` (one source of truth — served route == chrome ref).
        HTMX_ASSET_URL => Ok(htmx_asset()),
        "/claims" => Ok(claims_page(store.as_ref(), query.as_deref(), shape)),
        // `GET /score` — the contributor-score view (slice-09; ADR-039/040/041).
        // Reads the contributor's LOCAL attributed feed over the read-only store the
        // viewer ALREADY holds (NO new field, NO network — I-CS-5), runs the REUSED
        // pure `scoring::score` in the shell, and renders the ranked `WeightedView`.
        // Forks by `Shape` (ADR-033). Holds NO signing key (a read + pure compute).
        SCORE_URL => Ok(score_page(store.as_ref(), query.as_deref(), shape)),
        // `GET /project?subject=<uri>` — the project graph-traversal survey (slice-10;
        // ADR-042/043/044/045). Reads the project's LOCAL attributed survey over the
        // read-only store the viewer ALREADY holds (`query_project_survey` — claims ∪
        // local peer_claims, NO network — I-GT-2), groups the rows in the PURE
        // `viewer-domain::group_project` core (anti-merging, never SQL — I-GT-3), and
        // renders the `#traversal-results` region. Forks by `Shape` (ADR-033). Holds
        // NO signing key (a read + pure compute); renders NO write/sign/follow control.
        PROJECT_URL => Ok(project_page(store.as_ref(), query.as_deref(), shape)),
        // `GET /philosophy?object=<uri>` — the SYMMETRIC philosophy graph-traversal survey
        // (slice-10; ADR-042/043/044/045 / US-GT-003). Mirrors the `/project` route, swapping
        // subject↔object: reads the philosophy's LOCAL attributed survey over the read-only
        // store the viewer ALREADY holds (`query_philosophy_survey` — claims ∪ local
        // peer_claims, NO network — I-GT-2), groups the rows in the PURE
        // `viewer-domain::group_philosophy` core BY subject (anti-merging, never SQL — I-GT-3),
        // and renders the `#traversal-results` region. Forks by `Shape` (ADR-033). Holds NO
        // signing key (a read + pure compute); renders NO write/sign/follow control.
        PHILOSOPHY_URL => Ok(philosophy_page(store.as_ref(), query.as_deref(), shape)),
        // `GET /peers` — the Peer Subscriptions view (slice-15; ADR-052 / US-PS-002/003).
        // Reads the operator's ACTIVE subscriptions over the read-only store the viewer
        // ALREADY holds (`list_active_peer_subscriptions` — ONE aggregate query, peer_
        // subscriptions LEFT JOIN peer_claims, WHERE removed_at IS NULL, GROUP BY
        // COUNT(pc.cid), NO N+1, NO network — I-PS-4/8), maps the flat rows to a `PeersView`
        // in the PURE `viewer-domain::peers_view` core, and renders the `#peers` region.
        // SYNCHRONOUS (no `.await`). Forks by `Shape` (ADR-033). Holds NO signing key (a
        // read + pure compute); renders NO write/subscribe/unsubscribe control — the only
        // revocation affordance is the render-only `openlore peer remove <did>` command TEXT.
        PEERS_URL => Ok(peers_page(store.as_ref(), shape)),
        // `GET /peer-claims` — the Peer Claims view (US-VIEW-003). A SEPARATE
        // route from `/claims` so "mine vs federated" is never ambiguous
        // (BR-VIEW-5). slice-07: honours `?page=N` + forks the render by Shape.
        "/peer-claims" => Ok(peer_claims_page(store.as_ref(), query.as_deref(), shape)),
        // `GET /scrape` — the empty target form (AC-005.1 GET). Pure render; no
        // network, no store read. 200 even when no `GithubPort` is wired (the
        // form is harmless; only a POST runs the live harvest).
        "/scrape" => Ok(html_ok(render_scrape_page(&ScrapeState::Form))),
        _ => match path.strip_prefix("/claims/") {
            // `GET /claims/{cid}` — the claim detail view (US-VIEW-002). A
            // non-empty CID segment routes to the detail handler; everything
            // else is 404.
            Some(cid) if !cid.is_empty() => Ok(claim_detail_page(store.as_ref(), cid, shape)),
            _ => Ok(not_found()),
        },
    }
}

/// Render the read-only landing dashboard (`GET /`, slice-17 / US-LD-000/001 /
/// ADR-054). The SANDWICH (ADR-007): read (impure — THREE LOCAL `COUNT(*)`
/// aggregates over the read-only store the viewer ALREADY holds: `count_claims`,
/// `count_peer_claims`, `count_active_peer_subscriptions`) → build (pure — the flat
/// [`LandingSummary`]) → render (pure — `render_landing`). Each count is resolved
/// INDEPENDENTLY via `.ok()` (`Result<usize, StoreReadError>` → `Option<usize>`, the
/// slice-12 ADR-048 graceful-degrade precedent generalized): a failed read maps to
/// `None` → the missing-number marker "—", NEVER a fabricated 0 and NEVER a 5xx
/// (ADR-054 D2 / C-2 CARDINAL). Always 200 (`html_ok`). LOCAL + OFFLINE — the three
/// reads have NO outbound edge; the handler holds NO signing key and renders NO
/// write/sign/follow control. Full-page-only (ADR-054 D5 — no `Shape` fork).
fn landing_page(store: &dyn StoreReadPort) -> Response<Full<Bytes>> {
    let summary = LandingSummary {
        own_claims: store.count_claims().ok(),
        // The peer-claims count flows through the TEST-ONLY fault seam
        // ([`peer_claims_count_with_fault_seam`], `#[cfg(debug_assertions)]`-gated):
        // in a release build it is the identity, so the real read result flows
        // verbatim; in a debug/test build with `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT`
        // set it substitutes a genuine `Err`, exercising the SAME `.ok() → None →
        // MISSING_COUNT_MARKER` per-count degrade the production path already runs
        // (ADR-054 D2 / C-2 CARDINAL). The degrade is NOT weakened — the seam only
        // INDUCES the `Err` the `.ok()` already handles.
        peer_claims: peer_claims_count_with_fault_seam(store.count_peer_claims()).ok(),
        active_peers: store.count_active_peer_subscriptions().ok(),
        // slice-18 (ADR-055 D4): the FOURTH independent `.ok()` resolution — a failed
        // countered-count read maps to `None` → the missing marker, the other three
        // counts + the nav hub intact, always 200. The read flows through the TEST-ONLY
        // fault seam ([`countered_count_with_fault_seam`], `#[cfg(debug_assertions)]`-gated,
        // mirroring [`peer_claims_count_with_fault_seam`]): in a release build it is the
        // identity, so the real read result flows verbatim; in a debug/test build with
        // `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` set it substitutes a genuine `Err`,
        // exercising the SAME `.ok() → None → render_countered(None) → "(— countered)"`
        // per-count degrade the production path already runs. The degrade is NOT weakened —
        // the seam only INDUCES the `Err` the `.ok()` already handles.
        countered_own_claims: countered_count_with_fault_seam(store.count_countered_own_claims())
            .ok(),
        // slice-19 (ADR-056 D4): the FIFTH independent `.ok()` resolution — a failed
        // countered-PEER-count read maps to `None` → the missing marker, the other four
        // counts (incl. the slice-18 own-countered count) + the nav hub intact, always
        // 200. The per-count degrade is independent: this read failing degrades ONLY the
        // peer countered count, never the siblings (ADR-056 D2/D4). The read flows through
        // the TEST-ONLY fault seam ([`countered_peer_count_with_fault_seam`],
        // `#[cfg(debug_assertions)]`-gated, a 4th DISTINCT token so the PEER count fails
        // INDEPENDENTLY of the slice-18 own count): in a release build it is the identity,
        // so the real read result flows verbatim; in a debug/test build with
        // `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` set it substitutes a genuine `Err`,
        // exercising the SAME `.ok() → None → render_countered(None) → "(— countered)"`
        // per-count degrade the production path already runs. The degrade is NOT weakened —
        // the seam only INDUCES the `Err` the `.ok()` already handles.
        countered_peer_claims: countered_peer_count_with_fault_seam(
            store.count_countered_peer_claims(),
        )
        .ok(),
    };
    html_ok(render_landing(&summary))
}

/// Fault-injection seam (TEST-ONLY, `#[cfg(debug_assertions)]`-gated — NEVER ships
/// in a release binary, mirroring the slice-16 [`active_set_read_with_fault_seam`]
/// + the ADR-026 `OPENLORE_PEER_PUBKEY_HEX_` seam discipline, enforced by `xtask
/// check-arch`'s viewer-fail-seam guard).
///
/// slice-17 (US-LD-000/001 / Theme 4 / C-2 CARDINAL / WD-LD-2 / WD-LD-8 / ADR-054 D2):
/// the substrate "lie" the landing dashboard must survive is a MID-REQUEST per-count
/// read FAILURE — specifically the peer-claims count. When
/// `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` is set (acceptance fault-injection only),
/// this substitutes a genuine `Err(StoreReadError::Unreadable)` for the real
/// `count_peer_claims` result so the SAME production `.ok() → None →
/// MISSING_COUNT_MARKER "—"` per-count degrade branch in [`landing_page`] runs — the
/// own-claims + active-peer counts STILL resolve to their numbers, the nav hub renders
/// in full, the page stays 200 (never a 5xx, never a fabricated 0, never a raw stack
/// trace). The PRODUCTION per-count degrade path is the thing under test; the seam only
/// INDUCES the `Err` the path already handles.
///
/// In a release build (`debug_assertions` off) this is the identity function: the
/// real read result flows through verbatim, with NO env-var read compiled in.
#[cfg(debug_assertions)]
fn peer_claims_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT").is_some() {
        return Err(StoreReadError::Unreadable {
            detail: "peer-claims count read fault injected (test-only seam)".to_string(),
        });
    }
    read
}

/// Release identity: NO seam, NO env-var read compiled into the binary.
#[cfg(not(debug_assertions))]
#[inline]
fn peer_claims_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    read
}

/// Fault-injection seam (TEST-ONLY, `#[cfg(debug_assertions)]`-gated — NEVER ships
/// in a release binary, mirroring the slice-16 `active_set_read_with_fault_seam` +
/// the slice-17 [`peer_claims_count_with_fault_seam`] + the ADR-026
/// `OPENLORE_PEER_PUBKEY_HEX_` seam discipline, enforced by `xtask check-arch`'s
/// viewer-fail-seam guard token set).
///
/// slice-18 (US-CC-000/001/002 / Theme 4 / C-2 / C-5 CARDINAL / WD-CC-2/6 / ADR-055 D4):
/// the substrate "lie" BOTH counter-aware surfaces (`GET /` landing + `GET /claims`
/// header) must survive is a MID-REQUEST per-count read FAILURE — the countered-own-claims
/// count. When `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` is set (acceptance fault-injection
/// only), this substitutes a genuine `Err(StoreReadError::Unreadable)` for the real
/// `count_countered_own_claims` result so the SAME production `.ok() → None →
/// render_countered(None) → "(— countered)"` per-count degrade branch runs — the own-claims
/// "12" + the sibling landing counts + the nav hub + the `/claims` list rows STILL resolve,
/// the page stays 200 (never a 5xx, never a fabricated "(0 countered)", never a raw stack
/// trace). The PRODUCTION per-count degrade path is the thing under test; the seam only
/// INDUCES the `Err` the path already handles. Wired around the countered-count read in
/// BOTH [`landing_page`] and [`claims_page`] so a single failure exercises both surfaces.
///
/// In a release build (`debug_assertions` off) this is the identity function: the
/// real read result flows through verbatim, with NO env-var read compiled in.
#[cfg(debug_assertions)]
fn countered_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_COUNTERED_COUNT").is_some() {
        return Err(StoreReadError::Unreadable {
            detail: "countered-own-claims count read fault injected (test-only seam)".to_string(),
        });
    }
    read
}

/// Release identity: NO seam, NO env-var read compiled into the binary.
#[cfg(not(debug_assertions))]
#[inline]
fn countered_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    read
}

/// Fault-injection seam (TEST-ONLY, `#[cfg(debug_assertions)]`-gated — NEVER ships
/// in a release binary, mirroring the slice-16 `active_set_read_with_fault_seam` +
/// the slice-17 [`peer_claims_count_with_fault_seam`] + the slice-18
/// [`countered_count_with_fault_seam`] + the ADR-026 `OPENLORE_PEER_PUBKEY_HEX_` seam
/// discipline, enforced by `xtask check-arch`'s viewer-fail-seam guard token set).
///
/// slice-19 (US-PC-000/001/002 / Theme 4 / C-2 / C-5 CARDINAL / WD-PC-2/6 / ADR-056 D4):
/// the substrate "lie" BOTH counter-aware surfaces (`GET /` landing + `GET /peer-claims`
/// header) must survive is a MID-REQUEST per-count read FAILURE — the countered-PEER-claims
/// count. When `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` is set (acceptance fault-injection
/// only), this substitutes a genuine `Err(StoreReadError::Unreadable)` for the real
/// `count_countered_peer_claims` result so the SAME production `.ok() → None →
/// render_countered(None) → "(— countered)"` per-count degrade branch runs — the peer-claims
/// "4" + the slice-18 own line "12 own claims (3 countered)" + the sibling landing counts + the
/// nav hub + the `/peer-claims` list rows + slice-13 per-row flags STILL resolve, the page
/// stays 200 (never a 5xx, never a fabricated "(0 countered)", never a raw stack trace). This
/// is a 4th DISTINCT token (NOT a reuse of the slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`)
/// so the PEER count fails INDEPENDENTLY of the own count — the missing≠zero AT asserts the
/// slice-18 own line stays untouched while only the peer count degrades (WD-PC-7 / ADR-056 D4).
/// The PRODUCTION per-count degrade path is the thing under test; the seam only INDUCES the
/// `Err` the path already handles. Wired around the countered-peer-count read in BOTH
/// [`landing_page`] and [`peer_claims_page`] so a single failure exercises both surfaces.
///
/// In a release build (`debug_assertions` off) this is the identity function: the
/// real read result flows through verbatim, with NO env-var read compiled in.
#[cfg(debug_assertions)]
fn countered_peer_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT").is_some() {
        return Err(StoreReadError::Unreadable {
            detail: "countered-peer-claims count read fault injected (test-only seam)".to_string(),
        });
    }
    read
}

/// Release identity: NO seam, NO env-var read compiled into the binary.
#[cfg(not(debug_assertions))]
#[inline]
fn countered_peer_count_with_fault_seam(
    read: Result<usize, StoreReadError>,
) -> Result<usize, StoreReadError> {
    read
}

/// Parse the 1-based `?page=N` query into a page number, defaulting to 1 and
/// CLAMPING any invalid / non-positive / unparseable value to 1 (FR-VIEW-6: a
/// mistyped or out-of-range page never crashes the viewer — it lands on page 1).
/// PURE total function over the raw query string. `?page=0`, `?page=-3`,
/// `?page=abc`, and a missing `page` all resolve to 1.
fn parse_page(query: Option<&str>) -> u64 {
    query
        .into_iter()
        .flat_map(|q| q.split('&'))
        .filter_map(|pair| pair.strip_prefix("page="))
        .next()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(1)
}

/// Render the My Claims page for `?page=N`: parse + clamp the page (default 1),
/// read that page from the read-only store (`OFFSET (page-1)*size LIMIT size`,
/// ordered composed_at DESC, cid ASC), project the boundary rows into the pure
/// view-model, and render the position indicator + Next/Prev controls via
/// `viewer-domain` (FR-VIEW-6). A store read failure degrades to an empty guided
/// page rather than a crash (the viewer never shows a raw stack trace; NFR-VIEW-6).
fn claims_page(
    store: &dyn StoreReadPort,
    query: Option<&str>,
    shape: Shape,
) -> Response<Full<Bytes>> {
    let page = parse_page(query);
    let request = PageRequest {
        offset: (page - 1) * DEFAULT_PAGE_SIZE,
        limit: DEFAULT_PAGE_SIZE,
    };
    let page_view = match store.list_claims(request) {
        Ok(read_page) => {
            // slice-12 (US-LF-002/003 / ADR-048): read the per-CID counter PRESENCE for
            // the WHOLE page in ONE aggregate `IN (...)` UNION-ALL DISTINCT lookup over
            // the read-only store the viewer ALREADY holds (NO new field, NO network,
            // NO key, NO N+1). The list SQL + paging are UNCHANGED — the presence read
            // is a SEPARATE set lookup mapped onto rows AFTER `list_claims` pages them
            // (additive only; I-LF-2). A presence-read FAILURE degrades to an EMPTY set
            // (`unwrap_or_default`) → NO flags, never a 5xx (graceful degradation).
            let cids = read_page
                .rows
                .iter()
                .map(|row| row.cid.clone())
                .collect::<Vec<_>>();
            let presence = store.counter_presence_for(&cids).unwrap_or_default();
            let rows = read_page
                .rows
                .iter()
                .map(|row| ClaimRowView::from_row_with_presence(row, &presence))
                .collect::<Vec<_>>();
            PageView::paged(rows, page, DEFAULT_PAGE_SIZE, read_page.total)
        }
        // Degrade to an empty guided page (total 0) — no indicator, no controls.
        Err(_) => PageView::paged(Vec::new(), page, DEFAULT_PAGE_SIZE, 0),
    };
    // SHAPE fork (ADR-033): the htmx swap returns ONLY the `#view-panel` fragment
    // (the active My Claims list, which wraps the inner `#claims-table` region —
    // DESIGN §6 / ADR-034); the no-JS / bookmark / direct-URL request returns the
    // complete slice-06 full page. Both project the SAME `PageView` — the full page
    // EMBEDS the SAME view-panel fragment fn, so the two shapes agree by
    // construction (I-HX-5). The tab switch targets `#view-panel`; paging targets
    // the NESTED `#claims-table` (H-1a) — both land on this one response.
    match shape {
        Shape::Fragment => html_ok(render_claims_view_panel_fragment(&page_view).into_string()),
        // slice-18 (ADR-055 D3): the full page header renders the countered count from the
        // SAME `count_countered_own_claims` read the landing uses, resolved INDEPENDENTLY
        // via `.ok()` (`Result<usize, StoreReadError>` → `Option<usize>`) — a failed read
        // degrades to `None` → "(— countered)", never blanks the list, never a 5xx (the
        // list read above is INDEPENDENT of the countered-count read, ADR-055 D4). Both
        // surfaces render through the SAME `render_countered` helper (single source). The
        // read flows through the SAME TEST-ONLY `countered_count_with_fault_seam` the
        // landing uses (`#[cfg(debug_assertions)]`-gated, release-identity), so one
        // `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` fault exercises BOTH surfaces' degrade.
        Shape::FullPage => html_ok(render_claims_page(
            &page_view,
            countered_count_with_fault_seam(store.count_countered_own_claims()).ok(),
        )),
    }
}

/// Render the contributor-score page (`GET /score`, US-CS-001..003; slice-09 /
/// ADR-039/040/041). Parses `?contributor=<did>` from the query, reads that
/// contributor's LOCAL attributed feed over the read-only store the viewer ALREADY
/// holds (`query_contributor_scoring_feed` — claims ∪ local peer_claims, NO network
/// / I-CS-5), runs the REUSED PURE `scoring::score(&feed, &ScoringConfig::DEFAULT)`
/// in the effect shell, maps the outcome to a [`ScoreState`], and renders — forking
/// by [`Shape`] (ADR-033): the htmx swap returns ONLY the `#score-results` fragment;
/// the no-JS / bookmark / direct-URL request returns the complete `/score` full
/// page. Both project the SAME state — the full page EMBEDS the fragment fn (I-CS-7
/// parity by construction).
///
/// SANDWICH (ADR-007): read (impure store call) → decide (PURE `scoring::score`) →
/// render (pure). The handler holds NO signing key — the score is a read + pure
/// compute (I-CS-1 / WD-CS-3); it renders NO write/sign/follow control. A bare `GET
/// /score` with no `?contributor` renders the empty `Form`. A contributor with no
/// local rows → the guided `NoClaims` state (naming the queried DID; OD-CS-6). A
/// store read failure degrades to the SAME guided empty state rather than a crash
/// (NFR-VIEW-6 — never a raw stack trace).
fn score_page(
    store: &dyn StoreReadPort,
    query: Option<&str>,
    shape: Shape,
) -> Response<Full<Bytes>> {
    let state = resolve_score_state(store, query);
    // Read the per-contribution counter PRESENCE for the WHOLE breakdown in ONE
    // flattened aggregate lookup (slice-14 / US-CF-002 / ADR-051). Only the `Scored`
    // arm has contributions to flag; `Form`/`NoClaims` build no view → no query → the
    // empty set (so the render signature stays uniform across shapes/states).
    let presence = score_counter_presence(store, &state);
    match shape {
        Shape::Fragment => html_ok(render_score_results_fragment(&state, &presence).into_string()),
        Shape::FullPage => html_ok(render_score_page(&state, &presence)),
    }
}

/// Read the per-contribution counter PRESENCE for a WHOLE `/score` breakdown in ONE
/// aggregate `counter_presence_for` lookup (slice-14 / US-CF-002 / ADR-051). The
/// SCORING-surface sibling of [`survey_counter_presence`]: it FLATTENS every
/// `Contribution.cid` across EVERY `WeightedPairing` in `ScoreState::Scored { view }`
/// — the union over all pairings — and reads the presence set ONCE (never per-pairing,
/// never per-contribution / the N+1 guard, AC-001-ONE-CALL / I-CF-8). REUSES the
/// slice-12 `StoreReadPort::counter_presence_for` VERBATIM (NO new method, NO new SQL,
/// NO network, NO key). The render is then a TOTAL function of the
/// `(ScoreState, presence)` pair — the presence set can ONLY gate the additive
/// "Countered" marker, NEVER reach a weight/subtotal/rank (the sum-to-weight
/// orthogonality, ADR-051 §7). `Form`/`NoClaims` carry no view → NO query is issued →
/// the EMPTY set. A presence-read FAILURE degrades to an EMPTY set
/// (`unwrap_or_default`) → NO flags, never a 5xx (graceful degradation).
fn score_counter_presence(
    store: &dyn StoreReadPort,
    state: &ScoreState,
) -> std::collections::HashSet<String> {
    let ScoreState::Scored { view } = state else {
        // Form / NoClaims build no view → no contribution CIDs → no query.
        return std::collections::HashSet::new();
    };
    let cids = view
        .ranked
        .iter()
        .flat_map(|pairing| pairing.contributions())
        .map(|contribution| contribution.cid.0.clone())
        .collect::<Vec<_>>();
    store.counter_presence_for(&cids).unwrap_or_default()
}

/// Resolve the [`ScoreState`] for a `/score` request (the read + pure-compute
/// decision over the parsed `?contributor=`). No contributor value → [`ScoreState::
/// Form`]. A contributor with ≥1 local claim → [`ScoreState::Scored`] (the REUSED
/// `scoring::score` output). A contributor with zero local rows OR a store read
/// failure → [`ScoreState::NoClaims`] naming the queried DID (graceful degradation;
/// emptiness is never a fabricated zero score, and a read error never leaks).
fn resolve_score_state(store: &dyn StoreReadPort, query: Option<&str>) -> ScoreState {
    let Some(contributor) = query_param(query, "contributor").filter(|v| !v.is_empty()) else {
        // Bare `GET /score` — the empty contributor form.
        return ScoreState::Form;
    };
    match store.query_contributor_scoring_feed(&Did(contributor.clone())) {
        // ≥1 local claim: run the PURE scorer over the feed and render the ranked
        // WeightedView. The weight is computed HERE in Rust (the pure core), NEVER
        // in SQL — so the aggregate decomposes into the per-claim breakdown.
        Ok(feed) if !feed.is_empty() => {
            let view = scoring::score(&feed, &scoring::ScoringConfig::DEFAULT);
            ScoreState::Scored { view }
        }
        // Zero local rows OR a read failure: the guided NoClaims state naming the
        // queried DID (OD-CS-6 / I-CS-5) — never a blank region, never a stack trace.
        Ok(_) | Err(_) => ScoreState::NoClaims { contributor },
    }
}

/// Render the project graph-traversal survey page (`GET /project?subject=<uri>`,
/// US-GT-002; slice-10 / ADR-042/043/044/045). Parses `?subject=` from the query,
/// reads that subject's LOCAL attributed survey over the read-only store the viewer
/// ALREADY holds (`query_project_survey` — claims ∪ local peer_claims, NO network /
/// I-GT-2), groups the rows in the PURE `viewer-domain::group_project` core
/// (anti-merging, never SQL / I-GT-3), maps the outcome to a [`TraversalView`], and
/// renders — forking by [`Shape`] (ADR-033): the htmx swap returns ONLY the
/// `#traversal-results` fragment; the no-JS / bookmark / direct-URL request returns
/// the complete `/project` full page. Both project the SAME view — the full page
/// EMBEDS the fragment fn (I-GT-6 parity by construction).
///
/// SANDWICH (ADR-007): read (impure store call) → decide (PURE `group_project`) →
/// render (pure). The handler holds NO signing key — the survey is a read + pure
/// compute (I-GT-1); it renders NO write/sign/follow control. A bare `GET /project`
/// with no `?subject` OR a subject with no local rows OR a store read failure all
/// degrade to the guided [`TraversalView::NoClaims`] state (naming the queried entity;
/// I-GT-4) rather than a crash (NFR-VIEW-6 — never a raw stack trace).
fn project_page(
    store: &dyn StoreReadPort,
    query: Option<&str>,
    shape: Shape,
) -> Response<Full<Bytes>> {
    let view = resolve_project_view(store, query);
    match shape {
        Shape::Fragment => html_ok(render_project_fragment(&view).into_string()),
        Shape::FullPage => html_ok(render_project_page(&view)),
    }
}

/// Resolve the [`TraversalView`] for a `/project` request (the read + pure-group
/// decision over the parsed `?subject=`). No subject value → a `NoClaims` naming the
/// empty entity (a bare `GET /project`). A subject with ≥1 local claim → the PURE
/// `group_project` output (`Found`). A subject with zero local rows OR a store read
/// failure → `NoClaims` naming the queried subject (graceful degradation; emptiness is
/// never a fabricated edge, and a read error never leaks — I-GT-4 / NFR-VIEW-6).
fn resolve_project_view(store: &dyn StoreReadPort, query: Option<&str>) -> TraversalView {
    let subject = query_param(query, "subject").unwrap_or_default();
    match store.query_project_survey(&subject) {
        // group_project over the LOCAL survey rows: an empty Vec yields NoClaims, a
        // non-empty one the grouped Found view — grouping is in Rust (the pure core),
        // NEVER SQL, so two same-content claims by different authors stay two rows. The
        // ONE flattened counter-presence read (ADR-050) is threaded into the grouper so
        // EdgeRow.is_countered is set as each group is built (slice-13 / US-CF-003).
        Ok(rows) => {
            let presence = survey_counter_presence(store, &rows);
            group_project(&subject, &rows, &presence)
        }
        // A read failure degrades to the SAME guided NoClaims state naming the queried
        // subject — never a blank region, never a leaked stack trace (I-GT-4).
        Err(_) => TraversalView::NoClaims { entity: subject },
    }
}

/// Read the per-edge counter PRESENCE for a WHOLE traversal survey in ONE aggregate
/// `counter_presence_for` lookup (slice-13 / US-CF-003 / ADR-050). Collects EVERY edge's
/// CID from the FLAT survey rows BEFORE grouping — the flattened union across all future
/// groups — and reads the presence set ONCE (never per-group, never per-edge / I-CF-8).
/// REUSES the slice-12 `StoreReadPort::counter_presence_for` VERBATIM (NO new method, NO
/// new SQL, NO network, NO key). The grouper then sets `EdgeRow.is_countered` from this
/// set, so the render is a total function of the presence-projected `TraversalView`. A
/// presence-read FAILURE degrades to an EMPTY set (`unwrap_or_default`) → NO flags, never
/// a 5xx (graceful degradation). Shared by `/project` (and, 02-02, `/philosophy`) so the
/// flatten-once wiring lives in ONE place.
fn survey_counter_presence(
    store: &dyn StoreReadPort,
    rows: &[ports::SurveyRow],
) -> std::collections::HashSet<String> {
    let cids = rows.iter().map(|row| row.cid.clone()).collect::<Vec<_>>();
    store.counter_presence_for(&cids).unwrap_or_default()
}

/// Render the philosophy graph-traversal survey page (`GET /philosophy?object=<uri>`,
/// US-GT-003; slice-10 / ADR-042/043/044/045) — the SYMMETRIC mirror of [`project_page`],
/// swapping subject↔object. Parses `?object=` from the query, reads that philosophy's
/// LOCAL attributed survey over the read-only store the viewer ALREADY holds
/// (`query_philosophy_survey` — claims ∪ local peer_claims, NO network / I-GT-2), groups
/// the rows in the PURE `viewer-domain::group_philosophy` core BY subject (the projects
/// that embody it; anti-merging, never SQL / I-GT-3), maps the outcome to a
/// [`TraversalView`], and renders — forking by [`Shape`] (ADR-033): the htmx swap returns
/// ONLY the `#traversal-results` fragment; the no-JS / bookmark / direct-URL request
/// returns the complete `/philosophy` full page. Both project the SAME view — the full
/// page EMBEDS the fragment fn (I-GT-6 parity by construction).
///
/// SANDWICH (ADR-007): read (impure store call) → decide (PURE `group_philosophy`) →
/// render (pure). The handler holds NO signing key — the survey is a read + pure compute
/// (I-GT-1); it renders NO write/sign/follow control. A bare `GET /philosophy` with no
/// `?object` OR an object with no local rows OR a store read failure all degrade to the
/// guided [`TraversalView::NoClaims`] state (naming the queried entity; I-GT-4) rather
/// than a crash (NFR-VIEW-6 — never a raw stack trace).
fn philosophy_page(
    store: &dyn StoreReadPort,
    query: Option<&str>,
    shape: Shape,
) -> Response<Full<Bytes>> {
    let view = resolve_philosophy_view(store, query);
    match shape {
        Shape::Fragment => html_ok(render_philosophy_fragment(&view).into_string()),
        Shape::FullPage => html_ok(render_philosophy_page(&view)),
    }
}

/// Resolve the [`TraversalView`] for a `/philosophy` request (the read + pure-group
/// decision over the parsed `?object=`) — the SYMMETRIC mirror of [`resolve_project_view`].
/// No object value → a `NoClaims` naming the empty entity (a bare `GET /philosophy`). An
/// object with ≥1 local claim → the PURE `group_philosophy` output (`Found`). An object
/// with zero local rows OR a store read failure → `NoClaims` naming the queried object
/// (graceful degradation; emptiness is never a fabricated edge, and a read error never
/// leaks — I-GT-4 / NFR-VIEW-6).
fn resolve_philosophy_view(store: &dyn StoreReadPort, query: Option<&str>) -> TraversalView {
    let object = query_param(query, "object").unwrap_or_default();
    match store.query_philosophy_survey(&object) {
        // group_philosophy over the LOCAL survey rows: an empty Vec yields NoClaims, a
        // non-empty one the grouped Found view — grouping is in Rust (the pure core),
        // NEVER SQL, so two same-content claims by different authors stay two rows. The
        // SAME ONE flattened counter-presence read (ADR-050) is threaded into the grouper
        // — the SYMMETRIC seam the slice-13 02-02 `/philosophy` edge flag closes.
        Ok(rows) => {
            let presence = survey_counter_presence(store, &rows);
            group_philosophy(&object, &rows, &presence)
        }
        // A read failure degrades to the SAME guided NoClaims state naming the queried
        // object — never a blank region, never a leaked stack trace (I-GT-4).
        Err(_) => TraversalView::NoClaims { entity: object },
    }
}

/// Render the Peer Subscriptions page (`GET /peers`, US-PS-002/003; slice-15 /
/// ADR-052). Reads the operator's ACTIVE subscriptions over the read-only store the
/// viewer ALREADY holds (`list_active_peer_subscriptions` — ONE aggregate query,
/// LEFT JOIN + GROUP BY COUNT(pc.cid), NO network / I-PS-4/8), maps the flat
/// `Vec<PeerSubscriptionSummary>` to a [`PeersView`] in the PURE
/// `viewer-domain::peers_view` core, and renders — forking by [`Shape`] (ADR-033):
/// the htmx swap returns ONLY the `#peers` fragment; the no-JS / bookmark / direct-
/// URL request returns the complete `/peers` full page. Both project the SAME view —
/// the full page EMBEDS the fragment fn (I-PS-5 parity by construction).
///
/// SANDWICH (ADR-007): read (impure store call) → decide (PURE `peers_view`) →
/// render (pure). The handler holds NO signing key — the view is a read + pure
/// compute (I-PS-1); it renders NO write/subscribe/unsubscribe control. A store read
/// failure degrades to the guided [`PeersView::NoSubscriptions`] empty state rather
/// than a 5xx (graceful degradation; NFR-PS-6 — never a raw stack trace).
fn peers_page(store: &dyn StoreReadPort, shape: Shape) -> Response<Full<Bytes>> {
    // An empty active set OR a store read failure both map to NoSubscriptions (the
    // guided empty state) — never a blank region, never a leaked stack trace (I-PS-2 /
    // US-PS-003). A non-empty set maps to Subscriptions (one attributed row per peer).
    let view = match store.list_active_peer_subscriptions() {
        Ok(peers) => peers_view(peers),
        Err(_) => PeersView::NoSubscriptions,
    };
    match shape {
        Shape::Fragment => html_ok(render_peers_fragment(&view).into_string()),
        Shape::FullPage => html_ok(render_peers_page(&view)),
    }
}

/// Render the Peer Claims page for `?page=N` (`GET /peer-claims`, US-VIEW-003 /
/// slice-07 US-HX-002): parse + clamp the page (default 1), read THAT page from the
/// read-only store (`OFFSET (page-1)*size LIMIT size`), project the boundary rows
/// into the pure view-model, and render the position indicator + Next/Prev controls
/// via `viewer-domain` (FR-VIEW-6). A SEPARATE surface from the My Claims page
/// (BR-VIEW-5). A store read failure degrades to an empty guided page rather than a
/// crash (the viewer never shows a raw stack trace; NFR-VIEW-6).
///
/// slice-07 (H-2a): threads `?page=N` through the SAME `parse_page` + offset math +
/// `PageView::paged` + `list_peer_claims` machinery the My Claims handler uses
/// (slice-06 served only page 1), then forks at the render call by `Shape` — the
/// `HX-Request` swap returns ONLY the `#claims-table` peer fragment; the no-JS /
/// bookmark / direct-URL request returns the complete slice-06 full page. Both
/// project the SAME `PageView` (the full page EMBEDS the fragment fn, I-HX-5).
fn peer_claims_page(
    store: &dyn StoreReadPort,
    query: Option<&str>,
    shape: Shape,
) -> Response<Full<Bytes>> {
    let page = parse_page(query);
    let request = PageRequest {
        offset: (page - 1) * DEFAULT_PAGE_SIZE,
        limit: DEFAULT_PAGE_SIZE,
    };
    let page_view = match store.list_peer_claims(request) {
        Ok(read_page) => {
            // slice-13 (US-CF-002 / ADR-049): read the per-CID counter PRESENCE for the
            // WHOLE page in ONE aggregate lookup over the read-only store the viewer ALREADY
            // holds (the REUSED slice-12 `counter_presence_for` — NO new method, NO new SQL,
            // NO network, NO key, NO N+1). The `list_peer_claims` SQL + paging are UNCHANGED
            // — the presence read is a SEPARATE set lookup mapped onto rows AFTER paging
            // (additive only; I-CF-2). A presence-read FAILURE degrades to an EMPTY set
            // (`unwrap_or_default`) → NO flags, never a 5xx (graceful degradation).
            let cids = read_page
                .rows
                .iter()
                .map(|row| row.cid.clone())
                .collect::<Vec<_>>();
            let presence = store.counter_presence_for(&cids).unwrap_or_default();
            let rows = read_page
                .rows
                .iter()
                .map(|row| PeerClaimRowView::from_row_with_presence(row, &presence))
                .collect::<Vec<_>>();
            PageView::paged(rows, page, DEFAULT_PAGE_SIZE, read_page.total)
        }
        // Degrade to an empty guided page (total 0) — no indicator, no controls.
        Err(_) => PageView::paged(Vec::new(), page, DEFAULT_PAGE_SIZE, 0),
    };
    // SHAPE fork (ADR-033 / ADR-034): the htmx swap returns ONLY the `#view-panel`
    // fragment (the active Peer Claims list, wrapping the inner `#claims-table`
    // region — DESIGN §6); the no-JS / bookmark / direct-URL request returns the
    // complete slice-06 full page (both embed the SAME view-panel fragment fn,
    // I-HX-5). The tab switch (H-6a) targets `#view-panel`; peer paging (H-2a)
    // targets the NESTED `#claims-table` — both land on this one response.
    match shape {
        Shape::Fragment => {
            html_ok(render_peer_claims_view_panel_fragment(&page_view).into_string())
        }
        // slice-19 (ADR-056 D3): the full page header renders the countered-PEER count
        // from the SAME `count_countered_peer_claims` read the landing uses, resolved
        // INDEPENDENTLY via `.ok()` (`Result<usize, StoreReadError>` → `Option<usize>`) — a
        // failed read degrades to `None` → "(— countered)", never blanks the list, never a
        // 5xx (the list read above is INDEPENDENT of the countered-count read, ADR-056 D4).
        // Both surfaces render through the SAME `render_countered` helper (single source).
        // The read flows through the SAME TEST-ONLY `countered_peer_count_with_fault_seam`
        // the landing uses (`#[cfg(debug_assertions)]`-gated, release-identity), so one
        // `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` fault exercises BOTH surfaces' degrade.
        Shape::FullPage => html_ok(render_peer_claims_page(
            &page_view,
            countered_peer_count_with_fault_seam(store.count_countered_peer_claims()).ok(),
        )),
    }
}

/// Render one claim's detail page (`GET /claims/{cid}`, US-VIEW-002): read the
/// claim + its ordinal-ordered evidence over the read-only store, project into
/// the pure detail view-model, and render via `viewer-domain`. The `Some` (known
/// CID) path renders `200`; the `None` (unknown CID) + read-error paths render
/// the GUIDED not-found page (the pure `render_error` — plain-language message +
/// back link to /claims, FR-VIEW-3 / NFR-VIEW-6) at `404`. A read error degrades
/// to the SAME guided page rather than leaking a raw cause (no stack trace).
///
/// SHAPE fork (slice-07 H-4a; ADR-033): the fork is on the `Some(detail)` (found)
/// path ONLY — the htmx swap returns ONLY the `#claim-detail` fragment
/// ([`render_claim_detail_fragment`]); the no-JS / bookmark / direct-URL request
/// returns the complete slice-06 detail full page ([`render_claim_detail`]). Both
/// project the SAME [`ClaimDetailView`] — the full page EMBEDS the fragment fn, so
/// the two shapes agree by construction (I-HX-5). The `None` / read-error
/// not-found path ALSO forks by `Shape` (slice-07 H-4c): the htmx swap returns the
/// `#claim-detail` not-found fragment ([`render_claim_not_found_fragment`]), the
/// no-JS request the full `404` page ([`render_error`]) — the `404` status + the
/// guided message + back link carry through BOTH shapes (the fork is AFTER the
/// not-found decision).
fn claim_detail_page(store: &dyn StoreReadPort, cid: &str, shape: Shape) -> Response<Full<Bytes>> {
    match store.get_claim(cid) {
        Ok(Some(detail)) => {
            let view = ClaimDetailView::from_detail(&detail);
            // slice-11 (US-CT-002 / ADR-046/047): additionally read the LOCAL
            // counter-claim thread over the SAME read-only store the viewer ALREADY
            // holds (`query_counter_claims` — claims ∪ local peer_claims via the
            // ADR-046 2-step read, NO network, NO new field, NO key), project the
            // rows into the PURE `CounterThread` ADT (anti-merging — each row maps to
            // one entry, never SQL-merged; I-CT-3), and thread it BENEATH the verbatim
            // claim. A read failure degrades to `CounterThread::None` (the claim still
            // renders verbatim — the counter read never crashes the detail, and a
            // missing thread is never a stack trace; NFR-VIEW-6). The counter never
            // re-weights the claim above it (shown-never-applied; I-CT-2).
            let thread = match store.query_counter_claims(cid) {
                Ok(rows) => CounterThread::from_rows(&rows),
                Err(_) => CounterThread::None,
            };
            match shape {
                Shape::Fragment => {
                    html_ok(render_claim_detail_fragment(&view, &thread).into_string())
                }
                Shape::FullPage => html_ok(render_claim_detail(&view, &thread)),
            }
        }
        // Unknown CID / read failure: the GUIDED 404 — message + back link, never a
        // raw cause (NFR-VIEW-6). SHAPE fork (slice-07 H-4c; ADR-033) is AFTER the
        // not-found decision, so the `404` status carries through BOTH shapes: the
        // htmx swap returns ONLY the `#claim-detail` not-found fragment
        // ([`render_claim_not_found_fragment`]); the no-JS / bookmark / direct-URL
        // request returns the complete full 404 page ([`render_error`]). Both carry
        // the SAME guided message + `/claims` back link (I-HX-5).
        Ok(None) | Err(_) => match shape {
            Shape::Fragment => html_not_found(render_claim_not_found_fragment().into_string()),
            Shape::FullPage => html_not_found(render_error()),
        },
    }
}

/// Render the network-search page (`GET /search`, US-NS-001..004; slice-08;
/// ADR-037). Parses the dimension + value from the query string (object /
/// contributor / subject), queries the read-only `IndexQueryPort`,
/// re-composes the flat attributed rows per-author via the REUSED pure
/// `appview_domain::compose_results` (NO second grouping path in the viewer), maps
/// the outcome to a [`SearchState`], and renders — forking by [`Shape`] (ADR-033):
/// the htmx swap returns ONLY the `#search-results` fragment; the no-JS / bookmark /
/// direct-URL request returns the complete `/search` full page. Both project the
/// SAME state — the full page EMBEDS the fragment fn (I-NS-6 parity by
/// construction).
///
/// Graceful degradation (I-NS-2): an UNCONFIGURED viewer (`index_query == None`)
/// renders the fixed `Unavailable` notice WITHOUT any network call; an UNREACHABLE
/// configured index maps the SOFT `IndexQueryError::Unreachable` (and any other
/// transport error) to the SAME fixed `Unavailable` notice — never a crash/hang and
/// never a leaked transport internal (the `Unavailable` arm is a unit variant). A
/// bare `GET /search` with no dimension value renders the empty `Form`.
async fn search_page(
    index_query: Option<&dyn IndexQueryPort>,
    store: &dyn StoreReadPort,
    query: Option<&str>,
    shape: Shape,
) -> Response<Full<Bytes>> {
    let state = resolve_search_state(index_query, store, query).await;
    match shape {
        Shape::Fragment => html_ok(render_search_results_fragment(&state).into_string()),
        Shape::FullPage => html_ok(render_search_page(&state)),
    }
}

/// Resolve the [`SearchState`] for a `/search` request (the pure-ish decision over
/// the parsed query + the index outcome; the only effect is the `IndexQueryPort`
/// call). No dimension value → [`SearchState::Form`]. No configured index →
/// [`SearchState::Unavailable`] (no network call, I-NS-2). A reachable index with
/// rows → [`SearchState::Results`] (the REUSED `compose_results` output); zero rows
/// → [`SearchState::NoResults`]; any transport error → [`SearchState::Unavailable`]
/// (graceful degradation, never a leak).
async fn resolve_search_state(
    index_query: Option<&dyn IndexQueryPort>,
    store: &dyn StoreReadPort,
    query: Option<&str>,
) -> SearchState {
    let Some((dimension, query_value, display_value)) = parse_search_dimension(query) else {
        // No dimension value supplied — the empty form (the bare `GET /search`).
        return SearchState::Form;
    };
    // UNCONFIGURED index (the env seam was unset): the fixed Unavailable notice,
    // WITHOUT attempting any network call (I-NS-2 / US-NS-001 Ex 2).
    let Some(index_query) = index_query else {
        return SearchState::Unavailable;
    };
    // slice-16 (US-SF-001 / ADR-053): read the operator's LOCAL active-subscription set
    // ONCE per render (the slice-15 `list_active_peer_subscriptions` read, REUSED — NO
    // new method, NO per-result query → no N+1). Materialize the BARE `peer_did`s into a
    // `HashSet` for O(1) in-memory membership during per-row resolution. A read FAILURE
    // degrades to the EMPTY set (`unwrap_or_default`) → every author `NetworkUnfollowed`
    // (the slice-08 status quo; C-7 / WD-SF-6 — the enrichment's failure never breaks
    // discovery, never a 5xx). The set is transient (never persisted, WD-SF-9).
    let active: std::collections::HashSet<String> = read_local_active_set(store);
    // slice-20 (US-FS-001/002 / ADR-057 D1): read the operator's OWN author-DID set
    // (the `You` arm) AND her CACHED peer author-DID set (the `UnsubscribedCache`
    // residue arm) ONCE per render — alongside the slice-16 active set — so the
    // four-arm precedence resolves every result row IN MEMORY (batch-once, NO N+1,
    // invariant to result count). Each read degrades INDEPENDENTLY to the EMPTY set
    // (`unwrap_or_default` inside the read fns): a failed own read drops only the
    // `You` arm, a failed cached read drops only the `UnsubscribedCache` arm, and the
    // row falls through to the slice-16 binary outcome (C-8 / WD-FS-4) — never a 5xx.
    let own: std::collections::HashSet<String> = read_local_own_set(store);
    let cached: std::collections::HashSet<String> = read_local_cached_set(store);
    match index_query.search(dimension, &query_value, None).await {
        Ok(raw) if raw.results.is_empty() => SearchState::NoResults {
            // Name the DISPLAY value the operator typed (the handle for the
            // contributor dimension, the verbatim value otherwise) — never the
            // resolved DID (AV-17 / the slice-05 precedent).
            queried_value: display_value,
        },
        Ok(raw) => {
            // REUSE the pure anti-merging core: map the flat attributed transport
            // rows into `IndexedClaim`s and re-compose per-author (no merge, counter
            // kept). The viewer holds NO second grouping/verification path. The
            // `dimension` is carried into the state so the renderer adds the
            // dimension-specific honest-framing footer (CONTRIBUTOR → "not a
            // community consensus", US-NS-003 / AC-003.2); the grouping is identical
            // across dimensions.
            let claims = raw
                .results
                .into_iter()
                .map(|row| to_indexed_claim(row, &own, &active, &cached))
                .collect();
            let result: NetworkSearchResult = compose_results(claims, dimension);
            SearchState::Results { result, dimension }
        }
        // SOFT, non-fatal (I-NS-2 / WD-116): an unreachable/malformed/not-found
        // index degrades to the FIXED Unavailable notice — never a crash, never a
        // leaked transport internal (the error VALUE is discarded; the sanitized
        // copy lives entirely in `viewer-domain`).
        Err(IndexQueryError::Unreachable { .. })
        | Err(IndexQueryError::BadResponse { .. })
        | Err(IndexQueryError::NotFound { .. }) => SearchState::Unavailable,
    }
}

/// Parse the search dimension from the `/search` query string, returning a triple
/// `(dimension, query_value, display_value)`:
///
/// - `query_value` is what the wire query matches against. For OBJECT it is the
///   typed value verbatim; for CONTRIBUTOR it is the RESOLVED app-identity DID the
///   indexed `author_did` carries (`github:priya` → `did:plc:priya-test#org.openlore.application`).
/// - `display_value` is what the operator typed — surfaced verbatim in the
///   NoResults empty state (the contributor handle, never the resolved DID; AV-17).
///
/// All three dimensions are parsed here: OBJECT (the typed value verbatim),
/// CONTRIBUTOR (reusing the slice-05 handle→DID resolution), and SUBJECT (a project
/// target matched verbatim — no identity resolution). The params are checked in a
/// FIXED priority order — object, then contributor, then subject — so a query
/// carrying multiple keys is unambiguous. Returns `None` when no recognized
/// dimension value is present (a bare `GET /search` → the empty form). PURE total
/// function. An empty value (e.g. `?object=`) is "no value".
fn parse_search_dimension(query: Option<&str>) -> Option<(SearchDimension, String, String)> {
    if let Some(object) = query_param(query, "object").filter(|v| !v.is_empty()) {
        return Some((SearchDimension::Object, object.clone(), object));
    }
    if let Some(contributor) = query_param(query, "contributor").filter(|v| !v.is_empty()) {
        // REUSE the slice-05 handle→DID resolution: the wire query matches the
        // indexed `author_did` EXACTLY, so query with the RESOLVED app-identity DID;
        // the empty state names the ORIGINAL handle the operator typed (AV-17).
        let query_value = resolve_contributor_to_did(&contributor);
        return Some((SearchDimension::Contributor, query_value, contributor));
    }
    if let Some(subject) = query_param(query, "subject").filter(|v| !v.is_empty()) {
        // The SUBJECT value (`github:bazelbuild/bazel`) matches the indexed `subject`
        // field VERBATIM — no DID/handle resolution (a subject is a project target,
        // not an identity). `compose_results` groups the matching claims BY AUTHOR
        // (N distinct author groups, anti-merging, WD-103) and the renderer adds NO
        // contributor footer (the honesty footer is contributor-specific; a subject
        // survey speaks for itself — US-NS-003 / AC-003.3). Both the wire query and
        // the empty-state display value are the verbatim subject (no resolution).
        return Some((SearchDimension::Subject, subject.clone(), subject));
    }
    None
}

/// The app-identity verification-method fragment every signed/indexed claim's
/// `author_did` carries (`did:plc:X#org.openlore.application`). The contributor
/// query matches the indexed `author_did` exactly, so a resolved bare DID is lifted
/// to this app identity before the wire query (mirrors the slice-05 CLI
/// `search --contributor` resolver — the SAME handle→DID convention).
const APP_IDENTITY_FRAGMENT: &str = "#org.openlore.application";

/// Resolve a `?contributor=` value to the author's app-identity DID the indexed
/// `author_did` carries — the slice-05 handle→DID resolution REUSED on the viewer's
/// `/search` surface (US-NS-003 / AC-003.2). PURE total function — no I/O.
///
/// - A `github:<handle>` argument resolves via the slice-02/04 handle→DID convention
///   (`github:priya` → `did:plc:priya-test`) then lifts to the app identity
///   (`…#org.openlore.application`).
/// - A bare DID (`did:plc:…`) lifts the app-identity fragment if it lacks one; an
///   already-fragmented DID passes through unchanged.
///
/// The query matches the indexed `author_did` exactly (`author_did = ?`), so the
/// resolved value MUST carry the app-identity fragment.
fn resolve_contributor_to_did(contributor: &str) -> String {
    let bare = match contributor.strip_prefix("github:") {
        // `github:priya` → `did:plc:priya-test` (the slice-02/04 handle→DID mapping).
        Some(handle) => format!("did:plc:{handle}-test"),
        // Already a DID — use as-is (the bare form below lifts the fragment).
        None => contributor.to_string(),
    };
    if bare.contains('#') {
        bare
    } else {
        format!("{bare}{APP_IDENTITY_FRAGMENT}")
    }
}

/// Extract a single query parameter's percent-decoded value by `key`. PURE total
/// function over the raw query string (`a=1&object=x`). Returns `None` when the key
/// is absent. Reuses [`percent_decode_form`] so an encoded NSID/handle (`%2F` etc.)
/// decodes correctly.
fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    query
        .into_iter()
        .flat_map(|q| q.split('&'))
        .filter_map(|pair| pair.strip_prefix(prefix.as_str()))
        .next()
        .map(percent_decode_form)
}

/// Read the operator's LOCAL active-subscription set ONCE and materialize the BARE
/// `peer_did`s into a `HashSet` for O(1) in-memory membership during `/search`
/// relationship resolution (slice-16 / US-SF-001 / ADR-053). REUSES the slice-15
/// `StoreReadPort::list_active_peer_subscriptions` read VERBATIM (NO new method, NO new
/// SQL, NO network, NO key — `removed_at IS NULL` already filters soft-removed peers).
/// A read FAILURE degrades to the EMPTY set (`unwrap_or_default`) → every author resolves
/// to `NetworkUnfollowed` (the slice-08 status quo; C-7 / WD-SF-6) — never a 5xx, never a
/// leaked transport internal. `peer_did` is already the bare DID (the active-set row
/// shape, slice-15), so it is collected verbatim; the result-side fragment strip happens
/// in [`to_indexed_claim`] via [`bare_did`].
fn read_local_active_set(store: &dyn StoreReadPort) -> std::collections::HashSet<String> {
    active_set_read_with_fault_seam(store.list_active_peer_subscriptions())
        .map(|peers| peers.into_iter().map(|peer| peer.peer_did).collect())
        .unwrap_or_default()
}

/// Read the operator's OWN author-DID set ONCE for the `/search` four-arm follow-state
/// resolution (slice-20 / US-FS-001/002 / ADR-057 D1) — the `You`-arm presence read.
/// Materializes the DISTINCT own `author_did`s (`StoreReadPort::distinct_own_author_dids`,
/// `SELECT DISTINCT author_did FROM claims` — single-table, NO N+1) into a `HashSet` for
/// O(1) in-memory membership during per-row resolution. A read FAILURE degrades to the
/// EMPTY set (`unwrap_or_default`) → no row resolves `You` (the slice-16 status quo for the
/// own arm; C-8 / WD-FS-4) — never a 5xx, never a leaked transport internal, INDEPENDENT of
/// the active + cached reads. The own claims carry the `#org.openlore.application` signing
/// fragment; the result-side fragment strip happens in the pure `resolve_author_relationship`
/// (R-FS-6), so the set is collected VERBATIM (bared on both sides at membership time).
fn read_local_own_set(store: &dyn StoreReadPort) -> std::collections::HashSet<String> {
    store
        .distinct_own_author_dids()
        .map(|dids| dids.into_iter().map(|did| bare_did(&did).to_string()).collect())
        .unwrap_or_default()
}

/// Read the operator's CACHED peer author-DID set ONCE for the `/search` four-arm
/// follow-state resolution (slice-20 / US-FS-001/002 / ADR-057 D1) — the
/// `UnsubscribedCache`-arm presence read. Materializes the DISTINCT cached peer
/// `author_did`s (`StoreReadPort::distinct_cached_peer_author_dids`, `SELECT DISTINCT
/// author_did FROM peer_claims`, NO `removed_at` filter — single-table, NO N+1) into a
/// `HashSet` for O(1) in-memory membership. A read FAILURE degrades to the EMPTY set
/// (`unwrap_or_default`) → no row resolves `UnsubscribedCache`, a soft-removed peer falls
/// through to the slice-16 `NetworkUnfollowed` outcome (his arm's fallback; C-8 / WD-FS-4)
/// — never a 5xx, INDEPENDENT of the own + active reads. The cached set's DIDs are bared on
/// both sides at membership time via the pure `resolve_author_relationship` (R-FS-6), so the
/// set is bared here for symmetry with the own set.
fn read_local_cached_set(store: &dyn StoreReadPort) -> std::collections::HashSet<String> {
    store
        .distinct_cached_peer_author_dids()
        .map(|dids| dids.into_iter().map(|did| bare_did(&did).to_string()).collect())
        .unwrap_or_default()
}

/// Fault-injection seam (TEST-ONLY, `#[cfg(debug_assertions)]`-gated — NEVER ships
/// in a release binary, mirroring the ADR-026 `OPENLORE_PEER_PUBKEY_HEX_` seam
/// discipline and enforced by `xtask check-arch`'s active-set-fail-seam guard).
///
/// slice-16 (US-SF-001 / Theme E / C-7 / WD-SF-6 / ADR-053 §Earned-Trust): the
/// substrate "lie" this slice must survive is a MID-REQUEST active-set read FAILURE.
/// When `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` is set (acceptance fault-injection
/// only), this substitutes a genuine `Err(StoreReadError::Unreadable)` for the real
/// read result so the SAME production `unwrap_or_default()` degrade branch in
/// [`read_local_active_set`] runs — collapsing to an EMPTY active set → every author
/// `NetworkUnfollowed` (the slice-08 status quo). The PRODUCTION degrade path is the
/// thing under test; the seam only INDUCES the `Err` the path already handles.
///
/// In a release build (`debug_assertions` off) this is the identity function: the
/// real read result flows through verbatim, with NO env-var read compiled in.
#[cfg(debug_assertions)]
fn active_set_read_with_fault_seam<T>(
    read: Result<T, StoreReadError>,
) -> Result<T, StoreReadError> {
    if std::env::var_os("OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ").is_some() {
        return Err(StoreReadError::Unreadable {
            detail: "active-set read fault injected (test-only seam)".to_string(),
        });
    }
    read
}

/// Release identity: NO seam, NO env-var read compiled into the binary.
#[cfg(not(debug_assertions))]
#[inline]
fn active_set_read_with_fault_seam<T>(
    read: Result<T, StoreReadError>,
) -> Result<T, StoreReadError> {
    read
}

/// Map one flat attributed transport row ([`NetworkResultRowRaw`]) into the
/// [`IndexedClaim`] the pure `compose_results` consumes. Carries every load-bearing
/// field through unchanged — `author_did` (anti-merging, WD-103) and
/// `verified_against` (the `[verified]` marker, WD-104) are preserved byte-equal.
///
/// slice-16 (US-SF-001/002 / ADR-053 D1): the `relationship` is RESOLVED against the
/// operator's LOCAL active-subscription set (`active`, the bare peer DIDs read ONCE per
/// render in [`resolve_search_state`]). The author's `author_did` may carry the
/// `#org.openlore.application` signing fragment; the active set's `peer_did` is already
/// bare — so the membership test strips the fragment via [`bare_did`] on the result side
/// before `HashSet::contains` (R-SF-5). The resolution is BINARY (C-6):
/// `bare_did(author_did) ∈ active → SubscribedPeer` (→ the neutral render-only
/// "Following" indicator, no command); else → `NetworkUnfollowed` (→ the slice-08
/// render-only `openlore peer add <did>` GUIDANCE, N-17 / WD-NS-3, UNCHANGED). An empty
/// `active` (no subscriptions OR a read failure that degraded gracefully) yields all
/// `NetworkUnfollowed` — exactly the slice-08 status quo (C-7).
fn to_indexed_claim(
    row: NetworkResultRowRaw,
    own: &std::collections::HashSet<String>,
    active: &std::collections::HashSet<String>,
    cached: &std::collections::HashSet<String>,
) -> IndexedClaim {
    // slice-20 (US-FS-001/002 / ADR-057 D2): the FOUR-ARM precedence resolution is the
    // PURE `viewer_domain::resolve_author_relationship` SSOT — `You > SubscribedPeer >
    // UnsubscribedCache > NetworkUnfollowed`, stripping the `#org.openlore.application`
    // signing fragment on the result side via the shared `bare_did` SSOT before each
    // set membership (R-FS-6). The shell only WIRES the three LOCAL sets read once per
    // render; the precedence logic + fragment strip live in the pure core (the viewer
    // holds NO second resolution path). An empty `own`/`cached` (no own claims / no
    // cached peers, OR a per-read failure that degraded to the empty set) collapses the
    // resolution to the slice-16 binary outcome (C-7 byte-stable) — additive only.
    let relationship = resolve_author_relationship(&row.author_did.0, own, active, cached);
    IndexedClaim {
        author_did: Did(row.author_did.0),
        cid: Cid(row.cid.0),
        subject: row.subject,
        predicate: row.predicate,
        object: row.object,
        confidence: row.confidence,
        composed_at: row.composed_at,
        verified_against: KeyId(row.verified_against.0),
        evidence: row.evidence,
        references: row.references,
        relationship,
    }
}

/// Reduce a DID to its BARE form — everything before a `#fragment` signing locator
/// (`did:plc:x#org.openlore.application` → `did:plc:x`). PURE total function; a DID
/// without a fragment passes through unchanged. The adapter mirror of `viewer-domain`'s
/// `bare_did` SSOT (one bare-DID convention across the shell + the pure core) — used to
/// reconcile a fragmented result `author_did` against the bare `peer_did`s in the LOCAL
/// active-subscription set before membership (R-SF-5 / ADR-053).
fn bare_did(did: &str) -> &str {
    match did.split_once('#') {
        Some((bare, _)) => bare,
        None => did,
    }
}

/// Handle `POST /scrape` — the LIVE propose step (US-VIEW-005 / AC-005.1). Reads
/// the `target` form field, then runs the SLICE-02 propose pipeline LIVE through
/// the reused `GithubPort`: `resolve_target` -> `harvest_repo`/`harvest_user` ->
/// the PURE `scraper_domain::derive_candidates`. The derived `CandidateClaim`
/// values are projected into the pure [`CandidateRowView`] view-model (the ONLY
/// view-model carrying display-only `derived_from`, WD-62 / I-VIEW-5) and
/// rendered. PERSISTS NOTHING (BR-VIEW-2 / I-VIEW-1 — the viewer holds no write
/// surface) and renders NO sign control (BR-VIEW-1 / I-SCR-1).
///
/// Always returns `200` with a guided page (NFR-VIEW-6): a derive that yields
/// candidates renders the proposal rows; any other outcome renders a guided
/// message rather than a blank result or a stack trace.
///
/// SHAPE fork (slice-07 H-3a; ADR-033): the htmx swap returns ONLY the
/// `#scrape-results` fragment ([`render_scrape_results_fragment`]); the no-JS /
/// bookmark / direct-URL POST returns the complete slice-06 `/scrape` full page
/// ([`render_scrape_page`]). Both project the SAME [`ScrapeState`] — the full page
/// EMBEDS the fragment fn, so the two shapes agree by construction (I-HX-5). The
/// fork is at the render call ONLY: the resolve+harvest+derive pipeline is
/// shape-independent (it persists nothing and renders no sign control either way).
async fn scrape_post(
    req: Request<Incoming>,
    github: Option<&dyn GithubPort>,
    shape: Shape,
) -> Response<Full<Bytes>> {
    let body = read_request_body(req).await;
    let target = parse_form_target(&body);

    // No `GithubPort` wired (a store-only viewer somehow received a POST) — render
    // the guided message; the live propose step is unavailable.
    let Some(github) = github else {
        return render_scrape(
            &ScrapeState::Guidance(SCRAPE_NO_CANDIDATES_NOTICE.to_string()),
            shape,
        );
    };

    let state = match propose_candidates(github, &target).await {
        Ok(candidates) if !candidates.is_empty() => {
            let rows = candidates
                .iter()
                .map(CandidateRowView::from_candidate)
                .collect::<Vec<_>>();
            ScrapeState::Proposals(rows)
        }
        // A successful harvest that derived NOTHING (AC-005.3 / V-S3): the typed
        // zero-candidates state, which renders the guided "No candidate claims
        // could be derived" message + a suggested alternative — never a blank
        // result. DISTINCT from the network-down arm so the two failure modes do
        // not collapse into one generic guidance string.
        Ok(_) => ScrapeState::ZeroCandidates,
        // GitHub could not be reached — the transport/network failure class
        // (AC-005.4 / V-S4). Maps ONLY `GithubError::Network` to the typed
        // `NetworkDown` arm, whose PURE render emits the fixed plain-language
        // cause + offline-store reassurance and NEVER interpolates the raw error
        // string — so no HTTP status / "connection refused" / "DNS" / raw URL /
        // stack trace can leak (NFR-VIEW-6/7). The error VALUE is discarded here
        // (`Network(_)`): the sanitized copy lives entirely in `viewer-domain`.
        Err(GithubError::Network(_)) => ScrapeState::NetworkDown,
        // Any OTHER refusal class (NotFound / NotPublic / RateLimited /
        // TokenRejected / ApiShape — resolve/harvest errors that are NOT a
        // network failure): a neutral guided message, never a leaked cause. The
        // viewer never crashes or shows a blank result (NFR-VIEW-6).
        Err(_) => ScrapeState::Guidance(scrape_guidance_message()),
    };
    render_scrape(&state, shape)
}

/// Render a [`ScrapeState`] to a `200` HTML response, forking by [`Shape`]
/// (slice-07 H-3a; ADR-033): the htmx swap returns ONLY the `#scrape-results`
/// fragment; the no-JS / bookmark / direct-URL request returns the complete
/// slice-06 `/scrape` full page. Both project the SAME state — the full page
/// EMBEDS the fragment fn (I-HX-5 parity by construction). Held in ONE place so
/// every `POST /scrape` exit (the no-`GithubPort` guard + the post-derive arms)
/// forks identically.
fn render_scrape(state: &ScrapeState, shape: Shape) -> Response<Full<Bytes>> {
    match shape {
        Shape::Fragment => html_ok(render_scrape_results_fragment(state).into_string()),
        Shape::FullPage => html_ok(render_scrape_page(state)),
    }
}

/// Run the live propose step for `target`: resolve, harvest, derive. Returns the
/// derived candidates (possibly empty) or the `GithubError` from resolve/harvest.
/// PURE derivation (`derive_candidates`) wrapped around the two effectful port
/// calls — the effect/pure split (ADR-007/009).
async fn propose_candidates(
    github: &dyn GithubPort,
    target: &str,
) -> Result<Vec<ports::CandidateClaim>, GithubError> {
    let kind = github.resolve_target(target).await?;
    let signals = match &kind {
        TargetKind::Repo { owner, repo } => github.harvest_repo(owner, repo).await?,
        TargetKind::User { user } => github.harvest_user(user).await?,
    };
    // The embedded SSOT snapshot is build-time-verified to parse; a parse failure
    // degrades to zero candidates rather than panicking (railway discipline).
    let Ok(mapping) = load_mapping(EMBEDDED_MAPPING_YAML) else {
        return Ok(Vec::new());
    };
    let subject = subject_for(&kind);
    Ok(derive_candidates(&subject, &signals, &mapping))
}

/// The `github:<owner>/<repo>` or `github:<user>` subject string each candidate
/// carries (the `github_target` shared artifact — mirrors the cli verb).
fn subject_for(kind: &TargetKind) -> String {
    match kind {
        TargetKind::Repo { owner, repo } => format!("github:{owner}/{repo}"),
        TargetKind::User { user } => format!("github:{user}"),
    }
}

/// The neutral guided message for the CATCH-ALL non-network refusal classes
/// (resolve/harvest errors that are NOT `GithubError::Network` — NotFound /
/// NotPublic / RateLimited / TokenRejected / ApiShape). The zero-candidates and
/// network-down outcomes route to their OWN typed [`ScrapeState`] arms (each with
/// its own pinned copy); this line covers the remaining refusals so the viewer
/// never crashes or shows a blank result (NFR-VIEW-6).
fn scrape_guidance_message() -> String {
    "The live scrape did not produce any proposals to show.".to_string()
}

/// Read the full request body into a `String` (the `application/x-www-form-
/// urlencoded` form). A read failure degrades to an empty body (the target then
/// parses empty and the propose step guides the operator — never a crash).
async fn read_request_body(req: Request<Incoming>) -> String {
    use http_body_util::BodyExt;
    match req.into_body().collect().await {
        Ok(collected) => String::from_utf8_lossy(&collected.to_bytes()).into_owned(),
        Err(_) => String::new(),
    }
}

/// Extract the `target` field from an `application/x-www-form-urlencoded` body.
/// PURE total function: splits on `&`, finds `target=`, and percent-decodes the
/// value (`+` -> space, `%XX` -> byte). A missing field yields the empty string.
fn parse_form_target(body: &str) -> String {
    body.split('&')
        .filter_map(|pair| pair.strip_prefix("target="))
        .next()
        .map(percent_decode_form)
        .unwrap_or_default()
}

/// Percent-decode one `application/x-www-form-urlencoded` value: `+` decodes to a
/// space and `%XX` to the byte `0xXX`; any malformed escape is passed through
/// verbatim. PURE total function. A GitHub `owner/repo` / `user` target needs the
/// `/` (`%2F`) decoded, so a hand-rolled decode keeps the adapter free of a new
/// dependency edge.
fn percent_decode_form(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi * 16 + lo) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The single `Content-Type` every viewer response carries — server-rendered
/// HTML (maud), UTF-8. Held in ONE place so the content-type is a single site.
const HTML_CONTENT_TYPE: &str = "text/html; charset=utf-8";

/// Build an HTML response with the given `status` and `body`. The single
/// construction site for every viewer response — applies [`HTML_CONTENT_TYPE`]
/// and the well-formed-builder invariant in one place (the status + body are the
/// only things that vary across the routes).
fn html_response(status: StatusCode, body: Full<Bytes>) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", HTML_CONTENT_TYPE)
        .body(body)
        .expect("static response is well-formed")
}

/// A `200 OK` HTML response carrying a rendered page.
fn html_ok(body: String) -> Response<Full<Bytes>> {
    html_response(StatusCode::OK, Full::new(Bytes::from(body)))
}

/// Serve the vendored htmx asset (`GET /static/htmx.min.js`) — `200` with the
/// non-empty embedded bytes and a JavaScript content-type (slice-07; ADR-031 /
/// I-HX-2 offline-first). The bytes are compile-time-embedded ([`HTMX_ASSET`]) so
/// the viewer never reaches a CDN; their integrity is pinned by the SHA-256 unit
/// test. GET-only, loopback, no write surface.
fn htmx_asset() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/javascript; charset=utf-8")
        .body(Full::new(Bytes::from_static(HTMX_ASSET.as_bytes())))
        .expect("static htmx asset response is well-formed")
}

/// The terse `404 Not Found` route-miss response (an unrouted path/method). The
/// GUIDED not-found page (unknown CID) uses [`html_not_found`] instead.
fn not_found() -> Response<Full<Bytes>> {
    html_response(
        StatusCode::NOT_FOUND,
        Full::new(Bytes::from_static(b"<p>Not found.</p>")),
    )
}

/// A `404 Not Found` HTML response carrying a rendered `body` — used for the
/// GUIDED not-found page (unknown CID), which returns `404` (AC-002.3) yet shows
/// the operator a plain-language message + back link (NFR-VIEW-6) rather than the
/// terse route-miss `not_found()` body.
fn html_not_found(body: String) -> Response<Full<Bytes>> {
    html_response(StatusCode::NOT_FOUND, Full::new(Bytes::from(body)))
}

#[cfg(test)]
mod tests {
    //! Adapter-level unit tests for the slice-07 htmx surface. The asset integrity
    //! test pins the vendored htmx bytes so they cannot silently drift (ADR-031).

    use super::{
        bare_did, claims_page, countered_count_with_fault_seam,
        countered_peer_count_with_fault_seam, landing_page, peer_claims_count_with_fault_seam,
        peer_claims_page, to_indexed_claim, Shape, SharedStore, ViewerServer, HTMX_ASSET,
        HTMX_ASSET_SHA256,
    };
    use http_body_util::BodyExt;
    use hyper::StatusCode;
    use chrono::Utc;
    use claim_domain::{Cid, Did, KeyId};
    use ports::{
        AttributedClaim, AuthorRelationship, ClaimDetail, ClaimRow, CounterClaimRow,
        NetworkResultRowRaw, Page, PageRequest, PeerClaimRow, PeerSubscriptionSummary, StoreReadError,
        StoreReadPort, SurveyRow,
    };
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;
    use std::sync::Arc;

    /// Behavior (ADR-031 — vendored-asset integrity): the embedded
    /// `assets/htmx.min.js` bytes hash to the pinned [`HTMX_ASSET_SHA256`]
    /// (htmx 2.0.4, 0BSD). If the asset is swapped, tampered with, or upgraded
    /// without updating the pin, this test fails — so the bytes the viewer serves
    /// at `/static/htmx.min.js` can never silently change.
    #[test]
    fn vendored_htmx_asset_matches_the_pinned_sha256() {
        assert!(
            !HTMX_ASSET.is_empty(),
            "the vendored htmx asset must be non-empty"
        );
        let digest = Sha256::digest(HTMX_ASSET.as_bytes());
        let hex = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            hex, HTMX_ASSET_SHA256,
            "the embedded htmx asset SHA-256 must equal the pinned value — the \
             vendored bytes drifted (update the pin deliberately if the asset changed)"
        );
    }

    // -------------------------------------------------------------------------
    // slice-16 (step 02-01) — the PURE per-row relationship resolution
    // (`to_indexed_claim`) that the SF-5/SF-6 ATs exercise only through the slow
    // subprocess + real-I/O `GET /search` path. These fast (<1ms) port-to-port
    // unit tests pin the resolution edges directly: SF-6's fragment-strip
    // membership (a `#org.openlore.application`-fragmented result `author_did`
    // reconciled against a BARE active-set `peer_did` via `bare_did` before
    // `HashSet::contains`) and SF-5's LOCAL-set-drives-the-relationship invariant
    // (the SAME row resolves DIFFERENTLY depending only on the LOCAL active set;
    // the transport row carries no relationship of its own). The pure function's
    // signature IS the port — calling it directly is port-to-port at domain scope.
    // -------------------------------------------------------------------------

    /// One flat attributed transport row whose `author_did` carries the
    /// `#org.openlore.application` signing fragment (the SAME fragmented shape the
    /// indexer/viewer carry — SF-6). Every other field is a deterministic stub; the
    /// resolution under test reads ONLY `author_did`.
    fn fragmented_row(bare: &str) -> NetworkResultRowRaw {
        NetworkResultRowRaw {
            author_did: Did(format!("{bare}#org.openlore.application")),
            cid: Cid("bafyresolutionfixture".to_string()),
            subject: "reproducible-builds".to_string(),
            predicate: "supports".to_string(),
            object: "supply-chain-integrity".to_string(),
            confidence: 0.88,
            composed_at: Utc::now(),
            verified_against: KeyId("did:key:fixture".to_string()),
            evidence: vec![],
            references: vec![],
        }
    }

    /// SF-6 (R-SF-5 / FR-SF-3): a fragmented result `author_did`
    /// (`did:plc:rachel-test#org.openlore.application`) is matched against the BARE
    /// `did:plc:rachel-test` in the LOCAL active set — the fragment is stripped via
    /// `bare_did` on the result side BEFORE membership, so the row resolves to
    /// `SubscribedPeer` (never misclassified as `NetworkUnfollowed`).
    #[test]
    fn fragmented_author_did_matches_a_bare_active_set_entry_as_subscribed_peer() {
        let active: HashSet<String> = ["did:plc:rachel-test".to_string()].into_iter().collect();
        // slice-20: with empty own + cached sets, the four-arm resolution collapses to
        // the slice-16 binary outcome — the active set alone drives SubscribedPeer.
        let empty: HashSet<String> = HashSet::new();

        let claim = to_indexed_claim(
            fragmented_row("did:plc:rachel-test"),
            &empty,
            &active,
            &empty,
        );

        assert_eq!(
            claim.relationship,
            AuthorRelationship::SubscribedPeer,
            "a fragmented result DID must reconcile against the bare active-set DID \
             via the bare_did strip before membership (R-SF-5) → SubscribedPeer"
        );
        // …and the fragmented `author_did` is carried through UNCHANGED (the render
        // still shows the app-identity shape; only the relationship is enriched).
        assert_eq!(claim.author_did.0, "did:plc:rachel-test#org.openlore.application");
    }

    /// SF-5 (C-3 / NFR-SF-4): the relationship is resolved against the LOCAL active
    /// set, not anything on the transport row. The SAME fragmented row resolves to
    /// `NetworkUnfollowed` when its bare DID is ABSENT from the active set (a
    /// different author is followed) and to `SubscribedPeer` only when present —
    /// proving the affordance tracks the LOCAL set, and that an empty set (the
    /// graceful-degrade target, C-7) yields the slice-08 `NetworkUnfollowed` status quo.
    #[test]
    fn the_same_row_resolves_by_local_active_set_membership_only() {
        let row = fragmented_row("did:plc:rachel-test");

        // slice-20: own + cached empty throughout, so the four-arm resolution collapses
        // to the slice-16 binary outcome — the active set alone drives the relationship.
        let empty: HashSet<String> = HashSet::new();

        // Empty active set (no subscriptions OR a read that degraded) → status quo.
        assert_eq!(
            to_indexed_claim(row.clone(), &empty, &empty, &empty).relationship,
            AuthorRelationship::NetworkUnfollowed,
            "an empty LOCAL active set must yield the slice-08 NetworkUnfollowed status quo (C-7)"
        );

        // A non-member active set (a DIFFERENT author followed) → NetworkUnfollowed.
        let other: HashSet<String> = ["did:plc:priya-test".to_string()].into_iter().collect();
        assert_eq!(
            to_indexed_claim(row.clone(), &empty, &other, &empty).relationship,
            AuthorRelationship::NetworkUnfollowed,
            "a row whose bare DID is absent from the LOCAL set stays NetworkUnfollowed"
        );

        // The SAME row, once the LOCAL set contains its bare DID → SubscribedPeer.
        let following: HashSet<String> = ["did:plc:rachel-test".to_string()].into_iter().collect();
        assert_eq!(
            to_indexed_claim(row, &empty, &following, &empty).relationship,
            AuthorRelationship::SubscribedPeer,
            "the relationship flips with the LOCAL active set only (SF-5) → SubscribedPeer"
        );
    }

    /// `bare_did` SSOT (R-SF-5): strips a `#fragment` signing locator; a bare DID
    /// passes through unchanged. The total-function property underlying both SF-6
    /// (the result-side strip) and the active-set bare-DID convention.
    #[test]
    fn bare_did_strips_the_signing_fragment_and_is_identity_on_a_bare_did() {
        assert_eq!(
            bare_did("did:plc:rachel-test#org.openlore.application"),
            "did:plc:rachel-test"
        );
        assert_eq!(bare_did("did:plc:rachel-test"), "did:plc:rachel-test");
    }

    // -------------------------------------------------------------------------
    // slice-17 (US-LD-000/001 / ADR-054) — the landing dashboard (`GET /`). The
    // package-scoped `cargo mutants -p adapter-http-viewer --in-diff` harness runs
    // ONLY these in-crate unit tests (NOT the cli-package acceptance suite that
    // also covers this code), so these fast (<1ms) port-to-port tests pin the
    // landing surface directly: the `landing_page` render (the read→build→render
    // SANDWICH over a fake `StoreReadPort`), the `route` GET-/ dispatch (a real
    // loopback request through the hyper accept loop), and the peer-claims-count
    // fault seam (identity pass-through + the debug-only `Err` injection). Each
    // test fails if its target line is mutated as the survivor described.
    // -------------------------------------------------------------------------

    /// A canned read-only [`StoreReadPort`] for the landing-surface unit tests: the
    /// THREE landing counts (`count_claims`, `count_peer_claims`,
    /// `count_active_peer_subscriptions`) return the values it was built with; every
    /// other port method is unreachable on the `GET /` path and `unimplemented!()`s
    /// (the landing handler reads ONLY the three counts — ADR-054 D1). Mirrors the
    /// slice-16 fake-port-as-pure-function precedent: a hand-rolled stub at the port
    /// boundary, no mock library.
    struct FakeLandingStore {
        own_claims: Result<usize, StoreReadError>,
        peer_claims: Result<usize, StoreReadError>,
        active_peers: Result<usize, StoreReadError>,
        countered_own_claims: Result<usize, StoreReadError>,
        countered_peer_claims: Result<usize, StoreReadError>,
    }

    impl FakeLandingStore {
        /// A fake whose three slice-17 counts all succeed with the given numbers; the
        /// slice-18 countered-own + slice-19 countered-peer counts default to a successful
        /// `Ok(0)` (additive — the slice-17 tests do not assert on them).
        fn with_counts(own: usize, peer: usize, active: usize) -> Self {
            Self {
                own_claims: Ok(own),
                peer_claims: Ok(peer),
                active_peers: Ok(active),
                countered_own_claims: Ok(0),
                countered_peer_claims: Ok(0),
            }
        }
    }

    fn cloned_count(read: &Result<usize, StoreReadError>) -> Result<usize, StoreReadError> {
        match read {
            Ok(n) => Ok(*n),
            Err(_) => Err(StoreReadError::Unreadable {
                detail: "fake landing-store count read fault".to_string(),
            }),
        }
    }

    impl StoreReadPort for FakeLandingStore {
        fn count_claims(&self) -> Result<usize, StoreReadError> {
            cloned_count(&self.own_claims)
        }
        fn count_peer_claims(&self) -> Result<usize, StoreReadError> {
            cloned_count(&self.peer_claims)
        }
        fn count_active_peer_subscriptions(&self) -> Result<usize, StoreReadError> {
            cloned_count(&self.active_peers)
        }
        fn count_countered_own_claims(&self) -> Result<usize, StoreReadError> {
            cloned_count(&self.countered_own_claims)
        }
        fn count_countered_peer_claims(&self) -> Result<usize, StoreReadError> {
            cloned_count(&self.countered_peer_claims)
        }

        // slice-18 (`/claims` header coverage): the My Claims list read + the
        // per-page counter-presence lookup are exercised by the claims_page header
        // test. An EMPTY successful page (total 0) is the simplest stable input —
        // it drives the FullPage render WITHOUT requiring row fixtures, so the
        // header's countered-count marker is the thing under assertion. The
        // slice-17 landing tests never reach this method (the `GET /` path reads
        // ONLY the four counts), so returning an empty page here is inert for them.
        fn list_claims(&self, _r: PageRequest) -> Result<Page<ClaimRow>, StoreReadError> {
            Ok(Page {
                rows: Vec::new(),
                total: 0,
            })
        }
        // slice-18: the empty-page counter-presence lookup — no CIDs in, the empty
        // set out. Inert for the slice-17 landing tests (they never list claims).
        fn counter_presence_for(
            &self,
            _c: &[String],
        ) -> Result<HashSet<String>, StoreReadError> {
            Ok(HashSet::new())
        }

        // Not on the `GET /` landing path — unreachable for these tests.
        fn get_claim(&self, _cid: &str) -> Result<Option<ClaimDetail>, StoreReadError> {
            unimplemented!("not read by the landing dashboard")
        }
        // slice-19 (`/peer-claims` header coverage): the Peer Claims list read + the
        // per-page counter-presence lookup are exercised by the peer_claims_page header
        // test. An EMPTY successful page (total 0) is the simplest stable input — it
        // drives the FullPage render WITHOUT requiring row fixtures, so the header's
        // countered-PEER-count marker is the thing under assertion. The slice-17 landing
        // tests never reach this method (the `GET /` path reads ONLY the four counts),
        // so returning an empty page here is inert for them. Mirrors `list_claims`.
        fn list_peer_claims(&self, _r: PageRequest) -> Result<Page<PeerClaimRow>, StoreReadError> {
            Ok(Page {
                rows: Vec::new(),
                total: 0,
            })
        }
        fn query_contributor_scoring_feed(
            &self,
            _c: &Did,
        ) -> Result<Vec<AttributedClaim>, StoreReadError> {
            unimplemented!("not read by the landing dashboard")
        }
        fn query_project_survey(&self, _s: &str) -> Result<Vec<SurveyRow>, StoreReadError> {
            unimplemented!("not read by the landing dashboard")
        }
        fn query_philosophy_survey(&self, _o: &str) -> Result<Vec<SurveyRow>, StoreReadError> {
            unimplemented!("not read by the landing dashboard")
        }
        fn query_counter_claims(
            &self,
            _t: &str,
        ) -> Result<Vec<CounterClaimRow>, StoreReadError> {
            unimplemented!("not read by the landing dashboard")
        }
        fn list_active_peer_subscriptions(
            &self,
        ) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError> {
            unimplemented!("not read by the landing dashboard")
        }
        // slice-20 (`/search` four-arm follow-state): the own + cached presence reads
        // are NOT on the `GET /` landing path — unreachable for these tests. The
        // `/search` follow-state resolution is exercised end-to-end by the FF-* ATs
        // over the REAL DuckDB adapter (Pillar 3), not this fake.
        fn distinct_own_author_dids(
            &self,
        ) -> Result<HashSet<String>, StoreReadError> {
            unimplemented!("not read by the landing dashboard")
        }
        fn distinct_cached_peer_author_dids(
            &self,
        ) -> Result<HashSet<String>, StoreReadError> {
            unimplemented!("not read by the landing dashboard")
        }
    }

    /// The 8 shipped nav-hub surface labels the landing dashboard MUST link
    /// (WD-LD-7 / Theme 2). Pinned here so the in-crate render assertion checks the
    /// full hub, not just one link — a dropped surface fails this test.
    const LANDING_HUB_LABELS: &[&str] = &[
        "My Claims",
        "Peer Claims",
        "Project Survey",
        "Philosophy Survey",
        "Contributor Score",
        "Network Search",
        "Live Scrape",
        "Peer Subscriptions",
    ];

    /// Behavior (ADR-054 D1 / lib.rs:425): `landing_page` renders the at-a-glance
    /// summary (the three LOCAL counts read over the store) PLUS the 8-surface nav
    /// hub. With a fake store returning own=12 / peer=7 / active=2, the rendered body
    /// contains each labelled count and every hub link. Kills the `:425 landing_page
    /// -> empty Response` mutant: an empty body carries none of these markers, so the
    /// assertions fail under the mutation.
    #[tokio::test]
    async fn landing_page_renders_the_three_counts_and_the_eight_surface_hub() {
        let store = FakeLandingStore::with_counts(12, 7, 2);

        let response = landing_page(&store);
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("landing body collects")
            .to_bytes();
        let body = String::from_utf8(bytes.to_vec()).expect("landing body is UTF-8 HTML");

        // The three at-a-glance counts, each with its label (read→build→render).
        assert!(
            body.contains("12 own claims"),
            "the landing body must render the own-claims count: {body}"
        );
        assert!(
            body.contains("7 peer claims"),
            "the landing body must render the peer-claims count: {body}"
        );
        assert!(
            body.contains("2 active peers"),
            "the landing body must render the active-peers count: {body}"
        );
        // The full 8-surface discoverability hub (WD-LD-7) — a dropped surface fails.
        for label in LANDING_HUB_LABELS {
            assert!(
                body.contains(label),
                "the landing nav hub must link the {label:?} surface: {body}"
            );
        }
    }

    /// Behavior (lib.rs:355 + :321): `route` dispatches `GET /` to `landing_page` and
    /// returns `200` with the landing body. Exercised through the REAL hyper accept
    /// loop over a bound loopback `ViewerServer` (a raw HTTP/1.1 request on a TCP
    /// socket — `Request<Incoming>` is not constructible directly, so this is an
    /// in-crate wiring test that drives the genuine `route` path). Kills BOTH the
    /// `:355 delete "/" arm` mutant (which would fall through to the `_` 404 arm — a
    /// `404` "Not found." body, failing the 200 + landing-content assertion) AND the
    /// `:321 route -> empty Response` mutant (an empty body carries no landing
    /// markers).
    #[tokio::test]
    async fn route_dispatches_get_root_to_the_landing_page_with_200() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let store: SharedStore = Arc::new(FakeLandingStore::with_counts(12, 7, 2));
        let addr = "127.0.0.1:0".parse().expect("loopback addr parses");
        let server = ViewerServer::bind(addr, store).expect("bind loopback ephemeral port");
        let bound = server.local_addr();

        // Drive the real accept loop in the background; the client below sends one
        // request over a fresh TCP connection, so the server routes a genuine
        // `Request<Incoming>` through `route` -> the `GET /` arm -> `landing_page`.
        tokio::spawn(server.serve());

        let mut stream = TcpStream::connect(bound)
            .await
            .expect("connect to the bound viewer");
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .expect("write the GET / request");

        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .await
            .expect("read the full HTTP response");
        let response = String::from_utf8_lossy(&raw);

        assert!(
            response.starts_with("HTTP/1.1 200"),
            "GET / must dispatch to the landing page with 200 (not a 404 route-miss): {response}"
        );
        // The landing body (the dispatch reached `landing_page`, not an empty/404 body).
        assert!(
            response.contains("12 own claims"),
            "GET / must return the landing summary body: {response}"
        );
        assert!(
            response.contains("My Claims"),
            "GET / must return the landing nav hub: {response}"
        );
    }

    /// Behavior (lib.rs:464/:478): the peer-claims-count fault seam is the IDENTITY on
    /// the real read when the fault env-var is unset — the genuine `Ok(7)` flows
    /// through verbatim. Kills BOTH the `:464/:478 -> Ok(0)` and `-> Ok(1)` mutants:
    /// they would return `0` / `1`, not the `7` the real read carried, so the
    /// `assert_eq!(_, Ok(7))` fails under either mutation. (Asserted via the rendered
    /// landing count, since the seam fn is `#[cfg(debug_assertions)]` and returns the
    /// `StoreReadError` enum which is not `PartialEq`.)
    #[test]
    fn peer_claims_count_seam_passes_the_real_read_through_when_unset() {
        // Serialize with the sibling inject test so its `set_var` cannot leak into
        // this pass-through read window (poison-recovering).
        let _env = FAULT_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Guard: the fault env-var must be unset for the identity behavior.
        assert!(
            std::env::var_os(FAULT_ENV).is_none(),
            "the fault seam env-var must be unset for the pass-through assertion"
        );
        let passed = peer_claims_count_with_fault_seam(Ok(7));
        match passed {
            Ok(n) => assert_eq!(
                n, 7,
                "the seam must pass the real read through verbatim (kills Ok(0)/Ok(1))"
            ),
            Err(e) => panic!("the seam must not inject an error when unset: {e}"),
        }
    }

    /// The fault-injection env-var the slice-17 peer-claims-count seam reads (the
    /// production seam under test; ADR-054 D2). Pinned here so the env-var name has
    /// one in-crate source of truth.
    const FAULT_ENV: &str = "OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT";

    /// `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` is process-global, and Rust runs unit
    /// tests multi-threaded within one binary — so the pass-through test (asserts the
    /// var is UNSET) and the inject test (SETs it) would race (the `set_var` leaking
    /// into the pass-through read window). Serialize the two on this lock so each owns
    /// the env var for its read window (poison-recovering — a panic in one must not
    /// cascade), mirroring the serialized env-var discipline of commit 2629e56.
    static FAULT_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Behavior (lib.rs:464, `#[cfg(debug_assertions)]` only): with the fault env-var
    /// SET, the seam substitutes a genuine `Err` for the real read — exercising the
    /// SAME `.ok() -> None -> "—"` per-count degrade the production path runs (ADR-054
    /// D2 / C-2 CARDINAL). Pins the fault injection so the `:464 -> Ok(0)/Ok(1)`
    /// mutants (which would IGNORE the env-var and return a fabricated success) are
    /// killed from the inject side too. The env-var is set+removed within this single
    /// test (no parallel test reads it — the only other reader is this module's
    /// pass-through test, which asserts the var is unset; the two never overlap
    /// because this one removes the var before returning, mirroring the serialized
    /// env-var discipline of commit 2629e56).
    #[cfg(debug_assertions)]
    #[test]
    fn peer_claims_count_seam_injects_err_when_the_fault_env_var_is_set() {
        // Hold the env lock across the whole set -> exercise -> remove window so the
        // sibling pass-through test never observes this fault pin (poison-recovering).
        let _env = FAULT_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Isolation: set, exercise, then ALWAYS remove before releasing the lock.
        std::env::set_var(FAULT_ENV, "1");
        let injected = peer_claims_count_with_fault_seam(Ok(7));
        std::env::remove_var(FAULT_ENV);

        assert!(
            injected.is_err(),
            "with the fault env-var set, the seam must inject a genuine Err so the \
             production .ok() -> None -> missing-marker degrade runs (ADR-054 D2)"
        );
    }

    // -------------------------------------------------------------------------
    // slice-18 (US-CC-000/001/002 / ADR-055) — the counter-aware counts. The
    // package-scoped `cargo mutants -p adapter-http-viewer --in-diff` harness runs
    // ONLY these in-crate unit tests (NOT the cli-package acceptance suite that
    // also covers this code), so these fast (<1ms) port-to-port tests pin the
    // slice-18 surfaces directly: the `countered_count_with_fault_seam` identity
    // pass-through (kills :518 -> Ok(0)/Ok(1)) + the debug-only `Err` injection,
    // and the `claims_page` FullPage header rendering the countered count (kills
    // :562 claims_page -> empty Response). The :532 release-identity sibling is a
    // cfg-dead branch under the debug test profile (NOT compiled here) — it is
    // guarded by the xtask seam guard + the release-build seam-free check, not a
    // debug test, mirroring slice-16/17's lone cfg-dead survivor.
    // -------------------------------------------------------------------------

    /// The slice-18 fault-injection env-var the countered-count seam reads (the
    /// production seam under test; ADR-055 D4). Pinned here so the env-var name has
    /// one in-crate source of truth, mirroring the slice-17 [`FAULT_ENV`].
    const COUNTERED_FAULT_ENV: &str = "OPENLORE_VIEWER_FAIL_COUNTERED_COUNT";

    /// Build a fake landing store whose four counts all succeed, with an EXPLICIT
    /// countered count (the slice-17 [`FakeLandingStore::with_counts`] defaults it to
    /// `Ok(0)`; the `/claims` header test needs a distinctive non-zero value).
    fn fake_store_with_countered(own: usize, peer: usize, active: usize, countered: usize) -> FakeLandingStore {
        FakeLandingStore {
            own_claims: Ok(own),
            peer_claims: Ok(peer),
            active_peers: Ok(active),
            countered_own_claims: Ok(countered),
            countered_peer_claims: Ok(0),
        }
    }

    /// Behavior (lib.rs:518, the `#[cfg(debug_assertions)]` seam): the countered-count
    /// fault seam is the IDENTITY on the real read when the fault env-var is unset — the
    /// genuine `Ok(7)` flows through verbatim. Kills BOTH the `:518 -> Ok(0)` and
    /// `-> Ok(1)` mutants: they would return `0` / `1`, not the `7` the real read
    /// carried. (`StoreReadError` is not `PartialEq`, so we match the `Ok` arm directly
    /// rather than `assert_eq!` the whole `Result`.) Mirrors the slice-17
    /// [`peer_claims_count_seam_passes_the_real_read_through_when_unset`]. Serialized on
    /// the SAME [`FAULT_ENV_LOCK`] as the inject test so a sibling `set_var` cannot leak
    /// into this pass-through read window (poison-recovering).
    #[test]
    fn countered_count_seam_passes_the_real_read_through_when_unset() {
        let _env = FAULT_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Guard: the fault env-var must be unset for the identity behavior.
        assert!(
            std::env::var_os(COUNTERED_FAULT_ENV).is_none(),
            "the countered-count fault seam env-var must be unset for the pass-through assertion"
        );
        let passed = countered_count_with_fault_seam(Ok(7));
        match passed {
            Ok(n) => assert_eq!(
                n, 7,
                "the seam must pass the real read through verbatim (kills :518 Ok(0)/Ok(1))"
            ),
            Err(e) => panic!("the seam must not inject an error when unset: {e}"),
        }
    }

    /// Behavior (lib.rs:518, `#[cfg(debug_assertions)]` only): with the countered-count
    /// fault env-var SET, the seam substitutes a genuine `Err` for the real read —
    /// exercising the SAME `.ok() -> None -> render_countered(None) -> "(— countered)"`
    /// per-count degrade the production path runs (ADR-055 D4 / C-2 CARDINAL). Pins the
    /// fault injection so the `:518 -> Ok(0)/Ok(1)` mutants (which would IGNORE the
    /// env-var and return a fabricated success) are killed from the inject side too.
    /// Set + removed within this single test under the shared [`FAULT_ENV_LOCK`] so the
    /// sibling pass-through test never observes the pin (poison-recovering), mirroring
    /// the slice-17 inject test + commit 2629e56's serialized env-var discipline.
    #[cfg(debug_assertions)]
    #[test]
    fn countered_count_seam_injects_err_when_the_fault_env_var_is_set() {
        let _env = FAULT_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Isolation: set, exercise, then ALWAYS remove before releasing the lock.
        std::env::set_var(COUNTERED_FAULT_ENV, "1");
        let injected = countered_count_with_fault_seam(Ok(7));
        std::env::remove_var(COUNTERED_FAULT_ENV);

        assert!(
            injected.is_err(),
            "with the countered-count fault env-var set, the seam must inject a genuine \
             Err so the production .ok() -> None -> \"(— countered)\" degrade runs (ADR-055 D4)"
        );
    }

    /// Behavior (lib.rs:562 / ADR-055 D3): `claims_page` renders the My Claims FULL PAGE
    /// — the `<h1>My Claims (N countered)</h1>` header carries the countered count read
    /// over the store through the SAME `render_countered` helper the landing uses. With a
    /// fake store reporting `count_countered_own_claims() == Ok(3)`, the FullPage body
    /// contains both "My Claims" and "(3 countered)". Kills the `:562 claims_page ->
    /// empty Response` mutant: an empty body carries neither marker. The list read returns
    /// an empty page (total 0) so the header — not row fixtures — is the thing under test.
    /// Asserts the env-var is unset (serialized on [`FAULT_ENV_LOCK`]) so the seam is the
    /// identity and the real `Ok(3)` reaches the header.
    #[tokio::test]
    async fn claims_page_full_page_renders_the_countered_count_header() {
        let _env = FAULT_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(
            std::env::var_os(COUNTERED_FAULT_ENV).is_none(),
            "the countered-count fault seam env-var must be unset so the real Ok(3) reaches the header"
        );
        let store = fake_store_with_countered(12, 7, 2, 3);

        let response = claims_page(&store, None, Shape::FullPage);
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("claims-page body collects")
            .to_bytes();
        let body = String::from_utf8(bytes.to_vec()).expect("claims-page body is UTF-8 HTML");

        assert!(
            body.contains("My Claims"),
            "the claims-page full page must render the My Claims header: {body}"
        );
        assert!(
            body.contains("(3 countered)"),
            "the claims-page header must render the countered count via render_countered: {body}"
        );
    }

    // -------------------------------------------------------------------------
    // slice-19 (US-PC-000/001/002 / ADR-056) — the counter-aware PEER counts. The
    // package-scoped `cargo mutants -p adapter-http-viewer --in-diff` harness runs
    // ONLY these in-crate unit tests (NOT the cli-package acceptance suite that
    // also covers this code), so these fast (<1ms) port-to-port tests pin the
    // slice-19 surfaces directly, mirroring slice-18 EXACTLY: the
    // `countered_peer_count_with_fault_seam` identity pass-through (kills :581 ->
    // Ok(0)/Ok(1)) + the debug-only `Err` injection, and the `peer_claims_page`
    // FullPage header rendering the countered-PEER count (kills :944 peer_claims_page
    // -> empty Response). The :595 release-identity sibling is a cfg-dead branch under
    // the debug test profile (NOT compiled here) — it is guarded by the xtask seam
    // guard + the release-build seam-free check, not a debug test, mirroring
    // slice-16/17/18's lone cfg-dead survivor (unkillable by any debug test).
    // -------------------------------------------------------------------------

    /// The slice-19 fault-injection env-var the countered-PEER-count seam reads (the
    /// production seam under test; ADR-056 D4). A 4th DISTINCT token (NOT the slice-18
    /// [`COUNTERED_FAULT_ENV`]) so the PEER count fails INDEPENDENTLY of the own count.
    /// Pinned here so the env-var name has one in-crate source of truth.
    const PEER_COUNTERED_FAULT_ENV: &str = "OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT";

    /// Build a fake landing store whose four-plus-peer counts all succeed, with an
    /// EXPLICIT countered-PEER count (the slice-17 [`FakeLandingStore::with_counts`] and
    /// slice-18 [`fake_store_with_countered`] both default the countered-peer count to
    /// `Ok(0)`; the `/peer-claims` header test needs a distinctive non-zero value).
    fn fake_store_with_countered_peer(
        own: usize,
        peer: usize,
        active: usize,
        countered_peer: usize,
    ) -> FakeLandingStore {
        FakeLandingStore {
            own_claims: Ok(own),
            peer_claims: Ok(peer),
            active_peers: Ok(active),
            countered_own_claims: Ok(0),
            countered_peer_claims: Ok(countered_peer),
        }
    }

    /// Behavior (lib.rs:581, the `#[cfg(debug_assertions)]` seam): the countered-PEER-count
    /// fault seam is the IDENTITY on the real read when the fault env-var is unset — the
    /// genuine `Ok(7)` flows through verbatim. Kills BOTH the `:581 -> Ok(0)` and
    /// `-> Ok(1)` mutants: they would return `0` / `1`, not the `7` the real read carried.
    /// (`StoreReadError` is not `PartialEq`, so we match the `Ok` arm directly rather than
    /// `assert_eq!` the whole `Result`.) Mirrors the slice-18
    /// [`countered_count_seam_passes_the_real_read_through_when_unset`]. Serialized on the
    /// SAME [`FAULT_ENV_LOCK`] as the inject test so a sibling `set_var` cannot leak into
    /// this pass-through read window (poison-recovering).
    #[test]
    fn countered_peer_count_seam_passes_the_real_read_through_when_unset() {
        let _env = FAULT_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Guard: the fault env-var must be unset for the identity behavior.
        assert!(
            std::env::var_os(PEER_COUNTERED_FAULT_ENV).is_none(),
            "the countered-PEER-count fault seam env-var must be unset for the pass-through assertion"
        );
        let passed = countered_peer_count_with_fault_seam(Ok(7));
        match passed {
            Ok(n) => assert_eq!(
                n, 7,
                "the seam must pass the real read through verbatim (kills :581 Ok(0)/Ok(1))"
            ),
            Err(e) => panic!("the seam must not inject an error when unset: {e}"),
        }
    }

    /// Behavior (lib.rs:581, `#[cfg(debug_assertions)]` only): with the countered-PEER-count
    /// fault env-var SET, the seam substitutes a genuine `Err` for the real read —
    /// exercising the SAME `.ok() -> None -> render_countered(None) -> "(— countered)"`
    /// per-count degrade the production path runs (ADR-056 D4 / C-2 CARDINAL). Pins the
    /// fault injection so the `:581 -> Ok(0)/Ok(1)` mutants (which would IGNORE the env-var
    /// and return a fabricated success) are killed from the inject side too. Set + removed
    /// within this single test under the shared [`FAULT_ENV_LOCK`] so the sibling
    /// pass-through test never observes the pin (poison-recovering), mirroring the slice-18
    /// inject test + commit 2629e56's serialized env-var discipline.
    #[cfg(debug_assertions)]
    #[test]
    fn countered_peer_count_seam_injects_err_when_the_fault_env_var_is_set() {
        let _env = FAULT_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Isolation: set, exercise, then ALWAYS remove before releasing the lock.
        std::env::set_var(PEER_COUNTERED_FAULT_ENV, "1");
        let injected = countered_peer_count_with_fault_seam(Ok(7));
        std::env::remove_var(PEER_COUNTERED_FAULT_ENV);

        assert!(
            injected.is_err(),
            "with the countered-PEER-count fault env-var set, the seam must inject a genuine \
             Err so the production .ok() -> None -> \"(— countered)\" degrade runs (ADR-056 D4)"
        );
    }

    /// Behavior (lib.rs:944 / ADR-056 D3): `peer_claims_page` renders the Peer Claims FULL
    /// PAGE — the header carries the countered-PEER count read over the store through the
    /// SAME `render_countered` helper the landing uses. With a fake store reporting
    /// `count_countered_peer_claims() == Ok(3)`, the FullPage body contains both
    /// "Peer Claims" and "(3 countered)". Kills the `:944 peer_claims_page -> empty
    /// Response` mutant: an empty body carries neither marker. The list read returns an
    /// empty page (total 0) so the header — not row fixtures — is the thing under test.
    /// Asserts the env-var is unset (serialized on [`FAULT_ENV_LOCK`]) so the seam is the
    /// identity and the real `Ok(3)` reaches the header. Mirrors the slice-18
    /// [`claims_page_full_page_renders_the_countered_count_header`].
    #[tokio::test]
    async fn peer_claims_page_full_page_renders_the_countered_count_header() {
        let _env = FAULT_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(
            std::env::var_os(PEER_COUNTERED_FAULT_ENV).is_none(),
            "the countered-PEER-count fault seam env-var must be unset so the real Ok(3) reaches the header"
        );
        let store = fake_store_with_countered_peer(12, 7, 2, 3);

        let response = peer_claims_page(&store, None, Shape::FullPage);
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("peer-claims-page body collects")
            .to_bytes();
        let body = String::from_utf8(bytes.to_vec()).expect("peer-claims-page body is UTF-8 HTML");

        assert!(
            body.contains("Peer Claims"),
            "the peer-claims-page full page must render the Peer Claims header: {body}"
        );
        assert!(
            body.contains("(3 countered)"),
            "the peer-claims-page header must render the countered count via render_countered: {body}"
        );
    }
}
