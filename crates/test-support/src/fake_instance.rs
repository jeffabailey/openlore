//! `FakeInstance` — an OPAQUE, content-addressed HTTP double for a user's own
//! serverless openlore instance (ADR-062 §1; serverless-philosophy-federation).
//!
//! Mirrors the four-route opaque transport contract the `atproto/` Worker
//! implements:
//!
//! | Route | Behaviour |
//! |---|---|
//! | `PUT /records/:cid` | Store the body VERBATIM under `:cid` (first write wins; a re-PUT is an idempotent no-op on the blob). If the request carries the `x-openlore-manifest-entry` header (the CLI's display projection JSON), the entry is appended to the manifest — the COMMIT. |
//! | `GET /records/:cid` | Return the exact stored bytes, or 404. |
//! | `GET /manifest` | The openlore discriminator envelope + committed display entries (manifest v1). |
//! | `GET /` | The public read-only card (HTML), rendered from the committed manifest entries — see [`render_card`]. |
//!
//! The double NEVER computes a CID and NEVER parses the record blob — the
//! Rust `claim-domain` core is the sole canonicalizer (ADR-062). It parses
//! only the manifest-entry header JSON, exactly like the Worker.
//!
//! Runtime model: the double owns a small multi-threaded tokio runtime and a
//! hyper server bound to `127.0.0.1:0`, so the `openlore` subprocess can talk
//! to it while the test thread stays synchronous. Dropping the double aborts
//! the server (RAII per-scenario isolation — same shape as the acceptance
//! `FakePds` wrapper).
//!
//! Postures: `fresh` (an empty, well-behaved instance — step 01-01);
//! `unreachable` (nothing listens at the URL) and `not_an_openlore_instance`
//! (reachable, but an ordinary web site whose `/manifest` is HTML with no
//! openlore marker) — step 01-03, Q-SF-D5; `with_cid_mismatch` (stored bytes
//! drift so the Rust recompute differs) — step 01-05; `with_records` (an
//! instance that already holds committed records — a prior push) — step
//! 02-01; `requiring_write_token` (every write must carry
//! `Authorization: Bearer <owner token>`, else `401` and nothing is stored;
//! reads stay public) — step 02-03, DV-4 / Q-SF-D2; `for_peer` (ANOTHER
//! user's instance already holding her records, which ALSO answers the
//! `com.atproto.identity.resolveDid` resolver route with a DID document whose
//! `serviceEndpoint` is the instance itself — so the J-003 peer resolver seam
//! `OPENLORE_PEER_PDS_ENDPOINT_<did>` can point straight at it, exactly like
//! the slice-03 `FakePeerPds`) — step 05-01, US-SF-006 / OD-SF-3.
//!
//! Every request that crosses the seam — in any posture — is appended to a
//! request log ([`FakeInstance::recorded_requests`]): method, path, headers,
//! and body exactly as received. It is the port-exposed observation of what
//! the CLI sent the instance (PI-3: no key material ever crosses; PI-5:
//! `publish status` only reads).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The request header carrying the CLI-written display projection (JSON) on
/// the committing `PUT /records/:cid` (ADR-062 §1 manifest; DELIVER decision).
pub const MANIFEST_ENTRY_HEADER: &str = "x-openlore-manifest-entry";

/// The manifest v1 discriminator envelope (Q-SF-D5 detection marker).
fn openlore_marker() -> serde_json::Value {
    serde_json::json!({ "kind": "opaque-instance", "contract_version": 1 })
}

/// Shared in-memory store: blobs keyed by CID + the ordered committed
/// manifest entries. One source of truth for the HTTP task and the
/// in-process assertion accessors.
#[derive(Debug, Default)]
struct Store {
    blobs: BTreeMap<String, Vec<u8>>,
    manifest: Vec<(String, serde_json::Value)>,
    requests: Vec<RecordedRequest>,
    /// `Some(did)` ⇔ this instance belongs to a PEER and answers the DID
    /// resolver route for `did` (the `for_peer` posture).
    peer_did: Option<String>,
    /// The instance's own base URL — the `serviceEndpoint` its peer DID
    /// document advertises. Set once the listener is bound.
    base_url: String,
}

/// One request exactly as it crossed the CLI↔instance seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    /// Header `(name, value)` pairs in arrival order; names lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RecordedRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

impl Store {
    fn put_blob(&mut self, cid: &str, body: Vec<u8>) {
        // Content-addressed + idempotent: the first write wins; a re-PUT of
        // an existing key is a no-op success (ADR-062 §1).
        self.blobs.entry(cid.to_string()).or_insert(body);
    }

    fn commit_entry(&mut self, cid: &str, entry: serde_json::Value) {
        let already_committed = self.manifest.iter().any(|(c, _)| c == cid);
        if !already_committed {
            self.manifest.push((cid.to_string(), entry));
        }
    }

    fn manifest_json(&self) -> serde_json::Value {
        let records: Vec<serde_json::Value> =
            self.manifest.iter().map(|(_, e)| e.clone()).collect();
        serde_json::json!({ "openlore": openlore_marker(), "records": records })
    }
}

/// A record an instance already holds before the scenario runs: its blob
/// under `cid` and the committed manifest display entry (JSON), exactly as a
/// prior committing `PUT /records/:cid` would have left them.
#[derive(Debug, Clone, PartialEq)]
pub struct PreloadedRecord {
    pub cid: String,
    pub bytes: Vec<u8>,
    pub manifest_entry: serde_json::Value,
}

/// How the double answers — the posture a scenario puts it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Posture {
    /// A well-behaved openlore opaque instance (the four ADR-062 routes).
    OpenloreInstance,
    /// A reachable ordinary web site: every route (incl. `/manifest`) is an
    /// HTML page with no openlore marker.
    OrdinaryWebSite,
    /// An openlore instance with a boundary bug: it re-encodes the record's
    /// `confidence` through a lossy `f32` on store, so the bytes it returns
    /// recompute (in Rust) to a DIFFERENT CID than the key.
    CidMismatch,
}

impl Posture {
    /// What this posture stores for a `PUT /records/:cid` body.
    fn stored_body(self, body: &[u8]) -> Vec<u8> {
        match self {
            Posture::CidMismatch => drift_confidence(body),
            Posture::OpenloreInstance | Posture::OrdinaryWebSite => body.to_vec(),
        }
    }
}

/// The `with_cid_mismatch` boundary bug: parse the record JSON and re-encode
/// its `confidence` through `f32` (a lossy float re-encode). Diverges for any
/// confidence that is not exactly `f32`-representable (e.g. `0.86`); a body
/// that is not a JSON object with a numeric `confidence` is stored as-is.
fn drift_confidence(body: &[u8]) -> Vec<u8> {
    let Ok(mut record) = serde_json::from_slice::<serde_json::Value>(body) else {
        return body.to_vec();
    };
    let drifted = record
        .get("confidence")
        .and_then(serde_json::Value::as_f64)
        .map(|confidence| f64::from(confidence as f32));
    match (drifted, record.as_object_mut()) {
        (Some(drifted), Some(fields)) => {
            fields.insert("confidence".to_string(), serde_json::json!(drifted));
            serde_json::to_vec(&record).expect("a serde_json::Value always serializes")
        }
        _ => body.to_vec(),
    }
}

/// The owner write token a gated instance demands on every non-`GET`
/// request (DV-4). `None` = an ungated instance (every earlier posture).
type WriteGate = Option<Arc<str>>;

/// Does `request` pass the write gate? Reads are always public; a write
/// passes only with exactly `Authorization: Bearer <owner token>`.
fn passes_write_gate(gate: &WriteGate, request: &RecordedRequest) -> bool {
    match gate {
        None => true,
        Some(_) if request.method == "GET" => true,
        Some(token) => request.header("authorization") == Some(&format!("Bearer {token}")),
    }
}

/// Opaque content-addressed instance double. See module docs.
pub struct FakeInstance {
    store: Arc<Mutex<Store>>,
    base_url: String,
    /// The running server + its runtime; `None` for the `unreachable`
    /// posture, where nothing listens at `base_url`.
    server: Option<(tokio::task::JoinHandle<()>, tokio::runtime::Runtime)>,
}

impl FakeInstance {
    /// A reachable, empty, well-behaved openlore instance: its `/manifest`
    /// carries the openlore marker and lists no records yet.
    pub fn fresh() -> Self {
        Self::start(Store::default(), Posture::OpenloreInstance)
    }

    /// No instance is reachable at the URL: a loopback port that was bound
    /// then released, so connections are refused (PI-2).
    pub fn unreachable() -> Self {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("FakeInstance: reserve a loopback port")
            .port();
        Self {
            store: Arc::new(Mutex::new(Store::default())),
            base_url: format!("http://127.0.0.1:{port}"),
            server: None,
        }
    }

    /// A reachable URL that is NOT an openlore instance: an ordinary web
    /// site whose `GET /manifest` is an HTML page lacking the openlore
    /// marker envelope (PI-4, Q-SF-D5).
    pub fn not_an_openlore_instance() -> Self {
        Self::start(Store::default(), Posture::OrdinaryWebSite)
    }

    /// A reachable openlore instance with a canonicalization-drift boundary
    /// bug: it stores (and returns) the record with its `confidence`
    /// re-encoded through a lossy `f32`, so the Rust recompute of the returned
    /// bytes yields a CID different from the key it was pushed under (RT-2,
    /// KPI-SF-1). It still honours the manifest-entry commit header — whether
    /// anything is committed is entirely the CLI's decision.
    pub fn with_cid_mismatch() -> Self {
        Self::start(Store::default(), Posture::CidMismatch)
    }

    /// A reachable, well-behaved openlore instance that ALREADY holds
    /// `records` (blob + committed manifest entry each, in the given order) —
    /// the state a prior push leaves behind (PP-1..PP-3).
    pub fn with_records(records: impl IntoIterator<Item = PreloadedRecord>) -> Self {
        let preloaded = records
            .into_iter()
            .fold(Store::default(), |mut store, record| {
                store.put_blob(&record.cid, record.bytes);
                store.commit_entry(&record.cid, record.manifest_entry);
                store
            });
        Self::start(preloaded, Posture::OpenloreInstance)
    }

    /// A reachable, EMPTY, well-behaved openlore instance that accepts writes
    /// ONLY from its owner: a `PUT` without `Authorization: Bearer
    /// <owner_token>` (missing or wrong) is refused with `401` and stores
    /// nothing; every `GET` stays public (PP-5, DV-4 / Q-SF-D2).
    pub fn requiring_write_token(owner_token: &str) -> Self {
        Self::start_gated(
            Store::default(),
            Posture::OpenloreInstance,
            Some(Arc::from(owner_token)),
        )
    }

    /// ANOTHER user's reachable, well-behaved openlore instance that already
    /// holds `records` (blob + committed manifest entry each) AND answers the
    /// DID resolver route (`GET /xrpc/com.atproto.identity.resolveDid`) with
    /// `peer_did`'s DID document, whose `serviceEndpoint` is this instance —
    /// the peer's DID resolves to her Cloudflare instance (US-SF-006,
    /// OD-SF-3). The resolver route is read-only; every ADR-062 route behaves
    /// exactly as [`FakeInstance::with_records`].
    pub fn for_peer(peer_did: &str, records: impl IntoIterator<Item = PreloadedRecord>) -> Self {
        let preloaded = records.into_iter().fold(
            Store {
                peer_did: Some(peer_did.to_string()),
                ..Store::default()
            },
            |mut store, record| {
                store.put_blob(&record.cid, record.bytes);
                store.commit_entry(&record.cid, record.manifest_entry);
                store
            },
        );
        Self::start(preloaded, Posture::OpenloreInstance)
    }

    /// Base URL of the running double (e.g. `http://127.0.0.1:54321`).
    pub fn endpoint_url(&self) -> &str {
        &self.base_url
    }

    /// The manifest JSON exactly as `GET /manifest` serves it.
    pub fn manifest(&self) -> serde_json::Value {
        self.lock().manifest_json()
    }

    /// CIDs COMMITTED to the manifest, in commit order — the instance's
    /// observable record set (`instance.records.cids`).
    pub fn stored_cids(&self) -> Vec<String> {
        self.lock()
            .manifest
            .iter()
            .map(|(cid, _)| cid.clone())
            .collect()
    }

    /// The exact bytes stored under `cid` (what `GET /records/:cid` returns).
    pub fn record_bytes(&self, cid: &str) -> Option<Vec<u8>> {
        self.lock().blobs.get(cid).cloned()
    }

    /// Every request received so far, in arrival order (any posture).
    pub fn recorded_requests(&self) -> Vec<RecordedRequest> {
        self.lock().requests.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store
            .lock()
            .expect("FakeInstance store mutex poisoned")
    }

    fn start(initial: Store, posture: Posture) -> Self {
        Self::start_gated(initial, posture, None)
    }

    fn start_gated(initial: Store, posture: Posture, write_gate: WriteGate) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_io()
            .enable_time()
            .thread_name("fake-instance-rt")
            .build()
            .expect("FakeInstance: build tokio runtime");
        let store = Arc::new(Mutex::new(initial));
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .expect("FakeInstance: bind 127.0.0.1:0");
        let base_url = format!(
            "http://{}",
            listener.local_addr().expect("FakeInstance: local_addr")
        );
        store.lock().expect("FakeInstance store").base_url = base_url.clone();
        let server = runtime.spawn(serve(listener, Arc::clone(&store), posture, write_gate));
        Self {
            store,
            base_url,
            server: Some((server, runtime)),
        }
    }
}

impl Drop for FakeInstance {
    fn drop(&mut self) {
        // Shut the runtime down on a background thread so a worker parked on
        // `accept()` can never block the test thread (the macOS shutdown
        // hazard the acceptance `FakePds` wrapper documents).
        if let Some((server, runtime)) = self.server.take() {
            server.abort();
            let _ = std::thread::Builder::new()
                .name("fake-instance-shutdown".to_string())
                .spawn(move || drop(runtime));
        }
    }
}

// -----------------------------------------------------------------------------
// HTTP routing — the four ADR-062 §1 routes
// -----------------------------------------------------------------------------

type HttpRequest = hyper::Request<hyper::body::Incoming>;
type HttpResponse = hyper::Response<http_body_util::Full<bytes::Bytes>>;

async fn serve(
    listener: tokio::net::TcpListener,
    store: Arc<Mutex<Store>>,
    posture: Posture,
    write_gate: WriteGate,
) {
    use hyper::server::conn::http1;
    use hyper_util::rt::TokioIo;

    loop {
        let Ok((stream, _peer)) = listener.accept().await else {
            return;
        };
        let store = Arc::clone(&store);
        let write_gate = write_gate.clone();
        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req| {
                let store = Arc::clone(&store);
                let write_gate = write_gate.clone();
                async move {
                    Ok::<_, std::convert::Infallible>(
                        answer(&store, posture, &write_gate, req).await,
                    )
                }
            });
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), svc)
                .await;
        });
    }
}

/// Receive → log → gate writes → answer per posture.
async fn answer(
    store: &Mutex<Store>,
    posture: Posture,
    write_gate: &WriteGate,
    req: HttpRequest,
) -> HttpResponse {
    let Some(request) = receive(req).await else {
        return respond(400, "text/plain", b"unreadable body".to_vec());
    };
    let mut guard = store.lock().expect("store");
    guard.requests.push(request.clone());
    if !passes_write_gate(write_gate, &request) {
        return respond(
            401,
            "text/plain",
            b"missing or invalid owner token".to_vec(),
        );
    }
    match posture {
        Posture::OpenloreInstance | Posture::CidMismatch => route(&mut guard, posture, &request),
        Posture::OrdinaryWebSite => ordinary_web_page(),
    }
}

/// Read the whole request off the wire into a [`RecordedRequest`].
async fn receive(req: HttpRequest) -> Option<RecordedRequest> {
    use http_body_util::BodyExt;

    let (parts, body) = req.into_parts();
    let body = body.collect().await.ok()?.to_bytes().to_vec();
    let headers = parts
        .headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    Some(RecordedRequest {
        method: parts.method.as_str().to_string(),
        path: parts.uri.path().to_string(),
        headers,
        body,
    })
}

fn route(store: &mut Store, posture: Posture, request: &RecordedRequest) -> HttpResponse {
    let record_cid = request.path.strip_prefix("/records/");
    match (request.method.as_str(), request.path.as_str(), record_cid) {
        ("GET", "/manifest", _) => respond(
            200,
            "application/json",
            store.manifest_json().to_string().into_bytes(),
        ),
        ("GET", "/", _) => card_response(&store.manifest),
        ("GET", RESOLVE_DID_PATH, _) => match &store.peer_did {
            Some(peer_did) => respond(
                200,
                "application/json",
                peer_did_document(peer_did, &store.base_url)
                    .to_string()
                    .into_bytes(),
            ),
            None => respond(404, "text/plain", b"no such route".to_vec()),
        },
        ("GET", _, Some(cid)) => match store.blobs.get(cid) {
            Some(bytes) => respond(200, "application/octet-stream", bytes.clone()),
            None => respond(404, "text/plain", b"record not found".to_vec()),
        },
        ("PUT", _, Some(cid)) => put_record(store, posture, cid, request),
        _ => respond(404, "text/plain", b"no such route".to_vec()),
    }
}

/// The ATProto DID resolver route a `for_peer` instance answers.
const RESOLVE_DID_PATH: &str = "/xrpc/com.atproto.identity.resolveDid";

/// The peer's DID document: its `serviceEndpoint` is this instance (the
/// user's Cloudflare instance, not a bsky PDS). The verification method
/// carries a placeholder key — the acceptance pubkey seam supplies the real
/// one, exactly as for the slice-03 `FakePeerPds`.
fn peer_did_document(peer_did: &str, base_url: &str) -> serde_json::Value {
    let handle = peer_did.rsplit(':').next().unwrap_or("peer");
    serde_json::json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": peer_did,
        "alsoKnownAs": [format!("at://{handle}.test")],
        "verificationMethod": [{
            "id": format!("{peer_did}#org.openlore.application"),
            "type": "Multikey",
            "controller": peer_did,
            "publicKeyMultibase": "z6MkfakepeerinstancepublickeyMultibase000000000000000000"
        }],
        "service": [{
            "id": "#openlore_instance",
            "type": "OpenloreInstance",
            "serviceEndpoint": base_url,
        }]
    })
}

/// `PUT /records/:cid` — store the body (verbatim, unless the posture has a
/// boundary bug); commit the manifest entry when the header carries one.
fn put_record(
    store: &mut Store,
    posture: Posture,
    cid: &str,
    request: &RecordedRequest,
) -> HttpResponse {
    let parsed_entry = match request
        .header(MANIFEST_ENTRY_HEADER)
        .map(serde_json::from_str)
    {
        None => None,
        Some(Ok(entry)) => Some(entry),
        Some(Err(_)) => {
            return respond(
                400,
                "text/plain",
                b"malformed manifest entry header".to_vec(),
            )
        }
    };
    store.put_blob(cid, posture.stored_body(&request.body));
    if let Some(entry) = parsed_entry {
        store.commit_entry(cid, entry);
    }
    respond(201, "text/plain", Vec::new())
}

// -----------------------------------------------------------------------------
// The public read-only card (ADR-062 §1, US-SF-005) — mirrors
// `atproto/src/card.ts` byte-for-byte in STRUCTURE:
//
// * one `<li class="claim" data-cid="…" data-author="…">` row per committed
//   manifest entry, in manifest order, attributed to that entry's ONE
//   `author_did` (anti-merging: no merged/consensus row shape exists);
// * an empty manifest renders `<p class="empty-state">no claims published
//   yet</p>` — a valid 200 page, not an error;
// * no authoring/edit control and no script; every field HTML-escaped;
// * served as `text/html; charset=utf-8` with a restrictive CSP.
// -----------------------------------------------------------------------------

/// The card's content-security-policy: it needs no script, no fetch, no form.
pub const CARD_CONTENT_SECURITY_POLICY: &str =
    "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// Escape a manifest field for HTML text and attribute context (`& < > " '`).
fn escape_html(text: &str) -> String {
    text.chars()
        .fold(String::with_capacity(text.len()), |mut escaped, c| {
            match c {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&#39;"),
                other => escaped.push(other),
            }
            escaped
        })
}

/// A manifest entry field as display text: strings verbatim, numbers in
/// their shortest form (`1`, `0.5` — as JavaScript prints them), anything
/// else (absent / non-scalar) empty.
fn display_field(entry: &serde_json::Value, name: &str) -> String {
    match entry.get(name) {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Number(number)) => number
            .as_f64()
            .map_or_else(|| number.to_string(), |value| value.to_string()),
        _ => String::new(),
    }
}

/// One card row: the claim, attributed to its ONE author.
fn render_card_row(entry: &serde_json::Value) -> String {
    let field = |name: &str| escape_html(&display_field(entry, name));
    format!(
        "<li class=\"claim\" data-cid=\"{cid}\" data-author=\"{author}\">\
         <span class=\"subject\">{subject}</span> \
         <span class=\"predicate\">{predicate}</span> \
         <span class=\"object\">{object}</span> \
         <span class=\"confidence\">confidence {confidence}</span> \
         <span class=\"author\">by {author}</span> \
         <span class=\"cid\">{cid}</span></li>",
        cid = field("cid"),
        author = field("author_did"),
        subject = field("subject"),
        predicate = field("predicate"),
        object = field("object"),
        confidence = field("confidence"),
    )
}

/// The card body for the committed manifest entries (in manifest order).
fn card_body(entries: &[serde_json::Value]) -> String {
    if entries.is_empty() {
        return "<p class=\"empty-state\">no claims published yet</p>".to_string();
    }
    let rows: String = entries.iter().map(render_card_row).collect();
    format!("<ul class=\"claims\">{rows}</ul>")
}

/// Render the public read-only card HTML from committed manifest entries.
pub fn render_card(entries: &[serde_json::Value]) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>openlore — published claims</title></head>\
         <body><main class=\"openlore-card\"><h1>Published claims</h1>{}</main></body></html>",
        card_body(entries)
    )
}

/// `GET /` — the card as a `200` HTML response with the restrictive CSP.
fn card_response(manifest: &[(String, serde_json::Value)]) -> HttpResponse {
    let entries: Vec<serde_json::Value> = manifest.iter().map(|(_, e)| e.clone()).collect();
    hyper::Response::builder()
        .status(200)
        .header("content-type", "text/html; charset=utf-8")
        .header("content-security-policy", CARD_CONTENT_SECURITY_POLICY)
        .header("x-content-type-options", "nosniff")
        .body(http_body_util::Full::new(bytes::Bytes::from(render_card(
            &entries,
        ))))
        .expect("FakeInstance: build card response")
}

/// What an ordinary (non-openlore) web site serves on every route: a 200
/// HTML page with no openlore marker.
fn ordinary_web_page() -> HttpResponse {
    respond(
        200,
        "text/html; charset=utf-8",
        b"<!doctype html><title>Maria's blog</title><p>Just an ordinary web site.</p>".to_vec(),
    )
}

fn respond(status: u16, content_type: &str, body: Vec<u8>) -> HttpResponse {
    hyper::Response::builder()
        .status(status)
        .header("content-type", content_type)
        .body(http_body_util::Full::new(bytes::Bytes::from(body)))
        .expect("FakeInstance: build response")
}
