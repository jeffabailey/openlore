//! Behaviour contracts of the per-DID ingest pass decisions (ADR-077/078,
//! DD-IPF-6) at the crate's public API: the exit code of a pass, the event
//! tokens operators read, the listing base URLs and whether the fallback ran.

use appview_domain::ingest_pass::{
    classify_fetch_failure, pass_exit_code, summarize_outcomes, ClassifiedSkip, FallbackUrl,
    FetchFailure, ListingSource, PassOutcome, PassSummary, PdsEndpoint, RefusalCause, SkipReason,
    EXIT_PASS_COMPLETED, EXIT_TOTAL_OUTAGE,
};
use proptest::prelude::*;

fn summary(own_pds: u64, fallback: u64, skipped: u64) -> PassSummary {
    PassSummary {
        configured: own_pds + fallback + skipped,
        own_pds,
        fallback,
        skipped,
    }
}

#[test]
fn the_exit_codes_are_zero_for_completed_and_three_for_total_outage() {
    assert_eq!(EXIT_PASS_COMPLETED, 0);
    assert_eq!(EXIT_TOTAL_OUTAGE, 3);
}

#[test]
fn a_pass_with_no_configured_dids_completes() {
    assert_eq!(pass_exit_code(&summary(0, 0, 0)), 0);
}

#[test]
fn a_pass_where_every_did_was_skipped_is_a_total_outage() {
    assert_eq!(pass_exit_code(&summary(0, 0, 1)), 3);
    assert_eq!(pass_exit_code(&summary(0, 0, 5)), 3);
}

#[test]
fn a_pass_that_listed_any_did_completes() {
    assert_eq!(pass_exit_code(&summary(1, 0, 0)), 0);
    assert_eq!(pass_exit_code(&summary(0, 1, 0)), 0);
    assert_eq!(pass_exit_code(&summary(0, 1, 4)), 0);
    assert_eq!(pass_exit_code(&summary(1, 0, 4)), 0);
    assert_eq!(pass_exit_code(&summary(2, 3, 0)), 0);
}

proptest! {
    /// Universe {own_pds, fallback, skipped}: the pass is a total outage
    /// exactly when something was configured and nothing was listed.
    #[test]
    fn the_exit_code_is_outage_exactly_when_nothing_configured_was_listed(
        own in 0u64..4, fallback in 0u64..4, skipped in 0u64..4
    ) {
        let outcomes = std::iter::repeat_n(PassOutcome::ReadFromOwnPds, own as usize)
            .chain(std::iter::repeat_n(PassOutcome::ReadFromFallback, fallback as usize))
            .chain(std::iter::repeat_n(PassOutcome::Skipped, skipped as usize));
        let pass = summarize_outcomes(outcomes);
        let expected = if skipped > 0 && own + fallback == 0 { 3 } else { 0 };
        prop_assert_eq!(pass_exit_code(&pass), expected);
    }
}

#[test]
fn skip_reasons_report_their_documented_tokens() {
    let table = [
        (SkipReason::DidUnresolvable, "did_unresolvable"),
        (SkipReason::PdsUnreachable, "pds_unreachable"),
        (SkipReason::PdsTimeout, "pds_timeout"),
        (SkipReason::ListingFailed, "listing_failed"),
        (SkipReason::PdsAddressRefused, "pds_address_refused"),
    ];
    for (reason, token) in table {
        assert_eq!(reason.token(), token, "{reason:?}");
    }
}

#[test]
fn refusal_causes_report_their_by_reason_keys_in_reporting_order() {
    let tokens: Vec<&str> = RefusalCause::ALL
        .iter()
        .map(|cause| cause.token())
        .collect();
    assert_eq!(
        tokens,
        [
            "unsigned",
            "bad_signature",
            "cid_mismatch",
            "schema_unknown",
            "provenance",
            "foreign_repo"
        ]
    );
}

#[test]
fn a_resolved_pds_is_listed_from_its_url_without_the_trailing_slash() {
    let endpoint = PdsEndpoint::new("https://pds.example.com/");
    assert_eq!(endpoint.as_str(), "https://pds.example.com");
    assert_eq!(
        ListingSource::OwnPds(endpoint).base(),
        "https://pds.example.com"
    );
}

#[test]
fn the_fallback_is_listed_from_its_trimmed_url_and_a_blank_setting_is_no_fallback() {
    let fallback = FallbackUrl::new("  https://relay.example.org/  ").unwrap();
    assert_eq!(fallback.as_str(), "https://relay.example.org");
    assert_eq!(
        ListingSource::Fallback(fallback).base(),
        "https://relay.example.org"
    );
    assert_eq!(FallbackUrl::new("   "), None);
}

#[test]
fn only_a_failing_fallback_marks_the_fallback_as_used() {
    let own = ListingSource::OwnPds(PdsEndpoint::new("https://pds.example.com"));
    let fallback = ListingSource::Fallback(FallbackUrl::new("https://relay.example.org").unwrap());
    for failure in [
        FetchFailure::Unreachable,
        FetchFailure::BadResponse,
        FetchFailure::AddressRefused,
        FetchFailure::TimedOut,
    ] {
        assert!(
            !classify_fetch_failure(&own, failure).fallback_used(),
            "{failure:?}"
        );
        assert!(
            classify_fetch_failure(&fallback, failure).fallback_used(),
            "{failure:?}"
        );
    }
    assert!(!ClassifiedSkip::planned(SkipReason::DidUnresolvable).fallback_used());
}
