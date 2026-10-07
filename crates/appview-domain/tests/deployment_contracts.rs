//! Behaviour contracts of the indexer-deployment decisions (ADR-080..083) at
//! the crate's public API, checked against concrete, hand-written values (no
//! oracle that re-derives the answer the way the code does): the repo-DID list
//! read, the purge plan, the single-flight runner, the pass deadline, the
//! public health answer and the operator-facing tokens.

use std::collections::BTreeSet;
use std::time::Duration;

use appview_domain::did_list::{read_did_list, BadEntry, EntryProblem, MAX_DID_LENGTH};
use appview_domain::health::{health_of, HealthResponse, StoreHealth};
use appview_domain::ingest_pass::{within_deadline, PassExit, PassFailure};
use appview_domain::pass_runner::{
    single_flight, PassEnding, PassId, RunnerEvent, RunnerReply, RunnerSlot,
};
use appview_domain::purge_plan::{plan_purge, BareDid, PurgePlan, PurgeSuppressed};
use chrono::{TimeZone, Utc};
use claim_domain::Did;

fn dids(texts: &[&str]) -> Vec<Did> {
    texts.iter().map(|text| Did((*text).to_string())).collect()
}

fn refused(entry: &str, problem: EntryProblem) -> Result<Vec<Did>, BadEntry> {
    Err(BadEntry {
        entry: entry.to_string(),
        problem,
    })
}

// --- the repo-DID list -------------------------------------------------------

#[test]
fn a_list_separated_by_commas_and_whitespace_loads_every_did_in_order() {
    let text = "did:plc:priya,did:web:example.com did:plc:dmitri\r\ndid:plc:jeff\t,, ";
    assert_eq!(
        read_did_list(text),
        Ok(dids(&[
            "did:plc:priya",
            "did:web:example.com",
            "did:plc:dmitri",
            "did:plc:jeff",
        ]))
    );
}

#[test]
fn a_repeated_did_is_kept_once_at_its_first_position() {
    assert_eq!(
        read_did_list("did:plc:b did:plc:a did:plc:b,did:plc:a"),
        Ok(dids(&["did:plc:b", "did:plc:a"]))
    );
}

#[test]
fn an_empty_or_blank_list_loads_no_did() {
    assert_eq!(read_did_list(""), Ok(vec![]));
    assert_eq!(read_did_list(" ,\n\t,"), Ok(vec![]));
}

#[test]
fn every_identifier_character_the_did_syntax_allows_is_accepted() {
    assert_eq!(
        read_did_list("did:web:Ex-ample_1.com%3A8080:path"),
        Ok(dids(&["did:web:Ex-ample_1.com%3A8080:path"]))
    );
}

#[test]
fn an_entry_that_is_not_did_shaped_is_not_a_did() {
    assert_eq!(
        read_did_list("did:plc:ok tomas did:plc:later"),
        refused("tomas", EntryProblem::NotADid)
    );
    assert_eq!(
        read_did_list("did:plc"),
        refused("did:plc", EntryProblem::NotADid)
    );
}

#[test]
fn the_first_bad_entry_refuses_the_whole_list() {
    assert_eq!(
        read_did_list("did:plc:a,did:key:z6Mk,tomas"),
        refused("did:key:z6Mk", EntryProblem::NamesNoRepo)
    );
}

#[test]
fn a_well_formed_did_of_another_method_names_no_repo() {
    assert_eq!(
        read_did_list("did:key:z6Mkabc"),
        refused("did:key:z6Mkabc", EntryProblem::NamesNoRepo)
    );
}

#[test]
fn a_did_breaking_the_syntax_is_not_well_formed() {
    for entry in [
        "did::abc",          // empty method
        "did:PLC:abc",       // upper-case method
        "did:p1c:abc",       // digit in the method
        "did:plc:",          // empty identifier
        "did:plc:abc:",      // identifier ending in ':'
        "did:plc:abc#frag",  // '#' is not an identifier character
        "did:plc:a/b",       // '/' is not an identifier character
        "\u{feff}did:plc:x", // a byte-order mark is not whitespace
    ] {
        let expected = if entry.starts_with('\u{feff}') {
            EntryProblem::NotADid
        } else {
            EntryProblem::NotWellFormed
        };
        assert_eq!(read_did_list(entry), refused(entry, expected), "{entry:?}");
    }
}

#[test]
fn a_did_of_exactly_the_longest_length_loads_and_one_byte_more_does_not() {
    let prefix = "did:plc:";
    let longest = format!("{prefix}{}", "a".repeat(MAX_DID_LENGTH - prefix.len()));
    assert_eq!(longest.len(), 2048);
    assert_eq!(read_did_list(&longest), Ok(dids(&[longest.as_str()])));

    let too_long = format!("{longest}b");
    assert_eq!(
        read_did_list(&too_long),
        refused(&too_long, EntryProblem::NotWellFormed)
    );
}

#[test]
fn each_problem_reads_as_its_operator_facing_description() {
    assert_eq!(
        EntryProblem::NotADid.describe(),
        "is not a DID (did:<method>:<identifier>)"
    );
    assert_eq!(
        EntryProblem::NotWellFormed.describe(),
        "is not a well-formed DID"
    );
    assert_eq!(
        EntryProblem::NamesNoRepo.describe(),
        "names no repo: only did:plc and did:web DIDs do"
    );
}

// --- the purge plan ----------------------------------------------------------

#[test]
fn only_indexed_authors_the_list_no_longer_names_are_purged_by_bare_did() {
    let listed = dids(&["did:plc:priya"]);
    let indexed = [
        "did:plc:priya#org.openlore.application",
        "did:plc:priya",
        "did:plc:priya_x",
        "did:plc:dmitri#org.openlore.application",
        "did:plc:dmitri",
    ];
    let expected: BTreeSet<BareDid> = ["did:plc:dmitri", "did:plc:priya_x"]
        .into_iter()
        .map(|did| BareDid(did.to_string()))
        .collect();
    assert_eq!(plan_purge(&listed, indexed), PurgePlan::Purge(expected));
}

#[test]
fn an_empty_list_suppresses_the_purge_with_the_empty_list_reason() {
    assert_eq!(
        plan_purge(&[], ["did:plc:priya"]),
        PurgePlan::Suppressed(PurgeSuppressed::EmptyList)
    );
    assert_eq!(PurgeSuppressed::EmptyList.token(), "empty_list");
}

// --- the single-flight runner ------------------------------------------------

#[test]
fn the_next_pass_id_is_one_more() {
    assert_eq!(PassId(7).next(), PassId(8));
    assert_eq!(PassId(0).next(), PassId(1));
}

#[test]
fn the_runner_starts_one_pass_names_it_while_busy_and_frees_on_any_ending() {
    let (slot, reply) = single_flight(RunnerSlot::Idle, RunnerEvent::RunPassRequested, PassId(3));
    assert_eq!(
        (slot, reply),
        (
            RunnerSlot::Running(PassId(3)),
            RunnerReply::Started(PassId(3))
        )
    );

    let (slot, reply) = single_flight(slot, RunnerEvent::RunPassRequested, PassId(4));
    assert_eq!(
        (slot, reply),
        (
            RunnerSlot::Running(PassId(3)),
            RunnerReply::Busy {
                running_pass_id: PassId(3)
            }
        )
    );

    for ending in [
        PassEnding::Completed,
        PassEnding::Panicked,
        PassEnding::DeadlineExceeded,
    ] {
        assert_eq!(
            single_flight(slot, RunnerEvent::PassEnded(ending), PassId(4)),
            (RunnerSlot::Idle, RunnerReply::Freed)
        );
        assert_eq!(
            single_flight(RunnerSlot::Idle, RunnerEvent::PassEnded(ending), PassId(4)),
            (RunnerSlot::Idle, RunnerReply::Ignored)
        );
    }
}

// --- the pass deadline and the failure tokens --------------------------------

#[test]
fn a_pass_ending_exactly_at_its_deadline_keeps_its_own_exit() {
    let deadline = Duration::from_secs(600);
    assert_eq!(
        within_deadline(PassExit::Completed, deadline, deadline),
        PassExit::Completed
    );
    assert_eq!(
        within_deadline(
            PassExit::TotalOutage,
            deadline + Duration::from_nanos(1),
            deadline
        ),
        PassExit::Failed(PassFailure::PassDeadlineExceeded)
    );
}

#[test]
fn each_pass_failure_reads_as_its_summary_cause_token() {
    let tokens = [
        (PassFailure::ListMalformed, "repo_dids_malformed"),
        (PassFailure::ListUnreadable, "repo_dids_unreadable"),
        (PassFailure::UpsertFailed, "upsert_failed"),
        (PassFailure::PurgeFailed, "purge_failed"),
        (PassFailure::PassPanicked, "pass_panicked"),
        (PassFailure::PassDeadlineExceeded, "pass_deadline_exceeded"),
    ];
    for (failure, token) in tokens {
        assert_eq!(failure.token(), token);
    }
}

// --- the public health answer ------------------------------------------------

#[test]
fn a_usable_store_is_200_with_the_last_successful_pass_time() {
    let at = Utc.with_ymd_and_hms(2026, 10, 7, 12, 30, 5).unwrap();
    let health = health_of(StoreHealth::Usable, Some(at));
    assert_eq!(health.status_code(), 200);
    assert_eq!(
        health.body(),
        serde_json::json!({"status": "ok", "last_successful_pass_at": "2026-10-07T12:30:05Z"})
    );

    let before_any_pass = health_of(StoreHealth::Usable, None);
    assert_eq!(
        before_any_pass.body(),
        serde_json::json!({"status": "ok", "last_successful_pass_at": null})
    );
}

#[test]
fn an_unusable_store_is_503_store_unusable_whatever_the_last_pass() {
    let at = Utc.with_ymd_and_hms(2026, 10, 7, 12, 30, 5).unwrap();
    let health = health_of(StoreHealth::Unusable, Some(at));
    assert_eq!(health, HealthResponse::StoreUnusable);
    assert_eq!(health.status_code(), 503);
    assert_eq!(
        health.body(),
        serde_json::json!({"status": "store_unusable"})
    );
}
