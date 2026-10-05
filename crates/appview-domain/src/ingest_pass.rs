//! `ingest_pass` — the pure decisions of one per-DID ingest pass (ADR-077).
//!
//! Every repo DID is resolved to its own PDS every pass; the shell asks this
//! module where to list it ([`plan_listing`]), which origin its records carry
//! ([`origin_of`]), which listed records belong to it ([`records_of`]), why a
//! failed listing skips it ([`classify_fetch_failure`]) and how the pass is
//! summarised ([`summarize`]). Total functions over ADTs; no I/O.
//!
//! Earned Trust, subtype layer: [`ListingSource::Fallback`] carries no resolved
//! endpoint, and [`origin_of`] takes no URL into account on that arm — the
//! fallback can never be classified as the author's PDS.

use claim_domain::{decode_claim_record, ClaimRecord, Did, ProvenanceRejection, RecordOrigin};
use ports::net_policy::{url_admissible, TransportPolicy};
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

/// Why a DID contributed nothing this pass (ADR-078).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    DidUnresolvable,
    PdsUnreachable,
    PdsTimeout,
    ListingFailed,
    PdsAddressRefused,
}

impl SkipReason {
    /// The event token (`indexer.ingest.source_skipped` / `source_fallback`).
    pub const fn token(self) -> &'static str {
        match self {
            Self::DidUnresolvable => "did_unresolvable",
            Self::PdsUnreachable => "pds_unreachable",
            Self::PdsTimeout => "pds_timeout",
            Self::ListingFailed => "listing_failed",
            Self::PdsAddressRefused => "pds_address_refused",
        }
    }
}

/// Why a planned listing failed, as the shell reports it (mapped from the
/// listing port's error and the per-DID deadline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchFailure {
    /// Transport error, HTTP 5xx, HTTP 429.
    Unreachable,
    /// Any other non-2xx (redirects included — never followed), non-JSON, malformed.
    BadResponse,
    /// No admissible address was left for the source's host.
    AddressRefused,
    TimedOut,
}

/// A skip with its reason; when the fallback was the source that failed, the
/// DID stays `DidUnresolvable` and the fallback's own failure rides along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassifiedSkip {
    pub reason: SkipReason,
    pub fallback_failure: Option<SkipReason>,
}

impl ClassifiedSkip {
    /// A skip decided before any listing (the fallback was never tried).
    pub const fn planned(reason: SkipReason) -> Self {
        Self {
            reason,
            fallback_failure: None,
        }
    }

    /// Whether the fallback was tried (and failed) for this DID.
    pub const fn fallback_used(&self) -> bool {
        self.fallback_failure.is_some()
    }
}

/// The reason a failure of the DID's own PDS is reported under.
const fn own_pds_reason(failure: FetchFailure) -> SkipReason {
    match failure {
        FetchFailure::Unreachable => SkipReason::PdsUnreachable,
        FetchFailure::TimedOut => SkipReason::PdsTimeout,
        FetchFailure::BadResponse => SkipReason::ListingFailed,
        FetchFailure::AddressRefused => SkipReason::PdsAddressRefused,
    }
}

/// Every listing failure skips exactly that DID with one documented reason. A
/// failing fallback keeps the DID `DidUnresolvable` (the reason it was on the
/// fallback) and records what went wrong with the fallback.
pub fn classify_fetch_failure(source: &ListingSource, failure: FetchFailure) -> ClassifiedSkip {
    match source {
        ListingSource::OwnPds(_) => ClassifiedSkip::planned(own_pds_reason(failure)),
        ListingSource::Fallback(_) => ClassifiedSkip {
            reason: SkipReason::DidUnresolvable,
            fallback_failure: Some(own_pds_reason(failure)),
        },
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

/// The pre-check on a resolved PDS address (DD-IPF-5): the shared
/// [`url_admissible`] rule — `https`, or `http` only to a loopback IP literal
/// under [`TransportPolicy::HttpsOrLoopbackHttp`]; no userinfo; an IP-literal
/// host outside the refused ranges. Hostnames are checked after DNS by the
/// guarded adapters, not here.
pub fn pds_endpoint_admissible(
    endpoint: &str,
    policy: TransportPolicy,
) -> Result<PdsEndpoint, AddressRefused> {
    url::Url::parse(endpoint)
        .ok()
        .filter(|url| url_admissible(url, policy))
        .map(|_| PdsEndpoint::new(endpoint))
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
        skip: ClassifiedSkip,
        /// The source that failed (own PDS, or the fallback when it was
        /// used), when one was known.
        pds_url: Option<String>,
    },
}

/// How one configured DID ended the pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassOutcome {
    ReadFromOwnPds,
    ReadFromFallback,
    Skipped,
}

impl DidFetch {
    /// The DID's outcome, as the pass summary counts it.
    pub const fn outcome(&self) -> PassOutcome {
        match self {
            Self::Read {
                source: ListingSource::OwnPds(_),
                ..
            } => PassOutcome::ReadFromOwnPds,
            Self::Read {
                source: ListingSource::Fallback(_),
                ..
            } => PassOutcome::ReadFromFallback,
            Self::Skipped { .. } => PassOutcome::Skipped,
        }
    }
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
    fn count(self, outcome: PassOutcome) -> Self {
        let configured = self.configured + 1;
        match outcome {
            PassOutcome::ReadFromOwnPds => Self {
                configured,
                own_pds: self.own_pds + 1,
                ..self
            },
            PassOutcome::ReadFromFallback => Self {
                configured,
                fallback: self.fallback + 1,
                ..self
            },
            PassOutcome::Skipped => Self {
                configured,
                skipped: self.skipped + 1,
                ..self
            },
        }
    }
}

/// Fold the pass's per-DID outcomes into its summary.
pub fn summarize_outcomes(outcomes: impl IntoIterator<Item = PassOutcome>) -> PassSummary {
    outcomes
        .into_iter()
        .fold(PassSummary::default(), PassSummary::count)
}

/// Fold the pass's per-DID fetches into its summary.
pub fn summarize(fetches: &[DidFetch]) -> PassSummary {
    summarize_outcomes(fetches.iter().map(DidFetch::outcome))
}

/// The pass's exit code (DD-IPF-6): `3` (total outage) exactly when at least
/// one DID was configured and none was listed; `0` otherwise. A store failure
/// (exit 2) never reaches this decision.
pub const fn pass_exit_code(summary: &PassSummary) -> i32 {
    if summary.configured >= 1 && summary.own_pds + summary.fallback == 0 {
        3
    } else {
        0
    }
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
                skip: ClassifiedSkip::planned(SkipReason::DidUnresolvable),
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
