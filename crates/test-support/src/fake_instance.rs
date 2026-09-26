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
//! | `GET /` | Placeholder public card (HTML). |
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
//! openlore marker) — step 01-03, Q-SF-D5. The remaining adversarial
//! postures (`with_cid_mismatch`, `requiring_write_token`, `with_records`)
//! land with the scenarios that need them.
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

/// How the double answers — the posture a scenario puts it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Posture {
    /// A well-behaved openlore opaque instance (the four ADR-062 routes).
    OpenloreInstance,
    /// A reachable ordinary web site: every route (incl. `/manifest`) is an
    /// HTML page with no openlore marker.
    OrdinaryWebSite,
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
        let server = runtime.spawn(serve(listener, Arc::clone(&store), posture));
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

async fn serve(listener: tokio::net::TcpListener, store: Arc<Mutex<Store>>, posture: Posture) {
    use hyper::server::conn::http1;
    use hyper_util::rt::TokioIo;

    loop {
        let Ok((stream, _peer)) = listener.accept().await else {
            return;
        };
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req| {
                let store = Arc::clone(&store);
                async move { Ok::<_, std::convert::Infallible>(answer(&store, posture, req).await) }
            });
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), svc)
                .await;
        });
    }
}

/// Receive → log → answer per posture.
async fn answer(store: &Mutex<Store>, posture: Posture, req: HttpRequest) -> HttpResponse {
    let Some(request) = receive(req).await else {
        return respond(400, "text/plain", b"unreadable body".to_vec());
    };
    let mut guard = store.lock().expect("store");
    guard.requests.push(request.clone());
    match posture {
        Posture::OpenloreInstance => route(&mut guard, &request),
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

fn route(store: &mut Store, request: &RecordedRequest) -> HttpResponse {
    let record_cid = request.path.strip_prefix("/records/");
    match (request.method.as_str(), request.path.as_str(), record_cid) {
        ("GET", "/manifest", _) => respond(
            200,
            "application/json",
            store.manifest_json().to_string().into_bytes(),
        ),
        ("GET", "/", _) => respond(
            200,
            "text/html; charset=utf-8",
            b"<!doctype html><title>openlore instance</title><p>openlore opaque instance</p>"
                .to_vec(),
        ),
        ("GET", _, Some(cid)) => match store.blobs.get(cid) {
            Some(bytes) => respond(200, "application/octet-stream", bytes.clone()),
            None => respond(404, "text/plain", b"record not found".to_vec()),
        },
        ("PUT", _, Some(cid)) => put_record(store, cid, request),
        _ => respond(404, "text/plain", b"no such route".to_vec()),
    }
}

/// `PUT /records/:cid` — store verbatim; commit the manifest entry when the
/// header carries one.
fn put_record(store: &mut Store, cid: &str, request: &RecordedRequest) -> HttpResponse {
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
    store.put_blob(cid, request.body.clone());
    if let Some(entry) = parsed_entry {
        store.commit_entry(cid, entry);
    }
    respond(201, "text/plain", Vec::new())
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
