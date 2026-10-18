//! `adapter-atproto-ingest` — the indexer-side bounded-PULL ingest adapter.
//!
//! EFFECT shell for the `IngestSourcePort` trait (`crates/ports`). Performs a
//! bounded PULL of PUBLIC `org.openlore.claim` records via the ATProto
//! `com.atproto.repo.listRecords` XRPC (ADR-024). The fetched [`RawRecord`]s
//! flow to the pure `appview_domain::ingest_decision` gate; NO verification
//! happens here.
//!
//! ## Read-only by construction (capability boundary I-AV-5)
//!
//! This adapter holds NO `IdentityPort` / signing key and exposes NO write /
//! sign / publish method — the indexer is signing-incapable. The absence is the
//! design: there is structurally no path from this adapter to authoring or
//! mutating a claim.
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! Pure core (claim-domain, appview-domain) never imports this crate; the
//! indexer composition root wires an [`AtProtoIngestAdapter`] behind the
//! `IngestSourcePort` interface. The lexicon-JSON → domain `SignedClaim` parse
//! mirrors `adapter-atproto-pds::peer_read` byte-for-byte (the SAME wire shape:
//! `author`/`composedAt`/nested `signature:{kid,alg,sig}`; base64url-no-pad sig).
//
// SCAFFOLD: false  (step 03-01: live bounded-PULL `listRecords` for the AV-1
// walking skeleton; the relay/multi-source + network-lies probe arms are later).

#![allow(dead_code)]
#![forbid(unsafe_code)]

use async_trait::async_trait;
use claim_domain::{Cid, ClaimRecord, SignedClaim};
use std::sync::Arc;

use net_guard::{refused_by_policy, system_lookup};
pub use net_guard::{AddressRefusedByPolicy, GuardedResolver, HostLookup};
use ports::net_policy::{url_admissible, TransportPolicy};
use ports::{
    IngestError, IngestSourcePort, ProbeOutcome, RawRecord, RepoListing, RepoListingPort,
    RepoRecord,
};

/// The ATProto collection the indexer pulls (public signed claims).
const CLAIM_COLLECTION: &str = "org.openlore.claim";

/// How long a `listRecords` request may take: to connect, and in all
/// (connect, send and read the body). A PDS that does not answer in time is
/// unreachable, never a hung page load (the review app's GET /github reads
/// the owner's repo on every visit).
#[derive(Debug, Clone, Copy)]
struct ListingTimeouts {
    connect: std::time::Duration,
    request: std::time::Duration,
}

/// The bounds every adapter is built with; the connect bound mirrors
/// `adapter-atproto-pds::peer_read`.
const LISTING_TIMEOUTS: ListingTimeouts = ListingTimeouts {
    connect: std::time::Duration::from_secs(10),
    request: std::time::Duration::from_secs(30),
};

/// Bounded read-only PULL `IngestSourcePort` adapter over ATProto XRPC
/// (`listRecords`) — ADR-024.
///
/// READ-ONLY by construction (I-AV-5): holds NO signing identity and no local
/// store handle. Holds a `reqwest` client + the configured source base URL the
/// `enumerate` default reads when no explicit source is passed.
pub struct AtProtoIngestAdapter {
    client: reqwest::Client,
    /// The configured source base URL (a PDS / relay hosting `listRecords`).
    source: String,
    /// The transport policy every request obeys; `None` = unguarded (the
    /// review app's reader, never the indexer — xtask `indexer_guarded_clients_only`).
    policy: Option<TransportPolicy>,
}

impl AtProtoIngestAdapter {
    /// Construct the UNGUARDED ingest adapter pointed at `source` (a base URL
    /// hosting the public `com.atproto.repo.listRecords` surface). Redirects
    /// are never followed (ADR-078): a 3xx is a failed listing, its target is
    /// not contacted.
    pub fn new(source: &str) -> Self {
        Self::unguarded(source, LISTING_TIMEOUTS)
    }

    /// [`Self::new`] bounded by `timeouts` (the slow-PDS test seam).
    fn unguarded(source: &str, timeouts: ListingTimeouts) -> Self {
        let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
        Self::with_client(client, source, None, timeouts)
    }

    /// The SSRF-guarded adapter the indexer wires (ADR-077 §4, DD-IPF-5):
    /// every URL is pre-checked against `policy`, hostnames resolve through
    /// [`GuardedResolver`] (refused addresses dropped, the connection goes to
    /// a checked address), no proxy, redirects never followed.
    pub fn guarded(source: &str, policy: TransportPolicy) -> Self {
        Self::guarded_with_lookup(source, policy, system_lookup())
    }

    /// [`Self::guarded`] over an injected DNS lookup (the rebinding test seam).
    pub fn guarded_with_lookup(source: &str, policy: TransportPolicy, lookup: HostLookup) -> Self {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .dns_resolver(Arc::new(GuardedResolver::with_lookup(policy, lookup)));
        Self::with_client(client, source, Some(policy), LISTING_TIMEOUTS)
    }

    fn with_client(
        client: reqwest::ClientBuilder,
        source: &str,
        policy: Option<TransportPolicy>,
        timeouts: ListingTimeouts,
    ) -> Self {
        let client = client
            .connect_timeout(timeouts.connect)
            .timeout(timeouts.request);
        Self {
            client: client.build().unwrap_or_else(|_| reqwest::Client::new()),
            source: source.to_string(),
            policy,
        }
    }

    /// The configured source base URL.
    pub fn source(&self) -> &str {
        &self.source
    }
}

// -----------------------------------------------------------------------------
// The SSRF guard: DNS answers filtered by the transport policy (DD-IPF-5)
// -----------------------------------------------------------------------------

#[async_trait]
impl IngestSourcePort for AtProtoIngestAdapter {
    fn probe(&self) -> ProbeOutcome {
        // An empty source is a valid configuration since ADR-077: the indexer
        // has no fallback and lists every repo DID from the PDS its document
        // names. A configured source must be an absolute http(s) base URL —
        // and, for a guarded adapter, pass the transport-policy pre-check —
        // or the fallback could never be listed.
        let readiness = match self.policy {
            Some(policy) => guarded_source_readiness(&self.source, policy),
            None => source_readiness(&self.source),
        };
        match readiness {
            Ok(()) => ProbeOutcome::Ok,
            Err(detail) => ProbeOutcome::Refused {
                reason: ports::ProbeRefusalReason::PdsTlsHandshakeFailed,
                detail,
                structured: serde_json::json!({"adapter": "ingest_source"}),
            },
        }
    }

    /// Refused: `listRecords` lists ONE repo and a real PDS rejects a call
    /// without `repo`. The indexer enumerates each configured repo DID through
    /// [`RepoListingPort::list_repo_claims`] instead (ADR-071 §4, DWD-9).
    async fn enumerate(&self, _source: &str) -> Result<Vec<RawRecord>, IngestError> {
        Err(IngestError::BadResponse {
            message: "listRecords needs repo=<DID>: enumerate per repo DID".to_string(),
        })
    }
}

/// Whether a configured source can be listed from (pure): empty (no source)
/// or an absolute `http`/`https` URL.
fn source_readiness(source: &str) -> Result<(), String> {
    let source = source.trim();
    if source.is_empty() {
        return Ok(());
    }
    match url::Url::parse(source) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(()),
        _ => Err(format!(
            "ingest source {source:?} is not an absolute http(s) URL — cannot PULL listRecords"
        )),
    }
}

/// [`source_readiness`] plus the transport-policy pre-check (pure).
fn guarded_source_readiness(source: &str, policy: TransportPolicy) -> Result<(), String> {
    source_readiness(source)?;
    match url::Url::parse(source.trim()) {
        Ok(url) if !url_admissible(&url, policy) => Err(format!(
            "ingest source {source:?} is refused by the transport policy {policy:?}"
        )),
        _ => Ok(()),
    }
}

// -----------------------------------------------------------------------------
// One repo's claims: `listRecords` with `repo=<DID>`, cursor paging, bounded
// -----------------------------------------------------------------------------

/// Records asked for per `listRecords` page (the XRPC maximum).
const PAGE_LIMIT: usize = 100;

/// The most pages one repo enumeration reads (bounds a hostile or endless
/// cursor chain: at most `MAX_PAGES * PAGE_LIMIT` records).
pub const MAX_PAGES: usize = 50;

/// One `listRecords` page: its records and the cursor to the next page.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedPage<T> {
    pub records: Vec<T>,
    pub cursor: Option<String>,
}

/// Paging so far: the records collected, the pages read, and the cursor the
/// next page is requested with (`None` before the first page).
#[derive(Debug, Clone, PartialEq)]
pub struct Paging<T> {
    pub collected: Vec<T>,
    pub pages_read: usize,
    pub cursor: Option<String>,
}

impl<T> Paging<T> {
    pub fn start() -> Self {
        Self {
            collected: Vec::new(),
            pages_read: 0,
            cursor: None,
        }
    }
}

/// What to do after a page arrived.
#[derive(Debug, Clone, PartialEq)]
pub enum PagingStep<T> {
    /// Request the next page with `paging.cursor`.
    Fetch(Paging<T>),
    /// Every page has been read (or the bound was reached).
    Done(Vec<T>),
}

/// Fold one page into the paging state (pure). Paging stops when the page
/// names no cursor, is empty, repeats the cursor it was requested with, or
/// the page bound is reached, so every enumeration terminates.
pub fn take_page<T>(paging: Paging<T>, page: ListedPage<T>) -> PagingStep<T> {
    let Paging {
        mut collected,
        pages_read,
        cursor: requested_with,
    } = paging;
    let page_was_empty = page.records.is_empty();
    collected.extend(page.records);
    let pages_read = pages_read + 1;
    match page.cursor {
        Some(next)
            if !page_was_empty
                && pages_read < MAX_PAGES
                && requested_with.as_deref() != Some(next.as_str()) =>
        {
            PagingStep::Fetch(Paging {
                collected,
                pages_read,
                cursor: Some(next),
            })
        }
        _ => PagingStep::Done(collected),
    }
}

#[async_trait]
impl RepoListingPort for AtProtoIngestAdapter {
    async fn list_repo_claims(
        &self,
        pds_base: &str,
        repo_did: &str,
    ) -> Result<RepoListing, IngestError> {
        let fetched_from = pds_base.trim_end_matches('/').to_string();
        let mut paging = Paging::start();
        loop {
            let page = self
                .list_page(&fetched_from, repo_did, paging.cursor.as_deref())
                .await?;
            match take_page(paging, page) {
                PagingStep::Fetch(next) => paging = next,
                PagingStep::Done(records) => {
                    return Ok(RepoListing {
                        fetched_from,
                        records,
                    })
                }
            }
        }
    }
}

impl AtProtoIngestAdapter {
    /// One `listRecords` page of `repo_did`'s claims from `base`.
    async fn list_page(
        &self,
        base: &str,
        repo_did: &str,
        cursor: Option<&str>,
    ) -> Result<ListedPage<RepoRecord>, IngestError> {
        let mut url = url::Url::parse(&format!("{base}/xrpc/com.atproto.repo.listRecords"))
            .map_err(|err| IngestError::BadResponse {
                message: format!("PDS URL is not a URL: {err}"),
            })?;
        url.query_pairs_mut()
            .append_pair("repo", repo_did)
            .append_pair("collection", CLAIM_COLLECTION)
            .append_pair("limit", &PAGE_LIMIT.to_string());
        if let Some(cursor) = cursor {
            url.query_pairs_mut().append_pair("cursor", cursor);
        }
        if let Some(policy) = self.policy.filter(|policy| !url_admissible(&url, *policy)) {
            return Err(IngestError::AddressRefused {
                message: format!("{base} is refused by the transport policy {policy:?}"),
            });
        }
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(transport_failure)?;
        if let Some(failure) = listing_status_failure(response.status().as_u16()) {
            return Err(failure);
        }
        let body: serde_json::Value = response.json().await.map_err(body_failure)?;
        parse_listed_page(&body)
    }
}

/// A transport error: refused by the guard, or the source is unreachable.
fn transport_failure(err: reqwest::Error) -> IngestError {
    if refused_by_policy(&err) {
        IngestError::AddressRefused {
            message: format!("listRecords: {err}"),
        }
    } else {
        IngestError::Unreachable {
            message: format!("listRecords transport error: {err}"),
        }
    }
}

/// A body that did not arrive in time is an unreachable source; any other
/// unreadable body is a bad response.
fn body_failure(err: reqwest::Error) -> IngestError {
    if err.is_timeout() {
        IngestError::Unreachable {
            message: format!("listRecords body timed out: {err}"),
        }
    } else {
        IngestError::BadResponse {
            message: format!("listRecords body is not JSON: {err}"),
        }
    }
}

/// The failure a non-2xx `listRecords` status is reported as (pure): 5xx and
/// 429 mean the PDS is unreachable for now; any other non-2xx (3xx included,
/// redirects are never followed) is a bad response. `None` for 2xx.
fn listing_status_failure(status: u16) -> Option<IngestError> {
    let message = format!("listRecords returned HTTP {status}");
    match status {
        200..=299 => None,
        429 | 500..=599 => Some(IngestError::Unreachable { message }),
        _ => Some(IngestError::BadResponse { message }),
    }
}

/// A `listRecords` body as one page of repo records (pure).
fn parse_listed_page(body: &serde_json::Value) -> Result<ListedPage<RepoRecord>, IngestError> {
    let records = body
        .get("records")
        .and_then(|r| r.as_array())
        .ok_or_else(|| IngestError::BadResponse {
            message: "listRecords response missing `records` array".to_string(),
        })?
        .iter()
        .map(parse_repo_record)
        .collect::<Result<Vec<_>, _>>()?;
    let cursor = body
        .get("cursor")
        .and_then(|c| c.as_str())
        .map(str::to_string);
    Ok(ListedPage { records, cursor })
}

/// One record view as a [`RepoRecord`]: repo DID and rkey from its
/// `at://<did>/<collection>/<rkey>` URI.
fn parse_repo_record(view: &serde_json::Value) -> Result<RepoRecord, IngestError> {
    let uri = view.get("uri").and_then(|u| u.as_str()).unwrap_or_default();
    let (repo_did, rkey) = repo_and_rkey(uri).ok_or_else(|| IngestError::BadResponse {
        message: "record view has no `at://<did>/<collection>/<rkey>` uri".to_string(),
    })?;
    Ok(RepoRecord {
        repo_did,
        rkey,
        value: view.get("value").cloned().unwrap_or_default(),
    })
}

/// `(did, rkey)` of `at://<did>/<collection>/<rkey>`.
fn repo_and_rkey(uri: &str) -> Option<(String, String)> {
    let mut parts = uri.strip_prefix("at://")?.split('/');
    let (did, _collection, rkey) = (parts.next()?, parts.next()?, parts.next()?);
    let well_formed = did.starts_with("did:") && !rkey.is_empty() && parts.next().is_none();
    well_formed.then(|| (did.to_string(), rkey.to_string()))
}

// -----------------------------------------------------------------------------
// Lexicon-JSON → domain RawRecord parse (mirrors adapter-atproto-pds::peer_read)
// -----------------------------------------------------------------------------

/// Parse one ATProto record view (`{uri, cid, value}`) into a `RawRecord`. The
/// network-published `cid` becomes `RawRecord::published_cid` (recomputed +
/// verified by the pure gate); `value` is the lexicon claim body. NO trust
/// decision is made here — the gate verifies the signature + recomputes the CID.
fn parse_record_view(view: &serde_json::Value) -> Result<RawRecord, IngestError> {
    let bad = |detail: String| IngestError::BadResponse { message: detail };

    let body = view.get("value").unwrap_or(view);
    let published_cid = view
        .get("cid")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| bad("record view missing `cid`".to_string()))?;

    let signed = parse_signed_claim(body)?;
    Ok(RawRecord {
        published_cid: Cid(published_cid),
        raw_payload: signed,
        source_pds: view
            .get("uri")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default(),
    })
}

/// Parse a lexicon-shaped claim JSON body into the domain `SignedClaim`
/// through the ONE shared decoder (`claim_domain::decode_claim_record`). The
/// unsigned-CID is recomputed so the claim is self-consistent — but NO trust
/// decision is made (the gate verifies).
///
/// ADR-071 fail-closed: the indexer's source is an operator-configured URL,
/// so its origin is not yet computed against the repo DID's resolved PDS. A
/// self-attested (unsigned) record is therefore refused here exactly as an
/// unsigned record always was, until the indexer classifies origins.
fn parse_signed_claim(body: &serde_json::Value) -> Result<SignedClaim, IngestError> {
    let bad = |detail: String| IngestError::BadResponse { message: detail };
    match claim_domain::decode_claim_record(body, "").map_err(bad)? {
        ClaimRecord::AppSigned(signed) => Ok(signed),
        ClaimRecord::SelfAttested(_) => Err(bad("signature block missing".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Serve `records` split into pages of the given sizes; the cursor of a
    /// page is the index its successor starts at (the last page has none).
    fn serve(records: &[u32], sizes: &[usize], cursor: Option<&str>) -> ListedPage<u32> {
        let start: usize = cursor.and_then(|c| c.parse().ok()).unwrap_or(0);
        let boundaries: Vec<usize> = sizes
            .iter()
            .scan(0, |end, size| {
                *end += size;
                Some(*end)
            })
            .take_while(|end| *end < records.len())
            .collect();
        let end = boundaries
            .iter()
            .copied()
            .find(|b| *b > start)
            .unwrap_or(records.len());
        ListedPage {
            records: records[start.min(records.len())..end].to_vec(),
            cursor: (end < records.len()).then(|| end.to_string()),
        }
    }

    /// Run the pure pager against `fetch`, counting the requests.
    fn enumerate(mut fetch: impl FnMut(Option<&str>) -> ListedPage<u32>) -> (Vec<u32>, usize) {
        let mut paging = Paging::start();
        let mut requests = 0;
        loop {
            requests += 1;
            let page = fetch(paging.cursor.as_deref());
            match take_page(paging, page) {
                PagingStep::Fetch(next) => paging = next,
                PagingStep::Done(records) => return (records, requests),
            }
        }
    }

    proptest! {
        /// Universe: record lists (up to the bound) and every way of
        /// splitting them into pages. Paging terminates and yields every
        /// record exactly once, in order, including the last page.
        #[test]
        fn cursor_paging_yields_each_record_once_for_any_page_split(
            count in 0usize..400,
            sizes in proptest::collection::vec(1usize..=PAGE_LIMIT, 1..=MAX_PAGES),
        ) {
            let records: Vec<u32> = (0..count as u32).collect();
            prop_assume!(sizes.iter().sum::<usize>() >= count);
            let (seen, requests) = enumerate(|cursor| serve(&records, &sizes, cursor));
            prop_assert_eq!(&seen, &records);
            prop_assert!(requests <= MAX_PAGES);
        }

        /// Universe: servers that repeat a cursor forever or never stop.
        /// Paging still terminates within the page bound.
        #[test]
        fn a_cursor_chain_that_never_ends_is_bounded(repeat_cursor in any::<bool>(), size in 1usize..5) {
            let mut issued = 0u32;
            let (seen, requests) = enumerate(|_| {
                issued += 1;
                ListedPage {
                    records: vec![issued; size],
                    cursor: Some(if repeat_cursor { "same".to_string() } else { issued.to_string() }),
                }
            });
            prop_assert!(requests <= MAX_PAGES);
            prop_assert_eq!(seen.len(), requests * size);
        }
    }

    /// The `at://` URI names the repo and the rkey the claim CID is checked
    /// against; the view's `cid` field is ignored.
    // bypass: one wire example pins the URI-to-(repo, rkey) mapping.
    #[test]
    fn a_listed_page_takes_repo_and_rkey_from_the_record_uri() {
        let body = serde_json::json!({
            "records": [{
                "uri": "at://did:plc:priya/org.openlore.claim/bafyrkey",
                "cid": "bafyreiother",
                "value": {"subject": "github:priyaraman/tidepool"}
            }],
            "cursor": "next"
        });
        let page = parse_listed_page(&body).expect("well-formed page");
        assert_eq!(page.cursor.as_deref(), Some("next"));
        assert_eq!(page.records[0].repo_did, "did:plc:priya");
        assert_eq!(page.records[0].rkey, "bafyrkey");
        assert!(parse_listed_page(&serde_json::json!({"records": [{"uri": "nope"}]})).is_err());
    }

    /// The lexicon → RawRecord parse maps the wire fields onto the domain ADT and
    /// preserves the published CID + the author (the inner-loop contract the
    /// bounded PULL relies on; AV-1).
    #[test]
    fn parse_record_view_maps_wire_fields_to_raw_record() {
        let view = serde_json::json!({
            "uri": "at://did:plc:priya-test/org.openlore.claim/bafyabc",
            "cid": "bafyabc",
            "value": {
                "subject": "github:bazelbuild/bazel",
                "predicate": "embodiesPhilosophy",
                "object": "org.openlore.philosophy.reproducible-builds",
                "evidence": ["https://example.test/e"],
                "confidence": 0.82,
                "author": "did:plc:priya-test#org.openlore.application",
                "composedAt": "2026-05-26T12:00:00Z",
                "references": [],
                "signature": { "kid": "did:plc:priya-test#org.openlore.application", "alg": "EdDSA", "sig": "AAAA" }
            }
        });
        let record = parse_record_view(&view).expect("parse well-formed record view");
        assert_eq!(record.published_cid.0, "bafyabc");
        assert_eq!(
            record.raw_payload.unsigned.author_did.0,
            "did:plc:priya-test#org.openlore.application"
        );
        assert_eq!(
            record.raw_payload.unsigned.subject,
            "github:bazelbuild/bazel"
        );
    }

    proptest! {
        /// Universe: every HTTP status. 2xx lists; 5xx and 429 are
        /// unreachable; every other status (3xx, 4xx, 1xx) is a bad response.
        #[test]
        fn a_listing_status_maps_to_exactly_one_outcome(status in 100u16..600) {
            let outcome = listing_status_failure(status);
            let expected = match status {
                200..=299 => "listed",
                429 | 500..=599 => "unreachable",
                _ => "bad_response",
            };
            let observed = match outcome {
                None => "listed",
                Some(IngestError::Unreachable { .. }) => "unreachable",
                Some(IngestError::BadResponse { .. }) => "bad_response",
                Some(IngestError::ProbeRefused { .. }) => "probe_refused",
                Some(IngestError::AddressRefused { .. }) => "address_refused",
            };
            prop_assert_eq!(observed, expected);
        }

        /// A source is ready iff it is unset (blank) or an absolute http(s)
        /// URL; any other scheme or a bare host is refused.
        #[test]
        fn a_source_is_ready_only_when_blank_or_an_http_url(
            host in "[a-z]{1,10}\\.[a-z]{2,4}",
            scheme in prop_oneof![Just("http"), Just("https"), Just("ftp"), Just("")],
            blank in "[ \t]{0,3}",
        ) {
            prop_assert!(source_readiness(&blank).is_ok());
            let source = if scheme.is_empty() { host.clone() } else { format!("{scheme}://{host}") };
            prop_assert_eq!(
                source_readiness(&source).is_ok(),
                matches!(scheme, "http" | "https")
            );
        }
    }

    /// A listener on `[::1]` counting the connections that reached it.
    fn loopback_listener() -> (std::net::TcpListener, u16) {
        let listener = std::net::TcpListener::bind("[::1]:0").expect("bind [::1]");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        (listener, port)
    }

    fn connections(listener: &std::net::TcpListener) -> usize {
        std::iter::from_fn(|| listener.accept().ok()).count()
    }

    /// DNS answers only refused addresses (a rebinding-style answer for an
    /// innocent-looking name).
    fn rebinding_lookup() -> HostLookup {
        std::sync::Arc::new(|_host: String| {
            Box::pin(async {
                Ok(vec![
                    "10.0.0.1".parse().unwrap(),
                    "::1".parse().unwrap(),
                    "169.254.169.254".parse().unwrap(),
                ])
            })
        })
    }

    async fn list_through_rebinding_dns(policy: TransportPolicy, port: u16) -> Option<IngestError> {
        let adapter = AtProtoIngestAdapter::guarded_with_lookup("", policy, rebinding_lookup());
        let base = format!("https://pds.rebind.test:{port}");
        let listing = adapter.list_repo_claims(&base, "did:plc:x");
        match tokio::time::timeout(std::time::Duration::from_secs(2), listing).await {
            Ok(result) => result.err(),
            Err(_elapsed) => None,
        }
    }

    // bypass: a real socket + stub DNS (adapter integration), one example per policy.
    #[tokio::test]
    async fn a_name_resolving_only_to_refused_addresses_is_never_connected_to() {
        let (listener, port) = loopback_listener();
        let refused = list_through_rebinding_dns(TransportPolicy::HttpsPublicOnly, port).await;
        assert!(
            matches!(refused, Some(IngestError::AddressRefused { .. })),
            "{refused:?}"
        );
        assert_eq!(
            connections(&listener),
            0,
            "a refused address is never contacted"
        );

        // Non-vacuity: under the test policy the loopback answer is admitted
        // and the connection reaches the listener.
        let _ = list_through_rebinding_dns(TransportPolicy::HttpsOrLoopbackHttp, port).await;
        assert_eq!(connections(&listener), 1);
    }

    // bypass: a real socket that accepts and never answers (adapter integration).
    #[tokio::test]
    async fn a_pds_that_never_answers_is_unreachable_once_the_request_times_out() {
        let silent = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", silent.local_addr().expect("addr"));
        let adapter = AtProtoIngestAdapter::unguarded(
            "",
            ListingTimeouts {
                connect: std::time::Duration::from_millis(500),
                request: std::time::Duration::from_millis(300),
            },
        );
        let listing = adapter.list_repo_claims(&base, "did:plc:x");
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), listing)
            .await
            .expect("the adapter's own timeout ends the request");
        assert!(
            matches!(outcome, Err(IngestError::Unreachable { .. })),
            "{outcome:?}"
        );
    }

    #[test]
    fn every_listing_is_bounded_in_connecting_and_in_all() {
        assert_eq!(LISTING_TIMEOUTS.connect, std::time::Duration::from_secs(10));
        assert_eq!(LISTING_TIMEOUTS.request, std::time::Duration::from_secs(30));
    }

    // bypass: closed-world pre-check examples (no DNS involved), one per refusal kind.
    #[tokio::test]
    async fn refused_urls_are_rejected_before_any_request() {
        let adapter = AtProtoIngestAdapter::guarded("", TransportPolicy::HttpsPublicOnly);
        for base in [
            "http://pds.example.com",
            "https://10.0.0.1",
            "https://[::ffff:127.0.0.1]",
        ] {
            let refused = adapter.list_repo_claims(base, "did:plc:x").await;
            assert!(
                matches!(refused, Err(IngestError::AddressRefused { .. })),
                "{base}: {refused:?}"
            );
        }
        assert!(
            matches!(adapter.probe(), ProbeOutcome::Ok),
            "an empty source is ready"
        );
        let insecure = AtProtoIngestAdapter::guarded(
            "http://relay.example.com",
            TransportPolicy::HttpsPublicOnly,
        );
        assert!(matches!(insecure.probe(), ProbeOutcome::Refused { .. }));
    }
}
