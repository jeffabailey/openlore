//! `ingest_pass` — the pure decisions of one per-DID ingest pass (ADR-077).
//!
//! Every repo DID is resolved to its own PDS every pass; the shell asks this
//! module where to list it ([`plan_listing`]), which origin its records carry
//! ([`origin_of`]), which listed records belong to it ([`records_of`]) and how
//! the pass is summarised ([`summarize`]). Total functions over ADTs; no I/O.
//!
//! Earned Trust, subtype layer: [`ListingSource::Fallback`] carries no resolved
//! endpoint, and [`origin_of`] takes no URL into account on that arm — the
//! fallback can never be classified as the author's PDS.

use claim_domain::{decode_claim_record, ClaimRecord, Did, ProvenanceRejection, RecordOrigin};
use ports::net_policy::{address_refused, is_loopback, TransportPolicy};
use ports::{RepoListing, RepoRecord};

use crate::RejectReason;

/// Base URL without its trailing `/` (the form origin comparison uses).
fn base_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

/// The `#atproto_pds` endpoint freshly resolved from a DID's document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdsEndpoint(String);

impl PdsEndpoint {
    pub fn new(url: &str) -> Self {
        Self(base_url(url))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The operator-configured fallback listing source (`OPENLORE_INDEXER_SOURCE_URL`).
/// Whatever it serves is relay origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackUrl(String);

impl FallbackUrl {
    /// `None` for a blank setting (no fallback configured).
    pub fn new(url: &str) -> Option<Self> {
        let trimmed = base_url(url.trim());
        (!trimmed.is_empty()).then_some(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a DID did not yield a PDS this pass (fallback-eligible).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionFailure {
    NotFound,
    Unavailable,
    TimedOut,
}

/// Where a DID's records are listed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListingSource {
    /// The freshly resolved PDS: the only arm that can yield author-PDS origin.
    OwnPds(PdsEndpoint),
    /// The fallback: relay origin by construction.
    Fallback(FallbackUrl),
}

impl ListingSource {
    /// The base URL the listing is requested from.
    pub fn base(&self) -> &str {
        match self {
            Self::OwnPds(endpoint) => endpoint.as_str(),
            Self::Fallback(fallback) => fallback.as_str(),
        }
    }
}

/// Why a DID contributed nothing this pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    DidUnresolvable,
    PdsAddressRefused,
}

impl SkipReason {
    /// The event token (`indexer.ingest.source_skipped` / `source_fallback`).
    pub const fn token(self) -> &'static str {
        match self {
            Self::DidUnresolvable => "did_unresolvable",
            Self::PdsAddressRefused => "pds_address_refused",
        }
    }
}

/// The decision for one DID before any listing happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListingPlan {
    List(ListingSource),
    Skip(SkipReason),
}

/// A resolved PDS address the transport policy does not admit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressRefused;

/// The pre-check on a resolved PDS address (DD-IPF-5): `https`, or `http`
/// only to a loopback address under [`TransportPolicy::HttpsOrLoopbackHttp`];
/// no userinfo; an IP-literal host outside the refused ranges (loopback is
/// admitted under the test policy). Hostnames are checked after DNS by the
/// adapters, not here.
pub fn pds_endpoint_admissible(
    endpoint: &str,
    policy: TransportPolicy,
) -> Result<PdsEndpoint, AddressRefused> {
    let url = url::Url::parse(endpoint).map_err(|_| AddressRefused)?;
    let ip = match url.host() {
        Some(url::Host::Ipv4(v4)) => Some(std::net::IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => Some(std::net::IpAddr::V6(v6)),
        Some(url::Host::Domain(_)) => None,
        None => return Err(AddressRefused),
    };
    let test_loopback =
        policy == TransportPolicy::HttpsOrLoopbackHttp && ip.is_some_and(is_loopback);
    let scheme_admitted = url.scheme() == "https" || (url.scheme() == "http" && test_loopback);
    let host_admitted = test_loopback || !ip.is_some_and(address_refused);
    let no_userinfo = url.username().is_empty() && url.password().is_none();
    (scheme_admitted && host_admitted && no_userinfo)
        .then(|| PdsEndpoint::new(endpoint))
        .ok_or(AddressRefused)
}

/// An admissible resolved PDS is listed (never the fallback); a refused one is
/// skipped (never the fallback); an unresolved DID goes to the fallback when
/// there is one, else it is skipped.
pub fn plan_listing(
    resolution: Result<String, ResolutionFailure>,
    policy: TransportPolicy,
    fallback: Option<&FallbackUrl>,
) -> ListingPlan {
    match (resolution, fallback) {
        (Ok(endpoint), _) => match pds_endpoint_admissible(&endpoint, policy) {
            Ok(endpoint) => ListingPlan::List(ListingSource::OwnPds(endpoint)),
            Err(AddressRefused) => ListingPlan::Skip(SkipReason::PdsAddressRefused),
        },
        (Err(_), Some(fallback)) => ListingPlan::List(ListingSource::Fallback(fallback.clone())),
        (Err(_), None) => ListingPlan::Skip(SkipReason::DidUnresolvable),
    }
}

/// The origin of records listed from `source`: author-PDS only when they were
/// fetched from the resolved PDS itself; anything read through the fallback is
/// relay, whatever URL it came from.
pub fn origin_of(source: &ListingSource, fetched_from: &str) -> RecordOrigin {
    match source {
        ListingSource::OwnPds(endpoint) => RecordOrigin::of(fetched_from, endpoint.as_str()),
        ListingSource::Fallback(_) => RecordOrigin::Relay,
    }
}

/// A listing bound to its requested repo: the repo's own records (rkey +
/// decoded record) and how many records of other repos it also returned.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundRecords {
    pub own: Vec<(String, Result<ClaimRecord, String>)>,
    pub foreign: u64,
}

/// The records a listing holds for `repo_did`, each with its rkey and decoded
/// through the one shared decoder. A listing answers for one repo; anything
/// it returns from another repo is never this repo's record and is counted
/// as foreign (FR-4).
pub fn records_of(repo_did: &Did, listed: &[RepoRecord]) -> BoundRecords {
    let (own, foreign): (Vec<&RepoRecord>, Vec<&RepoRecord>) = listed
        .iter()
        .partition(|record| record.repo_did == repo_did.0);
    BoundRecords {
        own: own
            .into_iter()
            .map(|record| (record.rkey.clone(), decode_claim_record(&record.value, "")))
            .collect(),
        foreign: foreign.len() as u64,
    }
}

/// Why the pass refused a record, as reported in
/// `indexer.ingest.rejected.by_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefusalCause {
    Unsigned,
    BadSignature,
    CidMismatch,
    SchemaUnknown,
    Provenance,
    ForeignRepo,
}

impl RefusalCause {
    /// Every cause, in reporting order.
    pub const ALL: [Self; 6] = [
        Self::Unsigned,
        Self::BadSignature,
        Self::CidMismatch,
        Self::SchemaUnknown,
        Self::Provenance,
        Self::ForeignRepo,
    ];

    /// The `by_reason` key.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Unsigned => "unsigned",
            Self::BadSignature => "bad_signature",
            Self::CidMismatch => "cid_mismatch",
            Self::SchemaUnknown => "schema_unknown",
            Self::Provenance => "provenance",
            Self::ForeignRepo => "foreign_repo",
        }
    }
}

/// The reported cause of a gate refusal. A self-attested record whose content
/// does not hash to its rkey is a CID mismatch, wherever it was read from.
pub fn refusal_cause_of(reason: &RejectReason) -> RefusalCause {
    match reason {
        RejectReason::Unsigned => RefusalCause::Unsigned,
        RejectReason::BadSignature => RefusalCause::BadSignature,
        RejectReason::CidMismatch
        | RejectReason::Provenance(ProvenanceRejection::IntegrityFailure) => {
            RefusalCause::CidMismatch
        }
        RejectReason::SchemaUnknown => RefusalCause::SchemaUnknown,
        RejectReason::Provenance(_) => RefusalCause::Provenance,
    }
}

/// What one DID's fetch phase produced.
#[derive(Debug, Clone, PartialEq)]
pub enum DidFetch {
    Read {
        did: Did,
        source: ListingSource,
        listing: RepoListing,
    },
    Skipped {
        did: Did,
        reason: SkipReason,
        /// The resolved PDS, when one was known (never for `DidUnresolvable`).
        pds_url: Option<String>,
    },
}

/// The per-pass accounting: `own_pds + fallback + skipped == configured`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PassSummary {
    pub configured: u64,
    pub own_pds: u64,
    pub fallback: u64,
    pub skipped: u64,
}

impl PassSummary {
    fn count(self, fetch: &DidFetch) -> Self {
        let configured = self.configured + 1;
        match fetch {
            DidFetch::Read {
                source: ListingSource::OwnPds(_),
                ..
            } => Self {
                configured,
                own_pds: self.own_pds + 1,
                ..self
            },
            DidFetch::Read {
                source: ListingSource::Fallback(_),
                ..
            } => Self {
                configured,
                fallback: self.fallback + 1,
                ..self
            },
            DidFetch::Skipped { .. } => Self {
                configured,
                skipped: self.skipped + 1,
                ..self
            },
        }
    }
}

/// Fold the pass's per-DID outcomes into its summary.
pub fn summarize(fetches: &[DidFetch]) -> PassSummary {
    fetches
        .iter()
        .fold(PassSummary::default(), PassSummary::count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_url() -> impl Strategy<Value = String> {
        "[a-z]{1,10}\\.[a-z]{2,6}".prop_map(|host| format!("https://{host}"))
    }

    fn arb_failure() -> impl Strategy<Value = ResolutionFailure> {
        prop_oneof![
            Just(ResolutionFailure::NotFound),
            Just(ResolutionFailure::Unavailable),
            Just(ResolutionFailure::TimedOut),
        ]
    }

    #[derive(Debug, Clone, Copy)]
    enum Kind {
        Own,
        Fallback,
        Skip,
    }

    fn fetch_of(kind: Kind) -> DidFetch {
        let did = Did("did:plc:someone".to_string());
        let listing = RepoListing {
            fetched_from: "https://pds.example".to_string(),
            records: Vec::new(),
        };
        match kind {
            Kind::Own => DidFetch::Read {
                did,
                source: ListingSource::OwnPds(PdsEndpoint::new("https://pds.example")),
                listing,
            },
            Kind::Fallback => DidFetch::Read {
                did,
                source: ListingSource::Fallback(FallbackUrl::new("https://relay.example").unwrap()),
                listing,
            },
            Kind::Skip => DidFetch::Skipped {
                did,
                reason: SkipReason::DidUnresolvable,
                pds_url: None,
            },
        }
    }

    /// bypass: closed-world table — every gate refusal maps to exactly one
    /// reported cause; a self-attested integrity failure is a CID mismatch.
    #[test]
    fn every_gate_refusal_maps_to_its_reported_cause() {
        let table = [
            (RejectReason::Unsigned, RefusalCause::Unsigned),
            (RejectReason::BadSignature, RefusalCause::BadSignature),
            (RejectReason::CidMismatch, RefusalCause::CidMismatch),
            (RejectReason::SchemaUnknown, RefusalCause::SchemaUnknown),
            (
                RejectReason::Provenance(ProvenanceRejection::IntegrityFailure),
                RefusalCause::CidMismatch,
            ),
            (
                RejectReason::Provenance(ProvenanceRejection::UnverifiableProvenance),
                RefusalCause::Provenance,
            ),
            (
                RejectReason::Provenance(ProvenanceRejection::ForeignRepo),
                RefusalCause::Provenance,
            ),
            (
                RejectReason::Provenance(ProvenanceRejection::MalformedProvenance),
                RefusalCause::Provenance,
            ),
        ];
        for (reason, cause) in table {
            assert_eq!(refusal_cause_of(&reason), cause, "{reason:?}");
        }
    }

    proptest! {
        /// A resolved DID is always listed on its own PDS, never the fallback;
        /// an unresolved one goes to the fallback iff one is configured.
        #[test]
        fn a_resolved_did_is_listed_on_its_own_pds(
            resolved in arb_url(), failure in arb_failure(),
            fallback in proptest::option::of(arb_url())
        ) {
            let fallback = fallback.and_then(|f| FallbackUrl::new(&f));
            let policy = TransportPolicy::HttpsPublicOnly;
            prop_assert_eq!(
                plan_listing(Ok(format!("{resolved}/")), policy, fallback.as_ref()),
                ListingPlan::List(ListingSource::OwnPds(PdsEndpoint::new(&resolved)))
            );
            let expected = match &fallback {
                Some(f) => ListingPlan::List(ListingSource::Fallback(f.clone())),
                None => ListingPlan::Skip(SkipReason::DidUnresolvable),
            };
            prop_assert_eq!(plan_listing(Err(failure), policy, fallback.as_ref()), expected);
        }

        /// Universe {configured, own_pds, fallback, skipped}: each DID moves
        /// `configured` and exactly one of the other three by one.
        #[test]
        fn the_summary_accounts_for_every_configured_did(
            kinds in proptest::collection::vec(
                prop_oneof![Just(Kind::Own), Just(Kind::Fallback), Just(Kind::Skip)], 0..20)
        ) {
            let fetches: Vec<DidFetch> = kinds.iter().copied().map(fetch_of).collect();
            let summary = summarize(&fetches);
            let count = |k: fn(&Kind) -> bool| kinds.iter().filter(|x| k(x)).count() as u64;
            prop_assert_eq!(summary, PassSummary {
                configured: kinds.len() as u64,
                own_pds: count(|k| matches!(k, Kind::Own)),
                fallback: count(|k| matches!(k, Kind::Fallback)),
                skipped: count(|k| matches!(k, Kind::Skip)),
            });
        }

        /// Universe {own, foreign}: every record of the requested repo is
        /// kept in listing order, every other one is counted, none is lost.
        #[test]
        fn only_the_requested_repos_records_are_kept(
            repos in proptest::collection::vec(prop_oneof![Just("did:plc:a"), Just("did:plc:b")], 0..10)
        ) {
            let listed: Vec<RepoRecord> = repos.iter().enumerate().map(|(i, repo)| RepoRecord {
                repo_did: repo.to_string(),
                rkey: format!("r{i}"),
                value: serde_json::json!({}),
            }).collect();
            let bound = records_of(&Did("did:plc:a".to_string()), &listed);
            let kept: Vec<String> = bound.own.into_iter().map(|(rkey, _)| rkey).collect();
            let expected: Vec<String> = listed.iter()
                .filter(|r| r.repo_did == "did:plc:a").map(|r| r.rkey.clone()).collect();
            prop_assert_eq!(bound.foreign as usize, listed.len() - expected.len());
            prop_assert_eq!(kept, expected);
        }
    }
}
