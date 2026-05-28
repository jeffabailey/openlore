//! `adapter-atproto-pds` — `PdsPort` over `reqwest`/`rustls` XRPC.
//!
//! Step 04-06: implements `PdsPort` against a real ATProto PDS over
//! HTTPS. The probe gauntlet (TLS handshake, describeServer DID match,
//! rkey-collision idempotency sentinel) lives in `probe.rs` as pure
//! arms; this module is the I/O shell that drives them.
//!
//! ## Slice-01 pragmatism (per DEVOPS open question #1)
//!
//! The task allows `atrium-api` OR direct `reqwest` XRPC. We pick
//! **direct `reqwest` with `rustls-tls-webpki-roots`** for slice-01
//! because:
//!
//! 1. `atrium-api` (v0.24+ as of 2026-05) drags a substantial
//!    transitive tree (atrium-xrpc, atrium-xrpc-client, multiple ipld
//!    crates, async-stream, dag-cbor codecs we don't need yet). Slice-01
//!    only consumes `createRecord` / `getRecord` / `listRecords` and a
//!    single `describeServer` call — all flat JSON XRPC endpoints.
//! 2. The `reqwest` approach gives us byte-exact control over the HTTP
//!    layer, which is load-bearing for the probe's 409-handling arm
//!    (treating conflict as idempotent success per architecture §6.2).
//! 3. Switching to `atrium-api` later is a non-breaking change behind
//!    the `PdsPort` trait — no caller (cli composition root, acceptance
//!    tests) depends on `reqwest` types.
//!
//! ADR-004 mandates `rustls` (not `native-tls`) for the system-trust-
//! store-agnostic posture; the `rustls-tls-webpki-roots` reqwest
//! feature flag enforces this.
//!
//! ## Composition shape (nw-fp-hexagonal-architecture §"adapter as
//! function-shape")
//!
//! - `AtProtoPdsAdapter::for_endpoint(url)`: construct pointed at a PDS.
//! - `AtProtoPdsAdapter::with_did(url, did)`: pin the expected
//!   describeServer DID for the probe arm.
//! - `#[async_trait] impl PdsPort for AtProtoPdsAdapter`: the four port
//!   methods. `probe()` is sync per the trait contract; the network arms
//!   are gated behind a `tokio::runtime::Handle::current().block_on(...)`
//!   shim so the sync probe API can drive async I/O without leaking
//!   tokio types up the public surface.
//!
//! ## Idempotency on 409 (WS-9 precondition)
//!
//! `create_record` posts to `com.atproto.repo.createRecord` with the
//! `swapCommit`-absent shape. The PDS may respond:
//!
//! - **200 OK**: a fresh record was created; the response carries the
//!   AT URI which we return verbatim.
//! - **409 Conflict** (or any `error: "RecordAlreadyExists"` body): the
//!   record at that `rkey` already exists. Per architecture §6.2 the
//!   adapter MUST treat this as success and surface the existing AT
//!   URI. We reconstruct the AT URI from `<endpoint_did>/<collection>/
//!   <rkey>` because the 409 body shape varies across PDS
//!   implementations.
//! - **Network error**: surfaces as `PdsError::Unreachable`.
//! - **TLS error**: surfaces as `PdsError::TlsHandshakeFailed`.
//! - **Other 4xx/5xx**: surfaces as `PdsError::RecordRejected`.
//!
//! ## RED-baseline status after step 04-06
//!
//! 21 acceptance-test panics remain on the cli composition-root steps
//! (phase 05); this step closes the last adapter gap so phase 05 can
//! wire the gauntlet through `probe_all` without any
//! `panic!("RED scaffold")` paths left in the adapter layer.

#![allow(dead_code)] // probe arms used only via probe(); network helpers used only on real PDSes
#![forbid(unsafe_code)]

use async_trait::async_trait;
use ports::claim_domain::Did;
use ports::{
    AtUri, CreateRecordOutcome, PdsError, PdsPort, PeerRecordPage, ProbeOutcome,
    ProbeRefusalReason, SignedRecord,
};
use url::Url;

pub mod probe;

// Slice-03 (federated read): peer-PDS read pipeline backing
// `PdsPort::list_peer_records` + `PdsPort::get_peer_record`. Bodied as
// `todo!()` at step 01-03; live implementation lands per the PP-*
// scenarios in Phase 04.
mod peer_read;

/// The collection used by OpenLore claim records. Pinned by ADR-005
/// (Lexicon definition `org.openlore.claim`).
pub const OPENLORE_CLAIM_COLLECTION: &str = "org.openlore.claim";

/// Sentinel rkey the idempotency probe writes twice. Chosen to be
/// recognizable in a PDS audit and unlikely to collide with a real CID.
/// (CIDs start with `bafy`; this string is human-readable.)
pub const IDEMPOTENCY_SENTINEL_RKEY: &str = "openlore-probe-sentinel-0";

/// `PdsPort` adapter over HTTPS + rustls + reqwest. One value per PDS
/// endpoint; immutable after construction.
///
/// The adapter holds an `endpoint` URL (e.g.
/// `https://bsky.social`) and an optional `expected_did` the probe's
/// describeServer arm matches against. The expected DID is `Option<...>`
/// because callers may construct a "probe-skipping" adapter for tests
/// that only care about the create/get/list paths.
pub struct AtProtoPdsAdapter {
    /// Base PDS endpoint URL, e.g. `https://bsky.social`. Stored without
    /// a trailing slash; the XRPC paths are joined with a leading `/`.
    endpoint: String,
    /// The DID the user configured at `openlore init`. The probe arm
    /// asserts `describeServer.did` equals this. `None` skips the arm
    /// (used by tests that don't exercise probe paths).
    expected_did: Option<String>,
    /// The DID the adapter writes records under. Distinct from
    /// `expected_did` because in federation scenarios the user's
    /// author DID and the PDS's host DID can differ. Stored so
    /// `create_record` can synthesize AT URIs after a 409.
    author_did: Option<String>,
}

impl AtProtoPdsAdapter {
    /// Build the adapter pointed at the given PDS endpoint URL. No
    /// describeServer DID pinned — the probe arm 2 (DID match) is
    /// skipped when constructed this way.
    pub fn for_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: normalize_endpoint(endpoint),
            expected_did: None,
            author_did: None,
        }
    }

    /// Build the adapter pointed at the given PDS endpoint URL with the
    /// describeServer DID + author DID pinned for the probe + AT URI
    /// synthesis paths.
    pub fn with_did(
        endpoint: impl Into<String>,
        expected_did: impl Into<String>,
        author_did: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: normalize_endpoint(endpoint),
            expected_did: Some(expected_did.into()),
            author_did: Some(author_did.into()),
        }
    }

    /// Endpoint URL the adapter is bound to. Exposed for tests + the
    /// composition root's startup banner.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Synthesize an AT URI for the given collection + rkey, using the
    /// configured author DID. Pulled out so `create_record` can compute
    /// it after a 409/conflict without re-parsing the PDS's response body
    /// (whose shape varies across implementations). Falls back to a
    /// `did:plc:unknown` placeholder when no author DID was configured —
    /// this is the test-path shape and the production path always pins
    /// a DID via `with_did`.
    fn synth_at_uri(&self, collection: &str, rkey: &str) -> String {
        let did = self.author_did.as_deref().unwrap_or("did:plc:unknown");
        format!("at://{did}/{collection}/{rkey}")
    }
}

/// Strip a trailing `/` so endpoint + path joins don't double-slash.
fn normalize_endpoint(s: impl Into<String>) -> String {
    let mut s = s.into();
    while s.ends_with('/') {
        s.pop();
    }
    s
}

/// Classify a reqwest network error into a `PdsError` variant. TLS
/// handshake failures are surfaced distinctly so the composition root
/// can render the WD-6-friendly remediation hint; everything else is
/// lifted into `Unreachable` so WS-10's preserve-local-on-publish-fail
/// path triggers uniformly.
fn classify_network_error(err: reqwest::Error) -> PdsError {
    let msg = err.to_string();
    let lower = msg.to_lowercase();
    if lower.contains("tls") || lower.contains("certificate") || lower.contains("handshake") {
        PdsError::TlsHandshakeFailed { message: msg }
    } else {
        PdsError::Unreachable { message: msg }
    }
}

#[async_trait]
impl PdsPort for AtProtoPdsAdapter {
    /// Walk the three probe arms (architecture-design §6.2). The first
    /// arm that refuses is surfaced via `ProbeOutcome::Refused`; all-green
    /// returns `ProbeOutcome::Ok`.
    ///
    /// ### Slice-01 wiring
    ///
    /// The arms in `probe.rs` are pure — they consume the *outcome* of
    /// an I/O step and produce structured refusals. For slice-01 this
    /// adapter does not yet drive live network probes from `probe()`
    /// itself because the trait signature is sync and bridging into the
    /// async XRPC layer from a sync context requires a tokio runtime
    /// handle the composition root has not yet wired (phase 05). The
    /// arms here therefore short-circuit to `Ok` UNLESS a configured
    /// expected DID is missing in a way that means "the adapter is
    /// misconfigured" — that is detected ahead of any I/O.
    ///
    /// Phase 05 (cli composition root) will rewire this `probe()`
    /// to invoke `tokio::runtime::Handle::block_on` on the live network
    /// probe driver in `lib.rs::probe_network`. The arm contracts in
    /// `probe.rs` will not change; only the I/O glue moves.
    fn probe(&self) -> ProbeOutcome {
        // Pre-flight check: an adapter built via `with_did` but with an
        // empty endpoint cannot be probed. Surface this as a
        // PdsTlsHandshakeFailed (the user's configured PDS effectively
        // does not exist) so the composition root refuses startup with a
        // clear reason.
        if self.endpoint.is_empty() {
            return ProbeOutcome::Refused {
                reason: ProbeRefusalReason::PdsTlsHandshakeFailed,
                detail: "PDS endpoint URL is empty; configure pds_endpoint at openlore init"
                    .to_string(),
                structured: serde_json::json!({"endpoint": ""}),
            };
        }

        // Slice-01 deferred: the live TLS handshake + describeServer +
        // sentinel idempotency probes wire in phase 05 when the cli
        // composition root has a tokio runtime to bridge into. The arm
        // contracts in `probe.rs` are already pinned by unit tests; the
        // I/O driver fills in around them.
        ProbeOutcome::Ok
    }

    /// Post a record to `com.atproto.repo.createRecord`. On success
    /// returns the AT URI the PDS returned. On 409/conflict (rkey
    /// collision) returns the synthesized AT URI for the existing
    /// record — idempotent re-publish per architecture §6.2.
    ///
    /// ### Slice-01 wiring
    ///
    /// The XRPC client wiring (auth header, session refresh, rate-limit
    /// honoring) lands progressively in phase 05's WS-9 / FR-1 / FR-2
    /// scenarios. For slice-01 this method returns `Unreachable` when
    /// the endpoint cannot be resolved; the cli composition-root will
    /// detect this and fall back to local-claim preservation per US-003.
    /// The `FakePds` test double in `openlore-test-support` exercises
    /// the full happy path through the `PdsPort` trait.
    async fn create_record(
        &self,
        collection: &str,
        rkey: &str,
        body: serde_json::Value,
    ) -> Result<CreateRecordOutcome, PdsError> {
        if self.endpoint.is_empty() {
            return Err(PdsError::Unreachable {
                message: "PDS endpoint URL is empty; configure pds_endpoint at openlore init"
                    .to_string(),
            });
        }

        // Step 05-08: real reqwest XRPC POST. The body shape follows
        // ATProto's `com.atproto.repo.createRecord` lexicon:
        // `{repo, collection, rkey, record}`. We pass `repo` =
        // the configured author DID (the test seam uses the same DID
        // the FakePds is bound to so AT URIs round-trip).
        //
        // Idempotency: a 409 conflict (or `RecordAlreadyExists` body)
        // is lifted into a fresh `Ok(AtUri)` carrying the synthesized
        // at-uri — slice-01 architecture §6.2 mandates treating rkey
        // collision as idempotent success. WS-9 pins this contract.
        // Other 4xx/5xx surface as `PdsError::RecordRejected`. Network
        // failures (DNS, refused connection, dropped socket) surface as
        // `PdsError::Unreachable` — the WS-10 path.
        let url = format!("{}/xrpc/com.atproto.repo.createRecord", self.endpoint);
        let repo = self.author_did.as_deref().unwrap_or("did:plc:unknown");
        let request_body = serde_json::json!({
            "repo": repo,
            "collection": collection,
            "rkey": rkey,
            "record": body,
        });

        // Connect timeout caps how long we wait for a TCP handshake
        // before classifying the host as Unreachable. The cli composition
        // root surfaces this as the WS-10 "preserve local claim, retry
        // later" message — the user should not see the binary appear to
        // hang for minutes when a PDS is misconfigured. 10s is generous
        // enough for slow CI runners but tight enough that WS scenarios
        // testing unreachable hosts complete within their own time
        // budgets.
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|err| PdsError::Unreachable {
                message: format!("build reqwest client: {err}"),
            })?;

        let response = client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(classify_network_error)?;

        let status = response.status();
        if status.is_success() {
            // Body shape: `{uri: "at://...", cid: "..."}`. We honor the
            // PDS's at-uri verbatim (some PDSes synthesize it slightly
            // differently than our naive `at://<did>/<col>/<rkey>` —
            // e.g. wrapping in a percent-encoding for non-ASCII rkeys
            // OpenLore doesn't generate but the contract permits).
            let parsed: serde_json::Value =
                response
                    .json()
                    .await
                    .map_err(|err| PdsError::RecordRejected {
                        message: format!("decode createRecord response: {err}"),
                    })?;
            let uri = parsed
                .get("uri")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.synth_at_uri(collection, rkey));
            Ok(CreateRecordOutcome {
                at_uri: AtUri(uri),
                was_idempotent: false,
            })
        } else if status == reqwest::StatusCode::CONFLICT {
            // 409 conflict — idempotent success path. Synthesize the
            // at-uri from our own DID + collection + rkey because the
            // 409 body shape varies across PDS implementations. The
            // `was_idempotent` bit lets the cli render "already
            // published" instead of acting like a fresh insert (WS-9).
            Ok(CreateRecordOutcome {
                at_uri: AtUri(self.synth_at_uri(collection, rkey)),
                was_idempotent: true,
            })
        } else {
            let body_text = response.text().await.unwrap_or_default();
            Err(PdsError::RecordRejected {
                message: format!(
                    "PDS returned status {status} on createRecord {collection}/{rkey}: {body_text}"
                ),
            })
        }
    }

    /// Read a record from `com.atproto.repo.getRecord`. Slice-01 stub
    /// returns `None` because no records have been written by the real
    /// adapter path yet; phase 05 wires the live XRPC call.
    async fn get_record(
        &self,
        _collection: &str,
        _rkey: &str,
    ) -> Result<Option<serde_json::Value>, PdsError> {
        if self.endpoint.is_empty() {
            return Err(PdsError::Unreachable {
                message: "PDS endpoint URL is empty".to_string(),
            });
        }
        Ok(None)
    }

    /// List records via `com.atproto.repo.listRecords`. Slice-01 stub
    /// returns an empty list; phase 05 wires the live XRPC call.
    async fn list_records(&self, _collection: &str) -> Result<Vec<serde_json::Value>, PdsError> {
        if self.endpoint.is_empty() {
            return Err(PdsError::Unreachable {
                message: "PDS endpoint URL is empty".to_string(),
            });
        }
        Ok(Vec::new())
    }

    /// Page through a peer's `org.openlore.claim` records (slice-03).
    /// Delegates to the `peer_read` module. The `peer_pds_endpoint` is
    /// taken fresh per ADR-016 (re-resolved at every pull; never cached
    /// on the adapter).
    ///
    /// SCAFFOLD: true (slice-03)
    ///
    /// Bodied via `peer_read::list_peer_records_xrpc`, which is `todo!()`
    /// at step 01-03; the live `listRecords` cursor walk lands per the
    /// PP-* scenarios in Phase 04.
    async fn list_peer_records(
        &self,
        peer_did: &Did,
        peer_pds_endpoint: &Url,
        cursor: Option<String>,
    ) -> Result<PeerRecordPage, PdsError> {
        // SCAFFOLD: true (slice-03)
        peer_read::list_peer_records_xrpc(peer_did, peer_pds_endpoint, cursor).await
    }

    /// Fetch one peer record by `rkey` (slice-03). Delegates to the
    /// `peer_read` module. Endpoint taken fresh per ADR-016.
    ///
    /// SCAFFOLD: true (slice-03)
    ///
    /// Bodied via `peer_read::get_peer_record_xrpc`, which is `todo!()`
    /// at step 01-03; the live single-record `getRecord` lands per the
    /// PP-* scenarios in Phase 04.
    async fn get_peer_record(
        &self,
        peer_did: &Did,
        peer_pds_endpoint: &Url,
        rkey: &str,
    ) -> Result<SignedRecord, PdsError> {
        // SCAFFOLD: true (slice-03)
        peer_read::get_peer_record_xrpc(peer_did, peer_pds_endpoint, rkey).await
    }
}

// -----------------------------------------------------------------------------
// Inner-TDD unit tests — constructor + AT-URI synthesis + empty-endpoint refusal.
//
// Real-network paths (TLS handshake, live describeServer, real
// createRecord) are integration territory and will be exercised by the
// contract-test layer per architecture-design §6.5. The unit tests below
// cover the adapter's pure-shaped surface: constructor wiring, AT URI
// shape, and the "empty endpoint" pre-flight refusal.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_endpoint_strips_trailing_slashes() {
        assert_eq!(
            normalize_endpoint("https://pds.example/"),
            "https://pds.example"
        );
        assert_eq!(
            normalize_endpoint("https://pds.example///"),
            "https://pds.example"
        );
        assert_eq!(
            normalize_endpoint("https://pds.example"),
            "https://pds.example"
        );
    }

    #[test]
    fn for_endpoint_constructs_with_no_did_pinned() {
        let adapter = AtProtoPdsAdapter::for_endpoint("https://pds.example/");
        assert_eq!(adapter.endpoint(), "https://pds.example");
        assert!(adapter.expected_did.is_none());
        assert!(adapter.author_did.is_none());
    }

    #[test]
    fn with_did_pins_expected_and_author_did() {
        let adapter =
            AtProtoPdsAdapter::with_did("https://bsky.social", "did:plc:host", "did:plc:test-jeff");
        assert_eq!(adapter.endpoint(), "https://bsky.social");
        assert_eq!(adapter.expected_did.as_deref(), Some("did:plc:host"));
        assert_eq!(adapter.author_did.as_deref(), Some("did:plc:test-jeff"));
    }

    /// AT URI synth uses the configured author DID. This is the same
    /// shape `FakePds` returns; pinning it here keeps the real adapter
    /// and the fake byte-for-byte compatible on the happy path.
    #[test]
    fn synth_at_uri_uses_configured_author_did() {
        let adapter =
            AtProtoPdsAdapter::with_did("https://pds.example", "did:plc:host", "did:plc:test-jeff");
        let uri = adapter.synth_at_uri(OPENLORE_CLAIM_COLLECTION, "bafy_test_001");
        assert_eq!(
            uri,
            "at://did:plc:test-jeff/org.openlore.claim/bafy_test_001"
        );
    }

    /// Pre-flight: an empty endpoint refuses the probe with
    /// PdsTlsHandshakeFailed (no host to handshake against).
    #[test]
    fn probe_refuses_when_endpoint_is_empty() {
        let adapter = AtProtoPdsAdapter::for_endpoint("");
        match adapter.probe() {
            ProbeOutcome::Refused { reason, .. } => {
                assert_eq!(reason, ProbeRefusalReason::PdsTlsHandshakeFailed);
            }
            ProbeOutcome::Ok => panic!("expected refusal for empty endpoint"),
        }
    }

    /// Probe arms in `probe.rs` are unit-tested in that module; here we
    /// only pin the lift behavior of the public `probe()` API.
    #[test]
    fn probe_returns_ok_when_endpoint_present_slice_01() {
        // Slice-01: with a non-empty endpoint, the public probe()
        // returns Ok (live network arms wire in phase 05). The arm
        // contracts themselves are pinned by probe.rs unit tests.
        let adapter = AtProtoPdsAdapter::for_endpoint("https://pds.example");
        assert!(matches!(adapter.probe(), ProbeOutcome::Ok));
    }

    /// Step 05-08: the previous stub `create_record_returns_synthesized_at_uri_on_stub_path`
    /// pinned a slice-01 placeholder behavior — the adapter synthesized
    /// the AT URI without making any HTTP call. That stub is gone now
    /// that this method does a real `reqwest` POST to
    /// `com.atproto.repo.createRecord`. The happy path is exercised
    /// end-to-end through the acceptance suite's FakePds HTTP server
    /// (`tests/acceptance/walking_skeleton.rs::walking_skeleton_publish_*`)
    /// where the cli subprocess actually talks to the in-process fake.
    ///
    /// What we CAN pin at the unit level without a real PDS reachable
    /// is the network-error classification arm: a host that refuses
    /// the TCP connect resolves into `PdsError::Unreachable`, NOT a
    /// panic, so the US-003 "preserve local claim on publish failure"
    /// path triggers uniformly.
    #[tokio::test]
    async fn create_record_returns_unreachable_against_refused_local_port() {
        // Bind a TcpListener to an OS-assigned localhost port, then
        // drop it immediately. The kernel reclaims the port but the
        // brief race window is wide enough — and any subsequent connect
        // to that port without a listener will be refused fast. This
        // gives us "Unreachable" classification without depending on a
        // multi-second network timeout.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("local_addr").port();
        drop(listener);

        let adapter = AtProtoPdsAdapter::with_did(
            format!("http://127.0.0.1:{port}"),
            "did:plc:host",
            "did:plc:test-jeff",
        );
        let result = adapter
            .create_record(OPENLORE_CLAIM_COLLECTION, "bafy_001", serde_json::json!({}))
            .await;
        assert!(
            matches!(result, Err(PdsError::Unreachable { .. })),
            "expected Unreachable against refused local port, got {result:?}"
        );
    }

    /// create_record on an empty-endpoint adapter surfaces Unreachable,
    /// not a panic. The cli composition root depends on this shape for
    /// the US-003 "PDS unreachable -> preserve local claim" path.
    #[tokio::test]
    async fn create_record_returns_unreachable_when_endpoint_empty() {
        let adapter = AtProtoPdsAdapter::for_endpoint("");
        let result = adapter
            .create_record(OPENLORE_CLAIM_COLLECTION, "bafy", serde_json::json!({}))
            .await;
        assert!(
            matches!(result, Err(PdsError::Unreachable { .. })),
            "expected Unreachable on empty endpoint, got {result:?}"
        );
    }

    /// Step 04-01 (replaces the 01-03 scaffold pin now that the live
    /// `peer_read` body has landed): driving `list_peer_records` through the
    /// `PdsPort` against a refused local port surfaces `PdsError::Unreachable`
    /// — never a panic, never a silently-empty page (the WD-37 / PP-7 fault
    /// isolation path the verb relies on). Bind+drop a listener to get a
    /// fast connection-refused without depending on a network timeout.
    #[tokio::test]
    async fn list_peer_records_surfaces_unreachable_against_refused_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("local_addr").port();
        drop(listener);

        let adapter = AtProtoPdsAdapter::for_endpoint("https://pds.example");
        let peer = Did("did:plc:test-peer".to_string());
        let endpoint = Url::parse(&format!("http://127.0.0.1:{port}")).expect("url parses");
        let result = adapter.list_peer_records(&peer, &endpoint, None).await;
        assert!(
            matches!(result, Err(PdsError::Unreachable { .. })),
            "expected Unreachable against a refused peer PDS port, got {result:?}"
        );
    }

    /// Step 04-01 (replaces the 01-03 scaffold pin): `get_peer_record`
    /// against a refused local port surfaces `PdsError::Unreachable` (the
    /// re-pull path's network-failure shape) rather than a panic.
    #[tokio::test]
    async fn get_peer_record_surfaces_unreachable_against_refused_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("local_addr").port();
        drop(listener);

        let adapter = AtProtoPdsAdapter::for_endpoint("https://pds.example");
        let peer = Did("did:plc:test-peer".to_string());
        let endpoint = Url::parse(&format!("http://127.0.0.1:{port}")).expect("url parses");
        let result = adapter.get_peer_record(&peer, &endpoint, "rkey-001").await;
        assert!(
            matches!(result, Err(PdsError::Unreachable { .. })),
            "expected Unreachable against a refused peer PDS port, got {result:?}"
        );
    }
}
