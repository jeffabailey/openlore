//! scale-to-zero-hosting — the pure decisions of the S3 adapter.
//!
//! * WP-1 what the write probe saw (D-STZ-39). moto honours conditions, so it
//!   cannot play a store that lies, and RIO-6 can only see the honest rows.
//! * WP-2 the write probe's key can never be the index (D-STZ-39).
//! * TO-1 the one table of client bounds (DESIGN § Timeouts). This is the honest
//!   layer for a scaffold: there is no SDK client yet to read a setting from, so
//!   the test holds the table every client must be built from. The table says
//!   `max_attempts = 1` for every call. That a real client obeys it is RIO-9's
//!   half, against a silent store.
//!
//! No store is involved, so no moto. Layer 1, closed tables.
//!
//! RED at hand-off: the functions are scaffolds.

use std::time::Duration;

use adapter_s3_index_object::{
    call_budget, location_names_write_probe_key, write_probe_outcome, RepeatAnswer,
    S3IndexObjectAdapter, S3Location, StoreCall, WriteProbeSeen, WRITE_PROBE_KEY,
};
use ports::{IndexObjectError, ProbeOutcome, ProbeRefusalReason};

const INDEX_KEY: &str = "index/index.duckdb";
const LAST_PASS_KEY: &str = "index/last-pass.json";

/// WP-1
/// ```gherkin
/// @D-STZ-39 @UI-17 @UI-20 @C5a @contract-shape:pure-function
/// Scenario Outline: Only two accepted writes mean a store ignores the condition
///   Given the write check <saw>
///   Then the check <verdict>
///   Examples:
///     | saw                                      | verdict           |
///     | its write refused, as it should be       | passes            |
///     | no clear answer to its write             | passes            |
///     | its write taken, and its repeat refused  | passes            |
///     | its write taken, and no clear repeat     | passes            |
///     | its write taken, and its repeat taken    | refuses the store |
/// ```
#[test]
fn only_two_accepted_writes_mean_a_store_ignores_the_condition() {
    use RepeatAnswer as Repeat;
    let rows = [
        (WriteProbeSeen::Honoured, false),
        (WriteProbeSeen::Unknown, false),
        (WriteProbeSeen::Repeated(Repeat::Honoured), false),
        (WriteProbeSeen::Repeated(Repeat::Unknown), false),
        (WriteProbeSeen::Repeated(Repeat::Accepted), true),
    ];
    for (seen, refused) in rows {
        match (write_probe_outcome(seen), refused) {
            (ProbeOutcome::Ok, false) => {}
            (
                ProbeOutcome::Refused {
                    reason: ProbeRefusalReason::IndexObjectWriteConditionIgnored,
                    ..
                },
                true,
            ) => {}
            (outcome, _) => panic!("{seen:?}: {outcome:?}"),
        }
    }
}

/// WP-2 (D-STZ-39: the check is a pure function of the two keys, and `connect`
/// uses it, before any PUT)
/// ```gherkin
/// @D-STZ-39 @C6a @error @contract-shape:pure-function
/// Scenario Outline: The write check's marker can never be the index
///   Given a location that keeps <what>
///   Then it <verdict>
///   And an adapter cannot be built on a location that names the marker
///   Examples:
///     | what                           | verdict                |
///     | the index under the marker     | names the marker       |
///     | the record under the marker    | names the marker       |
///     | each under its own proper key  | does not name it       |
/// ```
#[test]
fn the_write_checks_marker_can_never_be_the_index() {
    assert_eq!(WRITE_PROBE_KEY, "index/.write-probe");
    for (index_key, last_pass_key, names_it) in [
        (WRITE_PROBE_KEY, LAST_PASS_KEY, true),
        (INDEX_KEY, WRITE_PROBE_KEY, true),
        (INDEX_KEY, LAST_PASS_KEY, false),
    ] {
        assert_eq!(
            location_names_write_probe_key(index_key, last_pass_key),
            names_it,
            "{index_key} {last_pass_key}"
        );
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime");
    let built = |index_key: &str| {
        rt.block_on(S3IndexObjectAdapter::connect(S3Location {
            region: "us-east-1".to_string(),
            endpoint_url: Some("http://127.0.0.1:9".to_string()),
            bucket: "openlore-indexer-index-test".to_string(),
            index_key: index_key.to_string(),
            last_pass_key: LAST_PASS_KEY.to_string(),
        }))
    };
    assert_eq!(
        built(WRITE_PROBE_KEY).err(),
        Some(IndexObjectError::ProbeRefused {
            detail: format!("a configured key is the write probe's key {WRITE_PROBE_KEY}"),
        }),
        "connect refuses a location naming the marker"
    );
    assert_eq!(
        built(INDEX_KEY).err(),
        None,
        "the same location on the proper key builds, so that refusal is the marker's"
    );
}

/// TO-1
/// ```gherkin
/// @D-STZ-10 @D-STZ-39 @D-STZ-40a @contract-shape:pure-function
/// Scenario: Every store call has one attempt and its own time limit
///   Given the store's client settings
///   Then each call has the limit the Timeouts table gives it
///   And no call may try twice, so each limit is the real worst case
/// ```
#[test]
fn every_store_call_has_one_attempt_and_its_own_time_limit() {
    let rows = [
        (StoreCall::WriteProbePut, 2),
        (StoreCall::ReadProbeHead, 2),
        (StoreCall::SearchInitGet, 8),
        (StoreCall::FreshnessHead, 1),
        (StoreCall::Reload, 3),
        (StoreCall::GetParameter, 5),
        (StoreCall::PassGet, 60),
        (StoreCall::Publish, 35),
        (StoreCall::PutLastPass, 5),
        (StoreCall::GetLastPass, 3),
    ];
    for (call, secs) in rows {
        let budget = call_budget(call);

        assert_eq!(
            budget.operation_timeout,
            Duration::from_secs(secs),
            "{call:?}"
        );
        assert_eq!(budget.max_attempts, 1, "{call:?}");
    }
}
