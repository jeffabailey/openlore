//! `FakePeerPds` — deterministic read-only test double for a PEER's ATProto PDS.
//!
//! Distinct from [`crate::FakePds`] (the user's-own-PDS double): peer PDSes
//! are HONESTLY a different actor. Peer pulls are UNAUTHENTICATED reads —
//! the user's CLI cannot publish to a peer's PDS and the peer's PDS does
//! NOT see the user's identity. This double therefore implements ONLY the
//! read paths slice-03 consumes per ADR-013 + DESIGN §6.2:
//!
//! - `com.atproto.repo.listRecords` (with optional cursor)
//! - `com.atproto.repo.getRecord`
//! - `com.atproto.identity.resolveDid` (resolves the peer's DID document so
//!   `IdentityPort::resolve_peer` can return a `PeerInfo`)
//!
//! Slice-03 explicitly REFUSES write paths against peer PDSes — there is
//! no `createRecord` handler. If the production code accidentally tries to
//! write to a peer endpoint the request returns 405 / surfaces a routing
//! refusal that DELIVER's wiring tests catch immediately.
//!
//! Functional-paradigm note (ADR-007): like `FakePds`, the fake owns a
//! small `Arc<...>` for record storage because port methods take `&self`.
//! The state is preconfigured by the test author at construction (via
//! `for_peer`, `with_tampered_signature`, `with_cross_attribution`, …) and
//! is read-only thereafter from the system-under-test's perspective. The
//! posture is constructor-time-pinned (DD-FED-3): deterministic for the
//! whole scenario lifetime.
//!
//! ## Adversarial fixtures (KPI-FED-6 + WD-40 + WD-41)
//!
//! Four preconfigured adversarial postures are exposed as constructors so
//! DELIVER does not have to re-invent them per scenario:
//!
//! - `with_tampered_signature(peer_did, fixture)`: the peer's PDS returns
//!   N records, one of which carries a signature byte flipped AFTER the
//!   peer's nominal sign step. Pulling MUST verify, MUST reject the one
//!   tampered record, MUST store the (N-1) honest records, MUST exit
//!   non-zero (WD-24). Drives KPI-FED-6.
//! - `with_cid_mismatch(peer_did, fixture)`: the peer's PDS publishes a
//!   record whose rkey does NOT match the locally-recomputed CID for its
//!   canonical CBOR. Drives integration gate `peer_cid_round_trip`.
//! - `with_self_attribution(peer_did, victim_did, fixture)`: the peer
//!   publishes a record whose `author` field is `victim_did` (the local
//!   user). Per WD-40 this MUST be rejected with `SelfAttribution`.
//! - `with_cross_attribution(peer_did, claimed_author_did, fixture)`: the
//!   peer publishes a record whose `author` field is a third-party DID
//!   that is NOT the subscribed peer's DID. Per WD-41 this MUST be
//!   rejected with `CrossAttribution`.
//!
//! Each adversarial constructor APPENDS one preconfigured offending record
//! to the supplied honest fixture set. The offending record's CID
//! (`rkey`) is well-known so DELIVER's per-scenario assertions can name it
//! directly (`ADVERSARIAL_RKEY`).
//!
//! ## Runtime model
//!
//! `serve_http` spins up an in-process HTTP server bound to a random
//! `127.0.0.1` port via `tokio::spawn`, returning a
//! [`FakePeerPdsHttpHandle`] whose `AbortOnDrop` guard stops the server
//! when the handle drops — RAII per-scenario isolation, byte-for-byte the
//! same pattern as [`crate::FakePds::serve_http`]. The caller owns a
//! tokio runtime (the acceptance `support::FakePeerPds` wrapper wires this
//! exactly like `support::FakePds`).

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Collection every slice-03 peer record lives under.
pub const PEER_CLAIM_COLLECTION: &str = "org.openlore.claim";

/// Well-known rkey of the single offending record appended by each
/// adversarial constructor. DELIVER's per-scenario assertions reference
/// this directly ("the record at `ADVERSARIAL_RKEY` MUST be rejected").
pub const ADVERSARIAL_RKEY: &str = "bafyadversarialrecord000000000000000000000000000000000000";

/// A peer's published claim record as the peer PDS would return it.
///
/// Slice-03 reads peer records as raw JSON values (per
/// `PdsPort::list_peer_records` returning `PeerRecordPage` of
/// `SignedRecord` per component-boundaries §`crates/ports`). This
/// fixture-shape mirrors that — `body` is the canonical JSON the peer
/// published; `rkey` is the published key (which the production code MUST
/// verify against `compute_cid(body)` per WD-24).
#[derive(Debug, Clone, PartialEq)]
pub struct FakePeerRecord {
    /// Always `"org.openlore.claim"` for slice-03.
    pub collection: String,
    /// Peer-published key; may or may not match the recomputed CID
    /// (the `with_cid_mismatch` posture deliberately desyncs it).
    pub rkey: String,
    /// The canonical JSON body the peer published (ATProto wire shape).
    pub body: serde_json::Value,
}

impl FakePeerRecord {
    /// Convenience constructor: a peer record under the canonical
    /// `org.openlore.claim` collection. Most fixtures use this.
    pub fn claim(rkey: impl Into<String>, body: serde_json::Value) -> Self {
        Self {
            collection: PEER_CLAIM_COLLECTION.to_string(),
            rkey: rkey.into(),
            body,
        }
    }

    /// The `author` field from the record's JSON body, if present. Used
    /// by the resolveDid handler + assertion helpers that need to know
    /// who a record is attributed to without re-parsing.
    pub fn author(&self) -> Option<&str> {
        self.body.get("author").and_then(|v| v.as_str())
    }
}

/// Internal shared state. Held inside an `Arc` so the HTTP server task and
/// in-process assertion calls observe one source of truth. Once
/// constructed the records are NOT mutated (the posture is fixed); only
/// `unreachable` toggles at runtime (for the unreachable-peer scenario).
#[derive(Debug)]
struct State {
    /// The peer DID this PDS hosts records for. Surfaced in the
    /// resolveDid DID document.
    peer_did: String,
    /// The full record set the peer "published" (honest + any one
    /// adversarial record appended by the posture constructor).
    records: Vec<FakePeerRecord>,
    /// Unreachable failure mode (PP-7): the HTTP server drops the
    /// connection without sending bytes; reqwest classifies that as a
    /// network error which the adapter lifts into `PdsError::Unreachable`.
    unreachable: AtomicBool,
}

/// Read-only test double for a peer's ATProto PDS.
///
/// Constructed via [`for_peer`](Self::for_peer) (well-behaved) or the
/// adversarial constructors. Once constructed, the record set is fixed for
/// the lifetime of the scenario (DD-FED-3 constructor-time-pinned posture).
///
/// Cloning shares the underlying `Arc<State>`, so the spawned HTTP server
/// and in-process assertions see the same records.
#[derive(Debug, Clone)]
pub struct FakePeerPds {
    state: Arc<State>,
}

impl FakePeerPds {
    /// Construct a well-behaved peer PDS hosting `records` under
    /// `peer_did`. This is the baseline happy-path fixture used by
    /// US-FED-002 Example 1 (Maria pulls Rachel's claims, all verified,
    /// all stored).
    ///
    /// No record is modified — whatever the caller hands in is served
    /// verbatim.
    pub fn for_peer(peer_did: &str, records: Vec<FakePeerRecord>) -> Self {
        Self {
            state: Arc::new(State {
                peer_did: peer_did.to_string(),
                records,
                unreachable: AtomicBool::new(false),
            }),
        }
    }

    /// Construct an adversarial peer PDS where exactly ONE appended record
    /// carries a tampered signature (last byte of `signature.sig` flipped
    /// after the peer's nominal sign step). The `honest` records verify
    /// cleanly. Drives KPI-FED-6 + US-FED-002 Example 2 (reject 1, store
    /// the rest).
    ///
    /// The offending record is appended at `ADVERSARIAL_RKEY` so DELIVER's
    /// assertions can name it. Its `author` is the subscribed `peer_did`
    /// (the ONLY defect is the signature — orthogonal to attribution).
    pub fn with_tampered_signature(peer_did: &str, honest: Vec<FakePeerRecord>) -> Self {
        let offending = adversarial_record(peer_did, peer_did, tamper_signature);
        Self::for_peer(peer_did, append(honest, offending))
    }

    /// Construct an adversarial peer PDS where exactly ONE appended record's
    /// rkey does NOT match its recomputed CID (canonicalization
    /// disagreement). Drives the "Peer claim with CID mismatch is rejected
    /// at ingest" UAT scenario + integration gate `peer_cid_round_trip`.
    ///
    /// The body is well-formed and the signature field is intact; only the
    /// rkey↔body CID relationship is broken — the offending record sits at
    /// `ADVERSARIAL_RKEY`, which does NOT equal `compute_cid(body)`.
    pub fn with_cid_mismatch(peer_did: &str, honest: Vec<FakePeerRecord>) -> Self {
        let offending = adversarial_record(peer_did, peer_did, |_| {});
        Self::for_peer(peer_did, append(honest, offending))
    }

    /// Construct an adversarial peer PDS where exactly ONE appended record's
    /// `author` field is the LOCAL USER's DID (`victim_did`). Per WD-40
    /// this MUST be rejected at write time with
    /// `PeerStorageError::SelfAttribution`, even if the signature were
    /// valid against the victim's key (which would indicate key compromise
    /// — orthogonal failure mode).
    pub fn with_self_attribution(
        peer_did: &str,
        victim_did: &str,
        honest: Vec<FakePeerRecord>,
    ) -> Self {
        let offending = adversarial_record(peer_did, victim_did, |_| {});
        Self::for_peer(peer_did, append(honest, offending))
    }

    /// Construct an adversarial peer PDS where exactly ONE appended record's
    /// `author` field is `claimed_author_did` (a third party that is NOT
    /// the subscribed `peer_did`). Per WD-41 this MUST be rejected with
    /// `PeerStorageError::CrossAttribution` — slice-03's trust model is
    /// "subscribing to a peer means accepting THEIR claims; cross-
    /// attributed records are out of scope for slice-03."
    pub fn with_cross_attribution(
        peer_did: &str,
        claimed_author_did: &str,
        honest: Vec<FakePeerRecord>,
    ) -> Self {
        let offending = adversarial_record(peer_did, claimed_author_did, |_| {});
        Self::for_peer(peer_did, append(honest, offending))
    }

    /// The peer DID this fake hosts records for.
    pub fn peer_did(&self) -> &str {
        &self.state.peer_did
    }

    /// Read access for assertions: all records the fake would return on a
    /// `listRecords` call (honest + any one adversarial record). Lets
    /// tests cross-check the production code stored only the verified
    /// subset.
    pub fn records(&self) -> Vec<FakePeerRecord> {
        self.state.records.clone()
    }

    /// Number of records hosted (honest + adversarial).
    pub fn record_count(&self) -> usize {
        self.state.records.len()
    }

    /// Toggle "unreachable" mode on. Subsequent HTTP calls drop the
    /// connection without sending bytes; reqwest classifies this as a
    /// network error. Used by PP-7 (one peer down; skip it, proceed with
    /// the others).
    pub fn simulate_unreachable(&self) {
        self.state.unreachable.store(true, Ordering::SeqCst);
    }

    /// Inverse of [`simulate_unreachable`](Self::simulate_unreachable).
    pub fn restore(&self) {
        self.state.unreachable.store(false, Ordering::SeqCst);
    }

    fn is_unreachable(&self) -> bool {
        self.state.unreachable.load(Ordering::SeqCst)
    }

    /// Spin up an in-process HTTP XRPC server bound to `127.0.0.1` on an
    /// OS-assigned port. Endpoints served (read-only):
    ///
    /// - `GET /xrpc/com.atproto.repo.listRecords?repo=&collection=&cursor=`
    ///     → 200 + `{records: [{uri, cid, value}], cursor?}`
    /// - `GET /xrpc/com.atproto.repo.getRecord?repo=&collection=&rkey=`
    ///     → 200 + `{uri, cid, value}` or 404 if absent.
    /// - `GET /xrpc/com.atproto.identity.resolveDid?did=`
    ///     → 200 + a minimal DID document whose `service[].serviceEndpoint`
    ///       points back at THIS server (so the peer's PDS endpoint
    ///       resolves to the same base URL — one server per peer keeps
    ///       wiring simple).
    ///
    /// Any other route (including any write path like `createRecord`)
    /// returns 405 to surface accidental writes to a peer endpoint.
    ///
    /// While `simulate_unreachable()` is engaged the server drops the
    /// connection without responding — reqwest sees a network error which
    /// the adapter lifts into `PdsError::Unreachable`.
    ///
    /// The returned handle aborts the server task when dropped.
    pub async fn serve_http(&self) -> FakePeerPdsHttpHandle {
        use hyper::server::conn::http1;
        use hyper_util::rt::TokioIo;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("FakePeerPds::serve_http: bind 127.0.0.1:0");
        let local_addr = listener
            .local_addr()
            .expect("FakePeerPds::serve_http: local_addr");
        let base_url = format!("http://{local_addr}");

        let fake = self.clone();
        let server_base = base_url.clone();
        let handle = tokio::spawn(async move {
            loop {
                let (stream, _peer) = match listener.accept().await {
                    Ok(io) => io,
                    Err(_) => return, // listener died — task shuts down
                };

                // Unreachable mode: drop the connection immediately. reqwest
                // sees this as a network error which the adapter lifts into
                // `PdsError::Unreachable` (PP-7).
                if fake.is_unreachable() {
                    drop(stream);
                    continue;
                }

                let fake_for_conn = fake.clone();
                let base_for_conn = server_base.clone();
                tokio::spawn(async move {
                    let io = TokioIo::new(stream);
                    let svc = hyper::service::service_fn(move |req| {
                        let fake = fake_for_conn.clone();
                        let base = base_for_conn.clone();
                        async move { peer_http_route(fake, base, req).await }
                    });
                    let _ = http1::Builder::new().serve_connection(io, svc).await;
                });
            }
        });

        FakePeerPdsHttpHandle {
            base_url,
            _task: AbortOnDrop(handle),
        }
    }
}

// -----------------------------------------------------------------------------
// Adversarial record construction
// -----------------------------------------------------------------------------

/// Build the single offending record appended by each adversarial
/// constructor. `signed_by` is the DID the record is attributed to (`author`
/// field); `mutate` is applied to the JSON body after construction so a
/// posture can tamper a specific field (e.g. flip a signature byte).
fn adversarial_record(
    peer_did: &str,
    author_did: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> FakePeerRecord {
    let mut body = serde_json::json!({
        "subject": "github:rust-lang/cargo",
        "predicate": "embodiesPhilosophy",
        "object": "org.openlore.philosophy.dependency-pinning",
        "evidence": ["https://github.com/rust-lang/cargo/issues/5359"],
        "confidence": 0.42,
        "author": format!("{author_did}#org.openlore.application"),
        "composedAt": "2026-05-22T09:18:44Z",
        "references": [],
        "signature": {
            "kid": format!("{peer_did}#org.openlore.application"),
            "alg": "EdDSA",
            "sig": "MEUCIQDzAdversarialPlaceholderSignatureBytesBase64Url00000000"
        }
    });
    mutate(&mut body);
    FakePeerRecord::claim(ADVERSARIAL_RKEY, body)
}

/// Flip the last character of `signature.sig` — the tampered-signature
/// posture (KPI-FED-6). The body is otherwise well-formed; only the
/// signature no longer verifies against the peer's key.
fn tamper_signature(body: &mut serde_json::Value) {
    if let Some(sig) = body
        .get_mut("signature")
        .and_then(|s| s.get_mut("sig"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        let flipped = flip_last_char(&sig);
        body["signature"]["sig"] = serde_json::Value::String(flipped);
    }
}

fn flip_last_char(s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    if let Some(last) = chars.last_mut() {
        *last = if *last == '0' { '1' } else { '0' };
    }
    chars.into_iter().collect()
}

fn append(mut records: Vec<FakePeerRecord>, one: FakePeerRecord) -> Vec<FakePeerRecord> {
    records.push(one);
    records
}

// -----------------------------------------------------------------------------
// HTTP routing — read-only peer-PDS XRPC subset + PLC resolveDid
// -----------------------------------------------------------------------------

type HttpRequest = hyper::Request<hyper::body::Incoming>;
type HttpResponse = hyper::Response<http_body_util::Full<bytes::Bytes>>;

/// Read-only XRPC route handler. Dispatches the peer-read subset slice-03
/// consumes; everything else (notably any write path) returns 405 so an
/// accidental write to a peer endpoint surfaces loudly.
async fn peer_http_route(
    fake: FakePeerPds,
    base_url: String,
    req: HttpRequest,
) -> Result<HttpResponse, std::convert::Infallible> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let query = req.uri().query().unwrap_or("").to_string();

    match (method.as_str(), path.as_str()) {
        ("GET", "/xrpc/com.atproto.repo.listRecords") => {
            let params = parse_query(&query);
            let collection = params
                .get("collection")
                .cloned()
                .unwrap_or_else(|| PEER_CLAIM_COLLECTION.to_string());
            let records: Vec<serde_json::Value> = fake
                .records()
                .into_iter()
                .filter(|r| r.collection == collection)
                .map(|r| record_view(&fake.state.peer_did, &r))
                .collect();
            Ok(json_response(
                200,
                serde_json::json!({ "records": records, "cursor": serde_json::Value::Null }),
            ))
        }
        ("GET", "/xrpc/com.atproto.repo.getRecord") => {
            let params = parse_query(&query);
            let rkey = params.get("rkey").cloned().unwrap_or_default();
            match fake.records().into_iter().find(|r| r.rkey == rkey) {
                Some(r) => Ok(json_response(200, record_view(&fake.state.peer_did, &r))),
                None => Ok(json_response(
                    404,
                    serde_json::json!({"error": "RecordNotFound"}),
                )),
            }
        }
        ("GET", "/xrpc/com.atproto.identity.resolveDid") => {
            let params = parse_query(&query);
            let did = params
                .get("did")
                .cloned()
                .unwrap_or_else(|| fake.state.peer_did.clone());
            Ok(json_response(200, peer_did_document(&did, &base_url)))
        }
        // Any write path against a peer endpoint is a slice-03 invariant
        // violation — surface it as 405 (method not allowed) so DELIVER's
        // wiring tests catch an accidental write immediately.
        ("POST", _) | ("PUT", _) | ("DELETE", _) => Ok(text_response(
            405,
            format!("FakePeerPds is read-only; refusing {method} {path}"),
        )),
        _ => Ok(text_response(
            404,
            format!("FakePeerPds: no route for {method} {path}"),
        )),
    }
}

/// ATProto `listRecords` / `getRecord` record view:
/// `{uri, cid, value}`. The `cid` is the peer-published rkey verbatim
/// (the production code re-derives + verifies it; the fake does not
/// pre-validate).
fn record_view(peer_did: &str, record: &FakePeerRecord) -> serde_json::Value {
    serde_json::json!({
        "uri": format!("at://{peer_did}/{}/{}", record.collection, record.rkey),
        "cid": record.rkey,
        "value": record.body,
    })
}

/// A minimal W3C DID document for the peer, with a `service[]` entry whose
/// `serviceEndpoint` points back at this fake's own base URL — so
/// `IdentityPort::resolve_peer` resolves the peer's PDS endpoint to the
/// same in-process server hosting its records (one server per peer).
fn peer_did_document(did: &str, base_url: &str) -> serde_json::Value {
    serde_json::json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": did,
        "alsoKnownAs": [format!("at://{}.test", short_handle(did))],
        "verificationMethod": [{
            "id": format!("{did}#org.openlore.application"),
            "type": "Multikey",
            "controller": did,
            "publicKeyMultibase": "z6MkfakepeertestpublickeyMultibase00000000000000000000"
        }],
        "service": [{
            "id": "#atproto_pds",
            "type": "AtprotoPersonalDataServer",
            "serviceEndpoint": base_url,
        }]
    })
}

/// Derive a short handle stub from a DID for `alsoKnownAs` — the last
/// colon-delimited segment (e.g. `did:plc:rachel-test` → `rachel-test`).
fn short_handle(did: &str) -> String {
    did.rsplit(':').next().unwrap_or("peer").to_string()
}

// -----------------------------------------------------------------------------
// Tiny HTTP helpers (mirrors fake_pds.rs; no extra crates)
// -----------------------------------------------------------------------------

fn parse_query(q: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let value = kv.next().unwrap_or("");
        if !key.is_empty() {
            map.insert(key.to_string(), url_decode(value));
        }
    }
    map
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            let hi = from_hex(bytes[i + 1]);
            let lo = from_hex(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        if b == b'+' {
            out.push(b' ');
        } else {
            out.push(b);
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn json_response(status: u16, body: serde_json::Value) -> HttpResponse {
    let bytes = bytes::Bytes::from(body.to_string());
    hyper::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(bytes))
        .expect("build JSON response")
}

fn text_response(status: u16, body: String) -> HttpResponse {
    let bytes = bytes::Bytes::from(body);
    hyper::Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(http_body_util::Full::new(bytes))
        .expect("build text response")
}

/// Owning handle to a running [`FakePeerPds::serve_http`] task.
///
/// Holds the listening URL plus an [`AbortOnDrop`] guard so dropping the
/// handle stops the background server and frees the OS port — same shape
/// as [`crate::FakePdsHttpHandle`].
#[derive(Debug)]
pub struct FakePeerPdsHttpHandle {
    pub base_url: String,
    _task: AbortOnDrop<()>,
}

/// Aborts the wrapped tokio task on drop.
#[derive(Debug)]
struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

// -----------------------------------------------------------------------------
// Unit tests — the FakePeerPds contract is load-bearing for the slice-03
// peer_pull acceptance scenarios, so we pin its shape with real
// async-runtime + real-HTTP tests here (RED_UNIT for step 01-05).
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rachel_record(rkey: &str, subject: &str) -> FakePeerRecord {
        FakePeerRecord::claim(
            rkey,
            serde_json::json!({
                "subject": subject,
                "predicate": "embodiesPhilosophy",
                "object": "org.openlore.philosophy.dependency-pinning",
                "evidence": ["https://example.test/evidence"],
                "confidence": 0.42,
                "author": "did:plc:rachel-test#org.openlore.application",
                "composedAt": "2026-05-22T09:18:44Z",
                "references": [],
                "signature": {
                    "kid": "did:plc:rachel-test#org.openlore.application",
                    "alg": "EdDSA",
                    "sig": "MEUCIQDzHonestSignatureBytes000000000000000000000000000000"
                }
            }),
        )
    }

    async fn get_json(url: &str) -> (u16, serde_json::Value) {
        // Minimal HTTP GET via hyper client over a raw TCP connection —
        // avoids pulling reqwest into the test-support dev-deps. The fake
        // server speaks HTTP/1.1; this client speaks the same.
        use http_body_util::BodyExt;
        use hyper::Request;
        use hyper_util::rt::TokioIo;

        let uri: hyper::Uri = url.parse().expect("parse url");
        let host = uri.host().expect("host");
        let port = uri.port_u16().expect("port");
        let stream = tokio::net::TcpStream::connect((host, port))
            .await
            .expect("connect");
        let io = TokioIo::new(stream);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
            .await
            .expect("handshake");
        tokio::spawn(async move {
            let _ = conn.await;
        });
        let authority = uri.authority().unwrap().clone();
        let req = Request::builder()
            .uri(&uri)
            .header(hyper::header::HOST, authority.as_str())
            .body(http_body_util::Empty::<bytes::Bytes>::new())
            .expect("build request");
        let resp = sender.send_request(req).await.expect("send");
        let status = resp.status().as_u16();
        let body = resp.into_body().collect().await.expect("collect").to_bytes();
        let json: serde_json::Value =
            serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    /// `for_peer` hosts the supplied records and serves them over
    /// `listRecords`. The load-bearing happy-path contract: PP-1 cannot
    /// wire without this returning every fixture record.
    #[tokio::test]
    async fn for_peer_serves_all_records_via_list_records() {
        let fake = FakePeerPds::for_peer(
            "did:plc:rachel-test",
            vec![
                rachel_record("bafyrecord001", "github:rust-lang/cargo"),
                rachel_record("bafyrecord002", "github:torvalds/linux"),
            ],
        );
        let handle = fake.serve_http().await;

        let (status, body) = get_json(&format!(
            "{}/xrpc/com.atproto.repo.listRecords?repo=did:plc:rachel-test&collection=org.openlore.claim",
            handle.base_url
        ))
        .await;

        assert_eq!(status, 200, "listRecords must return 200");
        let records = body.get("records").and_then(|r| r.as_array()).expect("records array");
        assert_eq!(records.len(), 2, "all fixture records must be served");
        assert_eq!(
            records[0]["value"]["subject"], "github:rust-lang/cargo",
            "first record body must be served verbatim"
        );
        assert_eq!(
            records[0]["cid"], "bafyrecord001",
            "cid view must echo the peer-published rkey"
        );
    }

    /// `getRecord` returns the one record by rkey, 404 if absent.
    #[tokio::test]
    async fn get_record_returns_one_by_rkey_and_404_when_absent() {
        let fake = FakePeerPds::for_peer(
            "did:plc:rachel-test",
            vec![rachel_record("bafyrecord001", "github:rust-lang/cargo")],
        );
        let handle = fake.serve_http().await;

        let (ok_status, ok_body) = get_json(&format!(
            "{}/xrpc/com.atproto.repo.getRecord?repo=did:plc:rachel-test&collection=org.openlore.claim&rkey=bafyrecord001",
            handle.base_url
        ))
        .await;
        assert_eq!(ok_status, 200);
        assert_eq!(ok_body["value"]["subject"], "github:rust-lang/cargo");

        let (missing_status, _) = get_json(&format!(
            "{}/xrpc/com.atproto.repo.getRecord?repo=did:plc:rachel-test&collection=org.openlore.claim&rkey=does-not-exist",
            handle.base_url
        ))
        .await;
        assert_eq!(missing_status, 404, "absent rkey must 404");
    }

    /// `resolveDid` returns a DID document whose PDS service endpoint
    /// points back at this server — the contract `IdentityPort::resolve_peer`
    /// relies on to discover the peer's PDS.
    #[tokio::test]
    async fn resolve_did_returns_did_document_pointing_at_this_server() {
        let fake = FakePeerPds::for_peer("did:plc:rachel-test", vec![]);
        let handle = fake.serve_http().await;

        let (status, body) = get_json(&format!(
            "{}/xrpc/com.atproto.identity.resolveDid?did=did:plc:rachel-test",
            handle.base_url
        ))
        .await;

        assert_eq!(status, 200);
        assert_eq!(body["id"], "did:plc:rachel-test");
        let endpoint = body["service"][0]["serviceEndpoint"]
            .as_str()
            .expect("service endpoint present");
        assert_eq!(
            endpoint, handle.base_url,
            "resolveDid PDS endpoint must point at this fake's own base URL"
        );
    }

    /// Adversarial postures append exactly one offending record at
    /// `ADVERSARIAL_RKEY`. The honest records are preserved; the count is
    /// honest+1; the offending record's defect matches the posture.
    #[tokio::test]
    async fn adversarial_constructors_append_one_offending_record() {
        let honest = vec![rachel_record("bafyrecord001", "github:rust-lang/cargo")];

        // Tampered signature: same author (peer), only the sig differs.
        let tampered = FakePeerPds::with_tampered_signature("did:plc:rachel-test", honest.clone());
        assert_eq!(tampered.record_count(), 2);
        let offending = tampered
            .records()
            .into_iter()
            .find(|r| r.rkey == ADVERSARIAL_RKEY)
            .expect("tampered record present");
        assert_eq!(
            offending.author(),
            Some("did:plc:rachel-test#org.openlore.application"),
            "tampered posture keeps peer attribution; only the sig is broken"
        );

        // Self-attribution: offending record's author is the victim.
        let self_attr = FakePeerPds::with_self_attribution(
            "did:plc:rachel-test",
            "did:plc:test-maria",
            honest.clone(),
        );
        let offending = self_attr
            .records()
            .into_iter()
            .find(|r| r.rkey == ADVERSARIAL_RKEY)
            .expect("self-attr record present");
        assert_eq!(
            offending.author(),
            Some("did:plc:test-maria#org.openlore.application"),
            "self-attribution posture attributes the offending record to the victim DID (WD-40)"
        );

        // Cross-attribution: offending record's author is a third party.
        let cross = FakePeerPds::with_cross_attribution(
            "did:plc:rachel-test",
            "did:plc:trusted-third-party-test",
            honest.clone(),
        );
        let offending = cross
            .records()
            .into_iter()
            .find(|r| r.rkey == ADVERSARIAL_RKEY)
            .expect("cross-attr record present");
        assert_eq!(
            offending.author(),
            Some("did:plc:trusted-third-party-test#org.openlore.application"),
            "cross-attribution posture attributes the offending record to a third-party DID (WD-41)"
        );

        // CID mismatch: rkey deliberately not equal to the body CID.
        let mismatch = FakePeerPds::with_cid_mismatch("did:plc:rachel-test", honest);
        assert_eq!(mismatch.record_count(), 2);
        assert!(
            mismatch.records().iter().any(|r| r.rkey == ADVERSARIAL_RKEY),
            "cid-mismatch posture appends the offending record at ADVERSARIAL_RKEY"
        );
    }

    /// Unreachable mode drops the connection — a client GET errors out
    /// rather than receiving a response. `restore()` re-enables serving.
    #[tokio::test]
    async fn simulate_unreachable_drops_connection_then_restore_serves() {
        let fake = FakePeerPds::for_peer(
            "did:plc:down-test",
            vec![rachel_record("bafyrecord001", "github:rust-lang/cargo")],
        );
        let handle = fake.serve_http().await;
        let url = format!(
            "{}/xrpc/com.atproto.repo.listRecords?collection=org.openlore.claim",
            handle.base_url
        );

        fake.simulate_unreachable();
        let down = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            // Connection accepted then immediately dropped → handshake or
            // request send fails. Catch the panic-free error path by
            // probing with a raw connect+send that we expect to fail.
            use hyper_util::rt::TokioIo;
            let uri: hyper::Uri = url.parse().unwrap();
            let host = uri.host().unwrap().to_string();
            let port = uri.port_u16().unwrap();
            let stream = tokio::net::TcpStream::connect((host.as_str(), port)).await;
            match stream {
                Err(_) => Err::<(), ()>(()),
                Ok(s) => {
                    let io = TokioIo::new(s);
                    match hyper::client::conn::http1::handshake::<_, http_body_util::Empty<bytes::Bytes>>(io).await {
                        Err(_) => Err(()),
                        Ok((mut sender, conn)) => {
                            tokio::spawn(async move { let _ = conn.await; });
                            let req = hyper::Request::builder()
                                .uri(&uri)
                                .header(hyper::header::HOST, uri.authority().unwrap().as_str())
                                .body(http_body_util::Empty::<bytes::Bytes>::new())
                                .unwrap();
                            match sender.send_request(req).await {
                                Err(_) => Err(()),
                                Ok(_) => Ok(()),
                            }
                        }
                    }
                }
            }
        })
        .await
        .expect("unreachable probe must not hang");
        assert!(down.is_err(), "while unreachable, the request must fail");

        fake.restore();
        let (status, _) = get_json(&url).await;
        assert_eq!(status, 200, "after restore, serving resumes");
    }
}
