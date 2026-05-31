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

use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use ports::{GithubError, GithubPort, PageRequest, StoreReadPort, TargetKind};
use scraper_domain::{derive_candidates, load_mapping, EMBEDDED_MAPPING_YAML};
use tokio::net::TcpListener;
use viewer_domain::{
    render_claim_detail, render_claims_page, render_error, render_landing, render_peer_claims_page,
    render_scrape_page, CandidateRowView, ClaimDetailView, ClaimRowView, PageView,
    PeerClaimRowView, ScrapeState, SCRAPE_NO_CANDIDATES_NOTICE,
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

/// The fixed rows-per-page for the My Claims list view (ADR-030). Drives the
/// `?page=N` offset math (`OFFSET (page-1)*size LIMIT size`) in [`claims_page`]
/// and the position-indicator + Next/Prev bounds the pure `viewer-domain`
/// `PageView` projects (FR-VIEW-6).
const DEFAULT_PAGE_SIZE: u64 = 50;

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
        Self::bind_inner(addr, store, None)
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
        Self::bind_inner(addr, store, Some(github))
    }

    /// Bind the HTTP listener at `addr` (use `:0` for an OS-assigned ephemeral
    /// port, read back via [`Self::local_addr`]). REFUSES any non-loopback
    /// address — the viewer is localhost-only (I-VIEW-4). Must be called inside a
    /// tokio runtime (the cli composition root provides one).
    fn bind_inner(
        addr: SocketAddr,
        store: SharedStore,
        github: Option<SharedGithub>,
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
            tokio::task::spawn(async move {
                let service = service_fn(move |req| route(req, Arc::clone(&store), github.clone()));
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
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);

    // `POST /scrape` — the LIVE propose step (US-VIEW-005). The ONLY non-GET
    // route + the ONLY route that reaches the network. Reads the form body, runs
    // resolve+harvest+derive via the reused `GithubPort`, and renders the
    // proposals. Persists NOTHING (BR-VIEW-2 / I-VIEW-1).
    if method == Method::POST && path == "/scrape" {
        return Ok(scrape_post(req, github.as_deref()).await);
    }
    if method != Method::GET {
        return Ok(not_found());
    }
    match path.as_str() {
        "/" => Ok(landing_page()),
        "/claims" => Ok(claims_page(store.as_ref(), query.as_deref())),
        // `GET /peer-claims` — the Peer Claims view (US-VIEW-003). A SEPARATE
        // route from `/claims` so "mine vs federated" is never ambiguous
        // (BR-VIEW-5).
        "/peer-claims" => Ok(peer_claims_page(store.as_ref())),
        // `GET /scrape` — the empty target form (AC-005.1 GET). Pure render; no
        // network, no store read. 200 even when no `GithubPort` is wired (the
        // form is harmless; only a POST runs the live harvest).
        "/scrape" => Ok(html_ok(render_scrape_page(&ScrapeState::Form))),
        _ => match path.strip_prefix("/claims/") {
            // `GET /claims/{cid}` — the claim detail view (US-VIEW-002). A
            // non-empty CID segment routes to the detail handler; everything
            // else is 404.
            Some(cid) if !cid.is_empty() => Ok(claim_detail_page(store.as_ref(), cid)),
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
fn claims_page(store: &dyn StoreReadPort, query: Option<&str>) -> Response<Full<Bytes>> {
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
    html_ok(render_claims_page(&page_view))
}

/// Render the Peer Claims page (`GET /peer-claims`, US-VIEW-003): read the
/// federated `peer_claims` over the read-only store (first page), project the
/// boundary rows into the pure view-model, and render via `viewer-domain`. A
/// SEPARATE surface from the My Claims page (BR-VIEW-5). A store read failure
/// degrades to an empty guided page rather than a crash (the viewer never shows
/// a raw stack trace; NFR-VIEW-6).
fn peer_claims_page(store: &dyn StoreReadPort) -> Response<Full<Bytes>> {
    let request = PageRequest {
        offset: 0,
        limit: DEFAULT_PAGE_SIZE,
    };
    let rows = match store.list_peer_claims(request) {
        Ok(page) => page
            .rows
            .iter()
            .map(PeerClaimRowView::from_row)
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    let page_view = PageView::new(rows);
    html_ok(render_peer_claims_page(&page_view))
}

/// Render one claim's detail page (`GET /claims/{cid}`, US-VIEW-002): read the
/// claim + its ordinal-ordered evidence over the read-only store, project into
/// the pure detail view-model, and render via `viewer-domain`. The `Some` (known
/// CID) path renders `200`; the `None` (unknown CID) + read-error paths render
/// the GUIDED not-found page (the pure `render_error` — plain-language message +
/// back link to /claims, FR-VIEW-3 / NFR-VIEW-6) at `404`. A read error degrades
/// to the SAME guided page rather than leaking a raw cause (no stack trace).
fn claim_detail_page(store: &dyn StoreReadPort, cid: &str) -> Response<Full<Bytes>> {
    match store.get_claim(cid) {
        Ok(Some(detail)) => {
            let view = ClaimDetailView::from_detail(&detail);
            html_ok(render_claim_detail(&view))
        }
        // Unknown CID / read failure: the GUIDED 404 (render_error) — message +
        // back link, never a raw cause (NFR-VIEW-6).
        Ok(None) | Err(_) => html_not_found(render_error()),
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
async fn scrape_post(
    req: Request<Incoming>,
    github: Option<&dyn GithubPort>,
) -> Response<Full<Bytes>> {
    let body = read_request_body(req).await;
    let target = parse_form_target(&body);

    // No `GithubPort` wired (a store-only viewer somehow received a POST) — render
    // the guided message; the live propose step is unavailable.
    let Some(github) = github else {
        return html_ok(render_scrape_page(&ScrapeState::Guidance(
            SCRAPE_NO_CANDIDATES_NOTICE.to_string(),
        )));
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
    html_ok(render_scrape_page(&state))
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
