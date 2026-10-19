//! review-app-repos-and-accounts — layer-2 PURE-CORE properties (ADR-007
//! functional core; Mandate 9: PBT full at layers 1-2).
//!
//! The rules this feature adds are total pure functions in the NEW modules
//! `review_domain::repo_scope` and `claim_domain::authority` (owner tier)
//! (component-boundaries §2; brief "Pure seams for property tests"). Their
//! contracts are stated here over generated inputs and checked against
//! ORACLES written from the ADR text (ADR-088 §6, ADR-089 §2-5,
//! architecture-design §6.2-6.3) — never from an implementation:
//!
//! | ID | Release | Contract | Source |
//! |---|---|---|---|
//! | CORE-RRA-1 | R1 | default ticks: verified accounts without a saved choice, in `added_at` order, get the first `min(5, eligible, room left)` of their ranking; `S + I + Σd ≤ cap` whenever `S + I ≤ cap` | §6.3, AC-001.1, AC-003.7 |
//! | CORE-RRA-2 | R1 | Save on account a with t ticks is admitted iff `t + (S − s_a) + I + D_others ≤ cap`; a refusal names limit, total and excess | §6.3, AC-001.3, AC-004.6 |
//! | CORE-RRA-3 | R3 | closure: every state reached by admitted Saves and imports passes the scan-start check (one definition of S, I-07) | §6.3, BR-RRA-3 |
//! | CORE-RRA-4 | R1 | scan intent + plan: refuses `NothingToRead` / `OverLimit` before admission; plan ⊆ choice ∪ defaults ∪ imports of passing accounts, ≤ cap; no row of a failed or unfinished account (every listed account is re-checked, unverified included — D-RRA-T1); every chosen repo not planned is skipped with a reason | §6.2, AC-002.1/2.4/2.5, AC-004.2/4.8, KPI-RRA-5/7 |
//! | CORE-RRA-5 | R1 | resume: the repos read across a run and its resumes ⊆ the first plan, each at most once; failed account → skipped "account unverified"; unfinished → carried; removed source → skipped | §6.2 step 8, ADR-088 §6, AC-004.7, AC-008.4, AC-009.4 |
//! | CORE-RRA-6 | R2 | `may_sign`: permit iff the repo's current owner id is listed, the owner is a user and its bio (read now) holds the signer DID; closed refusal set; the permit's subject is the repo | ADR-089 §2, AC-010.1/10.2/10.3, BR-RRA-1/2 |
//! | CORE-RRA-7 | R2 | visibility: an account row is shown iff its account is listed and verified; an imported row iff its repo is on the import list; unticking changes nothing | ADR-089 §5, AC-002.6, AC-004.3/4.4 |
//! | CORE-RRA-8 | R2 | account admission (the pure `repo_scope::admit_account` decision, not the write port): ≤ 3 per DID; one verified DID per GitHub id; re-adding a listed account changes nothing | D-RRA-15, AC-003.3/3.4/3.5 |
//! | CORE-RRA-9 | R3 | import classification: closed verdict set with precedence shape → read (not found / unreachable) → owned by a listed account → over limit → admitted | §9 R3, AC-005.1/5.2/5.3/5.6 |
//! | CORE-RRA-10 | R3 | BR-RRA-5 move: import −1, owning account's saved choice = its current ticks ∪ {repo}; the total never changes | BR-RRA-5, AC-006.5 |
//!
//! ## Binding seam (Mandate 7)
//!
//! Every property calls a `sut_*` binding that makes ONE call into the
//! production function and maps its ADTs onto the view types here (the view
//! types are the observable contract). `repo_scope` and `authority` do not
//! exist yet: the bindings are `todo!()` (RED, not BROKEN — the file compiles
//! today). DELIVER replaces each `todo!()` with the one call.
//!
//! Tier B (state-machine PBT) is not added: CORE-RRA-3 and CORE-RRA-5 already
//! drive generated op sequences through the pure core; the layer-4 journeys
//! (Save → scan → pause → resume, add → fail → re-verify, import → move) are
//! example-chained in the three HTTP suites.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;

const CAP: usize = 10;
const DEFAULT_TICKS: usize = 5;
const MAX_ACCOUNTS: usize = 3;

// =============================================================================
// Observable view types (the contract)
// =============================================================================

/// One listed GitHub account as `repo_scope` sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountView {
    id: u64,
    verified: bool,
    /// Saved ticked repo ids (`None` = no saved choice: default ticks).
    saved: Option<Vec<u64>>,
    /// Eligible (owned, non-fork, non-archived) repo ids, ranked.
    eligible: Vec<u64>,
}

/// The owner's scope: accounts in `added_at` order, imported repo ids.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ScopeView {
    accounts: Vec<AccountView>,
    imports: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LimitRefusalView {
    limit: usize,
    total: usize,
    excess: usize,
}

/// The bio check of one account in a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckView {
    Passed,
    Failed,
    Unfinished,
}

/// Why a chosen repo is not planned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SkipView {
    Fork,
    Archived,
    MissingOrTransferred,
    AccountUnverified,
    Removed,
}

/// What GitHub's listing says now about one chosen repo id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListedView {
    Eligible,
    Fork,
    Archived,
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum IntentView {
    NothingToRead,
    OverLimit(LimitRefusalView),
    Plan {
        rows: Vec<(Option<u64>, u64)>,
        skipped: BTreeSet<(u64, SkipView)>,
    },
}

/// The permit's basis (owner tier only; R4 would add an administered tier).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BasisView {
    Owner(u64),
}

/// ADR-089 §2.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PermitView {
    /// ADR-089 §2: `SigningPermit { signer, subject, basis: Owner(github_id) }`.
    Permit {
        signer: String,
        subject: String,
        basis: BasisView,
    },
    NotOwnedByListedAccount,
    OrgOwned,
    RepoGone,
    BioNoLongerHolds(u64),
    Unfinished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepoReadView {
    Found {
        full_name: String,
        owner_id: u64,
        owner_is_user: bool,
    },
    NotFound,
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProfileReadView {
    Bio(Option<String>),
    Unreachable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceView {
    Account(u64),
    Imported(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddAccountView {
    Added,
    Reverified,
    AlreadyListed,
    HeldByAnotherOwner,
    AtLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportVerdictView {
    Admitted,
    Shape,
    NotFound,
    Unreachable,
    OwnedByListedAccount,
    OverLimit,
}

// =============================================================================
// Binding seam — each binds to ONE production call (RED until DELIVER)
// =============================================================================

fn sut_default_ticks(scope: &ScopeView) -> Vec<(u64, Vec<u64>)> {
    use review_domain::repo_scope::{default_ticks, SavedTicks, ScopeAccount, Standing};
    let accounts: Vec<ScopeAccount> = scope
        .accounts
        .iter()
        .map(|a| ScopeAccount {
            github_user_id: a.id,
            standing: if a.verified {
                Standing::Verified
            } else {
                Standing::Unverified
            },
            saved: a
                .saved
                .clone()
                .map_or(SavedTicks::NeverSaved, SavedTicks::Saved),
            ranked_eligible: a.eligible.clone(),
        })
        .collect();
    default_ticks(&accounts, &scope.imports, CAP)
}

fn sut_save_admission(
    scope: &ScopeView,
    account: usize,
    ticked: &[u64],
) -> Result<(), LimitRefusalView> {
    use review_domain::repo_scope::{admit_save, SavedTicks, ScopeAccount, Standing};
    let accounts: Vec<ScopeAccount> = scope
        .accounts
        .iter()
        .map(|a| ScopeAccount {
            github_user_id: a.id,
            standing: if a.verified {
                Standing::Verified
            } else {
                Standing::Unverified
            },
            saved: a
                .saved
                .clone()
                .map_or(SavedTicks::NeverSaved, SavedTicks::Saved),
            ranked_eligible: a.eligible.clone(),
        })
        .collect();
    admit_save(
        &accounts,
        &scope.imports,
        scope.accounts[account].id,
        ticked,
        CAP,
    )
    .map(drop)
    .map_err(|refusal| LimitRefusalView {
        limit: refusal.limit,
        total: refusal.total,
        excess: refusal.excess,
    })
}

fn sut_import_admission(_scope: &ScopeView) -> Result<(), LimitRefusalView> {
    todo!("RED: bind to review_domain::repo_scope::admit_import (DELIVER 03-*)")
}

fn sut_scan_intent(
    scope: &ScopeView,
    checks: &[CheckView],
    listings: &[BTreeMap<u64, ListedView>],
) -> IntentView {
    use ports::{ListedRepo, OwnedRepo};
    use review_domain::repo_scope::{
        scan_intent, AccountCheck, AccountPlan, Listing, SavedTicks, ScanAccount, ScanIntent,
        SkipReason,
    };
    let accounts: Vec<ScanAccount> = scope
        .accounts
        .iter()
        .map(|a| ScanAccount {
            github_user_id: a.id,
            saved: a
                .saved
                .clone()
                .map_or(SavedTicks::NeverSaved, SavedTicks::Saved),
            eligible_seen: Some(a.eligible.len()),
        })
        .collect();
    let account_checks: Vec<AccountCheck> = scope
        .accounts
        .iter()
        .zip(checks.iter().zip(listings))
        .map(|(a, (check, listing))| match check {
            CheckView::Failed => AccountCheck::Failed,
            CheckView::Unfinished => AccountCheck::Unfinished,
            CheckView::Passed => AccountCheck::Passed(Listing {
                ranked_eligible: a.eligible.clone(),
                repos: listing
                    .iter()
                    .filter(|(_, now)| **now != ListedView::Absent)
                    .map(|(id, now)| ListedRepo {
                        repo_id: *id,
                        owner_id: a.id,
                        repo: OwnedRepo {
                            full_name: format!("owner{}/repo-{id}", a.id),
                            description: None,
                            language: None,
                            stars: 0,
                            fork: *now == ListedView::Fork,
                            archived: *now == ListedView::Archived,
                            pushed_at: None,
                        },
                    })
                    .collect(),
            }),
        })
        .collect();
    match scan_intent(&accounts, &scope.imports, CAP) {
        ScanIntent::NothingToRead => IntentView::NothingToRead,
        ScanIntent::OverLimit(r) => IntentView::OverLimit(LimitRefusalView {
            limit: r.limit,
            total: r.total,
            excess: r.excess,
        }),
        ScanIntent::Read(frozen) => {
            let plan = frozen.plan(&account_checks);
            let mut rows = Vec::new();
            let mut skipped = BTreeSet::new();
            for (id, account) in &plan.accounts {
                if let AccountPlan::Read {
                    planned,
                    skipped: s,
                } = account
                {
                    rows.extend(planned.iter().map(|p| (Some(*id), p.repo_id)));
                    skipped.extend(s.iter().map(|s| {
                        let reason = match s.reason {
                            SkipReason::Fork => SkipView::Fork,
                            SkipReason::Archived => SkipView::Archived,
                            SkipReason::MissingOrTransferred => SkipView::MissingOrTransferred,
                        };
                        (s.repo_id, reason)
                    }));
                }
            }
            rows.extend(plan.imports.iter().map(|r| (None, *r)));
            IntentView::Plan { rows, skipped }
        }
    }
}

fn sut_resume(
    _plan: &[(Option<u64>, u64)],
    _read: &BTreeSet<usize>,
    _checks: &BTreeMap<u64, CheckView>,
    _removed: &BTreeSet<u64>,
) -> (Vec<usize>, BTreeSet<(usize, SkipView)>) {
    todo!("RED: bind to review_domain::repo_scope::resume_plan (DELIVER 01-*)")
}

fn sut_may_sign(
    _signer: &str,
    _subject: &str,
    _listed: &[u64],
    _repo: &RepoReadView,
    _owner_profile: &ProfileReadView,
) -> PermitView {
    todo!("RED: bind to claim_domain::authority::may_sign (DELIVER 02-*)")
}

fn sut_visible(_rows: &[SourceView], _accounts: &[(u64, bool)], _imports: &[u64]) -> Vec<bool> {
    todo!("RED: bind to review_domain::repo_scope::visible (DELIVER 02-*)")
}

fn sut_add_account(
    _listed: &[u64],
    _id: u64,
    _verified_for_other_did: bool,
    _listed_unverified: bool,
) -> AddAccountView {
    todo!("RED: bind to the pure review_domain::repo_scope::admit_account decision (DELIVER 02-*)")
}

fn sut_classify_import(
    _typed: &str,
    _read: &RepoReadView,
    _listed: &[u64],
    _scope: &ScopeView,
) -> ImportVerdictView {
    todo!("RED: bind to review_domain::repo_scope::classify_import (DELIVER 03-*)")
}

fn sut_move(_scope: &ScopeView, _repo: u64, _account: usize) -> ScopeView {
    todo!("RED: bind to review_domain::repo_scope::moves (DELIVER 03-*)")
}

// =============================================================================
// Oracles (written from the ADR text)
// =============================================================================

/// `S`: ticked rows of every listed account with a saved choice, verified or
/// not (OQ-D3, one definition).
fn oracle_s(scope: &ScopeView) -> usize {
    scope
        .accounts
        .iter()
        .filter_map(|a| a.saved.as_ref())
        .map(Vec::len)
        .sum()
}

/// `d_k = min(5, e_k, cap − S − I − Σ_{j<k} d_j)` for verified accounts
/// without a saved choice, in order.
fn oracle_default_ticks(scope: &ScopeView) -> Vec<(u64, Vec<u64>)> {
    let mut room = CAP.saturating_sub(oracle_s(scope) + scope.imports.len());
    let mut out = Vec::new();
    for a in &scope.accounts {
        if a.verified && a.saved.is_none() {
            let d = DEFAULT_TICKS.min(a.eligible.len()).min(room);
            room -= d;
            out.push((a.id, a.eligible[..d].to_vec()));
        }
    }
    out
}

fn oracle_d_total(scope: &ScopeView) -> usize {
    oracle_default_ticks(scope)
        .iter()
        .map(|(_, t)| t.len())
        .sum()
}

fn oracle_save(scope: &ScopeView, account: usize, ticked: &[u64]) -> Result<(), LimitRefusalView> {
    let a = &scope.accounts[account];
    let s_a = a.saved.as_ref().map_or(0, Vec::len);
    // D_others: the OTHER accounts' current default ticks (as shown now).
    let d_others: usize = oracle_default_ticks(scope)
        .iter()
        .filter(|(id, _)| *id != a.id)
        .map(|(_, t)| t.len())
        .sum();
    let total = ticked.len() + (oracle_s(scope) - s_a) + scope.imports.len() + d_others;
    if total <= CAP {
        Ok(())
    } else {
        Err(LimitRefusalView {
            limit: CAP,
            total,
            excess: total - CAP,
        })
    }
}

fn oracle_import(scope: &ScopeView) -> Result<(), LimitRefusalView> {
    let total = oracle_s(scope) + scope.imports.len() + oracle_d_total(scope) + 1;
    if total <= CAP {
        Ok(())
    } else {
        Err(LimitRefusalView {
            limit: CAP,
            total: total - 1,
            excess: total - CAP,
        })
    }
}

fn oracle_did_in_bio(bio: Option<&str>, did: &str) -> bool {
    bio.is_some_and(|b| {
        b.split_whitespace()
            .any(|w| w.trim_end_matches(['.', ',', ';', ':', ')', '!', '?']) == did)
    })
}

fn oracle_may_sign(
    signer: &str,
    subject: &str,
    listed: &[u64],
    repo: &RepoReadView,
    profile: &ProfileReadView,
) -> PermitView {
    match repo {
        RepoReadView::Unreachable => PermitView::Unfinished,
        RepoReadView::NotFound => PermitView::RepoGone,
        RepoReadView::Found {
            owner_is_user: false,
            ..
        } => PermitView::OrgOwned,
        RepoReadView::Found { owner_id, .. } if !listed.contains(owner_id) => {
            PermitView::NotOwnedByListedAccount
        }
        RepoReadView::Found { owner_id, .. } => match profile {
            ProfileReadView::Unreachable => PermitView::Unfinished,
            ProfileReadView::Bio(bio) if oracle_did_in_bio(bio.as_deref(), signer) => {
                PermitView::Permit {
                    signer: signer.to_string(),
                    subject: subject.to_string(),
                    basis: BasisView::Owner(*owner_id),
                }
            }
            ProfileReadView::Bio(_) => PermitView::BioNoLongerHolds(*owner_id),
        },
    }
}

fn oracle_shape_ok(typed: &str) -> bool {
    let valid = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    let parts: Vec<&str> = typed.trim().split('/').collect();
    parts.len() == 2 && valid(parts[0]) && valid(parts[1])
}

fn oracle_classify(
    typed: &str,
    read: &RepoReadView,
    listed: &[u64],
    scope: &ScopeView,
) -> ImportVerdictView {
    if !oracle_shape_ok(typed) {
        return ImportVerdictView::Shape;
    }
    match read {
        RepoReadView::NotFound => ImportVerdictView::NotFound,
        RepoReadView::Unreachable => ImportVerdictView::Unreachable,
        RepoReadView::Found { owner_id, .. } if listed.contains(owner_id) => {
            ImportVerdictView::OwnedByListedAccount
        }
        RepoReadView::Found { .. } => match oracle_import(scope) {
            Ok(()) => ImportVerdictView::Admitted,
            Err(_) => ImportVerdictView::OverLimit,
        },
    }
}

/// The current ticks of an account: its saved choice, else its default ticks.
fn current_ticks(scope: &ScopeView, account: usize) -> Vec<u64> {
    let a = &scope.accounts[account];
    match &a.saved {
        Some(t) => t.clone(),
        None => oracle_default_ticks(scope)
            .into_iter()
            .find(|(id, _)| *id == a.id)
            .map(|(_, t)| t)
            .unwrap_or_default(),
    }
}

fn total_toward_limit(scope: &ScopeView) -> usize {
    oracle_s(scope) + scope.imports.len() + oracle_d_total(scope)
}

// =============================================================================
// Generators
// =============================================================================

fn arb_account(id: u64) -> impl Strategy<Value = AccountView> {
    (any::<bool>(), 0usize..12, proptest::option::of(0usize..12)).prop_map(
        move |(verified, eligible_n, saved_n)| {
            let eligible: Vec<u64> = (0..eligible_n as u64).map(|i| id * 100 + i).collect();
            let saved = saved_n.map(|n| eligible.iter().copied().take(n).collect());
            AccountView {
                id,
                verified,
                saved,
                eligible,
            }
        },
    )
}

/// Accounts are verified or not independently of having a saved choice, so
/// unverified accounts WITH saved ticks are generated (OQ-D3 is exercised by
/// CORE-RRA-1/2/3; CORE-RRA-2x pins one example).
fn arb_scope() -> impl Strategy<Value = ScopeView> {
    (0usize..=MAX_ACCOUNTS, 0usize..12).prop_flat_map(|(n, imports)| {
        let accounts: Vec<_> = (1..=n as u64).map(arb_account).collect();
        (accounts, Just(imports)).prop_map(|(accounts, imports)| ScopeView {
            accounts,
            imports: (0..imports as u64).map(|i| 9_000 + i).collect(),
        })
    })
}

fn arb_check() -> impl Strategy<Value = CheckView> {
    prop_oneof![
        Just(CheckView::Passed),
        Just(CheckView::Failed),
        Just(CheckView::Unfinished)
    ]
}

fn arb_listed() -> impl Strategy<Value = ListedView> {
    prop_oneof![
        6 => Just(ListedView::Eligible),
        1 => Just(ListedView::Fork),
        1 => Just(ListedView::Archived),
        1 => Just(ListedView::Absent),
    ]
}

const SIGNER: &str = "did:plc:7x3kq2mzv5rj4w6hbn2tqclp";

fn arb_bio() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some(format!("Builder. {SIGNER}"))),
        Just(Some(format!("{SIGNER}."))),
        Just(Some("Rust, tide models.".to_string())),
        Just(Some(format!("{SIGNER}x"))),
        Just(Some("did:plc:d3mv4lk8x2qp7ty5nw9rbcz6".to_string())),
    ]
}

fn arb_repo_read() -> impl Strategy<Value = RepoReadView> {
    prop_oneof![
        (1u64..6, any::<bool>()).prop_map(|(owner_id, owner_is_user)| RepoReadView::Found {
            full_name: format!("owner{owner_id}/repo"),
            owner_id,
            owner_is_user,
        }),
        Just(RepoReadView::NotFound),
        Just(RepoReadView::Unreachable),
    ]
}

fn arb_typed() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("rust-lang/cargo".to_string()),
        Just("cargo".to_string()),
        Just("rust-lang/".to_string()),
        Just("/cargo".to_string()),
        Just("a/b/c".to_string()),
        Just("rust lang/cargo".to_string()),
        "[a-zA-Z0-9_.-]{1,12}/[a-zA-Z0-9_.-]{1,12}",
        ".{0,20}",
    ]
}

// =============================================================================
// Properties
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// CORE-RRA-1 (@property @US-RRA-001 @US-RRA-003 @BR-RRA-3 @contract-shape:pure-function)
    #[test]
    fn core_rra1_default_ticks_fill_only_the_room_left(scope in arb_scope()) {
        let ticks = sut_default_ticks(&scope);
        prop_assert_eq!(&ticks, &oracle_default_ticks(&scope));
        if oracle_s(&scope) + scope.imports.len() <= CAP {
            let d: usize = ticks.iter().map(|(_, t)| t.len()).sum();
            prop_assert!(oracle_s(&scope) + scope.imports.len() + d <= CAP);
        }
    }

    /// CORE-RRA-2 (@property @US-RRA-001 @US-RRA-004 @AC-001.3 @AC-004.6 @C1b @contract-shape:pure-function)
    #[test]
    fn core_rra2_a_save_is_admitted_iff_the_total_fits(
        (scope, account, n) in arb_scope()
            .prop_filter("at least one account", |s| !s.accounts.is_empty())
            .prop_flat_map(|s| { let k = s.accounts.len(); (Just(s), 0..k, 0usize..13) })
    ) {
        let ticked: Vec<u64> = scope.accounts[account].eligible.iter().copied().take(n).collect();
        prop_assert_eq!(
            sut_save_admission(&scope, account, &ticked),
            oracle_save(&scope, account, &ticked)
        );
    }

    /// CORE-RRA-3 (@property @BR-RRA-3 @I-07 @contract-shape:pure-function)
    /// Admitted Saves and imports never reach a state the scan start refuses.
    #[test]
    #[ignore = "CORE-RRA-3 (bind with DELIVER step 03-02): one definition of S across Save, import and scan start (RED binding)"]
    fn core_rra3_admitted_changes_never_overfill_a_scan(
        start in arb_scope().prop_filter("start within the limit", |s| total_toward_limit(s) <= CAP),
        ops in proptest::collection::vec((any::<bool>(), 0usize..3, 0usize..12), 0..12)
    ) {
        let mut scope = start;
        for (is_import, account, n) in ops {
            if is_import {
                if sut_import_admission(&scope).is_ok() {
                    let next = 9_500 + scope.imports.len() as u64;
                    scope.imports.push(next);
                }
            } else if account < scope.accounts.len() {
                let ticked: Vec<u64> = scope.accounts[account].eligible.iter().copied().take(n).collect();
                if sut_save_admission(&scope, account, &ticked).is_ok() {
                    scope.accounts[account].saved = Some(ticked);
                }
            }
            let checks = vec![CheckView::Passed; scope.accounts.len()];
            let listings: Vec<BTreeMap<u64, ListedView>> = scope
                .accounts
                .iter()
                .map(|a| a.eligible.iter().map(|id| (*id, ListedView::Eligible)).collect())
                .collect();
            let intent = sut_scan_intent(&scope, &checks, &listings);
            prop_assert!(!matches!(intent, IntentView::OverLimit(_)), "{:?}", scope);
        }
    }

    /// CORE-RRA-4 (@property @US-RRA-002 @US-RRA-004 @KPI-RRA-5 @KPI-RRA-7 @contract-shape:pure-function)
    #[test]
    fn core_rra4_a_scan_plans_only_what_was_chosen_from_passing_accounts(
        (scope, checks, listings) in arb_scope().prop_flat_map(|s| {
            let n = s.accounts.len();
            let ids: Vec<Vec<u64>> = s.accounts.iter().map(|a| a.eligible.clone()).collect();
            let listings: Vec<_> = ids
                .into_iter()
                .map(|ids| {
                    proptest::collection::vec(arb_listed(), ids.len())
                        .prop_map(move |l| ids.iter().copied().zip(l).collect::<BTreeMap<_, _>>())
                })
                .collect();
            (Just(s), proptest::collection::vec(arb_check(), n), listings)
        })
    ) {
        let intent = sut_scan_intent(&scope, &checks, &listings);
        let s_plus_i = oracle_s(&scope) + scope.imports.len();
        // D-RRA-T1: every listed account is re-checked, unverified included,
        // so any listed account with a choice (or a default) is something to read.
        let anything = !scope.imports.is_empty()
            || scope.accounts.iter().any(|a| match &a.saved {
                Some(t) => !t.is_empty(),
                None => !a.eligible.is_empty(),
            });
        match intent {
            // Either refusal may win when both hold (open point DWD-RRA-8).
            IntentView::OverLimit(r) => prop_assert!(s_plus_i > CAP && r.limit == CAP && r.total == s_plus_i),
            IntentView::NothingToRead => prop_assert!(!anything),
            IntentView::Plan { rows, skipped } => {
                prop_assert!(anything && s_plus_i <= CAP);
                prop_assert!(rows.len() <= CAP, "KPI-RRA-7: at most the cap");
                // Default ticks follow the scan's own checks (D-RRA-T1), not the stored status.
                let mut checked = scope.clone();
                for (a, check) in checked.accounts.iter_mut().zip(&checks) {
                    a.verified = *check == CheckView::Passed;
                }
                let defaults: BTreeMap<u64, Vec<u64>> = oracle_default_ticks(&checked).into_iter().collect();
                for (i, a) in scope.accounts.iter().enumerate() {
                    let chosen = a.saved.clone().or_else(|| defaults.get(&a.id).cloned()).unwrap_or_default();
                    let planned: BTreeSet<u64> = rows.iter().filter(|(src, _)| *src == Some(a.id)).map(|(_, r)| *r).collect();
                    if checks[i] != CheckView::Passed {
                        prop_assert!(planned.is_empty(), "KPI-RRA-5: no repo of a non-passing account");
                        continue;
                    }
                    prop_assert!(planned.iter().all(|r| chosen.contains(r)), "KPI-RRA-7: only chosen repos");
                    for r in &chosen {
                        let expected = match listings[i].get(r).copied().unwrap_or(ListedView::Absent) {
                            ListedView::Eligible => None,
                            ListedView::Fork => Some(SkipView::Fork),
                            ListedView::Archived => Some(SkipView::Archived),
                            ListedView::Absent => Some(SkipView::MissingOrTransferred),
                        };
                        match expected {
                            None => prop_assert!(planned.contains(r)),
                            Some(reason) => prop_assert!(skipped.contains(&(*r, reason)), "{} skipped with {:?}", r, reason),
                        }
                    }
                }
                let imported: BTreeSet<u64> = rows.iter().filter(|(src, _)| src.is_none()).map(|(_, r)| *r).collect();
                prop_assert_eq!(imported, scope.imports.iter().copied().collect::<BTreeSet<_>>());
            }
        }
    }

    /// CORE-RRA-5 (@property @BR-RRA-4 @AC-004.7 @AC-008.4 @AC-009.4 @C7b @contract-shape:pure-function)
    #[test]
    #[ignore = "CORE-RRA-5 (bind with DELIVER step 01-10): repo_scope resume of the frozen plan (RED binding)"]
    fn core_rra5_resumes_read_only_the_frozen_plan_each_repo_once(
        plan_len in 1usize..=CAP,
        rounds in proptest::collection::vec(
            (proptest::collection::btree_set(0usize..CAP, 0..4),
             proptest::collection::btree_map(1u64..4, arb_check(), 0..3),
             proptest::collection::btree_set(1u64..5, 0..2)),
            1..4)
    ) {
        let plan: Vec<(Option<u64>, u64)> = (0..plan_len)
            .map(|i| (if i % 4 == 3 { None } else { Some(1 + (i as u64 % 3)) }, 500 + i as u64))
            .collect();
        let mut read: BTreeSet<usize> = BTreeSet::new();
        let mut removed: BTreeSet<u64> = BTreeSet::new();
        let mut skipped_ever: BTreeSet<usize> = BTreeSet::new();
        for (partial, checks, removed_now) in rounds {
            removed.extend(removed_now);
            let (to_read, skipped) = sut_resume(&plan, &read, &checks, &removed);
            for pos in &to_read {
                prop_assert!(*pos < plan.len(), "only the frozen plan");
                prop_assert!(!read.contains(pos) && !skipped_ever.contains(pos), "each at most once");
                let (src, _) = plan[*pos];
                if let Some(acc) = src {
                    prop_assert_eq!(checks.get(&acc).copied().unwrap_or(CheckView::Passed), CheckView::Passed);
                    prop_assert!(!removed.contains(&acc), "removed sources are never read");
                }
            }
            for (pos, reason) in &skipped {
                if let (Some(acc), SkipView::AccountUnverified) = (plan[*pos].0, reason) {
                    prop_assert_eq!(checks.get(&acc).copied(), Some(CheckView::Failed));
                }
                skipped_ever.insert(*pos);
            }
            for (pos, (src, _)) in plan.iter().enumerate() {
                if let Some(acc) = src {
                    if checks.get(acc) == Some(&CheckView::Unfinished) && !read.contains(&pos) {
                        prop_assert!(!skipped.iter().any(|(p, _)| *p == pos), "unfinished rows are carried");
                    }
                }
            }
            read.extend(to_read.iter().copied().filter(|p| partial.is_empty() || !partial.contains(p)));
        }
    }

    /// CORE-RRA-6 (@property @US-RRA-010 @US-RRA-006 @BR-RRA-1 @BR-RRA-2 @KPI-RRA-4 @contract-shape:pure-function)
    #[test]
    #[ignore = "CORE-RRA-6 (bind with DELIVER step 02-14): claim_domain::authority::may_sign owner tier (RED binding)"]
    fn core_rra6_only_a_listed_user_owner_whose_bio_holds_the_did_may_publish(
        listed in proptest::collection::vec(1u64..6, 0..3),
        repo in arb_repo_read(),
        bio in arb_bio(),
        profile_unreachable in any::<bool>()
    ) {
        let profile = if profile_unreachable { ProfileReadView::Unreachable } else { ProfileReadView::Bio(bio) };
        let subject = "github:owner1/repo";
        let got = sut_may_sign(SIGNER, subject, &listed, &repo, &profile);
        prop_assert_eq!(&got, &oracle_may_sign(SIGNER, subject, &listed, &repo, &profile));
        if let PermitView::Permit { signer, subject: s, .. } = got {
            prop_assert_eq!(s, subject, "the permit names the repo it was minted for");
            prop_assert_eq!(signer, SIGNER, "and the DID it was minted for");
        }
    }

    /// CORE-RRA-7 (@property @AC-004.3 @AC-004.4 @AC-002.6 @I-BRA-1 @contract-shape:pure-function)
    #[test]
    #[ignore = "CORE-RRA-7 (bind with DELIVER step 02-05): repo_scope visibility (RED binding)"]
    fn core_rra7_visibility_follows_the_source_never_the_tick(
        accounts in proptest::collection::vec((1u64..5, any::<bool>()), 0..3),
        imports in proptest::collection::vec(9_000u64..9_004, 0..3),
        rows in proptest::collection::vec(prop_oneof![
            (1u64..5).prop_map(SourceView::Account),
            (9_000u64..9_004).prop_map(SourceView::Imported)
        ], 0..12)
    ) {
        let shown = sut_visible(&rows, &accounts, &imports);
        prop_assert_eq!(shown.len(), rows.len());
        for (row, visible) in rows.iter().zip(shown) {
            let expected = match row {
                SourceView::Account(id) => accounts.iter().any(|(a, v)| a == id && *v),
                SourceView::Imported(repo) => imports.contains(repo),
            };
            prop_assert_eq!(visible, expected, "{:?}", row);
        }
    }

    /// CORE-RRA-8 (@property @US-RRA-003 @D-RRA-15 @C1b @contract-shape:pure-function)
    #[test]
    #[ignore = "CORE-RRA-8 (bind with DELIVER step 02-02): account admission (RED binding)"]
    fn core_rra8_at_most_three_accounts_one_owner_each(
        listed in proptest::collection::btree_set(1u64..8, 0..=MAX_ACCOUNTS),
        id in 1u64..8,
        other in any::<bool>(),
        unverified in any::<bool>()
    ) {
        let listed: Vec<u64> = listed.into_iter().collect();
        let got = sut_add_account(&listed, id, other, unverified);
        let expected = if listed.contains(&id) {
            if unverified { AddAccountView::Reverified } else { AddAccountView::AlreadyListed }
        } else if other {
            AddAccountView::HeldByAnotherOwner
        } else if listed.len() >= MAX_ACCOUNTS {
            AddAccountView::AtLimit
        } else {
            AddAccountView::Added
        };
        prop_assert_eq!(got, expected);
    }

    /// CORE-RRA-9 (@property @US-RRA-005 @AC-005.1 @AC-005.2 @AC-005.3 @AC-005.6 @C6a @C6c @contract-shape:pure-function)
    #[test]
    #[ignore = "CORE-RRA-9 (bind with DELIVER step 03-02): repo_scope import classification (RED binding)"]
    fn core_rra9_an_import_verdict_is_one_of_a_closed_set_in_a_fixed_order(
        typed in arb_typed(),
        read in arb_repo_read(),
        listed in proptest::collection::vec(1u64..6, 0..3),
        scope in arb_scope()
    ) {
        prop_assert_eq!(
            sut_classify_import(&typed, &read, &listed, &scope),
            oracle_classify(&typed, &read, &listed, &scope)
        );
    }

    /// CORE-RRA-10 (@property @BR-RRA-5 @AC-006.5 @contract-shape:pure-function)
    #[test]
    #[ignore = "CORE-RRA-10 (bind with DELIVER step 03-09): repo_scope BR-RRA-5 move (RED binding)"]
    fn core_rra10_a_move_keeps_the_total_and_ticks_the_repo(
        (scope, account) in arb_scope()
            .prop_filter("an import and a verified account", |s| !s.imports.is_empty() && s.accounts.iter().any(|a| a.verified))
            .prop_filter("within the limit", |s| total_toward_limit(s) <= CAP)
            .prop_flat_map(|s| {
                let verified: Vec<usize> = s.accounts.iter().enumerate().filter(|(_, a)| a.verified).map(|(i, _)| i).collect();
                (Just(s), proptest::sample::select(verified))
            })
    ) {
        let repo = scope.imports[0];
        let moved = sut_move(&scope, repo, account);
        prop_assert!(!moved.imports.contains(&repo));
        let mut expected_choice = current_ticks(&scope, account);
        expected_choice.push(repo);
        let got: BTreeSet<u64> = moved.accounts[account].saved.clone().unwrap_or_default().into_iter().collect();
        prop_assert_eq!(got, expected_choice.into_iter().collect::<BTreeSet<_>>());
        prop_assert_eq!(total_toward_limit(&moved), total_toward_limit(&scope), "the total never changes");
    }
}

/// CORE-RRA-2x (OQ-D3 example): an unverified account's 5 saved ticks still
/// count — a 6th tick on the verified account makes 11 and is refused.
#[test]
#[ignore = "CORE-RRA-2x (bind with DELIVER step 02-19): unverified saved ticks count (RED binding)"]
fn core_rra2x_an_unverified_accounts_saved_ticks_still_count() {
    let scope = ScopeView {
        accounts: vec![
            AccountView {
                id: 1,
                verified: true,
                saved: Some(vec![100, 101, 102, 103, 104]),
                eligible: (100..107).collect(),
            },
            AccountView {
                id: 2,
                verified: false,
                saved: Some(vec![200, 201, 202, 203, 204]),
                eligible: (200..205).collect(),
            },
        ],
        imports: Vec::new(),
    };
    let six: Vec<u64> = (100..106).collect();
    assert_eq!(
        sut_save_admission(&scope, 0, &six),
        Err(LimitRefusalView {
            limit: CAP,
            total: 11,
            excess: 1
        })
    );
    assert_eq!(
        oracle_save(&scope, 0, &six),
        sut_save_admission(&scope, 0, &six)
    );
}
