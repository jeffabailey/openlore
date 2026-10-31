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

mod dispatch;
pub mod rate_limit;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use appview_domain::health::HealthResponse;
use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use lexicon::{SearchDimensionDto, SearchQueryRequest, SearchQueryResponse};
use ports::{ProbeOutcome, ProbeRefusalReason};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

pub use dispatch::{dispatch, BodyRead, Handlers, PublicAnswer, PublicRequest};
use rate_limit::{client_ip, RateDecision, RateLimiter};
pub use rate_limit::{RateLimit, TrustedProxies};

/// The query handler the composition root wires: a pure-by-contract
/// `SearchQueryRequest -> SearchQueryResponse` that reads the `IndexStorePort` +
/// composes per-author via the pure `appview-domain` core. Fallible (ADR-080
/// §7, B12): a store that cannot be read is [`IndexUnavailable`], answered with
/// a 500, never an empty 200 that would read as "no results". `Send + Sync` so
/// the hyper accept loop can share it across per-connection tasks.
pub type QueryHandler =
    Arc<dyn Fn(SearchQueryRequest) -> Result<SearchQueryResponse, IndexUnavailable> + Send + Sync>;

/// The index could not be read for a search. Carries nothing about the
/// request: the 500 it becomes names no query value (ADR-083 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexUnavailable;

/// The health reader the composition root wires behind `GET /healthz` (ADR-083
/// §2): it reads the process's state and projects it, never changing it.
pub type HealthHandler = Arc<dyn Fn() -> HealthResponse + Send + Sync>;

/// The public health route.
pub const HEALTH_PATH: &str = "/healthz";

/// The largest request body the public listener reads (ADR-083 §3): one byte
/// more is 413, and the body is never buffered past it.
pub const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024;

/// The longest search `value`, in bytes (ADR-083 §3): one byte more is 400.
pub const MAX_SEARCH_VALUE_BYTES: usize = 512;

/// How long a connection may take to send its request headers (ADR-083 §3).
/// hyper restarts this timer whenever a kept-alive connection goes idle
/// waiting for its next request, so it is also the keep-alive idle bound.
pub const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a search may take from its headers to its answer: reading the
/// body (a slower body is 408 and the connection closes) and running the
/// query (a slower query is 503) (review H2).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The public listener's bounds: how long a connection may idle or take, how
/// often one client may ask, and whose `X-Forwarded-For` names the client.
#[derive(Debug, Clone, Default)]
pub struct ServeLimits {
    pub timeouts: ServeTimeouts,
    pub rate: RateLimit,
    pub trusted_proxies: TrustedProxies,
}

/// The listener's two clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServeTimeouts {
    /// Headers of a request, and the idle wait for the next one.
    pub header_read: Duration,
    /// A search's body read plus its query.
    pub request: Duration,
}

impl Default for ServeTimeouts {
    fn default() -> Self {
        Self {
            header_read: HEADER_READ_TIMEOUT,
            request: REQUEST_TIMEOUT,
        }
    }
}

/// How many connections are served at once (ADR-083 §3); further accepts wait.
pub const MAX_CONCURRENT_CONNECTIONS: usize = 64;

/// What the public listener does with a method and path (ADR-083 §1). The
/// table is closed: exactly two routes exist, everything else is not found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicRoute {
    /// `POST /xrpc/org.openlore.appview.searchClaims`.
    Search,
    /// `GET /healthz`.
    Health,
    /// Any other method or path, near misses included.
    NotFound,
}

/// Route a request by its exact method and path (case-sensitive, no trailing
/// slash tolerance): the binary's own allowlist, independent of Caddy.
#[must_use]
pub fn public_route(method: &str, path: &str) -> PublicRoute {
    let is_search_path = path
        .strip_prefix("/xrpc/")
        .is_some_and(|nsid| nsid == lexicon::SEARCH_CLAIMS_NSID);
    match method {
        "POST" if is_search_path => PublicRoute::Search,
        "GET" if path == HEALTH_PATH => PublicRoute::Health,
        _ => PublicRoute::NotFound,
    }
}

/// Whether a search request is within the public bounds (ADR-083 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Within both bounds: the search runs.
    Admitted,
    /// The body is over [`MAX_REQUEST_BODY_BYTES`] (413; checked first).
    TooLarge,
    /// The `value` is over [`MAX_SEARCH_VALUE_BYTES`] (400).
    BadRequest,
}

/// Admit a search by its body length and its `value` length, both in bytes.
#[must_use]
pub fn admit(body_len: usize, value_len: usize) -> Admission {
    if body_len > MAX_REQUEST_BODY_BYTES {
        Admission::TooLarge
    } else if value_len > MAX_SEARCH_VALUE_BYTES {
        Admission::BadRequest
    } else {
        Admission::Admitted
    }
}

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
    health: Option<HealthHandler>,
    limits: ServeLimits,
}

/// What every connection's requests are answered with.
struct Answering {
    handlers: Handlers,
    timeouts: ServeTimeouts,
    trusted_proxies: TrustedProxies,
    limiter: RateLimiter,
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
        let Ok(response) = (self.handler)(sentinel) else {
            return ProbeOutcome::Refused {
                reason: ProbeRefusalReason::StorageSchemaMismatch,
                detail: "searchClaims could not read the index".to_string(),
                structured: serde_json::json!({
                    "contract": "search_reads_the_index",
                    "violation": "index_unavailable",
                }),
            };
        };
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
            health: None,
            limits: ServeLimits::default(),
        })
    }

    /// Serve under `limits` instead of the defaults.
    #[must_use]
    pub fn with_limits(self, limits: ServeLimits) -> Self {
        Self { limits, ..self }
    }

    /// Answer `GET /healthz` from `health` (without it the route is not found).
    #[must_use]
    pub fn with_health(self, health: HealthHandler) -> Self {
        Self {
            health: Some(health),
            ..self
        }
    }

    /// The address the listener actually bound (the ephemeral port resolved when
    /// `:0` was requested). The composition root prints this so the test harness
    /// can point the CLI's `indexer_url` at it.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Serve `org.openlore.appview.searchClaims` until the process is killed.
    /// Runs the hyper accept loop under the connection cap: each connection is
    /// handled by [`route`], which applies the rate limit and calls [`dispatch`].
    /// Must be called inside a tokio runtime.
    pub async fn serve(self) -> Result<(), QueryServerError> {
        let connection_slots = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));
        let answering = Arc::new(Answering {
            handlers: Handlers {
                query: self.handler,
                health: self.health,
            },
            timeouts: self.limits.timeouts,
            trusted_proxies: self.limits.trusted_proxies,
            limiter: RateLimiter::new(self.limits.rate),
        });
        loop {
            let slot = Arc::clone(&connection_slots)
                .acquire_owned()
                .await
                .map_err(|err| QueryServerError::ServeFailed {
                    message: format!("connection slots: {err}"),
                })?;
            let (stream, peer) =
                self.listener
                    .accept()
                    .await
                    .map_err(|err| QueryServerError::ServeFailed {
                        message: format!("accept: {err}"),
                    })?;
            let io = TokioIo::new(stream);
            let answering = Arc::clone(&answering);
            tokio::task::spawn(async move {
                let header_read = answering.timeouts.header_read;
                let service = service_fn(move |req| route(req, Arc::clone(&answering), peer.ip()));
                let _ = hyper::server::conn::http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(header_read)
                    .serve_connection(io, service)
                    .await;
                drop(slot);
            });
        }
    }
}

/// Answer one HTTP request: every request but `/healthz` first spends one of
/// its client's tokens (429 when there is none), then [`dispatch`] answers it.
/// The rate limit lives here, in front of the transport-free surface (D-STZ-19).
async fn route(
    req: Request<Incoming>,
    answering: Arc<Answering>,
    peer: IpAddr,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    if public_route(req.method().as_str(), req.uri().path()) != PublicRoute::Health {
        let client = client_ip(peer, forwarded_for(&req), &answering.trusted_proxies);
        if let RateDecision::Limited { retry_after } =
            answering.limiter.check(client, Instant::now())
        {
            return Ok(too_many_requests(retry_after));
        }
    }
    let (parts, body) = req.into_parts();
    let request = PublicRequest {
        method: parts.method.as_str(),
        path: parts.uri.path(),
        body: read_bounded(body),
        time_allowed: answering.timeouts.request,
    };
    Ok(http_response(dispatch(request, &answering.handlers).await))
}

/// The last `X-Forwarded-For` header, when it is text.
fn forwarded_for(req: &Request<Incoming>) -> Option<&str> {
    req.headers()
        .get_all("x-forwarded-for")
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok())
}

/// Read a body off the connection, never buffering past
/// [`MAX_REQUEST_BODY_BYTES`].
async fn read_bounded(body: Incoming) -> BodyRead {
    match Limited::new(body, MAX_REQUEST_BODY_BYTES).collect().await {
        Ok(collected) => BodyRead::Complete(collected.to_bytes()),
        Err(err) if err.downcast_ref::<LengthLimitError>().is_some() => BodyRead::TooLarge,
        Err(err) => BodyRead::Unreadable(err.to_string()),
    }
}

/// A transport-free answer as an HTTP response.
fn http_response(answer: PublicAnswer) -> Response<Full<Bytes>> {
    let builder = Response::builder().status(answer.status);
    let builder = answer
        .headers
        .into_iter()
        .fold(builder, |builder, (name, value)| {
            builder.header(name, value)
        });
    builder
        .body(Full::new(answer.body))
        .expect("static response is well-formed")
}

/// Over the client's rate: 429 with the seconds to wait.
fn too_many_requests(retry_after: Duration) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("retry-after", retry_after.as_secs().max(1).to_string())
        .body(Full::new(Bytes::from_static(b"too many requests")))
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
            references: Vec::new(),
            provenance: None,
        }
    }

    /// Bind a server on an ephemeral localhost port over a handler that returns
    /// `rows`, so `probe()` can dispatch its sentinel through the SAME handler.
    fn server_serving(rows: Vec<SearchResultDto>) -> XrpcQueryServer {
        let addr: SocketAddr = "127.0.0.1:0".parse().expect("ephemeral addr parses");
        let handler: QueryHandler = Arc::new(move |_req: SearchQueryRequest| {
            Ok(SearchQueryResponse {
                distinct_author_count: rows.len() as u32,
                total_claims: rows.len() as u32,
                results: rows.clone(),
                suggestion: None,
            })
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
