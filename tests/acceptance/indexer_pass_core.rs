//! indexer-per-did-pds-fetch — layer-2 PURE-CORE properties (ADR-007 functional
//! core; Mandate 9: PBT full at layers 1-2).
//!
//! Every decision of the per-DID pass is a total pure function (component-
//! boundaries.md, data-models.md §2-§4): the listing plan, the origin rule, the
//! failure classification, the summary + exit code, the address guard, the
//! endpoint pre-check, config parsing, repo binding and the wire provenance
//! decode. Their CONTRACTS are stated here over generated inputs and checked
//! against ORACLES written from the ADR / data-model text — never from the
//! implementation. They pair with the subprocess scenarios in
//! `indexer_per_did_{fetch,resilience,config}.rs` and
//! `search_self_attested_label.rs` (same AC tags).
//!
//! ## Binding seam (RED scaffold, Mandate 7)
//!
//! `appview_domain::ingest_pass`, `ports::net_policy`, `openlore-indexer`'s
//! `config::parse_config` and the provenance decode do not exist yet, so each
//! property calls a `sut_*` binding whose body is `todo!()` — a panic,
//! classified RED (not BROKEN). DELIVER replaces each binding body with ONE call
//! into the production function (the view types here are the observable
//! contract; DELIVER maps its ADTs onto them). If a binding needs a function
//! that is private to the `openlore-indexer` binary crate (`parse_config`),
//! DELIVER either exposes it through a lib target or moves this property into
//! that crate's unit tests verbatim.
//!
//! Closed-world finite tables (8 failure classes, 3 provenance tokens) are
//! exhaustive example tables, not PBT (falsifier gate).
//
// SCAFFOLD: true

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;

// =============================================================================
// Observable view types (the contract)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    AuthorPds,
    Relay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    OwnPds(String),
    Fallback(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolutionFailure {
    NotFound,
    Unavailable,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Policy {
    HttpsPublicOnly,
    HttpsOrLoopbackHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    DidUnresolvable,
    PdsUnreachable,
    PdsTimeout,
    ListingFailed,
    PdsAddressRefused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Plan {
    List(Source),
    Skip(Reason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchFailure {
    Unreachable,
    BadResponse,
    AddressRefused,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Classified {
    reason: Reason,
    fallback_used: bool,
    fallback_failure: Option<Reason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PassOutcome {
    ReadFromOwnPds,
    ReadFromFallback,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Summary {
    configured: u64,
    own_pds: u64,
    fallback: u64,
    skipped: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedConfig {
    repo_dids: Vec<String>,
    fallback: Option<String>,
    max_concurrent_fetches: u64,
    per_did_time_budget_secs: u64,
    policy: Policy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigRefusal {
    variable: String,
    value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WireProvenance {
    AppSigned,
    SelfAttested,
}

// =============================================================================
// RED binding seam — DELIVER binds each to ONE production call
// =============================================================================

fn sut_origin_of(source: &Source, fetched_from: &str) -> Origin {
    use appview_domain::ingest_pass::{origin_of, FallbackUrl, ListingSource, PdsEndpoint};
    let source = match source {
        Source::OwnPds(endpoint) => ListingSource::OwnPds(PdsEndpoint::new(endpoint)),
        Source::Fallback(url) => {
            ListingSource::Fallback(FallbackUrl::new(url).expect("a non-empty fallback URL"))
        }
    };
    match origin_of(&source, fetched_from) {
        claim_domain::RecordOrigin::AuthorPds => Origin::AuthorPds,
        claim_domain::RecordOrigin::Relay => Origin::Relay,
    }
}

fn sut_plan_listing(
    resolution: Result<&str, ResolutionFailure>,
    policy: Policy,
    fallback: Option<&str>,
) -> Plan {
    use appview_domain::ingest_pass::{plan_listing, FallbackUrl, ListingPlan, ListingSource};
    let resolution = resolution
        .map(str::to_string)
        .map_err(|failure| match failure {
            ResolutionFailure::NotFound => appview_domain::ingest_pass::ResolutionFailure::NotFound,
            ResolutionFailure::Unavailable => {
                appview_domain::ingest_pass::ResolutionFailure::Unavailable
            }
            ResolutionFailure::TimedOut => appview_domain::ingest_pass::ResolutionFailure::TimedOut,
        });
    let policy = match policy {
        Policy::HttpsPublicOnly => ports::net_policy::TransportPolicy::HttpsPublicOnly,
        Policy::HttpsOrLoopbackHttp => ports::net_policy::TransportPolicy::HttpsOrLoopbackHttp,
    };
    let fallback = fallback.and_then(FallbackUrl::new);
    match plan_listing(resolution, policy, fallback.as_ref()) {
        ListingPlan::List(ListingSource::OwnPds(e)) => {
            Plan::List(Source::OwnPds(e.as_str().into()))
        }
        ListingPlan::List(ListingSource::Fallback(f)) => {
            Plan::List(Source::Fallback(f.as_str().into()))
        }
        ListingPlan::Skip(reason) => Plan::Skip(view_reason(reason)),
    }
}

fn view_reason(reason: appview_domain::ingest_pass::SkipReason) -> Reason {
    use appview_domain::ingest_pass::SkipReason;
    match reason {
        SkipReason::DidUnresolvable => Reason::DidUnresolvable,
        SkipReason::PdsUnreachable => Reason::PdsUnreachable,
        SkipReason::PdsTimeout => Reason::PdsTimeout,
        SkipReason::ListingFailed => Reason::ListingFailed,
        SkipReason::PdsAddressRefused => Reason::PdsAddressRefused,
    }
}

fn sut_classify_fetch_failure(source: &Source, failure: FetchFailure) -> Classified {
    use appview_domain::ingest_pass::{
        classify_fetch_failure, FallbackUrl, FetchFailure as Failure, ListingSource, PdsEndpoint,
    };
    let source = match source {
        Source::OwnPds(endpoint) => ListingSource::OwnPds(PdsEndpoint::new(endpoint)),
        Source::Fallback(url) => {
            ListingSource::Fallback(FallbackUrl::new(url).expect("a non-empty fallback URL"))
        }
    };
    let failure = match failure {
        FetchFailure::Unreachable => Failure::Unreachable,
        FetchFailure::BadResponse => Failure::BadResponse,
        FetchFailure::AddressRefused => Failure::AddressRefused,
        FetchFailure::TimedOut => Failure::TimedOut,
    };
    let classified = classify_fetch_failure(&source, failure);
    Classified {
        reason: view_reason(classified.reason),
        fallback_used: classified.fallback_used(),
        fallback_failure: classified.fallback_failure.map(view_reason),
    }
}

fn sut_summarize(outcomes: &[PassOutcome]) -> Summary {
    let _ = outcomes;
    todo!("SCAFFOLD: bind appview_domain::ingest_pass::summarize (ADR-078)")
}

fn sut_pass_exit_code(summary: Summary) -> i32 {
    let _ = summary;
    todo!("SCAFFOLD: bind appview_domain::ingest_pass::pass_exit_code (DD-IPF-6)")
}

fn sut_address_refused(ip: IpAddr) -> bool {
    let _ = ip;
    todo!("SCAFFOLD: bind ports::net_policy::address_refused (DD-IPF-5)")
}

fn sut_endpoint_admissible(url: &str, policy: Policy) -> Result<String, ()> {
    let _ = (url, policy);
    todo!("SCAFFOLD: bind appview_domain::ingest_pass::pds_endpoint_admissible (DD-IPF-5)")
}

fn sut_parse_config(
    env: &BTreeMap<String, String>,
    release_build: bool,
) -> Result<LoadedConfig, ConfigRefusal> {
    let _ = (env, release_build);
    todo!("SCAFFOLD: bind openlore-indexer config::parse_config (data-models §4)")
}

fn sut_records_of(did: &str, listed: &[(String, String)]) -> (Vec<String>, u64) {
    let listed: Vec<ports::RepoRecord> = listed
        .iter()
        .map(|(repo_did, rkey)| ports::RepoRecord {
            repo_did: repo_did.clone(),
            rkey: rkey.clone(),
            value: serde_json::json!({}),
        })
        .collect();
    let bound =
        appview_domain::ingest_pass::records_of(&claim_domain::Did(did.to_string()), &listed);
    (
        bound.own.into_iter().map(|(rkey, _)| rkey).collect(),
        bound.foreign,
    )
}

fn sut_decode_wire_provenance(token: Option<&str>) -> Option<WireProvenance> {
    let _ = token;
    todo!("SCAFFOLD: bind adapter-index-query provenance decode (ADR-079)")
}

/// `SearchResultDto` JSON → its `provenance` field after a serde round trip
/// (deserialize, then serialize again): `(field after decode, key present after encode)`.
fn sut_search_dto_provenance_round_trip(json: &serde_json::Value) -> (Option<String>, bool) {
    let _ = json;
    todo!("SCAFFOLD: bind lexicon::SearchResultDto serde (ADR-079)")
}

// =============================================================================
// Oracles (from the ADR / data-model text)
// =============================================================================

/// data-models.md §3: the refused ranges, IPv4-mapped IPv6 unwrapped first.
fn oracle_refused(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => oracle_refused_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => oracle_refused_v4(v4),
            None => {
                let s = v6.segments();
                v6 == Ipv6Addr::UNSPECIFIED
                    || v6 == Ipv6Addr::LOCALHOST
                    || (s[0] & 0xfe00) == 0xfc00 // fc00::/7
                    || (s[0] & 0xffc0) == 0xfe80 // fe80::/10
            }
        },
    }
}

fn oracle_refused_v4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 0
        || a == 127
        || a == 10
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 169 && b == 254)
}

fn oracle_exit_code(s: Summary) -> i32 {
    if s.configured >= 1 && s.own_pds + s.fallback == 0 {
        3
    } else {
        0
    }
}

// =============================================================================
// Strategies
// =============================================================================

fn arb_host() -> impl Strategy<Value = String> {
    "[a-z]{1,10}(\\.[a-z]{2,8}){1,2}"
}

fn arb_public_https_url() -> impl Strategy<Value = String> {
    arb_host().prop_map(|h| format!("https://{h}"))
}

fn arb_refused_v4() -> impl Strategy<Value = Ipv4Addr> {
    prop_oneof![
        any::<[u8; 3]>().prop_map(|[b, c, d]| Ipv4Addr::new(10, b, c, d)),
        any::<[u8; 3]>().prop_map(|[b, c, d]| Ipv4Addr::new(127, b, c, d)),
        (16u8..=31, any::<[u8; 2]>()).prop_map(|(b, [c, d])| Ipv4Addr::new(172, b, c, d)),
        any::<[u8; 2]>().prop_map(|[c, d]| Ipv4Addr::new(192, 168, c, d)),
        any::<[u8; 2]>().prop_map(|[c, d]| Ipv4Addr::new(169, 254, c, d)),
    ]
}

fn arb_refused_endpoint() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_host().prop_map(|h| format!("http://{h}")),
        arb_refused_v4().prop_map(|ip| format!("https://{ip}")),
        arb_refused_v4().prop_map(|ip| format!("https://[{}]", ip.to_ipv6_mapped())),
        Just("https://[fe80::1]".to_string()),
        Just("https://jeff:secret@pds.example.com".to_string()),
    ]
}

fn arb_resolution_failure() -> impl Strategy<Value = ResolutionFailure> {
    prop_oneof![
        Just(ResolutionFailure::NotFound),
        Just(ResolutionFailure::Unavailable),
        Just(ResolutionFailure::TimedOut),
    ]
}

fn arb_policy() -> impl Strategy<Value = Policy> {
    prop_oneof![
        Just(Policy::HttpsPublicOnly),
        Just(Policy::HttpsOrLoopbackHttp)
    ]
}

fn arb_outcome() -> impl Strategy<Value = PassOutcome> {
    prop_oneof![
        Just(PassOutcome::ReadFromOwnPds),
        Just(PassOutcome::ReadFromFallback),
        Just(PassOutcome::Skipped),
    ]
}

fn arb_did() -> impl Strategy<Value = String> {
    ("(plc|web)", "[a-z0-9]{1,24}").prop_map(|(m, id)| format!("did:{m}:{id}"))
}

const KNOWN_VARIABLES: [&str; 5] = [
    "OPENLORE_INDEXER_REPO_DIDS",
    "OPENLORE_INDEXER_SOURCE_URL",
    "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP",
    "OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES",
    "OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS",
];

fn arb_env() -> impl Strategy<Value = BTreeMap<String, String>> {
    proptest::collection::btree_map(
        proptest::sample::select(KNOWN_VARIABLES.to_vec()).prop_map(str::to_string),
        prop_oneof![
            Just(String::new()),
            "[ -~]{0,40}",
            "[0-9]{1,4}",
            arb_did(),
            arb_public_https_url(),
        ],
        0..5,
    )
}

// =============================================================================
// Properties
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// CORE-1 @property @US-IPF-003 @AC-003.1 @I-IPF-2 @ADR-077 @contract-shape:pure-function
    /// Anything listed through the fallback is relay origin — whatever URL it
    /// came from, even one equal to somebody's resolved PDS.
    #[test]
    fn anything_read_through_the_fallback_is_relay_origin(
        fallback in arb_public_https_url(), fetched_from in arb_public_https_url()
    ) {
        prop_assert_eq!(sut_origin_of(&Source::Fallback(fallback.clone()), &fetched_from), Origin::Relay);
        prop_assert_eq!(sut_origin_of(&Source::Fallback(fallback.clone()), &fallback), Origin::Relay);
    }

    /// CORE-2 @property @US-IPF-001 @AC-001.2 @I-IPF-1 @ADR-077 @contract-shape:pure-function
    /// A record read from the freshly resolved PDS is author-PDS origin exactly
    /// when it was fetched from that PDS.
    #[test]
    fn own_pds_origin_holds_exactly_when_fetched_from_the_resolved_pds(
        endpoint in arb_public_https_url(), other in arb_public_https_url()
    ) {
        prop_assume!(endpoint != other);
        prop_assert_eq!(sut_origin_of(&Source::OwnPds(endpoint.clone()), &endpoint), Origin::AuthorPds);
        prop_assert_eq!(sut_origin_of(&Source::OwnPds(endpoint.clone()), &other), Origin::Relay);
    }

    /// CORE-3 @property @US-IPF-003 @AC-003.2 @AC-002.9 @AC-002.2 @ADR-077 @C5a @contract-shape:pure-function
    /// The listing plan: an admissible resolved PDS is listed (never the
    /// fallback); a refused one is skipped (never the fallback); an unresolved
    /// DID goes to the fallback when there is one, else is skipped.
    #[test]
    fn the_listing_plan_never_sends_a_resolved_did_to_the_fallback(
        resolved in arb_public_https_url(),
        refused in arb_refused_endpoint(),
        failure in arb_resolution_failure(),
        fallback in proptest::option::of(arb_public_https_url()),
        policy in arb_policy(),
    ) {
        prop_assert_eq!(
            sut_plan_listing(Ok(&resolved), policy, fallback.as_deref()),
            Plan::List(Source::OwnPds(resolved.clone()))
        );
        prop_assert_eq!(
            sut_plan_listing(Ok(&refused), Policy::HttpsPublicOnly, fallback.as_deref()),
            Plan::Skip(Reason::PdsAddressRefused)
        );
        let expected = match &fallback {
            Some(f) => Plan::List(Source::Fallback(f.clone())),
            None => Plan::Skip(Reason::DidUnresolvable),
        };
        prop_assert_eq!(sut_plan_listing(Err(failure), policy, fallback.as_deref()), expected);
    }

    /// CORE-4 @property @US-IPF-002 @AC-002.7 @AC-002.8 @DD-IPF-6 @C3 @contract-shape:pure-function
    /// The pass summary accounts for every configured DID, and the exit code is
    /// 3 exactly when at least one DID was configured and none was listed.
    #[test]
    #[ignore = "DELIVER 02-01 (unit): summarize + pass_exit_code"]
    fn the_summary_accounts_for_every_did_and_exit_3_means_total_outage(
        outcomes in proptest::collection::vec(arb_outcome(), 0..60)
    ) {
        let s = sut_summarize(&outcomes);
        prop_assert_eq!(s.configured, outcomes.len() as u64);
        prop_assert_eq!(s.own_pds + s.fallback + s.skipped, s.configured);
        prop_assert_eq!(s.own_pds, outcomes.iter().filter(|o| **o == PassOutcome::ReadFromOwnPds).count() as u64);
        prop_assert_eq!(s.fallback, outcomes.iter().filter(|o| **o == PassOutcome::ReadFromFallback).count() as u64);
        prop_assert_eq!(sut_pass_exit_code(s), oracle_exit_code(s));
    }

    /// CORE-5 @property @US-IPF-002 @AC-002.9 @DD-IPF-5 @C1b @contract-shape:pure-function
    /// The address guard refuses exactly the documented ranges, for every IPv4
    /// address and every IPv6 address (IPv4-mapped addresses judged as IPv4).
    #[test]
    #[ignore = "DELIVER 02-02 (unit): ports::net_policy::address_refused"]
    fn the_address_guard_refuses_exactly_the_documented_ranges(v4 in any::<u32>(), v6 in any::<u128>()) {
        let v4 = Ipv4Addr::from(v4);
        let v6 = Ipv6Addr::from(v6);
        prop_assert_eq!(sut_address_refused(IpAddr::V4(v4)), oracle_refused(IpAddr::V4(v4)));
        prop_assert_eq!(sut_address_refused(IpAddr::V6(v6)), oracle_refused(IpAddr::V6(v6)));
        prop_assert_eq!(
            sut_address_refused(IpAddr::V6(v4.to_ipv6_mapped())),
            oracle_refused(IpAddr::V4(v4))
        );
    }

    /// CORE-6 @property @US-IPF-002 @AC-002.9 @AC-004.5 @DD-IPF-5 @C5a @contract-shape:pure-function
    /// The endpoint pre-check admits https to a public host (trailing `/`
    /// trimmed) under both policies, and plain http only to loopback under the
    /// test policy.
    #[test]
    #[ignore = "DELIVER 02-02 (unit): pds_endpoint_admissible"]
    fn the_endpoint_precheck_admits_https_public_and_loopback_http_only_under_the_test_policy(
        public in arb_public_https_url(), refused in arb_refused_endpoint(), port in 1u16..
    ) {
        for policy in [Policy::HttpsPublicOnly, Policy::HttpsOrLoopbackHttp] {
            prop_assert_eq!(sut_endpoint_admissible(&format!("{public}/"), policy), Ok(public.clone()));
            prop_assert_eq!(sut_endpoint_admissible(&public, policy), Ok(public.clone()));
        }
        prop_assert!(sut_endpoint_admissible(&refused, Policy::HttpsPublicOnly).is_err());
        let loopback = format!("http://127.0.0.1:{port}");
        prop_assert!(sut_endpoint_admissible(&loopback, Policy::HttpsPublicOnly).is_err());
        prop_assert_eq!(sut_endpoint_admissible(&loopback, Policy::HttpsOrLoopbackHttp), Ok(loopback.clone()));
        prop_assert!(sut_endpoint_admissible("http://10.0.0.1", Policy::HttpsOrLoopbackHttp).is_err());
    }

    /// CORE-7 @property @US-IPF-004 @AC-004.1 @AC-004.2 @AC-004.3 @C6a @C6c @contract-shape:pure-function
    /// Config parsing is total: for any environment it either loads or refuses
    /// naming one of the indexer's variables and a value taken from it.
    #[test]
    #[ignore = "DELIVER 02-03 (unit): parse_config is total, refusals name variable + value"]
    fn config_parsing_is_total_and_every_refusal_names_a_variable_and_its_value(env in arb_env()) {
        match sut_parse_config(&env, false) {
            Ok(cfg) => {
                prop_assert!((1..=16).contains(&cfg.max_concurrent_fetches));
                prop_assert!((1..=600).contains(&cfg.per_did_time_budget_secs));
                let distinct: BTreeSet<&String> = cfg.repo_dids.iter().collect();
                prop_assert_eq!(distinct.len(), cfg.repo_dids.len(), "duplicates collapsed");
            }
            Err(refusal) => {
                prop_assert!(KNOWN_VARIABLES.contains(&refusal.variable.as_str()), "{:?}", refusal);
                let raw = env.get(&refusal.variable).cloned().unwrap_or_default();
                prop_assert!(raw.contains(&refusal.value), "{:?} not in {:?}", refusal, raw);
            }
        }
    }

    /// CORE-8 @property @US-IPF-004 @AC-004.5 @R-IPF-9 @release-gate @contract-shape:pure-function
    /// A release build refuses the loopback test seam whatever else is set; a
    /// debug build turns it into the loopback-http test policy.
    #[test]
    #[ignore = "DELIVER 02-03 (unit): BuildProfile::Release + seam ⇒ refusal"]
    fn a_release_build_refuses_the_loopback_seam_whatever_else_is_set(mut env in arb_env()) {
        env.insert("OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP".to_string(), "1".to_string());
        let refusal = sut_parse_config(&env, true);
        prop_assert!(
            matches!(&refusal, Err(r) if r.variable == "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP"),
            "{:?}", refusal
        );
        env.retain(|k, _| k == "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP");
        prop_assert_eq!(
            sut_parse_config(&env, false).map(|c| c.policy),
            Ok(Policy::HttpsOrLoopbackHttp)
        );
    }

    /// CORE-9 @property @US-IPF-004 @AC-004.1 @AC-002.7 @C3 @C4a @contract-shape:pure-function
    /// Any list of valid DIDs (comma or whitespace separated, with repeats)
    /// loads as the distinct DIDs in first-seen order.
    #[test]
    #[ignore = "DELIVER 02-03 (unit): DID list parse + dedup, first wins"]
    fn a_list_of_valid_dids_loads_as_its_distinct_dids_in_order(
        dids in proptest::collection::vec(arb_did(), 0..12), comma in any::<bool>()
    ) {
        let mut repeated = dids.clone();
        repeated.extend(dids.iter().take(3).cloned());
        let sep = if comma { "," } else { " " };
        let env = BTreeMap::from([(
            "OPENLORE_INDEXER_REPO_DIDS".to_string(),
            repeated.join(sep),
        )]);
        let mut expected: Vec<String> = Vec::new();
        for d in &dids {
            if !expected.contains(d) {
                expected.push(d.clone());
            }
        }
        prop_assert_eq!(sut_parse_config(&env, false).map(|c| c.repo_dids), Ok(expected));
    }

    /// CORE-10 @property @US-IPF-001 @AC-001.5 @I-IPF-5 @C3 @contract-shape:pure-function
    /// Repo binding partitions a listing: records of the requested repo are
    /// kept, every other record is counted as foreign, none is lost.
    #[test]
    fn repo_binding_keeps_own_records_and_counts_every_foreign_one(
        did in arb_did(),
        listed in proptest::collection::vec((prop_oneof![Just(None), arb_did().prop_map(Some)], "[a-z0-9]{8}"), 0..20)
    ) {
        let listed: Vec<(String, String)> = listed
            .into_iter()
            .map(|(repo, rkey)| (repo.unwrap_or_else(|| did.clone()), rkey))
            .collect();
        let (kept, foreign) = sut_records_of(&did, &listed);
        let want_kept: Vec<String> = listed.iter().filter(|(r, _)| *r == did).map(|(_, k)| k.clone()).collect();
        prop_assert_eq!(&kept, &want_kept);
        prop_assert_eq!(foreign as usize, listed.len() - want_kept.len());
    }

    /// CORE-11 @property @US-IPF-005 @AC-005.2 @ADR-079 @C6a @contract-shape:pure-function
    /// Over the wire, a missing provenance reads as app-signed, the two known
    /// tokens read as themselves, and any other token is withheld (never guessed).
    #[test]
    #[ignore = "DELIVER 02-04 (unit): provenance decode in adapter-index-query"]
    fn an_unknown_wire_provenance_is_withheld_never_guessed(token in "[ -~]{0,24}") {
        let expected = match token.as_str() {
            "app-signed" => Some(WireProvenance::AppSigned),
            "self-attested" => Some(WireProvenance::SelfAttested),
            _ => None,
        };
        prop_assert_eq!(sut_decode_wire_provenance(Some(&token)), expected);
        prop_assert_eq!(sut_decode_wire_provenance(None), Some(WireProvenance::AppSigned));
    }
}

// =============================================================================
// Closed-world tables (exhaustive examples, not PBT)
// =============================================================================

/// CORE-12 @US-IPF-002 @US-IPF-003 @AC-002.1 @AC-002.5 @AC-003.3 @AC-002.9 @C6b @contract-shape:pure-function
/// Every fetch failure maps to exactly one documented reason; a failing
/// fallback keeps `did_unresolvable` and records the fallback failure.
#[test]
fn every_fetch_failure_maps_to_exactly_one_documented_reason() {
    let own = Source::OwnPds("https://pds.volkov.dev".to_string());
    let fallback = Source::Fallback("https://pds.jeffbailey.us".to_string());
    let table = [
        (FetchFailure::Unreachable, Reason::PdsUnreachable),
        (FetchFailure::TimedOut, Reason::PdsTimeout),
        (FetchFailure::BadResponse, Reason::ListingFailed),
        (FetchFailure::AddressRefused, Reason::PdsAddressRefused),
    ];
    for (failure, mapped) in table {
        assert_eq!(
            sut_classify_fetch_failure(&own, failure),
            Classified {
                reason: mapped,
                fallback_used: false,
                fallback_failure: None
            },
            "own PDS {failure:?}"
        );
        assert_eq!(
            sut_classify_fetch_failure(&fallback, failure),
            Classified {
                reason: Reason::DidUnresolvable,
                fallback_used: true,
                fallback_failure: Some(mapped)
            },
            "fallback {failure:?}"
        );
    }
}

/// CORE-13 @US-IPF-002 @AC-002.9 @DD-IPF-5 @C1b @boundary @contract-shape:pure-function
/// The range edges, pinned (172.15.255.255 is public, 172.16.0.0 is not, …).
#[test]
#[ignore = "DELIVER 02-02 (unit): address_refused at every range boundary"]
fn the_address_guard_holds_at_every_range_edge() {
    let cases: [(&str, bool); 20] = [
        ("0.0.0.0", true),
        ("0.255.255.255", true),
        ("1.0.0.0", false),
        ("9.255.255.255", false),
        ("10.0.0.0", true),
        ("10.255.255.255", true),
        ("11.0.0.0", false),
        ("126.255.255.255", false),
        ("127.0.0.1", true),
        ("169.253.255.255", false),
        ("169.254.169.254", true),
        ("172.15.255.255", false),
        ("172.16.0.0", true),
        ("172.31.255.255", true),
        ("172.32.0.0", false),
        ("192.167.255.255", false),
        ("192.168.0.0", true),
        ("::ffff:172.16.0.1", true),
        ("fbff:ffff::1", false),
        ("fec0::1", false),
    ];
    for (ip, refused) in cases {
        let addr: IpAddr = ip.parse().expect("valid literal");
        assert_eq!(sut_address_refused(addr), refused, "{ip}");
    }
    for ip in [
        "::",
        "::1",
        "fc00::1",
        "fdff:ffff::1",
        "fe80::1",
        "febf:ffff::1",
    ] {
        assert!(sut_address_refused(ip.parse().unwrap()), "{ip}");
    }
}

/// CORE-14 @US-IPF-005 @AC-005.2 @ADR-079 @regression @contract-shape:pure-function
/// An old server's search row (no provenance key) still decodes, as "no
/// provenance", and re-encodes without the key; a current row keeps it.
#[test]
#[ignore = "DELIVER 02-04 (unit): SearchResultDto.provenance is additive"]
fn an_old_search_row_without_provenance_still_decodes_and_reencodes_unchanged() {
    let old = serde_json::json!({
        "author_did": "did:plc:dvolkov3m9q#org.openlore.application",
        "cid": "bafyreiferritereproducible0001",
        "subject": "github:dvolkov/ferrite",
        "predicate": "embodiesPhilosophy",
        "object": "org.openlore.philosophy.reproducible-builds",
        "confidence": 0.7,
        "composed_at": "2026-10-04T15:02:11+00:00",
        "verified_against": "did:plc:dvolkov3m9q#org.openlore.application",
        "evidence": [],
    });
    assert_eq!(sut_search_dto_provenance_round_trip(&old), (None, false));
    let mut current = old.clone();
    current["provenance"] = serde_json::json!("self-attested");
    assert_eq!(
        sut_search_dto_provenance_round_trip(&current),
        (Some("self-attested".to_string()), true)
    );
}
