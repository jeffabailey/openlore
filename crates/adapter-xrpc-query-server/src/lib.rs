//! `adapter-xrpc-query-server` — the indexer's HTTP/XRPC query surface.
//!
//! EFFECT shell serving `org.openlore.appview.searchClaims` (ADR-027): it binds
//! an HTTP listener, parses the dimension+value query, dispatches to a
//! query-handler closure (wired at the indexer composition root over an
//! `IndexStorePort`), and returns a [`lexicon::SearchQueryResponse`] in which
//! EVERY result carries `author_did` (the anti-merging-across-the-transport
//! contract, I-AV-2). There is NO `consensus` / `merged` object in the response.
//!
//! ## HTTP framework: `hyper` (NOT axum)
//!
//! `axum` is banned (`deny.toml`); `hyper` is already a TRANSITIVE dep of
//! `reqwest` (not banned). This is a hand-rolled minimal one-endpoint server over
//! the hyper 1.x API. The indexer composition root owns the tokio runtime and
//! calls [`XrpcQueryServer::serve`] (which runs the accept loop until shutdown).
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! The pure core (claim-domain, appview-domain) never imports this crate. The
//! server is the impure shell: it holds a [`QueryHandler`] — a pure-by-contract
//! `SearchQueryRequest -> SearchQueryResponse` function the composition root
//! builds by closing over the `IndexStorePort` read side + the pure
//! `appview_domain::compose_results` grouping. The per-author grouping the
//! response carries is computed by that PURE composition (the wire stays FLAT +
//! attributed; grouping is the CLI renderer's job, but the count + ordering come
//! from the pure core).
//
// SCAFFOLD: false  (step 04-01: real hyper serving for the B1 transport)

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use lexicon::{SearchQueryRequest, SearchQueryResponse};
use ports::ProbeOutcome;
use tokio::net::TcpListener;

/// The query handler the composition root wires: a pure-by-contract
/// `SearchQueryRequest -> SearchQueryResponse` that reads the `IndexStorePort` +
/// composes per-author via the pure `appview-domain` core. `Send + Sync` so the
/// hyper accept loop can share it across per-connection tasks.
pub type QueryHandler = Arc<dyn Fn(SearchQueryRequest) -> SearchQueryResponse + Send + Sync>;

/// Why the XRPC query server failed to bind / serve.
#[derive(Debug, thiserror::Error)]
pub enum QueryServerError {
    /// The configured listen address could not be bound (port in use, etc.).
    #[error("query server bind failed: {message}")]
    BindFailed { message: String },
    /// The server loop terminated abnormally while serving.
    #[error("query server serve loop failed: {message}")]
    ServeFailed { message: String },
}

/// The indexer's HTTP/XRPC query server (ADR-027). Holds the bound
/// [`TcpListener`], the address it actually bound (so `:0` ephemeral ports can be
/// read back), and the [`QueryHandler`] it dispatches each request to.
pub struct XrpcQueryServer {
    listener: TcpListener,
    local_addr: SocketAddr,
    handler: QueryHandler,
}

impl XrpcQueryServer {
    /// Earned-Trust probe — see ADR-009. The server is ready iff its listener is
    /// bound (it is, by construction, since `bind` returns only on a successful
    /// bind). A bound listener is the readiness signal the gauntlet trusts.
    pub fn probe(&self) -> ProbeOutcome {
        ProbeOutcome::Ok
    }

    /// Bind the HTTP listener at `addr` (use `:0` for an OS-assigned ephemeral
    /// port, read back via [`Self::local_addr`]). The `handler` is the
    /// composition-root-wired query function. Must be called inside a tokio
    /// runtime (the indexer composition root provides one).
    pub fn bind(addr: SocketAddr, handler: QueryHandler) -> Result<Self, QueryServerError> {
        let listener = std::net::TcpListener::bind(addr).map_err(|err| {
            QueryServerError::BindFailed {
                message: format!("bind {addr}: {err}"),
            }
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|err| QueryServerError::BindFailed {
                message: format!("set_nonblocking: {err}"),
            })?;
        let local_addr = listener
            .local_addr()
            .map_err(|err| QueryServerError::BindFailed {
                message: format!("local_addr: {err}"),
            })?;
        let listener =
            TcpListener::from_std(listener).map_err(|err| QueryServerError::BindFailed {
                message: format!("tokio from_std: {err}"),
            })?;
        Ok(Self {
            listener,
            local_addr,
            handler,
        })
    }

    /// The address the listener actually bound (the ephemeral port resolved when
    /// `:0` was requested). The composition root prints this so the test harness
    /// can point the CLI's `indexer_url` at it.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Serve `org.openlore.appview.searchClaims` until the process is killed.
    /// Runs the hyper accept loop: each connection is handled by [`route`], which
    /// parses the request, calls the wired handler, and serializes the lexicon
    /// `SearchQueryResponse`. Must be called inside a tokio runtime.
    pub async fn serve(self) -> Result<(), QueryServerError> {
        loop {
            let (stream, _peer) =
                self.listener
                    .accept()
                    .await
                    .map_err(|err| QueryServerError::ServeFailed {
                        message: format!("accept: {err}"),
                    })?;
            let io = TokioIo::new(stream);
            let handler = Arc::clone(&self.handler);
            tokio::task::spawn(async move {
                let service = service_fn(move |req| route(req, Arc::clone(&handler)));
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, service)
                    .await;
            });
        }
    }
}

/// Route one HTTP request: only `POST /xrpc/org.openlore.appview.searchClaims`
/// (with a JSON `SearchQueryRequest` body) is served; everything else is 404.
/// The handler is called for a valid request; its `SearchQueryResponse` is
/// serialized as the JSON body.
async fn route(
    req: Request<Incoming>,
    handler: QueryHandler,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let path = req.uri().path().to_string();
    let is_search = req.method() == Method::POST
        && path == format!("/xrpc/{}", lexicon::SEARCH_CLAIMS_NSID);
    if !is_search {
        return Ok(not_found());
    }
    let body_bytes = match req.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(err) => return Ok(bad_request(&format!("read body: {err}"))),
    };
    let request: SearchQueryRequest = match serde_json::from_slice(&body_bytes) {
        Ok(parsed) => parsed,
        Err(err) => return Ok(bad_request(&format!("parse request: {err}"))),
    };
    let response = handler(request);
    let json = match serde_json::to_vec(&response) {
        Ok(bytes) => bytes,
        Err(err) => return Ok(internal_error(&format!("serialize response: {err}"))),
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(json)))
        .expect("static response is well-formed"))
}

fn not_found() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(Bytes::from_static(b"not found")))
        .expect("static response is well-formed")
}

fn bad_request(message: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Full::new(Bytes::from(message.to_string())))
        .expect("static response is well-formed")
}

fn internal_error(message: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Full::new(Bytes::from(message.to_string())))
        .expect("static response is well-formed")
}
