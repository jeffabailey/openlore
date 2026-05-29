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
use lexicon::{SearchDimensionDto, SearchQueryRequest, SearchQueryResponse};
use ports::{ProbeOutcome, ProbeRefusalReason};
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
    /// Earned-Trust probe — see ADR-009 §6.3. The server's listener is bound by
    /// construction (`bind` returns only on a successful bind). The load-bearing
    /// substrate-lie check is the ANTI-MERGING-ACROSS-THE-TRANSPORT contract
    /// (I-AV-2 / D-D36): the response shape this server serves MUST carry a
    /// non-empty `author_did` on EVERY result row. A response that dropped
    /// attribution is a contract violation caught HERE at probe time, before the
    /// server accepts traffic — not trusted, PROVEN.
    ///
    /// The probe is a SELF-PROBE: it dispatches a sentinel `SearchQueryRequest`
    /// through the SAME wired [`QueryHandler`] the accept loop uses and asserts
    /// every returned row carries a non-empty `author_did`. An empty result is
    /// vacuously safe (no row dropped attribution). The handler is pure-by-contract
    /// and side-effect-free over a read-only store, so the sentinel dispatch is
    /// safe to run at startup within the 250ms probe budget.
    pub fn probe(&self) -> ProbeOutcome {
        // A sentinel dimension query through the wired handler — the SAME path the
        // accept loop dispatches. We do not assert on the CONTENT (the index may be
        // empty); we assert the SHAPE invariant: no returned row drops author_did.
        let sentinel = SearchQueryRequest {
            dimension: SearchDimensionDto::Object,
            value: "org.openlore.appview.__probe__".to_string(),
            cid: None,
        };
        let response = (self.handler)(sentinel);
        for (index, row) in response.results.iter().enumerate() {
            if row.author_did.trim().is_empty() {
                return ProbeOutcome::Refused {
                    reason: ProbeRefusalReason::LexiconInvalid,
                    detail: format!(
                        "searchClaims response row {index} dropped author_did \
                         (anti-merging across the transport violated; I-AV-2/D-D36)"
                    ),
                    structured: serde_json::json!({
                        "contract": "anti_merging_across_transport",
                        "violation": "empty_author_did",
                        "row_index": index,
                    }),
                };
            }
        }
        ProbeOutcome::Ok
    }

    /// Bind the HTTP listener at `addr` (use `:0` for an OS-assigned ephemeral
    /// port, read back via [`Self::local_addr`]). The `handler` is the
    /// composition-root-wired query function. Must be called inside a tokio
    /// runtime (the indexer composition root provides one).
    pub fn bind(addr: SocketAddr, handler: QueryHandler) -> Result<Self, QueryServerError> {
        let listener =
            std::net::TcpListener::bind(addr).map_err(|err| QueryServerError::BindFailed {
                message: format!("bind {addr}: {err}"),
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
    let is_search =
        req.method() == Method::POST && path == format!("/xrpc/{}", lexicon::SEARCH_CLAIMS_NSID);
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

#[cfg(test)]
mod tests {
    //! DELIVER inner loop (step 04-06): the author_did-present self-probe — the
    //! anti-merging-across-the-transport contract (I-AV-2 / D-D36 / DESIGN §6.3).
    //! The probe dispatches a sentinel request through the wired handler and
    //! refuses if ANY returned row drops `author_did`. Pure (no socket I/O on the
    //! probe path); the live transport is exercised end-to-end by AV-14.

    use super::*;
    use lexicon::SearchResultDto;
    use std::net::SocketAddr;

    fn row(author_did: &str) -> SearchResultDto {
        SearchResultDto {
            author_did: author_did.to_string(),
            cid: "bafyprobe".to_string(),
            subject: "github:bazelbuild/bazel".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.reproducible-builds".to_string(),
            confidence: 0.82,
            composed_at: "2026-05-28T00:00:00Z".to_string(),
            verified_against: "did:plc:priya-test#org.openlore.application".to_string(),
            evidence: vec!["https://example.org/e1".to_string()],
        }
    }

    /// Bind a server on an ephemeral localhost port over a handler that returns
    /// `rows`, so `probe()` can dispatch its sentinel through the SAME handler.
    fn server_serving(rows: Vec<SearchResultDto>) -> XrpcQueryServer {
        let addr: SocketAddr = "127.0.0.1:0".parse().expect("ephemeral addr parses");
        let handler: QueryHandler = Arc::new(move |_req: SearchQueryRequest| SearchQueryResponse {
            distinct_author_count: rows.len() as u32,
            total_claims: rows.len() as u32,
            results: rows.clone(),
            suggestion: None,
        });
        XrpcQueryServer::bind(addr, handler).expect("bind ephemeral query server")
    }

    /// The author_did-present probe ACCEPTS a handler whose response carries a
    /// non-empty `author_did` on every row (the contract holds — Earned Trust).
    #[tokio::test]
    async fn probe_accepts_a_response_with_author_did_present_on_every_row() {
        let server = server_serving(vec![row("did:plc:priya-test"), row("did:plc:rachel-test")]);
        assert!(
            matches!(server.probe(), ProbeOutcome::Ok),
            "a response carrying author_did on every row must probe Ok"
        );
    }

    /// An EMPTY result is vacuously safe (no row dropped attribution) — the probe
    /// asserts the SHAPE invariant, not that the index is populated.
    #[tokio::test]
    async fn probe_accepts_an_empty_result() {
        let server = server_serving(Vec::new());
        assert!(
            matches!(server.probe(), ProbeOutcome::Ok),
            "an empty result drops no attribution; the probe must accept it"
        );
    }

    /// The load-bearing substrate-lie check: a handler that would serve a row with
    /// a DROPPED (empty) `author_did` is REFUSED at probe time (anti-merging across
    /// the transport violated; I-AV-2 / D-D36) — the contract is PROVEN, not trusted.
    #[tokio::test]
    async fn probe_refuses_a_response_that_dropped_author_did() {
        let server = server_serving(vec![row("did:plc:priya-test"), row("   ")]);
        match server.probe() {
            ProbeOutcome::Refused { reason, .. } => assert_eq!(
                reason,
                ProbeRefusalReason::LexiconInvalid,
                "a dropped author_did must refuse with the lexicon-contract reason"
            ),
            ProbeOutcome::Ok => {
                panic!("a response that dropped author_did must be REFUSED (I-AV-2/D-D36)")
            }
        }
    }
}
