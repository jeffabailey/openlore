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
    AuthorRelationship, GithubError, GithubPort, IndexQueryError, IndexQueryPort, IndexedClaim,
    NetworkResultRowRaw, PageRequest, SearchDimension, StoreReadPort, TargetKind,
};
use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};
use tokio::net::TcpListener;
use viewer_domain::{
    render_claim_detail, render_claim_detail_fragment, render_claim_not_found_fragment,
    render_claims_page, render_claims_view_panel_fragment, render_error, render_landing,
    render_peer_claims_page, render_peer_claims_view_panel_fragment, render_scrape_page,
    render_scrape_results_fragment, render_search_page, render_search_results_fragment,
    CandidateRowView, ClaimDetailView, ClaimRowView, PageView, PeerClaimRowView, ScrapeState,
    SearchState, HTMX_ASSET_URL, SCRAPE_NO_CANDIDATES_NOTICE, SEARCH_URL,
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
        return Ok(search_page(index_query.as_deref(), query.as_deref(), shape).await);
    }
    match path.as_str() {
        "/" => Ok(landing_page()),
        // `GET /static/htmx.min.js` — serve the vendored htmx asset locally (no
        // CDN; I-HX-2 offline-first). GET-only, loopback, no write surface. The
        // route path is the SAME `HTMX_ASSET_URL` const the pure chrome references
        // in its `<script src>` (one source of truth — served route == chrome ref).
        HTMX_ASSET_URL => Ok(htmx_asset()),
        "/claims" => Ok(claims_page(store.as_ref(), query.as_deref(), shape)),
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

/// Render the read-only landing page (`GET /`). Pure render — needs no store read
/// (the landing page states the read-only contract; it queries nothing).
fn landing_page() -> Response<Full<Bytes>> {
    html_ok(render_landing())
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
            let rows = read_page
                .rows
                .iter()
                .map(ClaimRowView::from_row)
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
        Shape::FullPage => html_ok(render_claims_page(&page_view)),
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
            let rows = read_page
                .rows
                .iter()
                .map(PeerClaimRowView::from_row)
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
        Shape::FullPage => html_ok(render_peer_claims_page(&page_view)),
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
            match shape {
                Shape::Fragment => html_ok(render_claim_detail_fragment(&view).into_string()),
                Shape::FullPage => html_ok(render_claim_detail(&view)),
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
/// ADR-037). Parses the dimension + value from the query string (the OBJECT
/// dimension for the walking skeleton), queries the read-only `IndexQueryPort`,
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
    query: Option<&str>,
    shape: Shape,
) -> Response<Full<Bytes>> {
    let state = resolve_search_state(index_query, query).await;
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
            let claims = raw.results.into_iter().map(to_indexed_claim).collect();
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
/// The OBJECT dimension is the walking-skeleton dimension (step 01-01); the
/// CONTRIBUTOR dimension lands here (step 02-01) reusing the slice-05 handle→DID
/// resolution; the SUBJECT dimension lands here (step 02-02). The OBJECT param is
/// checked FIRST so a query carrying multiple keys is unambiguous (object, then
/// contributor, then subject). Returns `None` when no recognized dimension value is
/// present (a bare `GET /search` → the empty form). PURE total function. An empty
/// value (e.g. `?object=`) is "no value".
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

/// Map one flat attributed transport row ([`NetworkResultRowRaw`]) into the
/// [`IndexedClaim`] the pure `compose_results` consumes. Carries every load-bearing
/// field through unchanged — `author_did` (anti-merging, WD-103) and
/// `verified_against` (the `[verified]` marker, WD-104) are preserved byte-equal.
/// The relationship is `NetworkUnfollowed` by default (the viewer is per-user-
/// neutral at this step; the per-user relationship label lands in a later step).
fn to_indexed_claim(row: NetworkResultRowRaw) -> IndexedClaim {
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
        relationship: AuthorRelationship::NetworkUnfollowed,
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

    use super::{HTMX_ASSET, HTMX_ASSET_SHA256};
    use sha2::{Digest, Sha256};

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
}
