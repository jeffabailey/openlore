//! `FakeAtprotoNetwork` — hermetic double for the ATProto systems the
//! bluesky-claim-review-app (`openlore-review-app`, ADR-072) talks to.
//!
//! DISTILL 2026-10-04. Per the Architecture of Reference these are
//! driven-EXTERNAL ports, so they are faked; everything inside the app is real.
//! One `FakeAtprotoNetwork` hosts, on loopback:
//!
//! * a **directory** server — the PLC directory (`GET /{did}`) plus handle
//!   resolution (`com.atproto.identity.resolveHandle`) and `resolveDid`;
//! * one server per **PDS host** (e.g. `bsky-social`, `volkov-dev`), each both
//!   the user's PDS (resource server) and its OAuth **authorization server**,
//!   the way a self-hosted PDS is:
//!   * `/.well-known/oauth-protected-resource`, `/.well-known/oauth-authorization-server`;
//!   * `POST /oauth/par` (PAR; DPoP required; first call answers `use_dpop_nonce`;
//!     the server FETCHES the app's `client_id` metadata document, like a real
//!     PDS — so AC-000.1 is observed from Bluesky's side);
//!   * `GET /oauth/authorize` (the user's consent screen; the per-account
//!     [`ConsentPosture`] decides approve vs cancel);
//!   * `POST /oauth/token` (`authorization_code` with PKCE S256 + `refresh_token`;
//!     [`TokenPosture`] can fail the code exchange — SPIKE finding 3 — or return a
//!     `sub` that differs from the resolved DID — AC-001.6);
//!   * `POST /oauth/revoke` — answers **200** with an empty body, which is what
//!     RFC 7009 and bsky.social do (SPIKE finding 1: `atrium-oauth` 0.1.7 expects
//!     204). Revocation kills the refresh token but NOT the access token already
//!     issued (SPIKE finding 2) — observable via [`FakeAtprotoNetwork::access_token_still_accepted`];
//!   * XRPC: `com.atproto.repo.createRecord` (DPoP-bound bearer, granular
//!     `repo:<collection>?action=create` scope enforced like SPIKE-3),
//!     `listRecords`, `getRecord`, `describeRepo`, `resolveDid`;
//!   * `deleteRecord` / `putRecord` / `applyWrites` are REFUSED (403, as with a
//!     create-only grant) and every attempt is recorded in
//!     [`FakeAtprotoNetwork::forbidden_write_attempts`] (I-BRA-8 oracle).
//!
//! The double validates *claims* (DPoP `htm` + `nonce`, PKCE, `client_id`,
//! scopes, repo == token subject) but never *signatures*; signature-level
//! fidelity is the nightly live contract smoke (DV-BRA-12).
//!
//! Observations (`records`, `write_attempts`, `authorizations`,
//! `client_metadata_seen`, `revocations`) are the port-exposed universe the
//! acceptance tests assert on.
//!
//! ## Indexer-facing postures (indexer-per-did-pds-fetch DISTILL, 2026-10-05)
//!
//! The network indexer resolves every repo DID to its own PDS and lists it there
//! (ADR-077/078), so the same double also serves as a **multi-PDS network** for
//! the `openlore-indexer` acceptance suites:
//!
//! * [`ListingPosture`] per host — serve, answer a status (502/503/429/404),
//!   answer non-JSON, redirect, answer slowly, or hang (the substrate lies of
//!   architecture-design.md §9);
//! * [`DidDocPosture`] per DID — published, 404, 500, hang, `id` mismatch, no
//!   `#atproto_pds` service, or an advertised PDS endpoint override (e.g. a
//!   private address literal for the SSRF guard);
//! * [`FakeAtprotoNetwork::move_account`] — the DID document names another host
//!   from now on (a PDS migration between passes);
//! * [`FakeAtprotoNetwork::serve_repo_as`] — a host answers `listRecords` for
//!   one repo with the records of another (the foreign-repo lie);
//! * an ordered request log ([`RequestSeen`]) and the peak number of requests in
//!   flight across the whole network ([`FakeAtprotoNetwork::max_requests_in_flight`]).
//!
//! Records live in one network-wide store: any host answers `listRecords` for
//! any repo (that is what lets a host stand in for a relay or a fallback). Which
//! host was asked is observable through the request log.

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::review_http::{
    body_string, empty, header, http_get, json, json_with_headers, jwt_claims, parse_form,
    pkce_s256, redirect, short_hash, test_runtime, url_encode, AbortOnDrop, HttpRequest,
    HttpResponse,
};

/// The OpenLore claim collection (ADR-071: unchanged lexicon).
pub const CLAIM_COLLECTION: &str = "org.openlore.claim";
/// The Bluesky post collection (the opt-in share post, D-11).
pub const POST_COLLECTION: &str = "app.bsky.feed.post";

/// A Bluesky account hosted on one of the fake PDS hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueskyAccount {
    pub handle: String,
    pub did: String,
    /// The PDS host label (e.g. `bsky-social`, `volkov-dev`).
    pub host: String,
}

impl BlueskyAccount {
    pub fn new(handle: &str, did: &str, host: &str) -> Self {
        Self {
            handle: handle.to_string(),
            did: did.to_string(),
            host: host.to_string(),
        }
    }
}

/// What the user does on their PDS's authorization screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentPosture {
    Approve,
    Cancel,
}

/// How the authorization server answers the token endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPosture {
    Honest,
    /// The code exchange fails (`400 invalid_grant`) — the path on which
    /// `atrium-oauth` 0.1.7 hits `todo!()` (SPIKE finding 3).
    ExchangeFails,
    /// The token response names a different `sub` than the resolved DID.
    SubjectMismatch(String),
}

/// How the PDS answers record writes for an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WritePosture {
    Accept,
    /// Every write answers `500`.
    ServerError,
    /// Writes into this collection are refused (`400 InvalidRequest`).
    RefuseCollection(String),
    /// The PDS-side session has expired: writes answer `401 invalid_token`
    /// and the refresh grant is refused (the user must sign in again).
    ExpiredSession,
}

/// A record held in a fake repo.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredRecord {
    pub repo: String,
    pub collection: String,
    pub rkey: String,
    pub value: serde_json::Value,
}

impl StoredRecord {
    pub fn uri(&self) -> String {
        format!("at://{}/{}/{}", self.repo, self.collection, self.rkey)
    }
}

/// How a PDS host answers `com.atproto.repo.listRecords` (indexer postures).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListingPosture {
    /// Answer with the repo's records (the default).
    Serve,
    /// Answer with this HTTP status and a small JSON error body (e.g. 502, 503, 429, 404).
    Status(u16),
    /// Answer `200` with an HTML body (not JSON).
    NotJson,
    /// Answer `302` to this location.
    RedirectTo(String),
    /// Wait this long, then serve normally.
    Slow(std::time::Duration),
    /// Accept the request and never answer.
    Hang,
}

/// How the directory answers a DID-document request for one DID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DidDocPosture {
    /// The honest document naming the account's current host (the default).
    Published,
    /// `404` — the DID is not known.
    NotFound,
    /// `500`.
    ServerError,
    /// Accept the request and never answer.
    Hang,
    /// A document whose `id` is a different DID.
    IdMismatch,
    /// A document with no `#atproto_pds` service.
    NoPdsService,
    /// A document whose `#atproto_pds` endpoint is this string (e.g. `http://10.0.0.1`).
    PdsEndpoint(String),
}

/// One request any server of the network received, in arrival order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSeen {
    /// `"directory"` or the PDS host label.
    pub server: String,
    pub method: String,
    pub path: String,
    /// The `repo` query parameter (listRecords / getRecord / describeRepo).
    pub repo: Option<String>,
    /// The DID a DID-document request asked for.
    pub did: Option<String>,
}

impl RequestSeen {
    /// Whether this was a `listRecords` call.
    pub fn is_listing(&self) -> bool {
        self.path == "/xrpc/com.atproto.repo.listRecords"
    }
}

/// One XRPC write attempt (accepted or not), in arrival order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteAttempt {
    pub host: String,
    pub repo: String,
    pub nsid: String,
    pub collection: String,
    pub status: u16,
}

/// One visit to an authorization screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationSeen {
    pub host: String,
    pub did: String,
    pub scope: String,
    pub client_id: String,
}

/// One call to the revocation endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationSeen {
    pub host: String,
    pub revoked_a_refresh_token: bool,
    pub status: u16,
}

#[derive(Debug, Clone)]
struct AccountState {
    account: BlueskyAccount,
    consent: ConsentPosture,
    token: TokenPosture,
    write: WritePosture,
    did_doc: DidDocPosture,
}

#[derive(Debug, Clone)]
struct HostState {
    base_url: String,
    reachable: bool,
    client_metadata_blocked: bool,
    nonce: String,
    listing: ListingPosture,
}

#[derive(Debug, Clone)]
struct PendingPar {
    host: String,
    did: Option<String>,
    client_id: String,
    redirect_uri: String,
    state: String,
    code_challenge: String,
    scope: String,
}

#[derive(Debug, Clone)]
struct IssuedCode {
    did: String,
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    scope: String,
    used: bool,
}

#[derive(Debug, Clone)]
struct TokenRecord {
    did: String,
    scope: String,
    access: String,
    refresh: String,
    access_expired: bool,
    refresh_revoked: bool,
}

#[derive(Default)]
struct NetState {
    accounts: Mutex<Vec<AccountState>>,
    hosts: Mutex<BTreeMap<String, HostState>>,
    directory_reachable: AtomicBool,
    pars: Mutex<HashMap<String, PendingPar>>,
    codes: Mutex<HashMap<String, IssuedCode>>,
    tokens: Mutex<Vec<TokenRecord>>,
    records: Mutex<Vec<StoredRecord>>,
    writes: Mutex<Vec<WriteAttempt>>,
    forbidden: Mutex<Vec<String>>,
    authorizations: Mutex<Vec<AuthorizationSeen>>,
    revocations: Mutex<Vec<RevocationSeen>>,
    client_metadata: Mutex<Vec<serde_json::Value>>,
    token_exchanges: AtomicU64,
    seq: AtomicU64,
    listing_aliases: Mutex<HashMap<String, String>>,
    requests: Mutex<Vec<RequestSeen>>,
    in_flight: AtomicU64,
    max_in_flight: AtomicU64,
}

impl NetState {
    fn next(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn account_by_did(&self, did: &str) -> Option<AccountState> {
        self.accounts
            .lock()
            .unwrap()
            .iter()
            .find(|a| a.account.did == did)
            .cloned()
    }

    fn account_by_handle(&self, handle: &str) -> Option<AccountState> {
        let wanted = handle.trim_start_matches('@').to_ascii_lowercase();
        self.accounts
            .lock()
            .unwrap()
            .iter()
            .find(|a| a.account.handle.to_ascii_lowercase() == wanted)
            .cloned()
    }

    fn host(&self, host: &str) -> Option<HostState> {
        self.hosts.lock().unwrap().get(host).cloned()
    }

    fn update_account(&self, did: &str, f: impl FnOnce(&mut AccountState)) {
        let mut accounts = self.accounts.lock().unwrap();
        let account = accounts
            .iter_mut()
            .find(|a| a.account.did == did)
            .unwrap_or_else(|| panic!("FakeAtprotoNetwork: unknown DID {did}"));
        f(account);
    }
}

/// The hermetic ATProto network (directory + PDS/authorization hosts).
pub struct FakeAtprotoNetwork {
    state: Arc<NetState>,
    directory_url: String,
    runtime: Option<tokio::runtime::Runtime>,
    _tasks: Vec<AbortOnDrop>,
}

impl std::fmt::Debug for FakeAtprotoNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never prints tokens or codes.
        f.debug_struct("FakeAtprotoNetwork")
            .field("directory_url", &self.directory_url)
            .finish_non_exhaustive()
    }
}

impl FakeAtprotoNetwork {
    /// Start the directory plus one server per distinct `host` among `accounts`.
    pub fn start(accounts: Vec<BlueskyAccount>) -> Self {
        Self::start_with_extra_hosts(accounts, &[])
    }

    /// Like [`Self::start`], plus a server for each label in `extra_hosts`
    /// that no account lives on yet (a fallback source, a host an author will
    /// move to, a host that answers for somebody else's repo).
    pub fn start_with_extra_hosts(accounts: Vec<BlueskyAccount>, extra_hosts: &[&str]) -> Self {
        let runtime = test_runtime("fake-atproto-rt");
        let state = Arc::new(NetState::default());
        state.directory_reachable.store(true, Ordering::SeqCst);
        *state.accounts.lock().unwrap() = accounts
            .iter()
            .cloned()
            .map(|account| AccountState {
                account,
                consent: ConsentPosture::Approve,
                token: TokenPosture::Honest,
                write: WritePosture::Accept,
                did_doc: DidDocPosture::Published,
            })
            .collect();

        let mut tasks = Vec::new();
        let mut host_labels: Vec<String> = accounts
            .iter()
            .map(|a| a.host.clone())
            .chain(extra_hosts.iter().map(|h| h.to_string()))
            .collect();
        host_labels.sort();
        host_labels.dedup();
        for label in host_labels {
            let (listener, base_url) = runtime.block_on(bind());
            state.hosts.lock().unwrap().insert(
                label.clone(),
                HostState {
                    base_url: base_url.clone(),
                    reachable: true,
                    client_metadata_blocked: false,
                    nonce: format!("nonce-{label}-1"),
                    listing: ListingPosture::Serve,
                },
            );
            let st = state.clone();
            let lbl = label.clone();
            tasks.push(AbortOnDrop(runtime.spawn(serve_loop(
                listener,
                Arc::new(move |s: &NetState| s.host(&lbl).map(|h| h.reachable).unwrap_or(false)),
                st,
                Route::Host { label, base_url },
            ))));
        }

        let (listener, directory_url) = runtime.block_on(bind());
        tasks.push(AbortOnDrop(runtime.spawn(serve_loop(
            listener,
            Arc::new(|s: &NetState| s.directory_reachable.load(Ordering::SeqCst)),
            state.clone(),
            Route::Directory,
        ))));

        Self {
            state,
            directory_url,
            runtime: Some(runtime),
            _tasks: tasks,
        }
    }

    // ------------------------------------------------------------------ urls

    /// PLC directory + handle resolver base URL.
    pub fn directory_url(&self) -> &str {
        &self.directory_url
    }

    /// The base URL of a PDS host (e.g. `bsky-social`).
    pub fn host_url(&self, host: &str) -> String {
        self.state
            .host(host)
            .unwrap_or_else(|| panic!("FakeAtprotoNetwork: unknown host {host}"))
            .base_url
    }

    /// The PDS base URL serving `did` (its DID document `#atproto_pds`).
    pub fn pds_url_for(&self, did: &str) -> String {
        let account = self
            .state
            .account_by_did(did)
            .unwrap_or_else(|| panic!("FakeAtprotoNetwork: unknown DID {did}"));
        self.host_url(&account.account.host)
    }

    // -------------------------------------------------------------- postures

    pub fn set_consent(&self, did: &str, consent: ConsentPosture) {
        self.state.update_account(did, |a| a.consent = consent);
    }

    pub fn set_token_posture(&self, did: &str, token: TokenPosture) {
        self.state.update_account(did, |a| a.token = token);
    }

    pub fn set_write_posture(&self, did: &str, write: WritePosture) {
        self.state.update_account(did, |a| a.write = write);
    }

    /// Make a PDS host (and its authorization server) unreachable / reachable.
    pub fn set_host_reachable(&self, host: &str, reachable: bool) {
        if let Some(h) = self.state.hosts.lock().unwrap().get_mut(host) {
            h.reachable = reachable;
        }
    }

    /// Make the directory (PLC + handle resolution) unreachable / reachable.
    pub fn set_directory_reachable(&self, reachable: bool) {
        self.state
            .directory_reachable
            .store(reachable, Ordering::SeqCst);
    }

    /// The authorization server on `host` cannot fetch the app's client
    /// metadata (a DNS/route failure between the PDS and the app — AC-000.3).
    pub fn block_client_metadata_fetch(&self, host: &str, blocked: bool) {
        if let Some(h) = self.state.hosts.lock().unwrap().get_mut(host) {
            h.client_metadata_blocked = blocked;
        }
    }

    /// Every access token issued for `did` so far is now expired (the app must
    /// refresh — the restore path of SPIKE finding 4).
    pub fn expire_access_tokens(&self, did: &str) {
        for t in self.state.tokens.lock().unwrap().iter_mut() {
            if t.did == did {
                t.access_expired = true;
            }
        }
    }

    /// Seed a record directly into a repo (e.g. an app-signed claim, a tampered
    /// record, a claim published by another tool).
    pub fn seed_record(&self, did: &str, collection: &str, rkey: &str, value: serde_json::Value) {
        self.state.records.lock().unwrap().push(StoredRecord {
            repo: did.to_string(),
            collection: collection.to_string(),
            rkey: rkey.to_string(),
            value,
        });
    }

    /// How `host` answers `listRecords` from now on (indexer postures).
    pub fn set_listing_posture(&self, host: &str, listing: ListingPosture) {
        let mut hosts = self.state.hosts.lock().unwrap();
        let h = hosts
            .get_mut(host)
            .unwrap_or_else(|| panic!("FakeAtprotoNetwork: unknown host {host}"));
        h.listing = listing;
    }

    /// How the directory answers `did`'s DID-document request from now on.
    pub fn set_did_doc_posture(&self, did: &str, posture: DidDocPosture) {
        self.state.update_account(did, |a| a.did_doc = posture);
    }

    /// `did` migrates to `host`: its DID document names that host from now on.
    pub fn move_account(&self, did: &str, host: &str) {
        assert!(
            self.state.host(host).is_some(),
            "FakeAtprotoNetwork: unknown host {host} (start it with start_with_extra_hosts)"
        );
        self.state
            .update_account(did, |a| a.account.host = host.to_string());
    }

    /// Every host answers `listRecords?repo=<requested>` with the records of
    /// `served` (whose `at://` URIs name `served`) — the foreign-repo lie.
    pub fn serve_repo_as(&self, requested: &str, served: &str) {
        self.state
            .listing_aliases
            .lock()
            .unwrap()
            .insert(requested.to_string(), served.to_string());
    }

    // ---------------------------------------------------------- observations

    /// Records in `did`'s repo for `collection`, in creation order.
    pub fn records(&self, did: &str, collection: &str) -> Vec<StoredRecord> {
        self.state
            .records
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.repo == did && r.collection == collection)
            .cloned()
            .collect()
    }

    /// Every record in every repo, in creation order.
    pub fn all_records(&self) -> Vec<StoredRecord> {
        self.state.records.lock().unwrap().clone()
    }

    /// Every request the directory and the hosts received, in arrival order.
    pub fn requests(&self) -> Vec<RequestSeen> {
        self.state.requests.lock().unwrap().clone()
    }

    /// The repos `host` was asked to list (`listRecords?repo=`), in order.
    pub fn listings_on(&self, host: &str) -> Vec<String> {
        self.requests()
            .into_iter()
            .filter(|r| r.server == host && r.is_listing())
            .filter_map(|r| r.repo)
            .collect()
    }

    /// Every request `host` received (any path).
    pub fn requests_to(&self, host: &str) -> Vec<RequestSeen> {
        self.requests()
            .into_iter()
            .filter(|r| r.server == host)
            .collect()
    }

    /// How many times the directory was asked for `did`'s DID document.
    pub fn did_document_fetches(&self, did: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| r.server == "directory" && r.did.as_deref() == Some(did))
            .count()
    }

    /// The peak number of requests in flight at once, across every server.
    pub fn max_requests_in_flight(&self) -> u64 {
        self.state.max_in_flight.load(Ordering::SeqCst)
    }

    /// Forget the request log and the in-flight peak (between two passes).
    pub fn clear_request_log(&self) {
        self.state.requests.lock().unwrap().clear();
        self.state.max_in_flight.store(0, Ordering::SeqCst);
    }

    /// Every XRPC write attempt (accepted or refused), in order.
    pub fn write_attempts(&self) -> Vec<WriteAttempt> {
        self.state.writes.lock().unwrap().clone()
    }

    /// Write attempts against `did`'s repo only.
    pub fn write_attempts_for(&self, did: &str) -> Vec<WriteAttempt> {
        self.write_attempts()
            .into_iter()
            .filter(|w| w.repo == did)
            .collect()
    }

    /// Every attempted delete / put / applyWrites (must stay empty — I-BRA-8).
    pub fn forbidden_write_attempts(&self) -> Vec<String> {
        self.state.forbidden.lock().unwrap().clone()
    }

    /// Every authorization-screen visit, in order.
    pub fn authorizations(&self) -> Vec<AuthorizationSeen> {
        self.state.authorizations.lock().unwrap().clone()
    }

    /// Every client-metadata document the authorization servers fetched.
    pub fn client_metadata_seen(&self) -> Vec<serde_json::Value> {
        self.state.client_metadata.lock().unwrap().clone()
    }

    /// Every call to a revocation endpoint.
    pub fn revocations(&self) -> Vec<RevocationSeen> {
        self.state.revocations.lock().unwrap().clone()
    }

    /// Number of successful authorization-code exchanges.
    pub fn token_exchanges(&self) -> u64 {
        self.state.token_exchanges.load(Ordering::SeqCst)
    }

    /// Number of refresh tokens for `did` that are still usable.
    pub fn live_refresh_tokens(&self, did: &str) -> usize {
        self.state
            .tokens
            .lock()
            .unwrap()
            .iter()
            .filter(|t| t.did == did && !t.refresh_revoked)
            .count()
    }

    /// `true` if some access token issued to `did` would still be accepted for
    /// a write (documents the residual window of SPIKE finding 2).
    pub fn access_token_still_accepted(&self, did: &str) -> bool {
        self.state
            .tokens
            .lock()
            .unwrap()
            .iter()
            .any(|t| t.did == did && !t.access_expired)
    }
}

impl Drop for FakeAtprotoNetwork {
    fn drop(&mut self) {
        self._tasks.clear();
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_background();
        }
    }
}

// =============================================================================
// Server plumbing
// =============================================================================

#[derive(Clone)]
enum Route {
    Directory,
    Host { label: String, base_url: String },
}

type Reachable = Arc<dyn Fn(&NetState) -> bool + Send + Sync>;

async fn bind() -> (tokio::net::TcpListener, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("FakeAtprotoNetwork: bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("local_addr");
    (listener, format!("http://{addr}"))
}

async fn serve_loop(
    listener: tokio::net::TcpListener,
    reachable: Reachable,
    state: Arc<NetState>,
    route: Route,
) {
    use hyper::server::conn::http1;
    use hyper_util::rt::TokioIo;
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(io) => io,
            Err(_) => return,
        };
        if !reachable(&state) {
            drop(stream);
            continue;
        }
        let st = state.clone();
        let rt = route.clone();
        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req| {
                let st = st.clone();
                let rt = rt.clone();
                async move { dispatch(st, rt, req).await }
            });
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), svc)
                .await;
        });
    }
}

/// Decrements the network's in-flight count when a request ends — including
/// when its connection is dropped mid-answer.
struct InFlight(Arc<NetState>);

impl InFlight {
    fn enter(state: &Arc<NetState>) -> Self {
        let now = state.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        state.max_in_flight.fetch_max(now, Ordering::SeqCst);
        Self(state.clone())
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The DID a directory request asks for (`GET /<did>` or `resolveDid?did=`).
fn requested_did(req: &HttpRequest) -> Option<String> {
    let path = crate::review_http::url_decode(req.uri().path());
    if let Some(did) = path.strip_prefix('/').filter(|p| p.starts_with("did:")) {
        return Some(did.to_string());
    }
    parse_form(req.uri().query().unwrap_or(""))
        .get("did")
        .cloned()
}

fn log_request(state: &NetState, route: &Route, req: &HttpRequest) {
    let query = parse_form(req.uri().query().unwrap_or(""));
    let server = match route {
        Route::Directory => "directory".to_string(),
        Route::Host { label, .. } => label.clone(),
    };
    state.requests.lock().unwrap().push(RequestSeen {
        server,
        method: req.method().as_str().to_string(),
        path: req.uri().path().to_string(),
        repo: query.get("repo").cloned(),
        did: match route {
            Route::Directory => requested_did(req),
            Route::Host { .. } => None,
        },
    });
}

/// Never answers (the indexer's per-DID deadline must abandon it).
async fn hang() {
    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
}

async fn dispatch(
    state: Arc<NetState>,
    route: Route,
    req: HttpRequest,
) -> Result<HttpResponse, Infallible> {
    let _in_flight = InFlight::enter(&state);
    log_request(&state, &route, &req);
    Ok(match route {
        Route::Directory => {
            let posture = requested_did(&req)
                .and_then(|did| state.account_by_did(&did))
                .map(|a| a.did_doc);
            if posture == Some(DidDocPosture::Hang) {
                hang().await;
            }
            directory_route(&state, req)
        }
        Route::Host { label, base_url } => {
            let listing = state.host(&label).map(|h| h.listing);
            let is_listing = req.uri().path() == "/xrpc/com.atproto.repo.listRecords";
            match (is_listing, listing) {
                (true, Some(ListingPosture::Hang)) => {
                    hang().await;
                    json(504, serde_json::json!({"error": "Hang"}))
                }
                (true, Some(ListingPosture::Status(code))) => json(
                    code,
                    serde_json::json!({"error": "UpstreamFailure", "message": format!("fake status {code}")}),
                ),
                (true, Some(ListingPosture::NotJson)) => hyper::Response::builder()
                    .status(200)
                    .header("content-type", "text/html")
                    .body(http_body_util::Full::new(bytes::Bytes::from_static(
                        b"<html><body>maintenance</body></html>",
                    )))
                    .expect("build html response"),
                (true, Some(ListingPosture::RedirectTo(location))) => redirect(&location),
                (true, Some(ListingPosture::Slow(delay))) => {
                    tokio::time::sleep(delay).await;
                    host_route(&state, &label, &base_url, req).await
                }
                _ => host_route(&state, &label, &base_url, req).await,
            }
        }
    })
}

// =============================================================================
// Directory: PLC + handle resolution
// =============================================================================

fn directory_route(state: &NetState, req: HttpRequest) -> HttpResponse {
    let path = req.uri().path().to_string();
    let query = parse_form(req.uri().query().unwrap_or(""));
    match path.as_str() {
        "/xrpc/com.atproto.identity.resolveHandle" => {
            let handle = query.get("handle").cloned().unwrap_or_default();
            match state.account_by_handle(&handle) {
                Some(a) => json(200, serde_json::json!({ "did": a.account.did })),
                None => json(
                    400,
                    serde_json::json!({"error": "InvalidRequest", "message": "Unable to resolve handle"}),
                ),
            }
        }
        "/xrpc/com.atproto.identity.resolveDid" => {
            let did = query.get("did").cloned().unwrap_or_default();
            did_doc_response(state, &did)
        }
        p if p.starts_with("/did:") => did_doc_response(state, p.trim_start_matches('/')),
        _ => json(404, serde_json::json!({"message": "not found"})),
    }
}

fn did_doc_response(state: &NetState, did: &str) -> HttpResponse {
    match state.account_by_did(did) {
        Some(a) => {
            let base = state
                .host(&a.account.host)
                .map(|h| h.base_url)
                .unwrap_or_default();
            match &a.did_doc {
                DidDocPosture::Published | DidDocPosture::Hang => {
                    json(200, did_document(&a.account, &base))
                }
                DidDocPosture::NotFound => json(
                    404,
                    serde_json::json!({"message": format!("DID not registered: {did}")}),
                ),
                DidDocPosture::ServerError => {
                    json(500, serde_json::json!({"message": "directory failure"}))
                }
                DidDocPosture::IdMismatch => {
                    let mut doc = did_document(&a.account, &base);
                    doc["id"] = serde_json::json!("did:plc:someoneelse0000000000000");
                    json(200, doc)
                }
                DidDocPosture::NoPdsService => {
                    let mut doc = did_document(&a.account, &base);
                    doc["service"] = serde_json::json!([]);
                    json(200, doc)
                }
                DidDocPosture::PdsEndpoint(endpoint) => {
                    json(200, did_document(&a.account, endpoint))
                }
            }
        }
        None => json(
            404,
            serde_json::json!({"message": format!("DID not registered: {did}")}),
        ),
    }
}

/// A realistic Bluesky DID document: an `#atproto` repo-signing key and the
/// `#atproto_pds` service — and, deliberately, NO `#org.openlore.application`
/// key (Bluesky users do not have one; D-5).
fn did_document(account: &BlueskyAccount, pds_base: &str) -> serde_json::Value {
    serde_json::json!({
        "@context": [
            "https://www.w3.org/ns/did/v1",
            "https://w3id.org/security/multikey/v1",
            "https://w3id.org/security/suites/secp256k1-2019/v1"
        ],
        "id": account.did,
        "alsoKnownAs": [format!("at://{}", account.handle)],
        "verificationMethod": [{
            "id": format!("{}#atproto", account.did),
            "type": "Multikey",
            "controller": account.did,
            "publicKeyMultibase": "zQ3shXjHeiBuRCKmM36cuYnm7YEMzhGnCmCyW92sRJ9pribSF"
        }],
        "service": [{
            "id": "#atproto_pds",
            "type": "AtprotoPersonalDataServer",
            "serviceEndpoint": pds_base
        }]
    })
}

// =============================================================================
// PDS host: OAuth authorization server + resource server
// =============================================================================

async fn host_route(state: &NetState, label: &str, base: &str, req: HttpRequest) -> HttpResponse {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    match (method.as_str(), path.as_str()) {
        ("GET", "/.well-known/oauth-protected-resource") => json(
            200,
            serde_json::json!({
                "resource": base,
                "authorization_servers": [base],
                "scopes_supported": [],
                "bearer_methods_supported": ["header"],
            }),
        ),
        ("GET", "/.well-known/oauth-authorization-server") => {
            json(200, authorization_server_metadata(base))
        }
        ("POST", "/oauth/par") => par(state, label, base, req).await,
        ("GET", "/oauth/authorize") => authorize(state, label, base, &req),
        ("POST", "/oauth/token") => token(state, label, req).await,
        ("POST", "/oauth/revoke") => revoke(state, label, req).await,
        ("POST", "/xrpc/com.atproto.repo.createRecord") => create_record(state, label, req).await,
        ("POST", "/xrpc/com.atproto.repo.deleteRecord")
        | ("POST", "/xrpc/com.atproto.repo.putRecord")
        | ("POST", "/xrpc/com.atproto.repo.applyWrites") => {
            forbidden_write(state, label, &path, req).await
        }
        ("GET", "/xrpc/com.atproto.repo.listRecords") => list_records(state, &req),
        ("GET", "/xrpc/com.atproto.repo.getRecord") => get_record(state, &req),
        ("GET", "/xrpc/com.atproto.repo.describeRepo") => {
            let q = parse_form(req.uri().query().unwrap_or(""));
            let repo = q.get("repo").cloned().unwrap_or_default();
            match state
                .account_by_did(&repo)
                .or_else(|| state.account_by_handle(&repo))
            {
                Some(a) => json(
                    200,
                    serde_json::json!({"did": a.account.did, "handle": a.account.handle, "handleIsCorrect": true}),
                ),
                None => json(400, serde_json::json!({"error": "RepoNotFound"})),
            }
        }
        ("GET", "/xrpc/com.atproto.identity.resolveDid") => {
            let q = parse_form(req.uri().query().unwrap_or(""));
            did_doc_response(state, q.get("did").map(String::as_str).unwrap_or(""))
        }
        ("GET", "/xrpc/com.atproto.identity.resolveHandle") => directory_route(state, req),
        _ => json(
            404,
            serde_json::json!({"error": "NotFound", "message": format!("{method} {path}")}),
        ),
    }
}

fn authorization_server_metadata(base: &str) -> serde_json::Value {
    serde_json::json!({
        "issuer": base,
        "pushed_authorization_request_endpoint": format!("{base}/oauth/par"),
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "revocation_endpoint": format!("{base}/oauth/revoke"),
        "require_pushed_authorization_requests": true,
        "response_types_supported": ["code"],
        "response_modes_supported": ["query", "fragment"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["private_key_jwt", "none"],
        "token_endpoint_auth_signing_alg_values_supported": ["ES256"],
        "dpop_signing_alg_values_supported": ["ES256"],
        "scopes_supported": ["atproto", "transition:generic"],
        "authorization_response_iss_parameter_supported": true,
        "client_id_metadata_document_supported": true,
        "require_request_uri_registration": true,
        "subject_types_supported": ["public"],
    })
}

/// DPoP gate. `Ok(())` when a DPoP proof with the current nonce is present.
fn dpop_gate(
    state: &NetState,
    label: &str,
    req: &HttpRequest,
    resource: bool,
) -> Result<(), Box<HttpResponse>> {
    let nonce = state.host(label).map(|h| h.nonce).unwrap_or_default();
    let Some(proof) = header(req, "dpop") else {
        return Err(Box::new(json(
            400,
            serde_json::json!({"error": "invalid_dpop_proof", "error_description": "DPoP proof required"}),
        )));
    };
    let claims = jwt_claims(&proof).unwrap_or_default();
    let method_ok = claims
        .get("htm")
        .and_then(|v| v.as_str())
        .map(|htm| htm.eq_ignore_ascii_case(req.method().as_str()))
        .unwrap_or(false);
    if !method_ok {
        return Err(Box::new(json(
            400,
            serde_json::json!({"error": "invalid_dpop_proof", "error_description": "htm mismatch"}),
        )));
    }
    if claims.get("nonce").and_then(|v| v.as_str()) != Some(nonce.as_str()) {
        let body = serde_json::json!({"error": "use_dpop_nonce", "error_description": "Authorization server requires nonce in DPoP proof"});
        return Err(Box::new(if resource {
            json_with_headers(
                401,
                &body,
                &[
                    ("dpop-nonce", nonce.clone()),
                    (
                        "www-authenticate",
                        "DPoP error=\"use_dpop_nonce\"".to_string(),
                    ),
                ],
            )
        } else {
            json_with_headers(400, &body, &[("dpop-nonce", nonce.clone())])
        }));
    }
    Ok(())
}

async fn par(state: &NetState, label: &str, base: &str, req: HttpRequest) -> HttpResponse {
    if let Err(resp) = dpop_gate(state, label, &req, false) {
        return *resp;
    }
    let host = state.host(label).expect("host");
    let form = parse_form(&body_string(req).await);
    let get = |k: &str| form.get(k).cloned().unwrap_or_default();
    let client_id = get("client_id");

    if host.client_metadata_blocked {
        return json(
            400,
            serde_json::json!({"error": "invalid_client", "error_description": "Unable to fetch client metadata"}),
        );
    }
    let metadata = match http_get(&client_id).await {
        Ok((200, body)) => match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(v) => v,
            Err(_) => {
                return json(400, serde_json::json!({"error": "invalid_client_metadata"}));
            }
        },
        _ => {
            return json(
                400,
                serde_json::json!({"error": "invalid_client", "error_description": "Unable to fetch client metadata"}),
            )
        }
    };
    state.client_metadata.lock().unwrap().push(metadata.clone());

    let redirect_uri = get("redirect_uri");
    let redirect_ok = metadata
        .get("redirect_uris")
        .and_then(|v| v.as_array())
        .map(|uris| {
            uris.iter()
                .any(|u| u.as_str() == Some(redirect_uri.as_str()))
        })
        .unwrap_or(false);
    if metadata.get("client_id").and_then(|v| v.as_str()) != Some(client_id.as_str())
        || !redirect_ok
    {
        return json(400, serde_json::json!({"error": "invalid_client_metadata"}));
    }
    if get("code_challenge_method") != "S256" || get("code_challenge").is_empty() {
        return json(
            400,
            serde_json::json!({"error": "invalid_request", "error_description": "PKCE S256 required"}),
        );
    }
    let wants_jwt = metadata
        .get("token_endpoint_auth_method")
        .and_then(|v| v.as_str())
        == Some("private_key_jwt");
    if wants_jwt && !client_assertion_ok(&form, &client_id) {
        return json(
            400,
            serde_json::json!({"error": "invalid_client", "error_description": "client assertion"}),
        );
    }

    let hint = get("login_hint");
    let did = if hint.is_empty() {
        None
    } else if hint.starts_with("did:") {
        Some(hint.clone())
    } else {
        state.account_by_handle(&hint).map(|a| a.account.did)
    };
    let request_uri = format!("urn:ietf:params:oauth:request_uri:req-{}", state.next());
    state.pars.lock().unwrap().insert(
        request_uri.clone(),
        PendingPar {
            host: label.to_string(),
            did,
            client_id,
            redirect_uri,
            state: get("state"),
            code_challenge: get("code_challenge"),
            scope: get("scope"),
        },
    );
    let _ = base;
    json_with_headers(
        201,
        &serde_json::json!({"request_uri": request_uri, "expires_in": 299}),
        &[("dpop-nonce", host.nonce)],
    )
}

fn client_assertion_ok(form: &HashMap<String, String>, client_id: &str) -> bool {
    let kind_ok = form.get("client_assertion_type").map(String::as_str)
        == Some("urn:ietf:params:oauth:client-assertion-type:jwt-bearer");
    let claims = form
        .get("client_assertion")
        .and_then(|a| jwt_claims(a))
        .unwrap_or_default();
    kind_ok
        && claims.get("iss").and_then(|v| v.as_str()) == Some(client_id)
        && claims.get("sub").and_then(|v| v.as_str()) == Some(client_id)
}

fn authorize(state: &NetState, label: &str, base: &str, req: &HttpRequest) -> HttpResponse {
    let q = parse_form(req.uri().query().unwrap_or(""));
    let request_uri = q.get("request_uri").cloned().unwrap_or_default();
    let Some(par) = state.pars.lock().unwrap().remove(&request_uri) else {
        return json(
            400,
            serde_json::json!({"error": "invalid_request", "error_description": "unknown request_uri"}),
        );
    };
    if par.host != label {
        return json(400, serde_json::json!({"error": "invalid_request"}));
    }
    let did = par.did.clone().or_else(|| {
        state
            .accounts
            .lock()
            .unwrap()
            .iter()
            .find(|a| a.account.host == label)
            .map(|a| a.account.did.clone())
    });
    let Some(did) = did else {
        return json(
            400,
            serde_json::json!({"error": "invalid_request", "error_description": "no account"}),
        );
    };
    let account = state.account_by_did(&did).expect("account");
    state
        .authorizations
        .lock()
        .unwrap()
        .push(AuthorizationSeen {
            host: label.to_string(),
            did: did.clone(),
            scope: par.scope.clone(),
            client_id: par.client_id.clone(),
        });
    let sep = if par.redirect_uri.contains('?') {
        '&'
    } else {
        '?'
    };
    match account.consent {
        ConsentPosture::Approve => {
            let code = format!("code-{}-{}", state.next(), short_hash(&did));
            state.codes.lock().unwrap().insert(
                code.clone(),
                IssuedCode {
                    did,
                    client_id: par.client_id,
                    redirect_uri: par.redirect_uri.clone(),
                    code_challenge: par.code_challenge,
                    scope: par.scope,
                    used: false,
                },
            );
            redirect(&format!(
                "{}{sep}code={}&state={}&iss={}",
                par.redirect_uri,
                url_encode(&code),
                url_encode(&par.state),
                url_encode(base)
            ))
        }
        ConsentPosture::Cancel => redirect(&format!(
            "{}{sep}error=access_denied&error_description={}&state={}&iss={}",
            par.redirect_uri,
            url_encode("The user denied the request"),
            url_encode(&par.state),
            url_encode(base)
        )),
    }
}

async fn token(state: &NetState, label: &str, req: HttpRequest) -> HttpResponse {
    if let Err(resp) = dpop_gate(state, label, &req, false) {
        return *resp;
    }
    let nonce = state.host(label).map(|h| h.nonce).unwrap_or_default();
    let form = parse_form(&body_string(req).await);
    let get = |k: &str| form.get(k).cloned().unwrap_or_default();
    let invalid_grant = |why: &str| {
        json_with_headers(
            400,
            &serde_json::json!({"error": "invalid_grant", "error_description": why}),
            &[("dpop-nonce", nonce.clone())],
        )
    };
    match get("grant_type").as_str() {
        "authorization_code" => {
            let code = get("code");
            let issued = state.codes.lock().unwrap().get(&code).cloned();
            let Some(issued) = issued else {
                return invalid_grant("Invalid code");
            };
            let account = state.account_by_did(&issued.did).expect("account");
            if account.token == TokenPosture::ExchangeFails {
                return invalid_grant("Invalid code");
            }
            if issued.used
                || issued.client_id != get("client_id")
                || issued.redirect_uri != get("redirect_uri")
                || pkce_s256(&get("code_verifier")) != issued.code_challenge
            {
                return invalid_grant("Invalid code");
            }
            if let Some(c) = state.codes.lock().unwrap().get_mut(&code) {
                c.used = true;
            }
            let sub = match &account.token {
                TokenPosture::SubjectMismatch(other) => other.clone(),
                _ => issued.did.clone(),
            };
            state.token_exchanges.fetch_add(1, Ordering::SeqCst);
            let record = new_tokens(state, &issued.did, &issued.scope);
            token_response(&record, &sub, &nonce)
        }
        "refresh_token" => {
            let refresh = get("refresh_token");
            let found = state
                .tokens
                .lock()
                .unwrap()
                .iter()
                .find(|t| t.refresh == refresh)
                .cloned();
            let Some(found) = found else {
                return invalid_grant("Invalid refresh token");
            };
            let account = state.account_by_did(&found.did).expect("account");
            if found.refresh_revoked || account.write == WritePosture::ExpiredSession {
                return invalid_grant("Refresh token revoked or expired");
            }
            for t in state.tokens.lock().unwrap().iter_mut() {
                if t.refresh == refresh {
                    t.refresh_revoked = true; // rotation: the old refresh dies
                }
            }
            let record = new_tokens(state, &found.did, &found.scope);
            token_response(&record, &found.did, &nonce)
        }
        _ => json(400, serde_json::json!({"error": "unsupported_grant_type"})),
    }
}

fn new_tokens(state: &NetState, did: &str, scope: &str) -> TokenRecord {
    let n = state.next();
    let record = TokenRecord {
        did: did.to_string(),
        scope: scope.to_string(),
        access: format!("fake-access-{n}-{}", short_hash(did)),
        refresh: format!("fake-refresh-{n}-{}", short_hash(did)),
        access_expired: false,
        refresh_revoked: false,
    };
    state.tokens.lock().unwrap().push(record.clone());
    record
}

fn token_response(record: &TokenRecord, sub: &str, nonce: &str) -> HttpResponse {
    json_with_headers(
        200,
        &serde_json::json!({
            "access_token": record.access,
            "token_type": "DPoP",
            "expires_in": 3600,
            "refresh_token": record.refresh,
            "scope": record.scope,
            "sub": sub,
        }),
        &[("dpop-nonce", nonce.to_string())],
    )
}

async fn revoke(state: &NetState, label: &str, req: HttpRequest) -> HttpResponse {
    let form = parse_form(&body_string(req).await);
    let token = form.get("token").cloned().unwrap_or_default();
    let mut revoked_refresh = false;
    {
        let mut tokens = state.tokens.lock().unwrap();
        let session_did = tokens
            .iter()
            .find(|t| t.refresh == token || t.access == token)
            .map(|t| t.did.clone());
        if let Some(did) = session_did {
            for t in tokens.iter_mut().filter(|t| t.did == did) {
                // SPIKE finding 2: revocation kills the refresh token; the access
                // token already issued keeps working until it expires.
                if !t.refresh_revoked {
                    t.refresh_revoked = true;
                    revoked_refresh = true;
                }
            }
        }
    }
    state.revocations.lock().unwrap().push(RevocationSeen {
        host: label.to_string(),
        revoked_a_refresh_token: revoked_refresh,
        status: 200,
    });
    // SPIKE finding 1: real servers answer 200 (RFC 7009), not 204.
    empty(200)
}

/// Resolve the DPoP-bound bearer token of a resource request.
fn bearer(state: &NetState, req: &HttpRequest) -> Result<TokenRecord, Box<HttpResponse>> {
    let auth = header(req, "authorization").unwrap_or_default();
    let Some(value) = auth.strip_prefix("DPoP ") else {
        return Err(Box::new(json(
            401,
            serde_json::json!({"error": "AuthMissing", "message": "DPoP-bound token required"}),
        )));
    };
    let found = state
        .tokens
        .lock()
        .unwrap()
        .iter()
        .find(|t| t.access == value)
        .cloned();
    match found {
        None => Err(Box::new(json(
            401,
            serde_json::json!({"error": "InvalidToken"}),
        ))),
        Some(t) if t.access_expired => Err(Box::new(json_with_headers(
            401,
            &serde_json::json!({"error": "invalid_token", "message": "\"exp\" claim timestamp check failed"}),
            &[(
                "www-authenticate",
                "DPoP error=\"invalid_token\"".to_string(),
            )],
        ))),
        Some(t) => Ok(t),
    }
}

fn scope_allows_create(scope: &str, collection: &str) -> bool {
    let parts: Vec<&str> = scope.split_whitespace().collect();
    if !parts.contains(&"atproto") {
        return false;
    }
    parts.iter().any(|p| {
        *p == "transition:generic"
            || *p == format!("repo:{collection}")
            || *p == format!("repo:{collection}?action=create")
            || (p.starts_with(&format!("repo:{collection}?")) && p.contains("action=create"))
    })
}

async fn create_record(state: &NetState, label: &str, req: HttpRequest) -> HttpResponse {
    if let Err(resp) = dpop_gate(state, label, &req, true) {
        return *resp;
    }
    let token = bearer(state, &req);
    let body: serde_json::Value =
        serde_json::from_str(&body_string(req).await).unwrap_or(serde_json::Value::Null);
    let repo = body
        .get("repo")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let collection = body
        .get("collection")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let record_attempt = |status: u16| {
        state.writes.lock().unwrap().push(WriteAttempt {
            host: label.to_string(),
            repo: repo.clone(),
            nsid: "com.atproto.repo.createRecord".to_string(),
            collection: collection.clone(),
            status,
        });
    };
    let token = match token {
        Ok(t) => t,
        Err(resp) => {
            record_attempt(resp.status().as_u16());
            return *resp;
        }
    };
    let account = state.account_by_did(&token.did).expect("account");
    let refusal = match &account.write {
        WritePosture::ServerError => {
            Some((500, serde_json::json!({"error": "InternalServerError"})))
        }
        WritePosture::ExpiredSession => Some((
            401,
            serde_json::json!({"error": "invalid_token", "message": "session expired"}),
        )),
        WritePosture::RefuseCollection(c) if *c == collection => Some((
            400,
            serde_json::json!({"error": "InvalidRequest", "message": "Record refused by this PDS"}),
        )),
        _ => None,
    };
    if let Some((status, body)) = refusal {
        record_attempt(status);
        return json(status, body);
    }
    if !scope_allows_create(&token.scope, &collection) {
        record_attempt(403);
        return json(
            403,
            serde_json::json!({"error": "ScopeMissingError", "message": format!("Missing required scope \"repo:{collection}?action=create\"")}),
        );
    }
    if repo != token.did {
        record_attempt(400);
        return json(
            400,
            serde_json::json!({"error": "InvalidRequest", "message": "repo does not match the authenticated account"}),
        );
    }
    let rkey = body
        .get("rkey")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("3l{:011}", state.next()));
    let exists = state
        .records
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.repo == repo && r.collection == collection && r.rkey == rkey);
    if exists {
        record_attempt(400);
        return json(
            400,
            serde_json::json!({"error": "InvalidRequest", "message": "Record already exists"}),
        );
    }
    let value = body
        .get("record")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let stored = StoredRecord {
        repo: repo.clone(),
        collection: collection.clone(),
        rkey,
        value: value.clone(),
    };
    state.records.lock().unwrap().push(stored.clone());
    record_attempt(200);
    json(
        200,
        serde_json::json!({
            "uri": stored.uri(),
            "cid": format!("bafyreifake{}", short_hash(&value.to_string())),
            "commit": {"cid": format!("bafyreicommit{}", state.next()), "rev": format!("3l{}", state.next())},
            "validationStatus": "unknown",
        }),
    )
}

async fn forbidden_write(
    state: &NetState,
    label: &str,
    path: &str,
    req: HttpRequest,
) -> HttpResponse {
    let nsid = path.trim_start_matches("/xrpc/").to_string();
    let body: serde_json::Value =
        serde_json::from_str(&body_string(req).await).unwrap_or(serde_json::Value::Null);
    let repo = body
        .get("repo")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let collection = body
        .get("collection")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    state
        .forbidden
        .lock()
        .unwrap()
        .push(format!("{nsid} repo={repo} collection={collection}"));
    state.writes.lock().unwrap().push(WriteAttempt {
        host: label.to_string(),
        repo,
        nsid,
        collection,
        status: 403,
    });
    json(
        403,
        serde_json::json!({"error": "ScopeMissingError", "message": "create-only grant"}),
    )
}

fn record_view(r: &StoredRecord) -> serde_json::Value {
    serde_json::json!({
        "uri": r.uri(),
        "cid": format!("bafyreifake{}", short_hash(&r.value.to_string())),
        "value": r.value,
    })
}

fn list_records(state: &NetState, req: &HttpRequest) -> HttpResponse {
    let q = parse_form(req.uri().query().unwrap_or(""));
    let repo = q.get("repo").cloned().unwrap_or_default();
    let collection = q.get("collection").cloned().unwrap_or_default();
    let repo_did = state
        .account_by_handle(&repo)
        .map(|a| a.account.did)
        .unwrap_or(repo);
    let repo_did = state
        .listing_aliases
        .lock()
        .unwrap()
        .get(&repo_did)
        .cloned()
        .unwrap_or(repo_did);
    let records: Vec<serde_json::Value> = state
        .records
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.repo == repo_did && r.collection == collection)
        .map(record_view)
        .collect();
    json(
        200,
        serde_json::json!({"records": records, "cursor": serde_json::Value::Null}),
    )
}

fn get_record(state: &NetState, req: &HttpRequest) -> HttpResponse {
    let q = parse_form(req.uri().query().unwrap_or(""));
    let get = |k: &str| q.get(k).cloned().unwrap_or_default();
    let found = state
        .records
        .lock()
        .unwrap()
        .iter()
        .find(|r| {
            r.repo == get("repo") && r.collection == get("collection") && r.rkey == get("rkey")
        })
        .cloned();
    match found {
        Some(r) => json(200, record_view(&r)),
        None => json(400, serde_json::json!({"error": "RecordNotFound"})),
    }
}

// =============================================================================
// Self-tests — the double's own contract (green at DISTILL hand-off)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn priya() -> BlueskyAccount {
        BlueskyAccount::new(
            "priyaraman.bsky.social",
            "did:plc:7x3kq2mzv5rj4w6hbn2tqclp",
            "bsky-social",
        )
    }

    fn get(net: &FakeAtprotoNetwork, url: &str) -> (u16, String) {
        net.runtime
            .as_ref()
            .unwrap()
            .block_on(http_get(url))
            .expect("GET")
    }

    #[test]
    fn directory_resolves_handle_and_did_document_points_at_the_users_pds() {
        let net = FakeAtprotoNetwork::start(vec![priya()]);
        let (status, body) = get(
            &net,
            &format!(
                "{}/xrpc/com.atproto.identity.resolveHandle?handle=priyaraman.bsky.social",
                net.directory_url()
            ),
        );
        assert_eq!(status, 200);
        assert!(body.contains("did:plc:7x3kq2mzv5rj4w6hbn2tqclp"));

        let (status, doc) = get(
            &net,
            &format!("{}/did:plc:7x3kq2mzv5rj4w6hbn2tqclp", net.directory_url()),
        );
        assert_eq!(status, 200);
        assert!(doc.contains(&net.host_url("bsky-social")));
        assert!(
            !doc.contains("org.openlore.application"),
            "Bluesky users have no app key (D-5)"
        );

        let (status, _) = get(
            &net,
            &format!(
                "{}/xrpc/com.atproto.identity.resolveHandle?handle=priyaramen.bsky.social",
                net.directory_url()
            ),
        );
        assert_eq!(status, 400, "an unknown handle does not resolve");
    }

    #[test]
    fn authorization_server_metadata_advertises_par_dpop_and_revocation() {
        let net = FakeAtprotoNetwork::start(vec![priya()]);
        let base = net.host_url("bsky-social");
        let (status, body) = get(
            &net,
            &format!("{base}/.well-known/oauth-authorization-server"),
        );
        assert_eq!(status, 200);
        let meta: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(meta["require_pushed_authorization_requests"], true);
        assert_eq!(meta["revocation_endpoint"], format!("{base}/oauth/revoke"));
    }

    #[test]
    fn records_listed_are_exactly_the_seeded_ones_and_nothing_is_written_by_reads() {
        let net = FakeAtprotoNetwork::start(vec![priya()]);
        net.seed_record(
            "did:plc:7x3kq2mzv5rj4w6hbn2tqclp",
            CLAIM_COLLECTION,
            "bafyseeded",
            serde_json::json!({"subject": "github:priyaraman/tidepool"}),
        );
        let base = net.host_url("bsky-social");
        let (status, body) = get(
            &net,
            &format!(
                "{base}/xrpc/com.atproto.repo.listRecords?repo=did:plc:7x3kq2mzv5rj4w6hbn2tqclp&collection=org.openlore.claim"
            ),
        );
        assert_eq!(status, 200);
        assert!(
            body.contains("at://did:plc:7x3kq2mzv5rj4w6hbn2tqclp/org.openlore.claim/bafyseeded")
        );
        assert!(net.write_attempts().is_empty());
        assert!(net.forbidden_write_attempts().is_empty());
    }

    #[test]
    fn an_unreachable_host_drops_connections() {
        let net = FakeAtprotoNetwork::start(vec![priya()]);
        net.set_host_reachable("bsky-social", false);
        let base = net.host_url("bsky-social");
        let result = net.runtime.as_ref().unwrap().block_on(http_get(&format!(
            "{base}/.well-known/oauth-protected-resource"
        )));
        assert!(result.is_err(), "unreachable host must not answer");
    }

    #[test]
    fn granular_scope_allows_only_the_named_collection() {
        let scope =
            "atproto repo:org.openlore.claim?action=create repo:app.bsky.feed.post?action=create";
        assert!(scope_allows_create(scope, CLAIM_COLLECTION));
        assert!(scope_allows_create(scope, POST_COLLECTION));
        assert!(!scope_allows_create(scope, "app.bsky.graph.follow"));
        assert!(scope_allows_create(
            "atproto transition:generic",
            "app.bsky.graph.follow"
        ));
    }

    /// A raw HTTP/1.1 POST (form or JSON) returning (status, headers-lowercased, body).
    async fn post(
        url: &str,
        content_type: &str,
        body: &str,
        headers: &[(&str, String)],
    ) -> (u16, String, String) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let parsed = url::Url::parse(url).unwrap();
        let mut stream =
            tokio::net::TcpStream::connect((parsed.host_str().unwrap(), parsed.port().unwrap()))
                .await
                .unwrap();
        let mut extra = String::new();
        for (k, v) in headers {
            extra.push_str(&format!("{k}: {v}\r\n"));
        }
        let req = format!(
            "POST {} HTTP/1.1\r\nHost: x\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
            parsed.path(),
            body.len()
        );
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let text = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap();
        let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, head.to_ascii_lowercase(), body.to_string())
    }

    fn unsigned_jwt(claims: serde_json::Value) -> String {
        use base64::Engine;
        let e = |v: &serde_json::Value| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string())
        };
        format!(
            "{}.{}.sig",
            e(&serde_json::json!({"alg": "ES256", "typ": "dpop+jwt"})),
            e(&claims)
        )
    }

    /// The whole confidential-client handshake against the double: PAR (with
    /// the nonce dance and the client-metadata fetch), consent, code exchange
    /// with PKCE, a granular-scope create, a forbidden delete, and an RFC 7009
    /// revocation answered 200 that leaves the access token usable.
    #[test]
    fn confidential_client_handshake_create_and_revoke_round_trip() {
        let net = FakeAtprotoNetwork::start(vec![priya()]);
        let rt = net.runtime.as_ref().unwrap();
        let base = net.host_url("bsky-social");

        // A client-metadata document server (stands in for the review app).
        let (listener, app_origin) = rt.block_on(bind());
        let client_id = format!("{app_origin}/oauth/client-metadata.json");
        let redirect_uri = format!("{app_origin}/oauth/callback");
        let metadata = serde_json::json!({
            "client_id": client_id, "client_name": "OpenLore review",
            "redirect_uris": [redirect_uri], "dpop_bound_access_tokens": true,
            "token_endpoint_auth_method": "private_key_jwt",
            "scope": "atproto repo:org.openlore.claim?action=create",
        });
        let served = metadata.clone();
        let _app = AbortOnDrop(rt.spawn(async move {
            use hyper::server::conn::http1;
            use hyper_util::rt::TokioIo;
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let doc = served.clone();
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |_req| {
                        let doc = doc.clone();
                        async move { Ok::<_, Infallible>(json(200, doc)) }
                    });
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), svc)
                        .await;
                });
            }
        }));

        let assertion = unsigned_jwt(serde_json::json!({"iss": client_id, "sub": client_id}));
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let par_body = format!(
            "client_id={}&redirect_uri={}&response_type=code&state=s1&code_challenge={}&code_challenge_method=S256&scope={}&login_hint=priyaraman.bsky.social&client_assertion_type={}&client_assertion={}",
            url_encode(&client_id), url_encode(&redirect_uri), pkce_s256(verifier),
            url_encode("atproto repo:org.openlore.claim?action=create"),
            url_encode("urn:ietf:params:oauth:client-assertion-type:jwt-bearer"), assertion
        );
        let form = "application/x-www-form-urlencoded";
        let no_nonce = unsigned_jwt(serde_json::json!({"htm": "POST"}));
        let (status, head, _) = rt.block_on(post(
            &format!("{base}/oauth/par"),
            form,
            &par_body,
            &[("DPoP", no_nonce)],
        ));
        assert_eq!(status, 400, "first PAR answers use_dpop_nonce");
        assert!(head.contains("dpop-nonce: nonce-bsky-social-1"));
        let proof =
            unsigned_jwt(serde_json::json!({"htm": "POST", "nonce": "nonce-bsky-social-1"}));
        let (status, _, body) = rt.block_on(post(
            &format!("{base}/oauth/par"),
            form,
            &par_body,
            &[("DPoP", proof.clone())],
        ));
        assert_eq!(status, 201, "PAR accepted: {body}");
        assert_eq!(
            net.client_metadata_seen()[0]["client_name"],
            "OpenLore review"
        );
        let request_uri = serde_json::from_str::<serde_json::Value>(&body).unwrap()["request_uri"]
            .as_str()
            .unwrap()
            .to_string();

        let consent = rt.block_on(http_get(&format!(
            "{base}/oauth/authorize?client_id={}&request_uri={}",
            url_encode(&client_id),
            url_encode(&request_uri)
        )));
        // http_get does not follow redirects; a 302 means the consent was given.
        assert_eq!(consent.unwrap().0, 302);
        assert_eq!(
            net.authorizations()[0].did,
            "did:plc:7x3kq2mzv5rj4w6hbn2tqclp"
        );
        let code = net
            .state
            .codes
            .lock()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();

        let token_body = format!(
            "grant_type=authorization_code&code={}&code_verifier={verifier}&redirect_uri={}&client_id={}",
            url_encode(&code), url_encode(&redirect_uri), url_encode(&client_id)
        );
        let (status, _, body) = rt.block_on(post(
            &format!("{base}/oauth/token"),
            form,
            &token_body,
            &[("DPoP", proof.clone())],
        ));
        assert_eq!(status, 200, "{body}");
        let tokens: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(tokens["sub"], "did:plc:7x3kq2mzv5rj4w6hbn2tqclp");
        let access = tokens["access_token"].as_str().unwrap().to_string();

        let create = serde_json::json!({"repo": "did:plc:7x3kq2mzv5rj4w6hbn2tqclp", "collection": CLAIM_COLLECTION, "rkey": "bafyclaim1", "record": {"subject": "github:priyaraman/tidepool"}});
        let auth = [
            ("Authorization", format!("DPoP {access}")),
            ("DPoP", proof.clone()),
        ];
        let (status, _, body) = rt.block_on(post(
            &format!("{base}/xrpc/com.atproto.repo.createRecord"),
            "application/json",
            &create.to_string(),
            &auth,
        ));
        assert_eq!(status, 200, "{body}");
        let post_in_other_collection = serde_json::json!({"repo": "did:plc:7x3kq2mzv5rj4w6hbn2tqclp", "collection": POST_COLLECTION, "record": {"text": "hi"}});
        let (status, _, _) = rt.block_on(post(
            &format!("{base}/xrpc/com.atproto.repo.createRecord"),
            "application/json",
            &post_in_other_collection.to_string(),
            &auth,
        ));
        assert_eq!(status, 403, "outside the granted collections");
        let (status, _, _) = rt.block_on(post(
            &format!("{base}/xrpc/com.atproto.repo.deleteRecord"),
            "application/json",
            &create.to_string(),
            &auth,
        ));
        assert_eq!(status, 403);
        assert_eq!(net.forbidden_write_attempts().len(), 1);
        assert_eq!(
            net.records("did:plc:7x3kq2mzv5rj4w6hbn2tqclp", CLAIM_COLLECTION)
                .len(),
            1
        );

        let (status, _, body) = rt.block_on(post(
            &format!("{base}/oauth/revoke"),
            form,
            &format!(
                "token={}",
                url_encode(tokens["refresh_token"].as_str().unwrap())
            ),
            &[],
        ));
        assert_eq!(
            status, 200,
            "RFC 7009: revocation answers 200 (SPIKE finding 1)"
        );
        assert!(body.is_empty());
        assert_eq!(
            net.live_refresh_tokens("did:plc:7x3kq2mzv5rj4w6hbn2tqclp"),
            0
        );
        assert!(
            net.access_token_still_accepted("did:plc:7x3kq2mzv5rj4w6hbn2tqclp"),
            "SPIKE finding 2"
        );
        let _ = metadata;
    }

    #[test]
    fn pkce_s256_matches_rfc7636_appendix_b() {
        assert_eq!(
            pkce_s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    // ------------------------------------------- indexer postures (2026-10-05)

    fn volkov() -> BlueskyAccount {
        BlueskyAccount::new("dmitri.volkov.dev", "did:plc:dvolkov3m9q", "volkov-dev")
    }

    #[test]
    fn each_did_document_names_its_own_host_and_a_move_is_followed() {
        let net = FakeAtprotoNetwork::start_with_extra_hosts(
            vec![priya(), volkov()],
            &["priyaraman-dev"],
        );
        let doc = |did: &str| {
            let (status, body) = get(&net, &format!("{}/{did}", net.directory_url()));
            assert_eq!(status, 200);
            let doc: serde_json::Value = serde_json::from_str(&body).unwrap();
            doc["service"][0]["serviceEndpoint"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(doc(&priya().did), net.host_url("bsky-social"));
        assert_eq!(doc(&volkov().did), net.host_url("volkov-dev"));
        net.move_account(&priya().did, "priyaraman-dev");
        assert_eq!(doc(&priya().did), net.host_url("priyaraman-dev"));
        assert_eq!(net.did_document_fetches(&priya().did), 2);
    }

    #[test]
    fn did_document_postures_answer_as_declared() {
        let net = FakeAtprotoNetwork::start(vec![priya()]);
        let url = format!("{}/{}", net.directory_url(), priya().did);
        net.set_did_doc_posture(&priya().did, DidDocPosture::NotFound);
        assert_eq!(get(&net, &url).0, 404);
        net.set_did_doc_posture(&priya().did, DidDocPosture::ServerError);
        assert_eq!(get(&net, &url).0, 500);
        net.set_did_doc_posture(
            &priya().did,
            DidDocPosture::PdsEndpoint("http://10.0.0.1".to_string()),
        );
        assert!(get(&net, &url).1.contains("http://10.0.0.1"));
        net.set_did_doc_posture(&priya().did, DidDocPosture::NoPdsService);
        assert!(!get(&net, &url).1.contains("atproto_pds"));
        net.set_did_doc_posture(&priya().did, DidDocPosture::IdMismatch);
        assert!(!get(&net, &url)
            .1
            .contains(&format!("\"id\":\"{}\"", priya().did)));
    }

    #[test]
    fn listing_postures_and_the_request_log_are_observable() {
        let net = FakeAtprotoNetwork::start(vec![priya(), volkov()]);
        net.seed_record(
            &priya().did,
            CLAIM_COLLECTION,
            "k1",
            serde_json::json!({"subject": "github:priyaraman/cargo-pin"}),
        );
        let list = |host: &str, repo: &str| {
            get(
                &net,
                &format!(
                    "{}/xrpc/com.atproto.repo.listRecords?repo={repo}&collection={CLAIM_COLLECTION}",
                    net.host_url(host)
                ),
            )
        };
        let (status, body) = list("bsky-social", &priya().did);
        assert_eq!(status, 200);
        assert!(body.contains("cargo-pin"));
        net.set_listing_posture("volkov-dev", ListingPosture::Status(502));
        assert_eq!(list("volkov-dev", &volkov().did).0, 502);
        net.set_listing_posture("volkov-dev", ListingPosture::NotJson);
        let (status, body) = list("volkov-dev", &volkov().did);
        assert_eq!(status, 200);
        assert!(serde_json::from_str::<serde_json::Value>(&body).is_err());
        net.set_listing_posture("volkov-dev", ListingPosture::Serve);
        net.serve_repo_as(&volkov().did, &priya().did);
        let (_, body) = list("volkov-dev", &volkov().did);
        assert!(body.contains(&format!("at://{}/", priya().did)));
        assert_eq!(net.listings_on("volkov-dev"), vec![volkov().did.clone(); 3]);
        assert_eq!(net.listings_on("bsky-social"), vec![priya().did.clone()]);
        assert!(net.max_requests_in_flight() >= 1);
        net.clear_request_log();
        assert!(net.requests().is_empty());
    }
}
