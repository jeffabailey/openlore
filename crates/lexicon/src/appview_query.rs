//! `appview_query` — the `org.openlore.appview.searchClaims` XRPC query lexicon
//! + the shared CLI↔indexer request/response DTOs (ADR-027).
//!
//! This is a `query` Lexicon — a READ query, NOT a signed record: there is NO
//! signed payload and NO CID-stability concern (unlike `org.openlore.claim`).
//! Nothing new is SIGNED in slice-05; this is the only Lexicon addition and it
//! is read-only.
//!
//! The DTOs live in `lexicon` (the pure-core query lexicon) so the CLI client
//! (`adapter-index-query`) and the indexer server (`adapter-xrpc-query-server`)
//! agree on the wire shape WITHOUT drift — DELIVER co-locates them here per
//! component-boundaries.md §"Grouping note".
//!
//! ## Anti-merging across the transport (I-AV-2 / WD-103)
//!
//! Every [`SearchResultDto`] carries `author_did` as a non-empty `String` (the
//! wire form of a non-`Option<Did>`). There is NO `consensus` / `merged` /
//! `aggregate` object in the response shape — grouping by author is the CLI
//! renderer's job (it re-composes via the pure `appview-domain` core). The wire
//! carries FLAT attributed rows. See data-models.md §"The XRPC query DTOs".
//
// SCAFFOLD: false  (the DTOs + their serde round-trip are real; the over-the-wire
// transport that uses them is the adapter layer's Phase 03/04 concern)

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// NSID for the `org.openlore.appview.searchClaims` query Lexicon (ADR-027). A
/// `query` (READ) type — no signed payload.
pub const SEARCH_CLAIMS_NSID: &str = "org.openlore.appview.searchClaims";

/// The search dimension a query addresses. The wire form of
/// `ports::SearchDimension` — serialized as the lowercase dimension keyword the
/// XRPC query-param contract uses (`"object" | "contributor" | "subject"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchDimensionDto {
    /// By philosophy URI — the headline dimension.
    Object,
    /// By author DID — one developer's whole network trail.
    Contributor,
    /// By project URI.
    Subject,
}

/// The `org.openlore.appview.searchClaims` request DTO (query params; ADR-027).
///
/// `dimension` + `value` are the dimension search; `cid = Some(..)` selects one
/// result for `--show` (inspect one result). The request carries NO auth scope —
/// the indexer serves only PUBLIC verified claims (I-AV-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQueryRequest {
    /// `"object" | "contributor" | "subject"`.
    pub dimension: SearchDimensionDto,
    /// The philosophy URI | DID | project URI to search for.
    pub value: String,
    /// For `--show`: inspect one result by CID. Absent ⇒ a dimension search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
}

/// One result row carried over the wire (CLI ← indexer; ADR-027).
///
/// `author_did` is ALWAYS present + non-empty — the anti-merging-across-the-
/// transport contract (I-AV-2): dropping attribution over the wire is a contract
/// violation the CLI's `BadResponse` arm catches. `verified_against` is never
/// empty (drives the `[verified]` marker; every result was verified at ingest,
/// WD-104).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResultDto {
    /// ALWAYS present + non-empty (anti-merging across the transport, I-AV-2).
    pub author_did: String,
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// Numeric `[0.0, 1.0]` (WD-10) — the display bucket is render-only.
    pub confidence: f64,
    pub composed_at: String,
    /// Drives the `[verified]` marker (never empty, WD-104).
    pub verified_against: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Typed inter-claim references the row carries (data-models.md §"The XRPC
    /// query DTOs" — `"references": [...]`). LOAD-BEARING for OD-AV-7: a
    /// countering claim K carries a `counters` reference to the countered claim
    /// C's CID, which the CLI render reads to annotate C `countered-by <K.cid>
    /// (by <K.author_did>)` (shown, never applied — I-AV-9). `#[serde(default)]`
    /// keeps a references-less row backward-compatible (the field is omitted on
    /// the wire when empty).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ClaimReferenceDto>,
}

/// One typed inter-claim reference carried over the wire (the wire form of
/// `claim_domain::ClaimReference`). `ref_type` is the lowercase token the
/// `indexed_claim_references` CHECK domain uses (`"retracts" | "corrects" |
/// "counters" | "supersedes"`) so the wire, the store schema, and the on-disk
/// artifact agree without drift; `cid` is the REFERENCED claim's CID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimReferenceDto {
    /// `"retracts" | "corrects" | "counters" | "supersedes"`.
    pub ref_type: String,
    /// The referenced (e.g. countered) claim's CID.
    pub cid: String,
}

/// The `org.openlore.appview.searchClaims` response DTO (ADR-027).
///
/// `results` is a FLAT list of attributed rows (NO per-author grouping, NO
/// merged/consensus object — I-AV-2); the CLI re-composes the per-author view
/// via the pure `appview-domain` core. `distinct_author_count` is reported by
/// the indexer (itself a COUNT over attributed rows; never a merge).
/// `suggestion` carries the near-match for an empty dimension result
/// (US-AV-002 Ex 4); the CLI exits 0 on an empty result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchQueryResponse {
    /// FLAT attributed rows; NO merged-author row exists over the wire.
    #[serde(default)]
    pub results: Vec<SearchResultDto>,
    /// COUNT over attributed rows; never a merge.
    #[serde(default)]
    pub distinct_author_count: u32,
    #[serde(default)]
    pub total_claims: u32,
    /// Near-match suggestion for an empty result (US-AV-002 Ex 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

// =============================================================================
// Config shapes (recognized, pure serde — NO I/O here)
// =============================================================================

/// The CLI's `[appview]` config section in `identity.toml` (ADR-023/027). The
/// CLI gains ONE optional key for the indexer URL so `search` knows where to
/// query; localhost default. Recognized here as a pure serde shape — the CLI
/// reads + applies it (the I/O is the CLI's concern, not the lexicon's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AppviewConfig {
    /// The self-hosted indexer URL (ADR-023/027); localhost default applied at
    /// the CLI layer when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexer_url: Option<String>,
}

/// The indexer's OWN config shape (NOT `identity.toml`; the two binaries are
/// config-disjoint, ADR-023). Recognized here as a pure serde shape — the
/// `openlore-indexer` binary reads + applies it. See data-models.md
/// §"indexer config".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// The SEPARATE `index.duckdb` store path (ADR-023/025).
    pub index_path: String,
    /// The HTTP/XRPC query surface listen address (ADR-027).
    pub listen_addr: String,
    /// DID-document resolution endpoint (ADR-026).
    pub plc_endpoint: String,
    /// Bounded-pull cadence (ADR-024; DELIVER tunes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingest_interval: Option<String>,
    /// The bounded ingest seed/relay sources (ADR-024).
    #[serde(default)]
    pub sources: IndexerSources,
}

/// The indexer's bounded ingest sources (ADR-024). A bounded seed-DID set + an
/// optional relay (still PULL, not a firehose subscription).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct IndexerSources {
    /// Bounded seed DID set (ADR-024).
    #[serde(default)]
    pub seed_dids: Vec<String>,
    /// Optional relay URL (still PULL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Property-shaped roundtrip pin (the lexicon's Earned-Trust contract): the
    /// response DTO serde round-trip ALWAYS preserves `author_did` on every
    /// result row (anti-merging across the transport, I-AV-2). A bare example is
    /// sufficient here — the load-bearing claim is "the field survives the
    /// round-trip", which `serde` either does for all rows or none.
    #[test]
    fn search_response_roundtrip_preserves_author_did_per_result() {
        let response = SearchQueryResponse {
            results: vec![
                SearchResultDto {
                    author_did: "did:plc:priya-test".to_string(),
                    cid: "bafyk2".to_string(),
                    subject: "github:bazelbuild/bazel".to_string(),
                    predicate: "embodiesPhilosophy".to_string(),
                    object: "org.openlore.philosophy.reproducible-builds".to_string(),
                    confidence: 0.82,
                    composed_at: "2026-05-28T00:00:00Z".to_string(),
                    verified_against: "did:plc:priya-test#org.openlore.application".to_string(),
                    evidence: vec!["https://example.org/e1".to_string()],
                    // A references-less row stays backward-compatible (field omitted).
                    references: vec![],
                },
                SearchResultDto {
                    author_did: "did:plc:rachel-test".to_string(),
                    cid: "bafyk3".to_string(),
                    subject: "github:bazelbuild/bazel".to_string(),
                    predicate: "embodiesPhilosophy".to_string(),
                    object: "org.openlore.philosophy.reproducible-builds".to_string(),
                    confidence: 0.7,
                    composed_at: "2026-05-28T00:00:00Z".to_string(),
                    verified_against: "did:plc:rachel-test#org.openlore.application".to_string(),
                    evidence: vec![],
                    // A typed counter reference must survive the wire round-trip (OD-AV-7).
                    references: vec![ClaimReferenceDto {
                        ref_type: "counters".to_string(),
                        cid: "bafyk2".to_string(),
                    }],
                },
            ],
            distinct_author_count: 2,
            total_claims: 2,
            suggestion: None,
        };

        let json = serde_json::to_string(&response).expect("serialize");
        let back: SearchQueryResponse = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            back, response,
            "the response DTO must round-trip byte-for-byte"
        );
        // The load-bearing anti-merging assertion: every result still carries a
        // non-empty author_did after the round-trip.
        for row in &back.results {
            assert!(
                !row.author_did.is_empty(),
                "every result row MUST carry a non-empty author_did (I-AV-2)"
            );
        }
        assert_eq!(
            back.results.len(),
            2,
            "two distinct-author rows stay TWO rows"
        );
    }

    /// The dimension keyword serializes to the XRPC query-param contract
    /// (lowercase) so the CLI client + indexer server agree on the wire token.
    #[test]
    fn dimension_serializes_to_lowercase_query_keyword() {
        assert_eq!(
            serde_json::to_string(&SearchDimensionDto::Object).expect("ser"),
            "\"object\""
        );
        assert_eq!(
            serde_json::to_string(&SearchDimensionDto::Contributor).expect("ser"),
            "\"contributor\""
        );
        assert_eq!(
            serde_json::to_string(&SearchDimensionDto::Subject).expect("ser"),
            "\"subject\""
        );
    }
}
