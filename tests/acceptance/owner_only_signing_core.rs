//! owner-only-signing — layer-2 PURE-CORE properties (ADR-007 functional
//! core; Mandate 9: PBT full at layers 1-2).
//!
//! The decisions this feature adds are total pure functions in
//! `claim_domain::authority` (ADR-086 §2-§5, ADR-087 §1-§3, §7) plus the
//! scoring choke point `scoring::score_attested` (ADR-086 §8). Their contracts
//! are stated over generated inputs and checked against ORACLES written from
//! the ADR text — never from an implementation. Properties target the
//! DERIVATIONS (`kind`, `requirement`, `classify`, `may_sign`), not only the
//! `score_attested` definition (review H-9):
//!
//! | ID | Contract | Source |
//! |---|---|---|
//! | CORE-OOS-1 | `kind`: non-weight (Retraction / Counter / IdentityLink) iff a `retracts` or `counters` reference or the predicate is exactly `sameAs`; `supersedes` / `corrects` stay Assertion; a single marker decides the exact kind | ADR-086 §2, UC-6 |
//! | CORE-OOS-2 | `requirement`: Exempt iff the subject's scheme is not `github` (case-insensitive) or the kind is non-weight; Required(Person{login}) for `github:<login>`, Required(Repo{owner,name}) for `github:<owner>/<repo>`; a malformed `github:` subject is never Exempt (fail closed) | ADR-086 §1-2, C-OOS-1, DWD-OOS-7 |
//! | CORE-OOS-3 | `classify` against the ADR-087 rule table: Attested(Owner) / Attested(RepoListed) exactly when the tiers hold (exact DID token; org never Owner; a person needs the LATEST check verified with `login_seen` = subject login); otherwise an Unattested reason from the applicable set, or NotChecked | ADR-087 §1-3, N-1, G-2 |
//! | CORE-OOS-4 | `may_sign`: permit iff Exempt, or an attested tier whose OLDER check used is ≥ 0 and ≤ 7 days old (a future-dated check is missing); a time-only refusal names `openlore authority verify --subject <s>` | ADR-086 §5, ADR-087 §7, A-6, KPI-OOS-1 |
//! | CORE-OOS-5 | `score_attested(F, V) == score(eligible(F, V))` and `score_attested(F ∪ S, V) == score_attested(F, V)` for every set S of non-weight rows (incl. spoofed `counters`) and unattested `github:` assertions — unbounded preservation | ADR-086 §8, KPI-OOS-2, KPI-GDL-2 |
//!
//! ## Binding seam (Mandate 7)
//!
//! Every property calls a `sut_*` binding that makes ONE call into the
//! production function and maps its ADTs onto the view types here (the view
//! types are the observable contract). Every binding is live: `kind`,
//! `requirement`, `classify` and the CLI gates `may_sign_from_checks` /
//! `may_sign_repo_from_checks` in `claim_domain::authority`, and
//! `score_attested` in `scoring` (the shipped `claim_domain::authority::may_sign`
//! is the review app's same-request owner gate, ADR-089 §2, with different
//! inputs). The oracle's right-hand side of CORE-OOS-5 calls the REAL
//! `scoring::score`.
//!
//! ## Layer-2 CM-A exception (Mandate 1, documented)
//!
//! These properties call the pure core directly, not a driving port. That is
//! the documented exception for layer 2 (Mandate 9: PBT full at layers 1-2;
//! ADR-007: `claim_domain::authority` and `scoring` are the functional core's
//! public API, with no I/O to wire). Mandate 1 (driving ports only) is
//! enforced at layer 3: every behaviour here is also pinned through the
//! `openlore` binary (CORE-OOS-4 ↔ PS-4/RA-10/RA-12b, CORE-OOS-3 ↔ RA-3/RD-1,
//! CORE-OOS-5 ↔ RD-2/RD-4). CM-A evidence lists this file separately.
//!
//! Every property carries `@contract-shape:pure-function` (review M-3).
//!
//! Tier B (state-machine PBT) is not added: `classify` / `may_sign` are pure
//! projections of (claim, latest checks, now) — a FORALL over generated
//! states explores the same space without a command protocol; the
//! observation-order journey (RA-9) is a layer-3 example (DWD-OOS-9).

#![allow(dead_code)]

use std::collections::BTreeSet;

use chrono::{TimeZone, Utc};
use claim_domain::{Cid, Did};
use ports::{AttributedClaim, AuthorRelationship};
use proptest::prelude::*;
use scoring::ScoringConfig;

const SAME_AS: &str = "sameAs";
const WINDOW_MINUTES: i64 = 7 * 24 * 60;

// =============================================================================
// Observable view types (the contract)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RefView {
    Retracts,
    Counters,
    Supersedes,
    Corrects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindView {
    Assertion,
    Retraction,
    Counter,
    IdentityLink,
}

/// The claim fields the derivations read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaimView {
    author: String,
    subject: String,
    predicate: String,
    references: Vec<RefView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RequirementView {
    Exempt,
    Person {
        login: String,
    },
    Repo {
        owner: String,
        name: String,
    },
    /// A `github:` assertion naming no account or repo: needs authority no
    /// one can hold (fail closed, DWD-OOS-7).
    Malformed,
}

/// One identity link of a DID to a GitHub account (the login it was linked
/// under) — ADR-084.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkView {
    did: String,
    github_id: u64,
    linked_login: String,
}

/// This reader's LATEST check of a (DID, id) link; `age_minutes` < 0 means
/// dated in the future.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkCheckView {
    did: String,
    github_id: u64,
    login_seen: String,
    verified: bool,
    age_minutes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileView {
    Absent,
    Present(Vec<String>),
    Invalid,
}

/// This reader's LATEST observation of the repo (signer-independent: no
/// field names who recorded it — review H-2).
#[derive(Debug, Clone, PartialEq, Eq)]
enum RepoObsView {
    Found {
        owner_id: u64,
        organization: bool,
        file: FileView,
        age_minutes: i64,
    },
    NotFound {
        age_minutes: i64,
    },
}

/// Everything local the reader holds for one decision.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EvidenceView {
    links: Vec<LinkView>,
    checks: Vec<LinkCheckView>,
    repo: Option<RepoObsView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ReasonView {
    NoLink,
    LinkNotVerified,
    LoginNotCurrent,
    NotRepoOwner,
    OrgOwnedNotListed,
    NotListed,
    FileInvalid,
    RepoNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AuthorityView {
    Owner { github_id: u64 },
    RepoListed,
    Unattested(ReasonView),
    NotChecked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PermitView {
    Permit,
    Refused { next_command: Option<String> },
}

/// One scoring-feed row: the claim as scored today plus the reference types
/// the feed gains (UC-20) and this reader's attested decision for it.
#[derive(Debug, Clone)]
struct FeedRow {
    claim: AttributedClaim,
    references: Vec<RefView>,
    attested: bool,
}

// =============================================================================
// SUT bindings (RED until DELIVER binds each to ONE production call)
// =============================================================================

fn sut_kind(claim: &ClaimView) -> KindView {
    use claim_domain::authority::{kind, ClaimKind, ClaimShape};
    use claim_domain::ReferenceType;
    let reference_types: Vec<ReferenceType> = claim
        .references
        .iter()
        .map(|r| match r {
            RefView::Retracts => ReferenceType::Retracts,
            RefView::Counters => ReferenceType::Counters,
            RefView::Supersedes => ReferenceType::Supersedes,
            RefView::Corrects => ReferenceType::Corrects,
        })
        .collect();
    match kind(ClaimShape {
        subject: &claim.subject,
        predicate: &claim.predicate,
        reference_types: &reference_types,
    }) {
        ClaimKind::Assertion => KindView::Assertion,
        ClaimKind::Retraction => KindView::Retraction,
        ClaimKind::Counter => KindView::Counter,
        ClaimKind::IdentityLink => KindView::IdentityLink,
    }
}

fn sut_requirement(claim: &ClaimView) -> RequirementView {
    use claim_domain::authority::{requirement, ClaimShape, Requirement};
    use claim_domain::ReferenceType;
    let reference_types: Vec<ReferenceType> = claim
        .references
        .iter()
        .map(|r| match r {
            RefView::Retracts => ReferenceType::Retracts,
            RefView::Counters => ReferenceType::Counters,
            RefView::Supersedes => ReferenceType::Supersedes,
            RefView::Corrects => ReferenceType::Corrects,
        })
        .collect();
    match requirement(ClaimShape {
        subject: &claim.subject,
        predicate: &claim.predicate,
        reference_types: &reference_types,
    }) {
        Requirement::Exempt => RequirementView::Exempt,
        Requirement::Person(person) => RequirementView::Person {
            login: person.login().to_string(),
        },
        Requirement::Repo { owner, name } => RequirementView::Repo { owner, name },
        Requirement::Malformed => RequirementView::Malformed,
    }
}

fn sut_classify(claim: &ClaimView, evidence: &EvidenceView) -> AuthorityView {
    use claim_domain::authority::{
        classify, AttestedTier, Authority, AuthorityEvidence, AuthorityFileSeen, ClaimShape,
        LatestLinkCheck, LatestRepoObservation, LinkedAccountEvidence, RepoFinding, RepoOwnerKind,
        Unattested,
    };
    use claim_domain::identity_link::CheckVerdict;
    use claim_domain::ReferenceType;
    let now = Utc.with_ymd_and_hms(2026, 10, 8, 9, 0, 0).unwrap();
    let at = |age_minutes: i64| now - chrono::Duration::minutes(age_minutes);
    // The DuckDB read port's join: the author's links, each with its latest check.
    let links: Vec<LinkedAccountEvidence> = evidence
        .links
        .iter()
        .filter(|link| link.did == claim.author)
        .map(|link| LinkedAccountEvidence {
            github_id: link.github_id,
            linked_login: link.linked_login.clone(),
            latest_check: evidence
                .checks
                .iter()
                .find(|c| c.did == link.did && c.github_id == link.github_id)
                .map(|c| LatestLinkCheck {
                    login_seen: c.login_seen.clone(),
                    verdict: if c.verified {
                        CheckVerdict::Verified
                    } else {
                        CheckVerdict::DidMissing
                    },
                    checked_at: at(c.age_minutes),
                }),
        })
        .collect();
    let repo = evidence.repo.as_ref().map(|observed| match observed {
        RepoObsView::Found {
            owner_id,
            organization,
            file,
            age_minutes,
        } => LatestRepoObservation {
            finding: RepoFinding::Found {
                owner_id: *owner_id,
                owner_kind: if *organization {
                    RepoOwnerKind::Organization
                } else {
                    RepoOwnerKind::User
                },
                authority_file: match file {
                    FileView::Absent => AuthorityFileSeen::Absent,
                    FileView::Present(dids) => AuthorityFileSeen::Present(dids.clone()),
                    FileView::Invalid => AuthorityFileSeen::Invalid,
                },
            },
            checked_at: at(*age_minutes),
        },
        RepoObsView::NotFound { age_minutes } => LatestRepoObservation {
            finding: RepoFinding::NotFound,
            checked_at: at(*age_minutes),
        },
    });
    let reference_types: Vec<ReferenceType> = claim
        .references
        .iter()
        .map(|r| match r {
            RefView::Retracts => ReferenceType::Retracts,
            RefView::Counters => ReferenceType::Counters,
            RefView::Supersedes => ReferenceType::Supersedes,
            RefView::Corrects => ReferenceType::Corrects,
        })
        .collect();
    let classified = classify(
        ClaimShape {
            subject: &claim.subject,
            predicate: &claim.predicate,
            reference_types: &reference_types,
        },
        &AuthorityEvidence {
            author: &claim.author,
            author_links: &links,
            repo: repo.as_ref(),
        },
        now,
    );
    match classified {
        Some(Authority::Attested(AttestedTier::Owner { github_id })) => {
            AuthorityView::Owner { github_id }
        }
        Some(Authority::Attested(AttestedTier::RepoListed)) => AuthorityView::RepoListed,
        Some(Authority::Unattested(reason)) => AuthorityView::Unattested(match reason {
            Unattested::NoLink => ReasonView::NoLink,
            Unattested::LinkNotVerified => ReasonView::LinkNotVerified,
            Unattested::LoginNotCurrent => ReasonView::LoginNotCurrent,
            Unattested::NotRepoOwner => ReasonView::NotRepoOwner,
            Unattested::OrgOwnedNotListed => ReasonView::OrgOwnedNotListed,
            Unattested::FileInvalid => ReasonView::FileInvalid,
            Unattested::RepoNotFound => ReasonView::RepoNotFound,
        }),
        Some(Authority::NotChecked) => AuthorityView::NotChecked,
        None => panic!("a github: assertion is always classified: {claim:?}"),
    }
}

/// The CLI gate from local checks (ADR-086 §4-5). NOT the shipped review-app
/// `may_sign` (ADR-089 §2, same-request reads) — see U-OOS-2.
fn sut_may_sign(claim: &ClaimView, evidence: &EvidenceView, _window_minutes: i64) -> PermitView {
    use claim_domain::authority::{
        may_sign_from_checks, may_sign_repo_from_checks, requirement, AuthorityFileSeen,
        AuthorityRefusal, ClaimShape, LatestLinkCheck, LatestRepoObservation,
        LinkedAccountEvidence, RepoFinding, RepoOwnerKind, Requirement,
    };
    use claim_domain::identity_link::CheckVerdict;
    use claim_domain::ReferenceType;
    let now = Utc.with_ymd_and_hms(2026, 10, 8, 9, 0, 0).unwrap();
    // The DuckDB read port's join: the author's links, each with its latest check.
    let links: Vec<LinkedAccountEvidence> = evidence
        .links
        .iter()
        .filter(|link| link.did == claim.author)
        .map(|link| LinkedAccountEvidence {
            github_id: link.github_id,
            linked_login: link.linked_login.clone(),
            latest_check: evidence
                .checks
                .iter()
                .find(|c| c.did == link.did && c.github_id == link.github_id)
                .map(|c| LatestLinkCheck {
                    login_seen: c.login_seen.clone(),
                    verdict: if c.verified {
                        CheckVerdict::Verified
                    } else {
                        CheckVerdict::DidMissing
                    },
                    checked_at: now - chrono::Duration::minutes(c.age_minutes),
                }),
        })
        .collect();
    let reference_types: Vec<ReferenceType> = claim
        .references
        .iter()
        .map(|r| match r {
            RefView::Retracts => ReferenceType::Retracts,
            RefView::Counters => ReferenceType::Counters,
            RefView::Supersedes => ReferenceType::Supersedes,
            RefView::Corrects => ReferenceType::Corrects,
        })
        .collect();
    let shape = ClaimShape {
        subject: &claim.subject,
        predicate: &claim.predicate,
        reference_types: &reference_types,
    };
    let at = |age_minutes: i64| now - chrono::Duration::minutes(age_minutes);
    // The repo read port: this reader's latest observation of the repo.
    let repo = evidence.repo.as_ref().map(|observed| match observed {
        RepoObsView::Found {
            owner_id,
            organization,
            file,
            age_minutes,
        } => LatestRepoObservation {
            finding: RepoFinding::Found {
                owner_id: *owner_id,
                owner_kind: if *organization {
                    RepoOwnerKind::Organization
                } else {
                    RepoOwnerKind::User
                },
                authority_file: match file {
                    FileView::Absent => AuthorityFileSeen::Absent,
                    FileView::Present(dids) => AuthorityFileSeen::Present(dids.clone()),
                    FileView::Invalid => AuthorityFileSeen::Invalid,
                },
            },
            checked_at: at(*age_minutes),
        },
        RepoObsView::NotFound { age_minutes } => LatestRepoObservation {
            finding: RepoFinding::NotFound,
            checked_at: at(*age_minutes),
        },
    });
    let decided = match requirement(shape) {
        Requirement::Person(person) => may_sign_from_checks(&claim.author, &person, &links, now),
        Requirement::Repo { .. } => {
            may_sign_repo_from_checks(&claim.author, &claim.subject, &links, repo.as_ref(), now)
        }
        Requirement::Exempt => return PermitView::Permit,
        Requirement::Malformed => return PermitView::Refused { next_command: None },
    };
    match decided {
        Ok(_) => PermitView::Permit,
        Err(AuthorityRefusal::Recheck { subject, .. }) => PermitView::Refused {
            next_command: Some(format!("openlore authority verify --subject {subject}")),
        },
        Err(AuthorityRefusal::Unattested { .. }) => PermitView::Refused { next_command: None },
    }
}

fn sut_score_attested(feed: &[FeedRow]) -> scoring::WeightedView {
    use claim_domain::authority::{AttestedTier, Authority, Unattested};
    use claim_domain::ReferenceType;
    use ports::ScoringFeedRow;
    let reference_types = |row: &FeedRow| -> Vec<ReferenceType> {
        row.references
            .iter()
            .map(|r| match r {
                RefView::Retracts => ReferenceType::Retracts,
                RefView::Counters => ReferenceType::Counters,
                RefView::Supersedes => ReferenceType::Supersedes,
                RefView::Corrects => ReferenceType::Corrects,
            })
            .collect()
    };
    let rows: Vec<ScoringFeedRow> = feed
        .iter()
        .map(|row| ScoringFeedRow {
            claim: row.claim.clone(),
            reference_types: reference_types(row),
            withdrawn_by_its_author: false,
        })
        .collect();
    // This reader's decision for each row, as the generated feed states it.
    let authority_of = |scored: &ScoringFeedRow| {
        feed.iter()
            .find(|row| row.claim == scored.claim && reference_types(row) == scored.reference_types)
            .map(|row| match row.attested {
                true => Authority::Attested(AttestedTier::RepoListed),
                false => Authority::Unattested(Unattested::NotRepoOwner),
            })
    };
    scoring::score_attested(&rows, authority_of, &ScoringConfig::DEFAULT)
}

/// The REAL scorer, for the oracle side of CORE-OOS-5.
fn real_score(claims: &[AttributedClaim]) -> scoring::WeightedView {
    scoring::score(claims, &ScoringConfig::DEFAULT)
}

// =============================================================================
// Oracles (from the ADR text)
// =============================================================================

fn oracle_is_non_weight(c: &ClaimView) -> bool {
    c.references
        .iter()
        .any(|r| matches!(r, RefView::Retracts | RefView::Counters))
        || c.predicate == SAME_AS
}

/// The exact kind when exactly one marker is present (precedence between
/// markers is unspecified upstream — U-OOS-5 — so not asserted).
fn oracle_single_marker_kind(c: &ClaimView) -> Option<KindView> {
    let retracts = c.references.contains(&RefView::Retracts);
    let counters = c.references.contains(&RefView::Counters);
    let same_as = c.predicate == SAME_AS;
    match (retracts, counters, same_as) {
        (false, false, false) => Some(KindView::Assertion),
        (true, false, false) => Some(KindView::Retraction),
        (false, true, false) => Some(KindView::Counter),
        (false, false, true) => Some(KindView::IdentityLink),
        _ => None,
    }
}

fn is_login(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 39
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

fn is_repo_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && s != "."
        && s != ".."
}

/// `Some(rest)` when the subject's scheme is `github` (any letter case).
fn github_rest(subject: &str) -> Option<&str> {
    let (scheme, rest) = subject.split_once(':')?;
    scheme.eq_ignore_ascii_case("github").then_some(rest)
}

fn oracle_requirement(c: &ClaimView) -> Option<RequirementView> {
    if oracle_is_non_weight(c) {
        return Some(RequirementView::Exempt);
    }
    let Some(rest) = github_rest(&c.subject) else {
        return Some(RequirementView::Exempt);
    };
    match rest.split('/').collect::<Vec<_>>().as_slice() {
        [login] if is_login(login) => Some(RequirementView::Person {
            login: login.to_string(),
        }),
        [owner, name] if is_login(owner) && is_repo_name(name) => Some(RequirementView::Repo {
            owner: owner.to_string(),
            name: name.to_string(),
        }),
        // Malformed github: subject — never Exempt, shape left to DELIVER.
        _ => None,
    }
}

fn latest_check<'a>(ev: &'a EvidenceView, did: &str, id: u64) -> Option<&'a LinkCheckView> {
    ev.checks
        .iter()
        .find(|c| c.did == did && c.github_id == id && c.age_minutes >= 0)
}

/// What the ADR-087 table says: the attested tiers that hold (time ignored —
/// age never downgrades a label) and the applicable unattested reasons.
#[derive(Debug, Default)]
struct Expected {
    owner_ids: BTreeSet<u64>,
    listed: bool,
    reasons: BTreeSet<ReasonView>,
    not_checked: bool,
}

fn oracle_person(author: &str, login: &str, ev: &EvidenceView) -> Expected {
    let mut e = Expected::default();
    let mine: Vec<&LinkView> = ev.links.iter().filter(|l| l.did == author).collect();
    let mut related = false;
    for link in &mine {
        let check = latest_check(ev, author, link.github_id);
        let names_login = link.linked_login.eq_ignore_ascii_case(login)
            || check.is_some_and(|c| c.login_seen.eq_ignore_ascii_case(login));
        if !names_login {
            continue;
        }
        related = true;
        match check {
            Some(c) if c.verified && c.login_seen.eq_ignore_ascii_case(login) => {
                e.owner_ids.insert(link.github_id);
            }
            Some(c) if c.verified => {
                e.reasons.insert(ReasonView::LoginNotCurrent);
            }
            Some(_) => {
                e.reasons.insert(ReasonView::LinkNotVerified);
            }
            None => e.not_checked = true,
        }
    }
    // EXPECTATION change (DELIVER review D27): a reader holding no identity
    // link of the author at all has not checked their authority (was: NoLink,
    // read "third-party"); an author with links, none naming the account,
    // still reads NoLink. The gate oracle reads only `owner_ids`.
    if mine.is_empty() {
        e.not_checked = true;
    } else if !related {
        e.reasons.insert(ReasonView::NoLink);
    }
    e
}

fn oracle_repo(author: &str, ev: &EvidenceView) -> Expected {
    let mut e = Expected::default();
    match &ev.repo {
        None => e.not_checked = true,
        Some(RepoObsView::NotFound { .. }) => {
            e.reasons.insert(ReasonView::RepoNotFound);
        }
        Some(RepoObsView::Found {
            owner_id,
            organization,
            file,
            ..
        }) => {
            let linked_to_owner = ev
                .links
                .iter()
                .any(|l| l.did == author && l.github_id == *owner_id);
            let owner_check = latest_check(ev, author, *owner_id);
            if !organization && linked_to_owner && owner_check.is_some_and(|c| c.verified) {
                e.owner_ids.insert(*owner_id);
            }
            match file {
                FileView::Present(dids) if dids.iter().any(|d| d == author) => e.listed = true,
                FileView::Present(_) => {
                    e.reasons.insert(ReasonView::NotListed);
                }
                FileView::Invalid => {
                    e.reasons.insert(ReasonView::FileInvalid);
                }
                FileView::Absent => {}
            }
            if *organization {
                e.reasons.insert(ReasonView::OrgOwnedNotListed);
            } else {
                e.reasons.insert(ReasonView::NotRepoOwner);
                if linked_to_owner {
                    match owner_check {
                        Some(c) if !c.verified => {
                            e.reasons.insert(ReasonView::LinkNotVerified);
                        }
                        None => e.not_checked = true,
                        _ => {}
                    }
                }
            }
        }
    }
    e
}

/// CORE-OOS-4 oracle — the ADR-086 §5 / ADR-087 §7 signing-gate decision
/// table, one variant per row (review M-5). "Fresh" = `0 ≤ age ≤ 7 days`; a
/// check dated in the future is missing (DWD-OOS-14); age never changes the
/// tier, only the gate.
///
/// | Row | Requirement (ADR-086 §1-2) | Tier that holds (ADR-087 §1-3) | Checks it rests on | Gate |
/// |---|---|---|---|---|
/// | G1 | Exempt (not `github:`, or a dispute / withdrawal / link) | — | — | permit |
/// | G2 | malformed `github:` subject | — | — | refuse (fail closed) |
/// | G3 | Person | Owner(X) | latest (D, X) link check, fresh | permit |
/// | G4 | Person | Owner(X) | latest (D, X) link check, > 7 days | refuse, names `authority verify` |
/// | G5 | Repo | RepoListed | latest repo observation, fresh | permit |
/// | G6 | Repo | Owner(X) | latest repo observation AND latest (D, X) link check, the older fresh | permit |
/// | G7 | Repo | RepoListed and/or Owner | none of the above fresh | refuse, names `authority verify` |
/// | G8 | Person / Repo | none (Unattested / NotChecked) | — | refuse with the hand-off |
///
/// CORE-OOS-4x pins G3 (0 and 7 days) and G4 (7 days + 1 minute), and a
/// future-dated check (G8: missing) by example.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateRow {
    G1Exempt,
    G2Malformed,
    G3PersonFresh,
    G4PersonStale,
    G5ListedFresh,
    G6OwnerFresh,
    G7RepoStale,
    G8Unattested,
}

impl GateRow {
    fn permits(self) -> bool {
        matches!(
            self,
            GateRow::G1Exempt
                | GateRow::G3PersonFresh
                | GateRow::G5ListedFresh
                | GateRow::G6OwnerFresh
        )
    }
}

fn is_fresh(age_minutes: i64) -> bool {
    (0..=WINDOW_MINUTES).contains(&age_minutes)
}

fn oracle_gate_row(claim: &ClaimView, ev: &EvidenceView) -> GateRow {
    match oracle_requirement(claim) {
        Some(RequirementView::Exempt) => GateRow::G1Exempt,
        None | Some(RequirementView::Malformed) => GateRow::G2Malformed,
        Some(RequirementView::Person { login }) => {
            let e = oracle_person(&claim.author, &login, ev);
            if e.owner_ids.is_empty() {
                return GateRow::G8Unattested;
            }
            let fresh = e.owner_ids.iter().any(|id| {
                latest_check(ev, &claim.author, *id)
                    .is_some_and(|c| c.verified && is_fresh(c.age_minutes))
            });
            if fresh {
                GateRow::G3PersonFresh
            } else {
                GateRow::G4PersonStale
            }
        }
        Some(RequirementView::Repo { .. }) => {
            let e = oracle_repo(&claim.author, ev);
            if e.owner_ids.is_empty() && !e.listed {
                return GateRow::G8Unattested;
            }
            let repo_age = match &ev.repo {
                Some(RepoObsView::Found { age_minutes, .. }) => *age_minutes,
                _ => -1,
            };
            if e.listed && is_fresh(repo_age) {
                return GateRow::G5ListedFresh;
            }
            let owner_fresh = is_fresh(repo_age)
                && e.owner_ids.iter().any(|id| {
                    latest_check(ev, &claim.author, *id).is_some_and(|c| {
                        is_fresh(c.age_minutes) && is_fresh(c.age_minutes.max(repo_age))
                    })
                });
            if owner_fresh {
                GateRow::G6OwnerFresh
            } else {
                GateRow::G7RepoStale
            }
        }
    }
}

// =============================================================================
// Generators
// =============================================================================

const AUTHORS: [&str; 3] = [
    "did:plc:test-jeff",
    "did:plc:rachel-test",
    "did:plc:tobias-test",
];

fn arb_did_token(author: &'static str) -> impl Strategy<Value = String> {
    // The exact DID and its lies (ADR-087 lie catalogue): uppercase, prefix
    // collision, trailing punctuation, another DID.
    prop_oneof![
        3 => Just(author.to_string()),
        1 => Just(author.to_ascii_uppercase()),
        1 => Just(format!("{author}rey")),
        1 => Just(format!("{author}.")),
        1 => prop::sample::select(AUTHORS.to_vec()).prop_map(str::to_string),
    ]
}

fn arb_refs() -> impl Strategy<Value = Vec<RefView>> {
    prop::collection::btree_set(
        prop_oneof![
            Just(RefView::Retracts),
            Just(RefView::Counters),
            Just(RefView::Supersedes),
            Just(RefView::Corrects)
        ],
        0..3,
    )
    .prop_map(|s| s.into_iter().collect())
}

fn arb_predicate() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => Just("embodiesPhilosophy".to_string()),
        2 => Just("adheresToPhilosophy".to_string()),
        2 => Just(SAME_AS.to_string()),
        1 => Just("SameAs".to_string()),
        1 => Just("sameAs ".to_string()),
    ]
}

fn arb_subject() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => Just("github:jeffbailey".to_string()),
        3 => Just("github:jeffbailey/tidepool".to_string()),
        2 => Just("github:acme-tools/anvil".to_string()),
        1 => Just("GitHub:JeffBailey".to_string()),
        1 => Just("github:".to_string()),
        1 => Just("github:/tidepool".to_string()),
        1 => Just("github:a/b/c".to_string()),
        1 => Just("github:jeff bailey".to_string()),
        1 => Just("https://crates.io/crates/serde".to_string()),
        1 => Just("did:plc:test-jeff".to_string()),
        1 => Just("npm:left-pad".to_string()),
    ]
}

fn arb_claim() -> impl Strategy<Value = ClaimView> {
    (
        prop::sample::select(AUTHORS.to_vec()),
        arb_subject(),
        arb_predicate(),
        arb_refs(),
    )
        .prop_map(|(author, subject, predicate, references)| ClaimView {
            author: author.to_string(),
            subject,
            predicate,
            references,
        })
}

/// A `github:` assertion (what `classify` decides).
fn arb_github_assertion() -> impl Strategy<Value = ClaimView> {
    (
        prop::sample::select(AUTHORS.to_vec()),
        prop_oneof![
            Just("github:jeffbailey".to_string()),
            Just("github:jeff-b".to_string()),
            Just("github:rachelk".to_string()),
            Just("github:jeffbailey/tidepool".to_string()),
            Just("github:acme-tools/anvil".to_string()),
        ],
        prop_oneof![Just(vec![]), Just(vec![RefView::Supersedes])],
    )
        .prop_map(|(author, subject, references)| ClaimView {
            author: author.to_string(),
            predicate: if subject.contains('/') {
                "embodiesPhilosophy".to_string()
            } else {
                "adheresToPhilosophy".to_string()
            },
            subject,
            references,
        })
}

const IDS: [u64; 3] = [1_234_567, 2_345_678, 8_200_001];
const LOGINS: [&str; 4] = ["jeffbailey", "jeff-b", "rachelk", "acme-tools"];

fn arb_age(window_heavy: bool) -> impl Strategy<Value = i64> {
    if window_heavy {
        prop_oneof![
            Just(0_i64),
            Just(WINDOW_MINUTES),
            Just(WINDOW_MINUTES + 1),
            Just(-1_i64),
            -60_i64..=20_000,
        ]
        .boxed()
    } else {
        (0_i64..=20_000).boxed()
    }
}

fn arb_evidence(window_heavy: bool) -> impl Strategy<Value = EvidenceView> {
    let link = (
        prop::sample::select(AUTHORS.to_vec()),
        prop::sample::select(IDS.to_vec()),
        prop::sample::select(LOGINS.to_vec()),
    )
        .prop_map(|(did, github_id, login)| LinkView {
            did: did.to_string(),
            github_id,
            linked_login: login.to_string(),
        });
    let check = (
        prop::sample::select(AUTHORS.to_vec()),
        prop::sample::select(IDS.to_vec()),
        prop::sample::select(LOGINS.to_vec()),
        any::<bool>(),
        arb_age(window_heavy),
    )
        .prop_map(
            |(did, github_id, login, verified, age_minutes)| LinkCheckView {
                did: did.to_string(),
                github_id,
                login_seen: login.to_string(),
                verified,
                age_minutes,
            },
        );
    let file = prop_oneof![
        Just(FileView::Absent),
        Just(FileView::Invalid),
        prop::collection::vec(arb_did_token(AUTHORS[0]), 0..3).prop_map(FileView::Present),
        prop::collection::vec(arb_did_token(AUTHORS[1]), 0..3).prop_map(FileView::Present),
    ];
    let repo = prop::option::of(prop_oneof![
        4 => (prop::sample::select(IDS.to_vec()), any::<bool>(), file, arb_age(window_heavy))
            .prop_map(|(owner_id, organization, file, age_minutes)| RepoObsView::Found {
                owner_id,
                organization,
                file,
                age_minutes,
            }),
        1 => arb_age(window_heavy).prop_map(|age_minutes| RepoObsView::NotFound { age_minutes }),
    ]);
    (
        prop::collection::vec(link, 0..3),
        prop::collection::vec(check, 0..4),
        repo,
    )
        .prop_map(|(links, mut checks, repo)| {
            // The read port returns the LATEST check per (DID, id): keep one.
            let mut seen = BTreeSet::new();
            checks.retain(|c| seen.insert((c.did.clone(), c.github_id)));
            EvidenceView {
                links,
                checks,
                repo,
            }
        })
}

fn arb_feed_claim(github_share: bool) -> impl Strategy<Value = AttributedClaim> {
    let subject = if github_share {
        prop_oneof![
            Just("github:jeffbailey/tidepool"),
            Just("github:acme-tools/anvil"),
            Just("github:rachelk"),
        ]
        .boxed()
    } else {
        prop_oneof![
            Just("github:jeffbailey/tidepool"),
            Just("github:acme-tools/anvil"),
            Just("https://crates.io/crates/serde"),
        ]
        .boxed()
    };
    (
        subject,
        prop_oneof![Just("dependency-pinning"), Just("memory-safety")],
        prop::sample::select(AUTHORS.to_vec()),
        0.0_f64..=1.0,
        0u32..1_000_000,
    )
        .prop_map(|(s, o, a, c, n)| AttributedClaim {
            author_did: Did(a.to_string()),
            cid: Cid(format!("bafy-{n}-{s}-{o}-{a}")),
            subject: s.to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: o.to_string(),
            confidence: c,
            composed_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            relationship: AuthorRelationship::SubscribedPeer,
        })
}

fn arb_feed() -> impl Strategy<Value = Vec<FeedRow>> {
    prop::collection::vec(
        (arb_feed_claim(false), arb_refs(), any::<bool>()).prop_map(
            |(claim, references, attested)| FeedRow {
                claim,
                references,
                attested,
            },
        ),
        0..10,
    )
}

/// Rows that must never move the weight: non-weight kinds (counters incl.
/// spoofed ones naming no known claim, retractions, `sameAs` links) and
/// unattested `github:` assertions — on the SAME pairings as the feed.
fn arb_weightless_rows() -> impl Strategy<Value = Vec<FeedRow>> {
    let row = (
        arb_feed_claim(true),
        prop_oneof![
            Just((vec![RefView::Counters], false)),
            Just((vec![RefView::Retracts], false)),
            Just((vec![RefView::Counters, RefView::Supersedes], true)),
            Just((vec![], false)),
        ],
        any::<bool>(),
    )
        .prop_map(|(mut claim, (references, attested), as_link)| {
            if as_link && references.is_empty() {
                claim.predicate = SAME_AS.to_string();
            }
            FeedRow {
                attested: attested && !references.is_empty(),
                claim,
                references,
            }
        })
        // An attested plain assertion would legitimately move the weight.
        .prop_filter("weightless", |r| {
            !(r.references.is_empty() && r.claim.predicate != SAME_AS && r.attested)
        });
    prop::collection::vec(row, 1..5)
}

fn eligible(feed: &[FeedRow]) -> Vec<AttributedClaim> {
    feed.iter()
        .filter(|r| {
            let view = ClaimView {
                author: r.claim.author_did.0.clone(),
                subject: r.claim.subject.clone(),
                predicate: r.claim.predicate.clone(),
                references: r.references.clone(),
            };
            !oracle_is_non_weight(&view) && (github_rest(&r.claim.subject).is_none() || r.attested)
        })
        .map(|r| r.claim.clone())
        .collect()
}

// =============================================================================
// Properties
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// CORE-OOS-1 — a dispute, a withdrawal or an identity link is never an
    /// assertion, and nothing else is anything but an assertion.
    /// `@property @s-oos-2 @contract-shape:pure-function`
    #[test]
    fn only_disputes_withdrawals_and_links_are_weightless_kinds(claim in arb_claim()) {
        let kind = sut_kind(&claim);
        prop_assert_eq!(kind != KindView::Assertion, oracle_is_non_weight(&claim), "{:?}", claim);
        if let Some(exact) = oracle_single_marker_kind(&claim) {
            prop_assert_eq!(kind, exact, "{:?}", claim);
        }
    }

    /// CORE-OOS-2 — only GitHub assertions need authority, and a malformed
    /// GitHub subject never slips through as exempt.
    /// `@property @s-oos-1 @s-oos-2 @contract-shape:pure-function`
    #[test]
    fn only_github_assertions_need_authority(claim in arb_claim()) {
        let got = sut_requirement(&claim);
        match oracle_requirement(&claim) {
            Some(expected) => prop_assert_eq!(got, expected, "{:?}", claim),
            None => prop_assert_eq!(got, RequirementView::Malformed, "fail closed: {:?}", claim),
        }
    }

    /// CORE-OOS-3 — a claim reads as its author's word about a GitHub
    /// subject exactly when the ADR-087 table says so.
    /// `@property @s-oos-1 @s-oos-4 @contract-shape:pure-function`
    #[test]
    fn a_claim_is_attested_exactly_when_the_authority_table_says_so(
        claim in arb_github_assertion(),
        evidence in arb_evidence(false),
    ) {
        let expected = match github_rest(&claim.subject).unwrap_or_default().split_once('/') {
            None => oracle_person(&claim.author, github_rest(&claim.subject).unwrap_or_default(), &evidence),
            Some(_) => oracle_repo(&claim.author, &evidence),
        };
        let got = sut_classify(&claim, &evidence);
        let attested = !expected.owner_ids.is_empty() || expected.listed;
        match &got {
            AuthorityView::Owner { github_id } => {
                prop_assert!(expected.owner_ids.contains(github_id), "{:?} {:?} {:?}", claim, evidence, got)
            }
            AuthorityView::RepoListed => prop_assert!(expected.listed, "{:?} {:?}", claim, evidence),
            AuthorityView::Unattested(reason) => {
                prop_assert!(!attested, "attested but {:?}: {:?} {:?}", reason, claim, evidence);
                prop_assert!(expected.reasons.contains(reason), "{:?} not in {:?}: {:?} {:?}", reason, expected.reasons, claim, evidence);
            }
            AuthorityView::NotChecked => {
                prop_assert!(!attested && expected.not_checked, "{:?} {:?}", claim, evidence)
            }
        }
    }

    /// CORE-OOS-4 — the signing gate permits only an exempt claim or an
    /// attested one whose older check is at most 7 days old: the oracle is
    /// the ADR-086 §5 / ADR-087 §7 decision table [`GateRow`] (review M-5),
    /// anchored by CORE-OOS-4x.
    /// `@property @s-oos-1 @s-oos-3 @contract-shape:pure-function`
    #[test]
    fn the_gate_permits_only_exempt_or_freshly_attested_claims(
        claim in arb_claim(),
        evidence in arb_evidence(true),
    ) {
        let row = oracle_gate_row(&claim, &evidence);
        let got = sut_may_sign(&claim, &evidence, WINDOW_MINUTES);
        match &got {
            PermitView::Permit => prop_assert!(row.permits(), "{:?} permits nothing: {:?} {:?}", row, claim, evidence),
            PermitView::Refused { .. } => prop_assert!(!row.permits(), "{:?} must permit: {:?} {:?}", row, claim, evidence),
        }
    }

    /// CORE-OOS-5 — the weight is the score of exactly the attested (or
    /// non-GitHub) assertions, and adding disputes, withdrawals, links or
    /// unattested GitHub assertions never moves it (KPI-OOS-2).
    /// `@property @s-oos-7 @contract-shape:pure-function`
    #[test]
    fn nothing_but_attested_assertions_moves_the_weight(
        feed in arb_feed(),
        extra in arb_weightless_rows(),
    ) {
        let base = sut_score_attested(&feed);
        prop_assert_eq!(&base, &real_score(&eligible(&feed)));
        let mut grown = feed.clone();
        grown.extend(extra);
        prop_assert_eq!(sut_score_attested(&grown), base);
    }
}

/// CORE-OOS-4x — the window's edges, by example (A-6: "≤ 7 days"); anchors
/// [`GateRow`] G3 / G4 / G8 (future-dated = missing).
/// `@s-oos-3 @contract-shape:pure-function`
#[test]
fn the_signing_window_includes_seven_days_and_excludes_the_minute_after() {
    let claim = ClaimView {
        author: AUTHORS[0].to_string(),
        subject: "github:jeffbailey".to_string(),
        predicate: "adheresToPhilosophy".to_string(),
        references: vec![],
    };
    for (age, permitted) in [
        (0, true),
        (WINDOW_MINUTES, true),
        (WINDOW_MINUTES + 1, false),
        (-1, false),
    ] {
        let evidence = EvidenceView {
            links: vec![LinkView {
                did: AUTHORS[0].to_string(),
                github_id: IDS[0],
                linked_login: "jeffbailey".to_string(),
            }],
            checks: vec![LinkCheckView {
                did: AUTHORS[0].to_string(),
                github_id: IDS[0],
                login_seen: "jeffbailey".to_string(),
                verified: true,
                age_minutes: age,
            }],
            repo: None,
        };
        let got = sut_may_sign(&claim, &evidence, WINDOW_MINUTES);
        match permitted {
            true => assert_eq!(got, PermitView::Permit, "age {age}"),
            false => assert_eq!(
                got,
                PermitView::Refused {
                    next_command: Some(
                        "openlore authority verify --subject github:jeffbailey".to_string()
                    )
                },
                "age {age}"
            ),
        }
    }
}
