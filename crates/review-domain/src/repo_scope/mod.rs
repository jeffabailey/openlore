//! Which owned repos a scan reads (US-RRA-001, US-RRA-002; ADR-088 §3, §6).
//! PURE: inputs are the live listing and the saved choice, outputs are the
//! choose page's rows and the scan's plan. No clock, no I/O.
//!
//! An account's eligible repos (owned, not a fork, not archived) are ranked
//! by `scraper_domain::select_person_repos`, the ranking today's scan uses.
//! Without a saved choice its top [`DEFAULT_TICKS`] are ticked; with one,
//! exactly the saved ticks are. A scan reads exactly the ticked repos, and a
//! repo outside today's top 5 is tagged [`ports::OutsideDefault::OwnedChoice`].
//!
//! By concern: `limits` (the account and repo limits, the one
//! total `S + I + D`), `choose` (the choose page and what a Save stores),
//! `plan` (a scan's start and frozen plan), `imports` ("Add repo"),
//! `moves` (BR-RRA-5), `resume` (a paused run's plan) and `visibility`.

mod choose;
mod imports;
mod limits;
mod moves;
mod plan;
mod resume;
mod visibility;

#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
use ports::{
    AccountRepoListing, ListedRepo, ListingExtent, OutsideDefault, OwnedRepo, PlanRowStatus,
    RepoFacts, SavedRepoChoice, SkippedScanRepo,
};
use scraper_domain::DEFAULT_PERSON_REPO_COUNT;

pub use choose::{
    choice_to_save, choose_list, differs_from_default, eligible_count, ticked_ids, AccountChoice,
    ChooseList, ChooseRow, PlannedRepo, TicksRoom,
};
pub use imports::{classify_import, import_of, import_shape, ImportRefusal, ImportVerdict};
pub use limits::{
    admit_account, admit_import, admit_save, chosen_total, default_ticks, LimitRefusal, OtherOwner,
    SavedTicks, ScopeAccount, Standing,
};
pub use moves::{current_ticks, moves, owned_imports, Scope, ScopeMove};
pub use plan::{
    carried_repo, listing_with_reads, named_skips, plan_account, plan_row, scan_intent,
    unlisted_ticks, AccountCheck, AccountPlan, FrozenScope, Listing, ScanAccount, ScanIntent,
    ScanPlan, SkipReason, SkippedRepo,
};
pub use resume::{
    resume_plan, resumes_frozen_plan, standing_after_check, BioCheck, CarriedSkip, FrozenRow,
    ListedSources, Resume, StandingChange,
};
pub use visibility::visible;

#[cfg(test)]
use choose::{account_plan, ranked};

/// Repos ticked for an account without a saved choice: today's top 5.
pub const DEFAULT_TICKS: usize = DEFAULT_PERSON_REPO_COUNT;

/// Repos one scan may read across every account and import (BR-RRA-3,
/// D-RRA-16): one constant, no setting.
pub const SCAN_REPO_CAP: usize = 10;

/// GitHub accounts one Bluesky sign-in may list (D-RRA-15): one constant.
pub const MAX_GITHUB_ACCOUNTS: usize = 3;

/// Where a row of a frozen plan (and each suggestion it yields) comes
/// from: a listed account, or an imported repo.
pub use ports::RepoSource as PlanSource;

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn listed(id: u64, stars: u64, fork: bool, archived: bool) -> ListedRepo {
        ListedRepo {
            repo_id: id,
            owner_id: 1,
            repo: OwnedRepo {
                full_name: format!("me/repo-{id}"),
                description: None,
                language: None,
                stars,
                fork,
                archived,
                pushed_at: None,
            },
        }
    }

    fn arb_listing() -> impl Strategy<Value = Vec<ListedRepo>> {
        prop::collection::vec((0u64..500, any::<bool>(), any::<bool>()), 0..12).prop_map(|rows| {
            rows.into_iter()
                .enumerate()
                .map(|(i, (stars, fork, archived))| listed(i as u64 + 1, stars, fork, archived))
                .collect()
        })
    }

    proptest! {
        /// U-RRA-D2: a planned repo frozen as a plan row and read back by a
        /// resume is the same repo with the same outside-default tag.
        #[test]
        fn a_resumed_row_is_the_planned_repo_with_its_tag(
            repo_id in 1u64..10_000,
            account_id in 1u64..10_000,
            tag in proptest::option::of(proptest::sample::select(OutsideDefault::ALL.to_vec())),
        ) {
            let planned = PlannedRepo {
                repo_id,
                full_name: format!("me/repo-{repo_id}"),
                outside_default: tag,
            };
            let row = plan_row(PlanSource::Account(account_id), &planned);
            prop_assert_eq!(row.source, PlanSource::Account(account_id));
            prop_assert_eq!(carried_repo(&row), planned);
        }

        /// Universe: listings and any set of ticked ids. A scan reads exactly
        /// the eligible repos ticked at Save, and tags exactly those ranked
        /// outside today's top 5; with no saved choice it reads today's top 5,
        /// none tagged.
        #[test]
        fn a_scan_reads_exactly_the_saved_ticks(
            listing in arb_listing(),
            ticked in prop::collection::btree_set(0u64..14, 0..14),
        ) {
            let (ranked, _, _) = ranked(&listing);
            let saved = choice_to_save(&listing, &ticked);
            let plan = account_plan(&listing, AccountChoice::of(&saved));
            let expected: Vec<u64> = ranked
                .iter()
                .map(|l| l.repo_id)
                .filter(|id| ticked.contains(id))
                .collect();
            prop_assert_eq!(plan.iter().map(|p| p.repo_id).collect::<Vec<_>>(), expected);
            for planned in &plan {
                let rank = ranked.iter().position(|l| l.repo_id == planned.repo_id).unwrap();
                prop_assert_eq!(planned.outside_default.is_some(), rank >= DEFAULT_TICKS);
            }

            let default = account_plan(&listing, AccountChoice::Default);
            let top: Vec<u64> = ranked.iter().take(DEFAULT_TICKS).map(|l| l.repo_id).collect();
            prop_assert_eq!(default.iter().map(|p| p.repo_id).collect::<Vec<_>>(), top);
            prop_assert!(default.iter().all(|p| p.outside_default.is_none()));
        }
    }

    fn facts(id: u64, owner_id: u64) -> RepoFacts {
        RepoFacts {
            repo_id: id,
            full_name: format!("me/repo-{id}"),
            owner_id,
            owner_login: "me".to_string(),
            owner_kind: ports::RepoOwnerKind::User,
            fork: false,
            archived: false,
        }
    }

    // bypass: I-06 as three named cases (complete, truncated + still owned,
    // truncated + gone); the AT covers only complete listings.
    #[test]
    fn a_truncated_listing_never_proves_a_chosen_repo_missing() {
        let saved = [
            SavedRepoChoice {
                repo_id: 1,
                full_name: "me/repo-1".to_string(),
                ticked: true,
            },
            SavedRepoChoice {
                repo_id: 9,
                full_name: "me/repo-9".to_string(),
                ticked: true,
            },
        ];
        let listing = |extent| AccountRepoListing {
            repos: vec![listed(1, 5, false, false)],
            extent,
        };
        assert!(unlisted_ticks(&saved, &listing(ListingExtent::Complete)).is_empty());
        let unread: Vec<u64> = unlisted_ticks(&saved, &listing(ListingExtent::Truncated))
            .iter()
            .map(|row| row.repo_id)
            .collect();
        assert_eq!(unread, [9]);

        let read_back = |reads: &[(u64, RepoFacts)]| {
            let listing = listing_with_reads(listing(ListingExtent::Truncated), 1, reads);
            plan_account(&[1, 9], &AccountCheck::Passed(listing))
        };
        let AccountPlan::Read { planned, skipped } = read_back(&[(9, facts(9, 1))]) else {
            panic!("passed");
        };
        assert_eq!(
            planned.iter().map(|p| p.repo_id).collect::<Vec<_>>(),
            [1, 9]
        );
        assert!(skipped.is_empty());
        for gone in [vec![], vec![(9, facts(9, 2))], vec![(9, facts(10, 1))]] {
            let AccountPlan::Read { skipped, .. } = read_back(&gone) else {
                panic!("passed");
            };
            let reasons: Vec<_> = skipped.iter().map(|s| (s.repo_id, s.reason)).collect();
            assert_eq!(reasons, [(9, SkipReason::MissingOrTransferred)], "{gone:?}");
        }

        let plan = ScanPlan {
            accounts: vec![(1, read_back(&[]))],
            imports: Vec::new(),
        };
        assert_eq!(
            named_skips(&plan, &saved),
            [SkippedScanRepo {
                repo_id: 9,
                full_name: "me/repo-9".to_string(),
                source_account_id: 1,
                reason: SkipReason::MissingOrTransferred,
            }],
            "a skip is named as chosen, and carries nothing else"
        );
    }

    // bypass: the DISCUSS example (7 owned, 1 fork, 1 archived) as a table.
    #[test]
    fn first_visit_ticks_the_top_5_and_counts_what_was_skipped() {
        let listing = [
            listed(1, 412, false, false),
            listed(2, 900, true, false),
            listed(3, 88, false, false),
            listed(4, 700, false, true),
            listed(5, 51, false, false),
            listed(6, 37, false, false),
            listed(7, 12, false, false),
            listed(8, 3, false, false),
            listed(9, 0, false, false),
        ];
        let list = choose_list(&listing, AccountChoice::Default);
        let ticked: Vec<u64> = list
            .rows
            .iter()
            .filter(|r| r.ticked)
            .map(|r| r.repo_id)
            .collect();
        assert_eq!(list.rows.len(), 7);
        assert_eq!(ticked, [1, 3, 5, 6, 7]);
        assert_eq!((list.forks_skipped, list.archived_skipped), (1, 1));
    }

    fn plan_source() -> impl Strategy<Value = PlanSource> {
        prop_oneof![
            (1u64..5).prop_map(PlanSource::Account),
            (1u64..5).prop_map(PlanSource::Imported),
        ]
    }

    proptest! {
        /// Universe: frozen rows from any sources, and any listed accounts
        /// and imports. A resume skips "removed" exactly the planned rows
        /// whose source is no longer listed, whatever their bio check.
        #[test]
        fn a_resume_skips_as_removed_exactly_the_rows_of_unlisted_sources(
            sources in prop::collection::vec(plan_source(), 0..8),
            accounts in prop::collection::vec(1u64..5, 0..4),
            imports in prop::collection::vec(1u64..5, 0..4),
        ) {
            let listed = ListedSources { accounts, imports };
            let rows: Vec<FrozenRow> = sources
                .iter()
                .map(|source| FrozenRow { source: *source, status: PlanRowStatus::Planned })
                .collect();
            let checks: BTreeMap<u64, BioCheck> =
                (1u64..5).map(|id| (id, BioCheck::Passed)).collect();
            let resume = resume_plan(&rows, &checks, &listed.removed_from(&rows));
            let removed: Vec<usize> = resume
                .skipped
                .iter()
                .filter(|(_, why)| *why == CarriedSkip::Removed)
                .map(|(position, _)| *position)
                .collect();
            let unlisted: Vec<usize> = (0..sources.len())
                .filter(|position| !listed.holds(sources[*position]))
                .collect();
            prop_assert_eq!(removed, unlisted);
            prop_assert!(resume.to_read.iter().all(|position| listed.holds(sources[*position])));
        }
    }
}
