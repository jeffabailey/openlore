//! `search_handler` — the `serve` query handler over the index (B7, ADR-082).
//!
//! Runs one dimension query, composes per-author via the PURE
//! `appview_domain::compose_results` (the same core AVC-2 proves) and projects
//! FLAT attributed wire rows (every `author_did` present; anti-merging across
//! the transport, I-AV-2).
//!
//! The handler holds `IndexReadPort` and nothing else: by type it cannot change
//! the index, so no public request can (FR-IXD-2, DD-IXD-9). `xtask
//! check-arch`'s `indexer_search_handler_read_only` rule keeps every
//! write-capability name out of this module.

use std::sync::Arc;

use adapter_index_store::SEARCH_ROW_CAP;
use adapter_xrpc_query_server::{IndexUnavailable, QueryHandler};
use appview_domain::{
    compose_results, near_match_suggestion, NetworkSearchResult, SUGGESTION_MAX_DISTANCE,
};
use claim_domain::{Cid, Did};
use lexicon::{
    ClaimReferenceDto, SearchDimensionDto, SearchQueryRequest, SearchQueryResponse, SearchResultDto,
};
use ports::{IndexReadPort, IndexStoreError, IndexedClaim, SearchDimension};

/// The shared read handle the search handler closes over: the process's one
/// index handle (ADR-080), seen through its read port only.
pub type SharedIndexReads = Arc<dyn IndexReadPort + Send + Sync>;

/// A search that matched more than [`SEARCH_ROW_CAP`] claims and was cut
/// there (ADR-083 §3). Carries the dimension and the cap only: never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchTruncated {
    pub dimension: SearchDimension,
    pub cap: usize,
}

/// Where a cut search is reported (the composition root logs it).
pub type TruncationLog = Arc<dyn Fn(SearchTruncated) + Send + Sync>;

/// The `serve` query handler: every request is answered from `reads`; a
/// search cut at the row cap is reported to `on_truncated`.
pub fn search_handler(reads: SharedIndexReads, on_truncated: TruncationLog) -> QueryHandler {
    Arc::new(move |request: SearchQueryRequest| {
        handle_search(reads.as_ref(), request, on_truncated.as_ref())
    })
}

/// Keep at most [`SEARCH_ROW_CAP`] rows, saying whether any were cut.
fn capped(mut rows: Vec<IndexedClaim>) -> (Vec<IndexedClaim>, bool) {
    let cut = rows.len() > SEARCH_ROW_CAP;
    rows.truncate(SEARCH_ROW_CAP);
    (rows, cut)
}

/// TEST-FAULT seam (`OPENLORE_INDEXER_TEST_FAULT=search_store_read_fails`,
/// debug builds only): an index every read of which fails.
pub fn unreadable_index() -> SharedIndexReads {
    Arc::new(UnreadableIndex)
}

/// The read port of an index that cannot be read.
struct UnreadableIndex;

impl UnreadableIndex {
    fn failure<T>() -> Result<T, IndexStoreError> {
        Err(IndexStoreError::QueryFailed {
            message: "test fault: the index cannot be read".to_string(),
        })
    }
}

impl IndexReadPort for UnreadableIndex {
    fn query_by_object(&self, _: &str) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        Self::failure()
    }
    fn query_by_contributor(&self, _: &Did) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        Self::failure()
    }
    fn query_by_subject(&self, _: &str) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        Self::failure()
    }
    fn get_by_cid(&self, _: &Cid) -> Result<Option<IndexedClaim>, IndexStoreError> {
        Self::failure()
    }
    fn objects_near(&self, _: &str, _: usize) -> Result<Vec<String>, IndexStoreError> {
        Self::failure()
    }
}

/// One search: read the index along `request.dimension`,
/// compose per-author via the PURE `appview_domain::compose_results`, and project
/// the per-author structure back to a FLAT attributed wire response (every
/// `author_did` present; the `distinct_author_count` is the pure COUNT, never a
/// merge). A store error is [`IndexUnavailable`] (a 500), never an empty
/// result the client would read as "no results" (ADR-080 §7).
fn handle_search(
    reads: &dyn IndexReadPort,
    request: SearchQueryRequest,
    on_truncated: &(dyn Fn(SearchTruncated) + Send + Sync),
) -> Result<SearchQueryResponse, IndexUnavailable> {
    let dimension = from_dto_dimension(request.dimension);
    let rows = match dimension {
        SearchDimension::Object => reads.query_by_object(&request.value),
        SearchDimension::Subject => reads.query_by_subject(&request.value),
        SearchDimension::Contributor => reads.query_by_contributor(&Did(request.value.clone())),
    };
    let (rows, cut) = capped(rows.map_err(|_store_error| IndexUnavailable)?);
    if cut {
        on_truncated(SearchTruncated {
            dimension,
            cap: SEARCH_ROW_CAP,
        });
    }

    // The per-author grouping + the distinct-author COUNT come from the PURE
    // composition (the SAME core proven at layer 2 by AVC-2). The author ORDER on
    // the wire follows that stable composition; the per-row payload is projected
    // from the original `IndexedClaim` rows (which carry composed_at + evidence the
    // composed `NetworkResultRow` does not). The wire stays FLAT + attributed.
    let composed = compose_results(rows.clone(), dimension);
    let results = flat_attributed_rows(&composed, &rows);
    let suggestion = match (dimension, results.is_empty()) {
        (SearchDimension::Object, true) => suggest_object(reads, &request.value)?,
        _ => composed.suggestion,
    };
    Ok(SearchQueryResponse {
        results,
        distinct_author_count: composed.distinct_author_count,
        total_claims: composed.total_claims,
        suggestion,
    })
}

/// The near-match for an object nothing asserts (US-AV-002 Ex 4): one bounded
/// read of the indexed objects within the suggestion distance, ranked by the
/// PURE `near_match_suggestion`. The client asks once; it never sweeps the
/// index with probe searches (review H3).
fn suggest_object(
    reads: &dyn IndexReadPort,
    object: &str,
) -> Result<Option<String>, IndexUnavailable> {
    let near = reads
        .objects_near(object, SUGGESTION_MAX_DISTANCE)
        .map_err(|_store_error| IndexUnavailable)?;
    Ok(near_match_suggestion(object, &near))
}

/// Project the per-author `NetworkSearchResult` (the pure composition's stable
/// author order + within-group cid order) into FLAT attributed wire rows, looking
/// each row's full payload (composed_at, evidence) up from the original
/// `IndexedClaim` rows by cid. The wire carries one row per attributed claim (NO
/// merged/consensus object — I-AV-2).
fn flat_attributed_rows(
    composed: &NetworkSearchResult,
    rows: &[ports::IndexedClaim],
) -> Vec<SearchResultDto> {
    let mut out = Vec::new();
    for (_author, group) in &composed.by_author {
        for composed_row in group {
            let source = rows.iter().find(|r| r.cid == composed_row.cid);
            let composed_at = source
                .map(|r| r.composed_at.to_rfc3339())
                .unwrap_or_default();
            let evidence = source.map(|r| r.evidence.clone()).unwrap_or_default();
            // Carry the row's typed references over the wire (OD-AV-7): a countering
            // claim K's `counters` reference to the countered claim C's CID lets the
            // CLI render reconstruct C's `countered-by <K.cid> (by <K.author>)`
            // annotation (shown, never applied — I-AV-9). The reference rows carry no
            // author (anti-merging preserved); K's author is K's own `author_did`.
            let references = source
                .map(|r| r.references.iter().map(reference_to_dto).collect())
                .unwrap_or_default();
            // ADR-079: the stored provenance travels over the wire. App-signed rows
            // send the explicit token; an old reader ignores the additive field.
            let provenance = source.map(|r| provenance_token(r.provenance).to_string());
            out.push(SearchResultDto {
                author_did: composed_row.author_did.0.clone(),
                cid: composed_row.cid.0.clone(),
                subject: composed_row.subject.clone(),
                predicate: composed_row.predicate.clone(),
                object: composed_row.object.clone(),
                confidence: composed_row.confidence,
                composed_at,
                verified_against: composed_row.verified_against.0.clone(),
                evidence,
                references,
                provenance,
            });
        }
    }
    out
}

/// The ADR-079 wire token for a stored provenance (the same domain as the
/// `indexed_claims.provenance` column).
fn provenance_token(provenance: ports::PeerClaimProvenance) -> &'static str {
    match provenance {
        ports::PeerClaimProvenance::AppSigned => "app-signed",
        ports::PeerClaimProvenance::SelfAttested => "self-attested",
    }
}

/// Map a typed `claim_domain::ClaimReference` to its wire DTO, using the lowercase
/// `ref_type` token the `indexed_claim_references` CHECK domain + the on-disk
/// artifact use (so the wire, the store, and the artifact agree without drift).
fn reference_to_dto(reference: &claim_domain::ClaimReference) -> ClaimReferenceDto {
    let ref_type = match reference.ref_type {
        claim_domain::ReferenceType::Retracts => "retracts",
        claim_domain::ReferenceType::Corrects => "corrects",
        claim_domain::ReferenceType::Counters => "counters",
        claim_domain::ReferenceType::Supersedes => "supersedes",
    };
    ClaimReferenceDto {
        ref_type: ref_type.to_string(),
        cid: reference.cid.0.clone(),
    }
}

/// Map a wire DTO dimension to the domain `SearchDimension`.
fn from_dto_dimension(dim: SearchDimensionDto) -> SearchDimension {
    match dim {
        SearchDimensionDto::Object => SearchDimension::Object,
        SearchDimensionDto::Contributor => SearchDimension::Contributor,
        SearchDimensionDto::Subject => SearchDimension::Subject,
    }
}
