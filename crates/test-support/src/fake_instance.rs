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
//! Postures: slice-01 step 01-01 ships `fresh` (an empty, well-behaved
//! instance). The adversarial postures (`unreachable`,
//! `not_an_openlore_instance`, `with_cid_mismatch`, `requiring_write_token`,
//! `with_records`) land with the scenarios that need them.

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

/// Opaque content-addressed instance double. See module docs.
pub struct FakeInstance {
    store: Arc<Mutex<Store>>,
    base_url: String,
    server: tokio::task::JoinHandle<()>,
    runtime: Option<tokio::runtime::Runtime>,
}

impl FakeInstance {
    /// A reachable, empty, well-behaved openlore instance: its `/manifest`
    /// carries the openlore marker and lists no records yet.
    pub fn fresh() -> Self {
        Self::start(Store::default())
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

    fn lock(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store
            .lock()
            .expect("FakeInstance store mutex poisoned")
    }

    fn start(initial: Store) -> Self {
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
        let server = runtime.spawn(serve(listener, Arc::clone(&store)));
        Self {
            store,
            base_url,
            server,
            runtime: Some(runtime),
        }
    }
}

impl Drop for FakeInstance {
    fn drop(&mut self) {
        self.server.abort();
        // Shut the runtime down on a background thread so a worker parked on
        // `accept()` can never block the test thread (the macOS shutdown
        // hazard the acceptance `FakePds` wrapper documents).
        if let Some(runtime) = self.runtime.take() {
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

async fn serve(listener: tokio::net::TcpListener, store: Arc<Mutex<Store>>) {
    use hyper::server::conn::http1;
    use hyper_util::rt::TokioIo;

    loop {
        let Ok((stream, _peer)) = listener.accept().await else {
            return;
        };
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req| route(Arc::clone(&store), req));
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), svc)
                .await;
        });
    }
}

async fn route(
    store: Arc<Mutex<Store>>,
    req: HttpRequest,
) -> Result<HttpResponse, std::convert::Infallible> {
    use http_body_util::BodyExt;

    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    let record_cid = path.strip_prefix("/records/").map(str::to_string);
    let manifest_entry = req
        .headers()
        .get(MANIFEST_ENTRY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let response = match (method.as_str(), path.as_str(), record_cid) {
        ("GET", "/manifest", _) => {
            let manifest = store.lock().expect("store").manifest_json();
            respond(200, "application/json", manifest.to_string().into_bytes())
        }
        ("GET", "/", _) => respond(
            200,
            "text/html; charset=utf-8",
            b"<!doctype html><title>openlore instance</title><p>openlore opaque instance</p>"
                .to_vec(),
        ),
        ("GET", _, Some(cid)) => match store.lock().expect("store").blobs.get(&cid) {
            Some(bytes) => respond(200, "application/octet-stream", bytes.clone()),
            None => respond(404, "text/plain", b"record not found".to_vec()),
        },
        ("PUT", _, Some(cid)) => {
            let body = match req.into_body().collect().await {
                Ok(collected) => collected.to_bytes().to_vec(),
                Err(_) => return Ok(respond(400, "text/plain", b"unreadable body".to_vec())),
            };
            let parsed_entry = match manifest_entry.as_deref().map(serde_json::from_str) {
                None => None,
                Some(Ok(entry)) => Some(entry),
                Some(Err(_)) => {
                    return Ok(respond(
                        400,
                        "text/plain",
                        b"malformed manifest entry header".to_vec(),
                    ))
                }
            };
            let mut guard = store.lock().expect("store");
            guard.put_blob(&cid, body);
            if let Some(entry) = parsed_entry {
                guard.commit_entry(&cid, entry);
            }
            respond(201, "text/plain", Vec::new())
        }
        _ => respond(404, "text/plain", b"no such route".to_vec()),
    };
    Ok(response)
}

fn respond(status: u16, content_type: &str, body: Vec<u8>) -> HttpResponse {
    hyper::Response::builder()
        .status(status)
        .header("content-type", content_type)
        .body(http_body_util::Full::new(bytes::Bytes::from(body)))
        .expect("FakeInstance: build response")
}
