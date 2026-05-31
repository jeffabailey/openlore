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
use ports::{PageRequest, StoreReadPort};
use tokio::net::TcpListener;
use viewer_domain::{
    render_claim_detail, render_claims_page, render_error, render_landing,
    render_peer_claims_page, ClaimDetailView, ClaimRowView, PageView, PeerClaimRowView,
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

/// The default page size for the My Claims list view (ADR-030). For the walking
/// skeleton a single ordered read of the first page is enough; full pagination
/// (the `?page=N` offset math) lands in step 04-01.
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

    /// Bind the HTTP listener at `addr` (use `:0` for an OS-assigned ephemeral
    /// port, read back via [`Self::local_addr`]). REFUSES any non-loopback
    /// address — the viewer is localhost-only (I-VIEW-4). Must be called inside a
    /// tokio runtime (the cli composition root provides one).
    pub fn bind(addr: SocketAddr, store: SharedStore) -> Result<Self, ViewerServerError> {
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
            tokio::task::spawn(async move {
                let service = service_fn(move |req| route(req, Arc::clone(&store)));
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, service)
                    .await;
            });
        }
    }
}

/// Route one HTTP request. Serves `GET /` (the read-only landing page that states
/// the view is read-only — AC-001.2 / NFR-VIEW-1) and `GET /claims` (the My Claims
/// list); everything else is 404.
async fn route(
    req: Request<Incoming>,
    store: SharedStore,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    if method != Method::GET {
        return Ok(not_found());
    }
    match path.as_str() {
        "/" => Ok(landing_page()),
        "/claims" => Ok(claims_page(store.as_ref())),
        // `GET /peer-claims` — the Peer Claims view (US-VIEW-003). A SEPARATE
        // route from `/claims` so "mine vs federated" is never ambiguous
        // (BR-VIEW-5).
        "/peer-claims" => Ok(peer_claims_page(store.as_ref())),
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

/// Render the My Claims page: read the read-only store (first page), project the
/// boundary rows into the pure view-model, and render via `viewer-domain`. A
/// store read failure degrades to an empty guided page rather than a crash
/// (the viewer never shows a raw stack trace; NFR-VIEW-6).
fn claims_page(store: &dyn StoreReadPort) -> Response<Full<Bytes>> {
    let request = PageRequest {
        offset: 0,
        limit: DEFAULT_PAGE_SIZE,
    };
    let rows = match store.list_claims(request) {
        Ok(page) => page
            .rows
            .iter()
            .map(ClaimRowView::from_row)
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    let page_view = PageView::new(rows);
    let html = render_claims_page(&page_view);
    html_ok(html)
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

/// A `200 OK` HTML response.
fn html_ok(body: String) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(body)))
        .expect("static response is well-formed")
}

fn not_found() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from_static(b"<p>Not found.</p>")))
        .expect("static response is well-formed")
}

/// A `404 Not Found` HTML response carrying a rendered `body` — used for the
/// GUIDED not-found page (unknown CID), which returns `404` (AC-002.3) yet shows
/// the operator a plain-language message + back link (NFR-VIEW-6) rather than the
/// terse route-miss `not_found()` body.
fn html_not_found(body: String) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(body)))
        .expect("static response is well-formed")
}
