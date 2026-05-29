//! `adapter-index-query` — the CLI-side index-query adapter (over HTTP/XRPC).
//!
//! EFFECT shell for the `IndexQueryPort` trait (`crates/ports`). Queries the
//! self-hosted indexer at a CONFIGURED URL over HTTP/XRPC
//! (`org.openlore.appview.searchClaims`, ADR-027) and decodes the FLAT attributed
//! transport response ([`lexicon::SearchQueryResponse`]) into the raw
//! [`NetworkSearchResultRaw`] the CLI re-composes per-author via the pure
//! `appview-domain` core.
//!
//! ## Unreachable is SOFT and NON-FATAL (KPI-AV-5 / KPI-5 / WD-116)
//!
//! A connection failure maps to [`IndexQueryError::Unreachable`] — a SOFT,
//! NON-FATAL outcome. An unreachable indexer NEVER blocks local CLI startup nor
//! any local-first verb; `search` degrades to a clear local-only message and
//! exits 0. READ-ONLY by construction (no sign/write method).
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! The pure core never imports this crate; the CLI composition root wires a
//! [`HttpIndexQueryAdapter`] behind the `IndexQueryPort` interface. Per the
//! anti-merging-across-the-transport contract (I-AV-2), a response row dropping
//! `author_did` is a `BadResponse` (the wire MUST carry attribution).
//
// SCAFFOLD: false  (step 04-01: live B1 transport for the `--object` walking skeleton)

#![forbid(unsafe_code)]

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did, KeyId};
use lexicon::{
    SearchDimensionDto, SearchQueryRequest, SearchQueryResponse, SearchResultDto,
    SEARCH_CLAIMS_NSID,
};
use ports::{
    IndexQueryError, IndexQueryPort, NetworkResultRowRaw, NetworkSearchResultRaw, ProbeOutcome,
    ProbeRefusalReason, SearchDimension,
};

/// The bounded connect timeout (KPI-5 / WD-116): an unreachable indexer that does
/// NOT promptly refuse (an unrouted/blackhole address) MUST fail fast to
/// `Unreachable` rather than blocking the CLI indefinitely. This is the
/// bounded-wall-clock guarantee AV-13 (the cardinal local-first gate) depends on.
const INDEXER_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// The bounded total request timeout: caps the whole round-trip (connect + send +
/// receive) so a stalled-mid-response indexer also fails fast to `Unreachable`.
const INDEXER_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// CLI-side `IndexQueryPort` adapter over HTTP/XRPC to the configured indexer
/// URL (ADR-027). Holds the reqwest client + the configured indexer base URL. No
/// signing/identity field exists (read-only by construction).
pub struct HttpIndexQueryAdapter {
    client: reqwest::Client,
    /// The configured indexer base URL (e.g. `http://127.0.0.1:54321`).
    base_url: String,
}

impl HttpIndexQueryAdapter {
    /// Construct the CLI-side query adapter with NO configured URL (the empty
    /// base). Kept zero-arg so the composition root's `_phantom_index_query`
    /// dep-graph anchor (`fn() -> HttpIndexQueryAdapter`) keeps compiling. A
    /// search through this degenerate adapter degrades to the SOFT `Unreachable`
    /// outcome; the configured adapter is built via [`Self::for_url`].
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::for_url(String::new())
    }

    /// Construct the CLI-side query adapter for a configured indexer base URL.
    ///
    /// The reqwest client carries a BOUNDED connect + total-request timeout
    /// (KPI-5 / WD-116): an unreachable indexer — whether it refuses promptly OR
    /// silently blackholes the connect — fails fast to [`IndexQueryError::
    /// Unreachable`] rather than hanging the CLI indefinitely. This is the
    /// bounded-wall-clock guarantee the cardinal local-first gate (AV-13) rests
    /// on; a misconfigured client (no timeout) would let `search` hang forever on
    /// an unrouted address, breaking the soft-degradation contract.
    pub fn for_url(base_url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(INDEXER_CONNECT_TIMEOUT)
            .timeout(INDEXER_REQUEST_TIMEOUT)
            .build()
            // A builder failure here is a static-config bug (TLS backend init),
            // not a runtime condition; fall back to a defaulted client so the
            // adapter still constructs (degenerate, but never panics startup).
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            base_url: base_url.into(),
        }
    }

    /// The full XRPC endpoint URL for the searchClaims query.
    fn search_endpoint(&self) -> String {
        format!(
            "{}/xrpc/{SEARCH_CLAIMS_NSID}",
            self.base_url.trim_end_matches('/')
        )
    }
}

#[async_trait]
impl IndexQueryPort for HttpIndexQueryAdapter {
    fn probe(&self) -> ProbeOutcome {
        // SOFT at startup (DESIGN §6.3 / KPI-5 / WD-116): an unreachable indexer is
        // INFORMATIONAL, NOT a refusal — the CLI MUST start (and `claim add` /
        // offline `claim publish` / `graph query` MUST succeed) WITHOUT a reachable
        // indexer. So the probe performs NO network round-trip; reachability is the
        // SOFT `Unreachable` outcome `search` returns at use time, never a startup
        // refusal. A CLI that hard-refused on an unreachable indexer is the cardinal
        // regression AV-13 disproves — the INVERTED check.
        //
        // The real, deterministic readiness work this probe DOES do is the
        // reachable-SHAPE contract (DESIGN §6.3(a)): the decode path this adapter
        // will run over the wire correctly PRESERVES per-row `author_did` on a
        // well-formed response AND REFUSES a response that dropped it (anti-merging
        // across the transport, I-AV-2 / D-D36). This is an in-process self-probe
        // (no I/O) — a regression in the decode contract is a genuine startup-worthy
        // refusal, while an unreachable indexer stays SOFT.
        match probe_decode_shape_contract() {
            Ok(()) => ProbeOutcome::Ok,
            Err(detail) => ProbeOutcome::Refused {
                reason: ProbeRefusalReason::LexiconInvalid,
                detail,
                structured: serde_json::json!({
                    "contract": "anti_merging_across_transport",
                    "check": "decode_shape",
                }),
            },
        }
    }

    async fn search(
        &self,
        dim: SearchDimension,
        value: &str,
        cid: Option<&Cid>,
    ) -> Result<NetworkSearchResultRaw, IndexQueryError> {
        let request = SearchQueryRequest {
            dimension: to_dto_dimension(dim),
            value: value.to_string(),
            cid: cid.map(|c| c.0.clone()),
        };

        let response = self
            .client
            .post(self.search_endpoint())
            .json(&request)
            .send()
            .await
            // A connection failure (refused, DNS, timeout) is the SOFT,
            // NON-FATAL Unreachable outcome (WD-116) — NEVER a panic.
            .map_err(|err| IndexQueryError::Unreachable {
                message: format!("indexer at {} unreachable: {err}", self.base_url),
            })?;

        if !response.status().is_success() {
            return Err(IndexQueryError::BadResponse {
                message: format!("indexer returned HTTP {}", response.status()),
            });
        }

        let body: SearchQueryResponse =
            response
                .json()
                .await
                .map_err(|err| IndexQueryError::BadResponse {
                    message: format!("decode searchClaims response: {err}"),
                })?;

        decode_response(body)
    }
}

/// Map a domain `SearchDimension` to its wire DTO keyword.
fn to_dto_dimension(dim: SearchDimension) -> SearchDimensionDto {
    match dim {
        SearchDimension::Object => SearchDimensionDto::Object,
        SearchDimension::Contributor => SearchDimensionDto::Contributor,
        SearchDimension::Subject => SearchDimensionDto::Subject,
    }
}

/// Decode the lexicon `SearchQueryResponse` into the raw FLAT attributed
/// transport result. Every row MUST carry a non-empty `author_did` (the
/// anti-merging-across-the-transport contract, I-AV-2) — a dropped attribution is
/// a `BadResponse`.
fn decode_response(body: SearchQueryResponse) -> Result<NetworkSearchResultRaw, IndexQueryError> {
    let mut results = Vec::with_capacity(body.results.len());
    for row in body.results {
        results.push(decode_row(row)?);
    }
    Ok(NetworkSearchResultRaw {
        results,
        distinct_author_count: body.distinct_author_count,
        total_claims: body.total_claims,
        suggestion: body.suggestion,
    })
}

/// Decode one wire `SearchResultDto` into a `NetworkResultRowRaw`. Refuses an
/// empty `author_did` (I-AV-2 — the wire dropped attribution).
fn decode_row(row: SearchResultDto) -> Result<NetworkResultRowRaw, IndexQueryError> {
    if row.author_did.is_empty() {
        return Err(IndexQueryError::BadResponse {
            message: "a result row carried an empty author_did (I-AV-2 violation)".to_string(),
        });
    }
    let composed_at: DateTime<Utc> =
        row.composed_at
            .parse()
            .map_err(|err| IndexQueryError::BadResponse {
                message: format!("parse composed_at {:?}: {err}", row.composed_at),
            })?;
    Ok(NetworkResultRowRaw {
        author_did: Did(row.author_did),
        cid: Cid(row.cid),
        subject: row.subject,
        predicate: row.predicate,
        object: row.object,
        confidence: row.confidence,
        composed_at,
        verified_against: KeyId(row.verified_against),
        evidence: row.evidence,
        references: Vec::new(),
    })
}

/// The reachable-SHAPE self-probe (DESIGN §6.3(a); step 04-06). Exercise the
/// decode path this adapter runs over the wire against two sentinel responses —
/// WITHOUT any network I/O (reachability is SOFT, never a startup refusal):
///
///   1. a well-formed response decodes Ok and PRESERVES per-row `author_did`
///      (anti-merging across the transport, I-AV-2 / D-D36);
///   2. a response that DROPPED `author_did` is refused as `BadResponse` (the
///      client's attribution gate is intact).
///
/// Returns `Err(detail)` if the decode contract regressed (a genuine
/// startup-worthy refusal — the wire-shape gate is broken), else `Ok(())`.
fn probe_decode_shape_contract() -> Result<(), String> {
    fn sentinel_row(author_did: &str) -> SearchResultDto {
        SearchResultDto {
            author_did: author_did.to_string(),
            cid: "bafyprobe".to_string(),
            subject: "github:bazelbuild/bazel".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.appview.__probe__".to_string(),
            confidence: 0.82,
            composed_at: "2026-05-28T00:00:00Z".to_string(),
            verified_against: "did:plc:priya-test#org.openlore.application".to_string(),
            evidence: Vec::new(),
        }
    }

    // (1) A well-formed response preserves per-row author_did across the decode.
    let well_formed = SearchQueryResponse {
        results: vec![sentinel_row("did:plc:priya-test")],
        distinct_author_count: 1,
        total_claims: 1,
        suggestion: None,
    };
    let decoded = decode_response(well_formed).map_err(|err| {
        format!(
            "reachable-shape probe: a well-formed searchClaims response failed to decode: {err:?}"
        )
    })?;
    match decoded.results.first() {
        Some(row) if row.author_did == Did("did:plc:priya-test".to_string()) => {}
        other => {
            return Err(format!(
                "reachable-shape probe: decode dropped/altered per-row author_did \
                 (anti-merging across the transport violated; I-AV-2/D-D36); got {other:?}"
            ));
        }
    }

    // (2) A response that DROPPED author_did must be refused as BadResponse — the
    // client's attribution gate is intact.
    let dropped = SearchQueryResponse {
        results: vec![sentinel_row("")],
        distinct_author_count: 0,
        total_claims: 1,
        suggestion: None,
    };
    match decode_response(dropped) {
        Err(IndexQueryError::BadResponse { .. }) => Ok(()),
        other => Err(format!(
            "reachable-shape probe: a dropped-author response must decode to a \
             BadResponse (the attribution gate must reject it; I-AV-2/D-D36); got {other:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    //! DELIVER inner loop (step 04-01): the decode contract — the lexicon
    //! `SearchQueryResponse` decodes into `NetworkSearchResultRaw` preserving
    //! per-row `author_did` (I-AV-2), and a row with an empty `author_did` is a
    //! `BadResponse`. Pure-function unit (no network); the live transport is
    //! exercised end-to-end by AV-8.
    //!
    //! Step 04-06 adds the SOFT reachable-shape probe contract (DESIGN §6.3).

    use super::*;

    fn dto_row(author_did: &str) -> SearchResultDto {
        SearchResultDto {
            author_did: author_did.to_string(),
            cid: "bafyk2".to_string(),
            subject: "github:bazelbuild/bazel".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.reproducible-builds".to_string(),
            confidence: 0.82,
            composed_at: "2026-05-28T00:00:00Z".to_string(),
            verified_against: "did:plc:priya-test#org.openlore.application".to_string(),
            evidence: vec!["https://example.org/e1".to_string()],
        }
    }

    /// The load-bearing decode contract: every wire row preserves its non-empty
    /// `author_did` into the raw transport result (anti-merging across the
    /// transport, I-AV-2). Two distinct-author rows stay TWO rows.
    #[test]
    fn decode_preserves_author_did_per_row() {
        let body = SearchQueryResponse {
            results: vec![
                dto_row("did:plc:priya-test"),
                dto_row("did:plc:rachel-test"),
            ],
            distinct_author_count: 2,
            total_claims: 2,
            suggestion: None,
        };

        let raw = decode_response(body).expect("decode succeeds");

        assert_eq!(raw.results.len(), 2, "two rows stay two rows (no merge)");
        assert_eq!(
            raw.results[0].author_did,
            Did("did:plc:priya-test".to_string())
        );
        assert_eq!(
            raw.results[1].author_did,
            Did("did:plc:rachel-test".to_string())
        );
        assert_eq!(raw.distinct_author_count, 2);
    }

    /// A wire row that dropped `author_did` is a `BadResponse` (the transport MUST
    /// carry attribution — I-AV-2).
    #[test]
    fn empty_author_did_row_is_bad_response() {
        let body = SearchQueryResponse {
            results: vec![dto_row("")],
            distinct_author_count: 0,
            total_claims: 1,
            suggestion: None,
        };

        let err = decode_response(body).expect_err("empty author_did must be rejected");
        assert!(
            matches!(err, IndexQueryError::BadResponse { .. }),
            "an empty author_did over the wire is a BadResponse; got {err:?}"
        );
    }

    /// The reachable-shape self-probe (DESIGN §6.3(a)) accepts when the decode
    /// contract holds — a well-formed response preserves author_did AND a
    /// dropped-author response is refused. No network I/O.
    #[test]
    fn probe_decode_shape_contract_accepts_when_the_decode_gate_holds() {
        assert!(
            probe_decode_shape_contract().is_ok(),
            "the reachable-shape decode contract must hold (preserve author_did; \
             refuse a dropped-author response)"
        );
    }

    /// The INVERTED/degradation check (DESIGN §6.3(b) / KPI-5 / WD-116): a
    /// CONFIGURED-but-UNREACHABLE indexer is SOFT — the probe returns `Ok`, NOT a
    /// startup refusal. The probe does NO network round-trip; reachability is the
    /// SOFT `Unreachable` outcome `search` returns at USE time. A CLI that hard-
    /// refused here on an unreachable indexer is the cardinal AV-13 regression.
    #[test]
    fn probe_is_soft_for_a_configured_but_unreachable_indexer() {
        // A configured URL pointing at a port nothing listens on (unreachable).
        let adapter = HttpIndexQueryAdapter::for_url("http://127.0.0.1:1");
        assert!(
            matches!(adapter.probe(), ProbeOutcome::Ok),
            "an unreachable indexer must be SOFT — the probe must NOT refuse startup \
             (KPI-5 / WD-116 inverted check)"
        );
    }

    /// The degenerate (no configured URL) adapter is ALSO SOFT — the bootstrap
    /// composition-root anchor must never hard-refuse startup either.
    #[test]
    fn probe_is_soft_for_the_unconfigured_adapter() {
        let adapter = HttpIndexQueryAdapter::new();
        assert!(
            matches!(adapter.probe(), ProbeOutcome::Ok),
            "the unconfigured adapter must be SOFT (never a startup refusal)"
        );
    }

    /// RED_UNIT for AV-13 (the cardinal local-first gate, KPI-5 / WD-116): the
    /// bounded-wall-clock guarantee. A `search` against an indexer that ACCEPTS
    /// the connection but NEVER responds (a stalled / blackhole indexer) MUST
    /// fail fast to the SOFT, NON-FATAL `IndexQueryError::Unreachable` within the
    /// bounded request timeout — it must NOT hang the CLI indefinitely.
    ///
    /// This is the load-bearing proof for the timeout added to [`for_url`]:
    /// WITHOUT `.timeout(...)`, the accept-but-never-respond connection would
    /// block forever (this test would hang). The closed-port AT path (a refused
    /// connect) resolves promptly even without a timeout, so the timeout's
    /// correctness is proven HERE, against a connection that connects but stalls.
    #[tokio::test]
    async fn search_against_a_stalled_indexer_fails_fast_to_unreachable() {
        use std::io::Read;
        use std::net::TcpListener;
        use std::time::Instant;

        // A localhost listener that ACCEPTS connections but never writes a
        // response (it just holds the accepted socket open + drains a little of
        // the request). connect() succeeds; the POST send/recv stalls — so the
        // bounded REQUEST timeout (not the connect timeout) is what must fire.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stall listener");
        let addr = listener.local_addr().expect("read stall listener addr");
        let _stall = std::thread::spawn(move || {
            // Accept one connection and hold it open without ever responding.
            if let Ok((mut stream, _)) = listener.accept() {
                let mut sink = [0u8; 1024];
                // Drain a little so the client's send completes; then stall
                // forever (never write a response). Holding `stream` keeps the
                // socket open so the client waits on the response, not on a RST.
                let _ = stream.read(&mut sink);
                std::thread::sleep(std::time::Duration::from_secs(120));
                drop(stream);
            }
        });

        let adapter = HttpIndexQueryAdapter::for_url(format!("http://{addr}"));

        let started = Instant::now();
        let result = adapter
            .search(SearchDimension::Object, "org.openlore.philosophy.x", None)
            .await;
        let elapsed = started.elapsed();

        // SOFT, NON-FATAL: a stalled indexer maps to Unreachable (WD-116), never a
        // panic and never a hang.
        assert!(
            matches!(result, Err(IndexQueryError::Unreachable { .. })),
            "a stalled indexer must map to the SOFT Unreachable outcome; got {result:?}"
        );
        // Bounded wall-clock: the call returned well within the request-timeout +
        // slack ceiling — it did NOT hang indefinitely. (Without `.timeout(...)`
        // on the client, this would never return and the test would hang.)
        assert!(
            elapsed < INDEXER_REQUEST_TIMEOUT + std::time::Duration::from_secs(5),
            "the bounded request timeout must fire promptly (≈{:?}); the search took {:?} \
             — an unbounded client would hang forever (KPI-5 / WD-116)",
            INDEXER_REQUEST_TIMEOUT,
            elapsed
        );
    }
}
