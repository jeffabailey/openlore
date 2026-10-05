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

use claim_domain::{decode_claim_record, ClaimRecord, Did, RecordOrigin};
use ports::{RepoListing, RepoRecord};

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
}

/// The decision for one DID before any listing happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListingPlan {
    List(ListingSource),
    Skip(SkipReason),
}

/// A resolved DID is listed on its own PDS; an unresolved one goes to the
/// fallback when there is one, else it is skipped.
pub fn plan_listing(
    resolution: Result<String, ResolutionFailure>,
    fallback: Option<&FallbackUrl>,
) -> ListingPlan {
    match (resolution, fallback) {
        (Ok(endpoint), _) => ListingPlan::List(ListingSource::OwnPds(PdsEndpoint::new(&endpoint))),
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

/// The records a listing holds for `repo_did`, each with its rkey and decoded
/// through the one shared decoder. A listing answers for one repo; anything
/// it returns from another repo is not this repo's record.
pub fn records_of(
    repo_did: &Did,
    listed: &[RepoRecord],
) -> Vec<(String, Result<ClaimRecord, String>)> {
    listed
        .iter()
        .filter(|record| record.repo_did == repo_did.0)
        .map(|record| (record.rkey.clone(), decode_claim_record(&record.value, "")))
        .collect()
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
            },
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
            prop_assert_eq!(
                plan_listing(Ok(format!("{resolved}/")), fallback.as_ref()),
                ListingPlan::List(ListingSource::OwnPds(PdsEndpoint::new(&resolved)))
            );
            let expected = match &fallback {
                Some(f) => ListingPlan::List(ListingSource::Fallback(f.clone())),
                None => ListingPlan::Skip(SkipReason::DidUnresolvable),
            };
            prop_assert_eq!(plan_listing(Err(failure), fallback.as_ref()), expected);
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

        /// Only records of the requested repo are kept, in listing order.
        #[test]
        fn only_the_requested_repos_records_are_kept(
            repos in proptest::collection::vec(prop_oneof![Just("did:plc:a"), Just("did:plc:b")], 0..10)
        ) {
            let listed: Vec<RepoRecord> = repos.iter().enumerate().map(|(i, repo)| RepoRecord {
                repo_did: repo.to_string(),
                rkey: format!("r{i}"),
                value: serde_json::json!({}),
            }).collect();
            let kept: Vec<String> = records_of(&Did("did:plc:a".to_string()), &listed)
                .into_iter().map(|(rkey, _)| rkey).collect();
            let expected: Vec<String> = listed.iter()
                .filter(|r| r.repo_did == "did:plc:a").map(|r| r.rkey.clone()).collect();
            prop_assert_eq!(kept, expected);
        }
    }
}
