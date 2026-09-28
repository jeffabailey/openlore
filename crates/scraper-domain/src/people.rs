//! `people` — the pure person-inference area of the scraper domain
//! (contributor-philosophy-inference DDD-1 / DDD-3 / ADR-063 §1).
//!
//! Slice-01 lands the contributor SELECTION: from GitHub's RAW contributors
//! rows (bots included, API order untrusted) to the ranked top-N HUMANS the
//! scrape records as contribution links, plus the bots that were skipped on
//! the way (named to the user). Values in, values out; no I/O.
//!
//! The pipeline:
//! `collapse_by_user_id |> rank_by_contributions |> take_top_humans`.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use ports::{
    AuthorRelationship, ContributionLink, FederatedRow, RankedContributor, RawContributor,
};

/// Default number of HUMAN contributors recorded per scraped repo (D-3: one
/// API page's worth, top by commits).
pub const DEFAULT_CONTRIBUTOR_COUNT: usize = 30;

/// The largest `--contributors N` a scrape accepts: one GitHub page (per_page
/// max 100), so recording contributors stays ONE request (OD-CPI-7 / DDD-13).
pub const MAX_CONTRIBUTOR_COUNT: usize = 100;

/// Why a `--contributors N` request is refused — before any GitHub request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributorCountError {
    /// N exceeds one page of contributors (OD-CPI-7).
    AboveOnePage { requested: usize },
    /// `--contributors` was given for a person target (UC-3): a person scrape
    /// never crawls contributors.
    PersonTarget { target: String },
}

impl std::fmt::Display for ContributorCountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AboveOnePage { requested } => write!(
                f,
                "--contributors {requested} is too many: at most \
                 {MAX_CONTRIBUTOR_COUNT} (one GitHub page) may be recorded"
            ),
            Self::PersonTarget { target } => write!(
                f,
                "--contributors applies only to owner/repo targets; \
                 `{target}` is a person, whose scrape records no contributors"
            ),
        }
    }
}

impl std::error::Error for ContributorCountError {}

/// A scrape target names a repo when it has the `owner/repo` form.
fn is_repo_target(target: &str) -> bool {
    target.contains('/')
}

/// The number of HUMAN contributors to record should `target` be a repo:
/// the `--contributors` override (0..=100) or the default. Refuses an
/// override above one page, or any override on a person target — pure over
/// the raw target, so the shell can refuse BEFORE any GitHub request.
pub fn contributor_count_for(
    target: &str,
    requested: Option<usize>,
) -> Result<usize, ContributorCountError> {
    match requested {
        None => Ok(DEFAULT_CONTRIBUTOR_COUNT),
        Some(_) if !is_repo_target(target) => Err(ContributorCountError::PersonTarget {
            target: target.to_string(),
        }),
        Some(requested) if requested > MAX_CONTRIBUTOR_COUNT => {
            Err(ContributorCountError::AboveOnePage { requested })
        }
        Some(requested) => Ok(requested),
    }
}

/// The outcome of selecting a repo's contributors: the ranked humans to
/// record, and the bots skipped while collecting them (named, never linked).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributorSelection {
    /// Ranked humans, rank 1..=k contiguous (k = min(N, distinct humans)).
    pub people: Vec<RankedContributor>,
    /// Logins of the bots ranked above the last recorded human (DDD-3).
    pub bots_excluded: Vec<String>,
}

/// The bot rule (DDD-3): GitHub typed the account `Bot`, OR its login ends in
/// `[bot]` (case-insensitive) — the latter catches the API lie of a `[bot]`
/// account typed `User`.
pub fn is_bot(row: &RawContributor) -> bool {
    row.account_type.eq_ignore_ascii_case("bot")
        || row.login.to_ascii_lowercase().ends_with("[bot]")
}

/// Select the top `top_n` HUMAN contributors from raw rows. Total: any input
/// (empty, all bots, duplicates, ties) yields a well-formed selection.
pub fn select_contributors(rows: &[RawContributor], top_n: usize) -> ContributorSelection {
    let accounts = rank_by_contributions(collapse_by_user_id(rows));
    take_top_humans(&accounts, top_n)
}

/// One GitHub account after de-duplication by user id.
#[derive(Debug, Clone)]
struct Account {
    best_row: RawContributor,
    is_bot: bool,
}

/// De-duplicate by numeric user id: keep the account's best row (most
/// contributions, then login asc); the account is a bot if ANY of its rows
/// matches the bot rule (conservative — a bot never sneaks in as a human).
fn collapse_by_user_id(rows: &[RawContributor]) -> Vec<Account> {
    rows.iter()
        .fold(BTreeMap::<u64, Account>::new(), |mut accounts, row| {
            accounts
                .entry(row.user_id)
                .and_modify(|account| {
                    account.is_bot |= is_bot(row);
                    if contribution_order_key(row) < contribution_order_key(&account.best_row) {
                        account.best_row = row.clone();
                    }
                })
                .or_insert_with(|| Account {
                    best_row: row.clone(),
                    is_bot: is_bot(row),
                });
            accounts
        })
        .into_values()
        .collect()
}

/// Re-rank by contributions desc, then login asc (API order is not trusted),
/// with the user id as a final tiebreak so the order is total.
fn rank_by_contributions(mut accounts: Vec<Account>) -> Vec<Account> {
    accounts.sort_by(|a, b| {
        contribution_order_key(&a.best_row).cmp(&contribution_order_key(&b.best_row))
    });
    accounts
}

fn contribution_order_key(row: &RawContributor) -> (Reverse<u64>, String, String, u64, String) {
    (
        Reverse(row.contributions),
        row.login.to_ascii_lowercase(),
        row.login.clone(),
        row.user_id,
        row.account_type.clone(),
    )
}

/// Walk the ranked accounts collecting humans until `top_n` are taken; every
/// bot passed on the way is named in `bots_excluded`.
fn take_top_humans(ranked: &[Account], top_n: usize) -> ContributorSelection {
    let mut people = Vec::new();
    let mut bots_excluded = Vec::new();
    for account in ranked {
        if people.len() >= top_n {
            break;
        }
        if account.is_bot {
            bots_excluded.push(account.best_row.login.clone());
        } else {
            people.push(RankedContributor {
                login: account.best_row.login.clone(),
                github_user_id: account.best_row.user_id,
                rank: u32::try_from(people.len() + 1).unwrap_or(u32::MAX),
                contributions: account.best_row.contributions,
            });
        }
    }
    ContributorSelection {
        people,
        bots_excluded,
    }
}

// =============================================================================
// Cross-repo overlap (US-CPI-001 AC4; DDD-5 case-folded keys)
// =============================================================================

/// One recorded person of the scraped repo who is ALSO linked to another
/// repo the user scraped earlier — rendered `login → owner/repo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedContributor {
    /// The person's login as the current selection ranked it.
    pub login: String,
    /// The other repo, display form without the `github:` scheme.
    pub other_repo: String,
}

/// The people of `current_repo` also linked to OTHER scraped repos, in the
/// selection's rank order then by other repo — one line per (person, repo).
/// Subjects are compared case-folded (DDD-5 keys); the scraped repo itself is
/// never "other", so the result is the same whether `recorded_links` was read
/// before or after this scrape's snapshot was recorded.
pub fn shared_contributors(
    current_repo: &str,
    people: &[RankedContributor],
    recorded_links: &[ContributionLink],
) -> Vec<SharedContributor> {
    let current_repo_key = subject_key(current_repo);
    let other_repos_by_person = recorded_links
        .iter()
        .filter(|link| subject_key(&link.repo_subject) != current_repo_key)
        .fold(
            BTreeMap::<String, BTreeMap<String, String>>::new(),
            |mut by_person, link| {
                by_person
                    .entry(subject_key(&link.person_subject))
                    .or_default()
                    .entry(subject_key(&link.repo_subject))
                    .or_insert_with(|| repo_display(&link.repo_subject));
                by_person
            },
        );
    let mut people_seen = BTreeSet::new();
    people
        .iter()
        .filter(|person| people_seen.insert(subject_key(&person.person_subject())))
        .flat_map(|person| {
            other_repos_by_person
                .get(&subject_key(&person.person_subject()))
                .into_iter()
                .flat_map(BTreeMap::values)
                .map(|other_repo| SharedContributor {
                    login: person.login.clone(),
                    other_repo: other_repo.clone(),
                })
        })
        .collect()
}

/// A repo subject's display form without the `github:` scheme.
fn repo_display(repo_subject: &str) -> String {
    repo_subject
        .strip_prefix("github:")
        .unwrap_or(repo_subject)
        .to_string()
}

// =============================================================================
// Inference (slices 02 + 03 thin thread; DDD-7 / DDD-9 / DDD-10; ADR-064)
// =============================================================================
//
// `collapse_links |> join eligible repo claims |> group by (person,
// philosophy) |> PersonCandidate::new |> sort (numbering)`.

/// The person-level predicate an inferred adherence claim carries (ADR-064 §2).
pub const ADHERES_TO_PHILOSOPHY: &str = "adheresToPhilosophy";

/// Ceiling of an inferred confidence, in hundredths: inference never leaves
/// the speculative bucket on its own (OD-CPI-3 / DDD-9).
const CONFIDENCE_CAP: u32 = 29;
/// Base confidence for a single supporting repo, in hundredths.
const CONFIDENCE_BASE: u32 = 15;
/// Confidence gained per additional supporting repo, in hundredths.
const CONFIDENCE_STEP: u32 = 5;

/// A confidence in whole hundredths (DDD-9) — integer arithmetic, so no float
/// noise ever reaches a signed payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hundredths(u32);

impl Hundredths {
    /// A confidence of `value` hundredths, clamped to `[0, 100]`.
    pub fn new(value: u32) -> Self {
        Self(value.min(100))
    }

    /// `floor(100 × confidence)` of a `[0.0, 1.0]` claim confidence. A tiny
    /// epsilon absorbs binary representation error (`0.29 × 100 = 28.999…`).
    pub fn floor_of(confidence: f64) -> Self {
        let scaled = (confidence.clamp(0.0, 1.0) * 100.0 + 1e-6).floor();
        Self::new(scaled as u32)
    }

    /// The whole-hundredths value.
    pub fn value(self) -> u32 {
        self.0
    }

    /// The `[0.0, 1.0]` decimal this confidence denotes.
    pub fn as_decimal(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

impl std::fmt::Display for Hundredths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

/// DDD-9: `min(29, 15 + 5·(k − 1), floor(100·max))` for `k` supporting repos.
pub fn inferred_confidence(supporting_repos: usize, max_supporting: Hundredths) -> Hundredths {
    let extra_repos = u32::try_from(supporting_repos.saturating_sub(1)).unwrap_or(u32::MAX);
    let by_breadth = CONFIDENCE_BASE.saturating_add(CONFIDENCE_STEP.saturating_mul(extra_repos));
    Hundredths::new(CONFIDENCE_CAP.min(by_breadth).min(max_supporting.value()))
}

/// The arithmetic behind [`inferred_confidence`], reproducible by hand
/// (J-002c), e.g. `min(0.29, 0.15 + 0.05 × (2 − 1), 0.60) = 0.20`.
pub fn confidence_arithmetic(supporting_repos: usize, max_supporting: Hundredths) -> String {
    format!(
        "min({}, {} + {} × ({supporting_repos} − 1), {max_supporting}) = {}",
        Hundredths::new(CONFIDENCE_CAP),
        Hundredths::new(CONFIDENCE_BASE),
        Hundredths::new(CONFIDENCE_STEP),
        inferred_confidence(supporting_repos, max_supporting)
    )
}

/// A signed repo claim as inference sees it: the port-level facts of one
/// `FederatedRow`, confidence already in hundredths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoClaim {
    pub repo_subject: String,
    pub predicate: String,
    pub philosophy: String,
    pub author_did: String,
    pub relationship: AuthorRelationship,
    pub cid: String,
    pub confidence: Hundredths,
}

impl From<&FederatedRow> for RepoClaim {
    fn from(row: &FederatedRow) -> Self {
        let unsigned = &row.signed_claim.unsigned;
        Self {
            repo_subject: unsigned.subject.clone(),
            predicate: unsigned.predicate.clone(),
            philosophy: unsigned.object.clone(),
            author_did: row.author_did.0.clone(),
            relationship: row.author_relationship,
            cid: row.signed_claim.signature.signed_cid.0.clone(),
            confidence: Hundredths::floor_of(unsigned.confidence.value()),
        }
    }
}

/// One supporting claim, cited by its author (bare DID) and CID (D-7 / D-9).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CitedClaim {
    pub author_did: String,
    pub cid: String,
}

impl CitedClaim {
    /// Cite a claim; the author DID is stored bare (fragment stripped), the
    /// form `claim publish` mints into at-uris (Q-CPI-D3).
    pub fn new(author_did: &str, cid: &str) -> Self {
        Self {
            author_did: bare_did(author_did).to_string(),
            cid: cid.to_string(),
        }
    }

    /// ADR-064 §3 canonical AT-URI of the cited claim.
    pub fn at_uri(&self) -> String {
        format!("at://{}/{CLAIM_COLLECTION}/{}", self.author_did, self.cid)
    }
}

const CLAIM_COLLECTION: &str = "org.openlore.claim";

fn bare_did(did: &str) -> &str {
    did.split('#').next().unwrap_or(did)
}

/// One repo supporting an inference: the person's rank there and the eligible
/// signed claims (each with its own author) that support the philosophy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportingRepo {
    pub repo_subject: String,
    pub rank: u32,
    pub claims: Vec<SupportingClaim>,
}

/// A cited supporting claim plus the confidence its author signed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SupportingClaim {
    pub cited: CitedClaim,
    pub confidence: Hundredths,
}

/// Why a [`PersonCandidate`] could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateError {
    /// No supporting repo, or a supporting repo citing no claim: an inference
    /// without provenance is never proposed (D-9).
    EmptyProvenance,
}

/// "`person` adheres to `philosophy`", proposed from signed repo claims. Only
/// constructible with non-empty provenance; support is kept in canonical
/// (ADR-064 §3) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonCandidate {
    person_subject: String,
    philosophy: String,
    support: Vec<SupportingRepo>,
    confidence: Hundredths,
}

impl PersonCandidate {
    /// Smart constructor: rejects empty provenance; canonicalizes support
    /// order; derives the DDD-9 confidence.
    pub fn new(
        person_subject: &str,
        philosophy: &str,
        support: Vec<SupportingRepo>,
    ) -> Result<Self, CandidateError> {
        let hollow = support.is_empty() || support.iter().any(|repo| repo.claims.is_empty());
        if hollow {
            return Err(CandidateError::EmptyProvenance);
        }
        let support = canonical_support_order(support);
        let max_supporting = max_claim_confidence(&support);
        Ok(Self {
            person_subject: person_subject.to_string(),
            philosophy: philosophy.to_string(),
            confidence: inferred_confidence(support.len(), max_supporting),
            support,
        })
    }

    /// `github:<login>`.
    pub fn person_subject(&self) -> &str {
        &self.person_subject
    }

    /// The philosophy id (claim object).
    pub fn philosophy(&self) -> &str {
        &self.philosophy
    }

    /// Supporting repos in ADR-064 order.
    pub fn support(&self) -> &[SupportingRepo] {
        &self.support
    }

    /// The proposed (DDD-9) confidence.
    pub fn confidence(&self) -> Hundredths {
        self.confidence
    }

    /// The strongest supporting claim's confidence.
    pub fn max_supporting_confidence(&self) -> Hundredths {
        max_claim_confidence(&self.support)
    }

    /// The GitHub login of the person.
    pub fn login(&self) -> &str {
        strip_github_prefix(&self.person_subject)
    }
}

/// ADR-064 §3 order: repos by case-folded subject; claims within a repo by
/// (author, cid). Exact duplicates collapse.
fn canonical_support_order(support: Vec<SupportingRepo>) -> Vec<SupportingRepo> {
    let mut repos: Vec<SupportingRepo> = support
        .into_iter()
        .map(|mut repo| {
            repo.claims.sort();
            repo.claims.dedup();
            repo
        })
        .collect();
    repos.sort_by(|a, b| {
        (a.repo_subject.to_ascii_lowercase(), &a.repo_subject, a.rank).cmp(&(
            b.repo_subject.to_ascii_lowercase(),
            &b.repo_subject,
            b.rank,
        ))
    });
    repos
}

fn max_claim_confidence(support: &[SupportingRepo]) -> Hundredths {
    support
        .iter()
        .flat_map(|repo| repo.claims.iter().map(|c| c.confidence))
        .max()
        .unwrap_or(Hundredths::new(0))
}

fn strip_github_prefix(subject: &str) -> &str {
    subject
        .get(..GITHUB_PREFIX.len())
        .filter(|head| head.eq_ignore_ascii_case(GITHUB_PREFIX))
        .map_or(subject, |_| &subject[GITHUB_PREFIX.len()..])
}

const GITHUB_PREFIX: &str = "github:";

/// ADR-064 §3: the candidate's provenance as `evidence[]` strings.
pub fn encode_provenance(candidate: &PersonCandidate) -> Vec<String> {
    candidate
        .support
        .iter()
        .flat_map(|repo| {
            repo.claims
                .iter()
                .map(|claim| claim.cited.at_uri())
                .chain(std::iter::once(commits_url(
                    &repo.repo_subject,
                    candidate.login(),
                )))
        })
        .collect()
}

/// The person-specific public source for one repo (ADR-064 §3).
fn commits_url(repo_subject: &str, login: &str) -> String {
    format!(
        "https://github.com/{}/commits?author={login}",
        strip_github_prefix(repo_subject)
    )
}

/// ADR-064 §5: the claims an `evidence[]` cites — every entry that parses as
/// `at://<did>/org.openlore.claim/<cid>`, in order.
pub fn parse_provenance(evidence: &[String]) -> Vec<CitedClaim> {
    evidence
        .iter()
        .filter_map(|entry| parse_claim_at_uri(entry))
        .collect()
}

/// `at://<did>/org.openlore.claim/<cid>` → the cited claim, or `None`.
fn parse_claim_at_uri(entry: &str) -> Option<CitedClaim> {
    let (author_did, tail) = entry.strip_prefix("at://")?.split_once('/')?;
    let cid = tail.strip_prefix(CLAIM_COLLECTION)?.strip_prefix('/')?;
    let well_formed = !author_did.is_empty() && !cid.is_empty() && !cid.contains('/');
    well_formed.then(|| CitedClaim::new(author_did, cid))
}

/// Infer the numbered person candidates from the recorded contribution links
/// and the signed repo claims. Output order IS the numbering (person key,
/// then philosophy) and is independent of input order.
pub fn infer_person_candidates(
    links: &[ContributionLink],
    repo_claims: &[RepoClaim],
) -> Vec<PersonCandidate> {
    let links = collapse_links(links);
    let eligible: Vec<&RepoClaim> = repo_claims.iter().filter(|c| is_eligible(c)).collect();
    group_support(&links, &eligible)
        .into_values()
        .filter_map(|group| {
            PersonCandidate::new(
                &group.person_subject,
                &group.philosophy,
                group.repos.into_values().collect(),
            )
            .ok()
        })
        .collect()
}

/// One `infer people` run as pure data: the numbered candidates plus the
/// linked repos that contributed nothing because no signed philosophy claim
/// is about them (D-2 / KPI-CPI-3 footer) — so the render needs no second
/// query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceReport {
    pub candidates: Vec<PersonCandidate>,
    pub repos_without_signed_claims: Vec<String>,
}

/// Infer the report, optionally scoped to one `github:<login>` person
/// (compared case-insensitively). Scoping narrows the links first, so the
/// footer names only the scoped person's repos.
pub fn infer_people_report(
    links: &[ContributionLink],
    repo_claims: &[RepoClaim],
    person: Option<&str>,
) -> InferenceReport {
    let scoped_links: Vec<ContributionLink> = links
        .iter()
        .filter(|link| is_in_scope(link, person))
        .cloned()
        .collect();
    InferenceReport {
        candidates: infer_person_candidates(&scoped_links, repo_claims),
        repos_without_signed_claims: repos_without_signed_claims(&scoped_links, repo_claims),
    }
}

/// A link is in scope when no person is named, or it names that person.
fn is_in_scope(link: &ContributionLink, person: Option<&str>) -> bool {
    person.is_none_or(|p| subject_key(&link.person_subject) == subject_key(p))
}

/// Linked repos (once per case-folded subject, smallest spelling shown) about
/// which no eligible signed philosophy claim exists.
fn repos_without_signed_claims(
    links: &[ContributionLink],
    repo_claims: &[RepoClaim],
) -> Vec<String> {
    let supported: BTreeSet<String> = repo_claims
        .iter()
        .filter(|claim| is_eligible(claim))
        .map(|claim| subject_key(&claim.repo_subject))
        .collect();
    links
        .iter()
        .filter(|link| !supported.contains(&subject_key(&link.repo_subject)))
        .fold(BTreeMap::<String, &str>::new(), |mut by_key, link| {
            by_key
                .entry(subject_key(&link.repo_subject))
                .and_modify(|shown| *shown = (*shown).min(link.repo_subject.as_str()))
                .or_insert(&link.repo_subject);
            by_key
        })
        .into_values()
        .map(str::to_string)
        .collect()
}

/// DDD-7 (thin): an `embodiesPhilosophy` claim by me or a subscribed peer.
fn is_eligible(claim: &RepoClaim) -> bool {
    claim.predicate == crate::EMBODIES_PHILOSOPHY
        && matches!(
            claim.relationship,
            AuthorRelationship::You | AuthorRelationship::SubscribedPeer
        )
}

/// A `github:` subject's case-folded join key (logins are case-insensitive).
fn subject_key(subject: &str) -> String {
    subject.to_ascii_lowercase()
}

/// One link per (repo, person) key: the most recently observed, then the best
/// rank, then the smallest original spelling — a total, order-free choice.
fn collapse_links(links: &[ContributionLink]) -> Vec<&ContributionLink> {
    links
        .iter()
        .fold(
            BTreeMap::<(String, String), &ContributionLink>::new(),
            |mut by_key, link| {
                let key = (
                    subject_key(&link.repo_subject),
                    subject_key(&link.person_subject),
                );
                by_key
                    .entry(key)
                    .and_modify(|kept| {
                        if link_preference(link) < link_preference(kept) {
                            *kept = link;
                        }
                    })
                    .or_insert(link);
                by_key
            },
        )
        .into_values()
        .collect()
}

/// Preference key: latest observation (micros since epoch) first.
fn link_preference(link: &ContributionLink) -> (Reverse<i64>, u32, &str, &str) {
    (
        Reverse(link.last_observed_at.timestamp_micros()),
        link.rank,
        &link.repo_subject,
        &link.person_subject,
    )
}

/// The support gathered for one (person, philosophy) before construction.
struct SupportGroup {
    person_subject: String,
    philosophy: String,
    repos: BTreeMap<String, SupportingRepo>,
}

/// Join each collapsed link with the eligible claims on its repo, grouped by
/// (person key, philosophy) — the BTreeMap key order IS the numbering.
fn group_support(
    links: &[&ContributionLink],
    eligible: &[&RepoClaim],
) -> BTreeMap<(String, String), SupportGroup> {
    let mut groups = BTreeMap::<(String, String), SupportGroup>::new();
    for link in links {
        let repo_key = subject_key(&link.repo_subject);
        let on_repo = eligible
            .iter()
            .filter(|claim| subject_key(&claim.repo_subject) == repo_key);
        for claim in on_repo {
            let group = groups
                .entry((subject_key(&link.person_subject), claim.philosophy.clone()))
                .or_insert_with(|| SupportGroup {
                    person_subject: link.person_subject.clone(),
                    philosophy: claim.philosophy.clone(),
                    repos: BTreeMap::new(),
                });
            if link.person_subject < group.person_subject {
                group.person_subject = link.person_subject.clone();
            }
            group
                .repos
                .entry(repo_key.clone())
                .or_insert_with(|| SupportingRepo {
                    repo_subject: link.repo_subject.clone(),
                    rank: link.rank,
                    claims: Vec::new(),
                })
                .claims
                .push(SupportingClaim {
                    cited: CitedClaim::new(&claim.author_did, &claim.cid),
                    confidence: claim.confidence,
                });
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proptest_strategies::arb_raw_contributors;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    /// Oracle: user ids with NO bot-rule row (the distinct humans).
    fn distinct_human_ids(rows: &[RawContributor]) -> BTreeSet<u64> {
        let bot_ids: BTreeSet<u64> = rows
            .iter()
            .filter(|r| is_bot(r))
            .map(|r| r.user_id)
            .collect();
        rows.iter()
            .map(|r| r.user_id)
            .filter(|id| !bot_ids.contains(id))
            .collect()
    }

    proptest! {
        /// On an `owner/repo` target the count is the override when it fits
        /// one page, a refusal naming the request above it, the default when
        /// absent.
        #[test]
        fn a_repo_target_accepts_counts_up_to_one_page(
            owner in "[a-z]{1,8}",
            repo in "[a-z]{1,8}",
            requested in proptest::option::of(0usize..=250),
        ) {
            let target = format!("{owner}/{repo}");
            let expected = match requested {
                None => Ok(DEFAULT_CONTRIBUTOR_COUNT),
                Some(n) if n <= MAX_CONTRIBUTOR_COUNT => Ok(n),
                Some(n) => Err(ContributorCountError::AboveOnePage { requested: n }),
            };
            prop_assert_eq!(contributor_count_for(&target, requested), expected);
        }

        /// On a person target any override is refused (whatever N), and no
        /// override is fine.
        #[test]
        fn a_person_target_refuses_any_contributor_override(
            user in "[A-Za-z][A-Za-z0-9-]{0,15}",
            requested in proptest::option::of(any::<usize>()),
        ) {
            let outcome = contributor_count_for(&user, requested);
            match requested {
                None => prop_assert_eq!(outcome, Ok(DEFAULT_CONTRIBUTOR_COUNT)),
                Some(_) => prop_assert_eq!(
                    outcome,
                    Err(ContributorCountError::PersonTarget { target: user.clone() })
                ),
            }
        }
    }

    proptest! {
        /// Top-N cut + de-dup: exactly min(N, distinct humans) people, each
        /// user id at most once.
        #[test]
        fn selection_records_min_of_n_and_distinct_humans_each_once(
            rows in arb_raw_contributors(),
            top_n in 0usize..=40,
        ) {
            let selection = select_contributors(&rows, top_n);
            let humans = distinct_human_ids(&rows);
            prop_assert_eq!(selection.people.len(), top_n.min(humans.len()));
            let ids: BTreeSet<u64> = selection.people.iter().map(|p| p.github_user_id).collect();
            prop_assert_eq!(ids.len(), selection.people.len());
            prop_assert!(ids.is_subset(&humans));
        }

        /// Bot rule: no recorded person matches it; every named bot does, and
        /// bots are named only while humans are still being collected.
        #[test]
        fn no_bot_is_ever_recorded_and_only_bots_are_named_excluded(
            rows in arb_raw_contributors(),
            top_n in 0usize..=40,
        ) {
            let selection = select_contributors(&rows, top_n);
            let humans = distinct_human_ids(&rows);
            prop_assert!(selection.people.iter().all(|p| !p.login.to_ascii_lowercase().ends_with("[bot]")));
            for bot in &selection.bots_excluded {
                prop_assert!(rows.iter().any(|r| &r.login == bot && !humans.contains(&r.user_id)));
            }
            if top_n == 0 {
                prop_assert!(selection.bots_excluded.is_empty());
            }
        }

        /// Re-rank: ranks are contiguous 1..=k and contributions never rise
        /// as rank falls.
        #[test]
        fn ranks_are_contiguous_and_ordered_by_contributions(
            rows in arb_raw_contributors(),
            top_n in 0usize..=40,
        ) {
            let selection = select_contributors(&rows, top_n);
            let ranks: Vec<u32> = selection.people.iter().map(|p| p.rank).collect();
            let expected: Vec<u32> = (1..=selection.people.len() as u32).collect();
            prop_assert_eq!(ranks, expected);
            prop_assert!(selection.people.windows(2).all(|w| w[0].contributions >= w[1].contributions));
        }

        /// API order is not trusted: any permutation of the rows selects the
        /// identical people and bots.
        #[test]
        fn selection_is_invariant_under_api_row_order(
            (rows, shuffled) in arb_raw_contributors()
                .prop_flat_map(|rows| (Just(rows.clone()), Just(rows).prop_shuffle())),
            top_n in 0usize..=40,
        ) {
            prop_assert_eq!(select_contributors(&rows, top_n), select_contributors(&shuffled, top_n));
        }
    }
    // ---------------------------------------------------------------------
    // Inference properties (DDD-7 / DDD-9 / DDD-10; ADR-064)
    // ---------------------------------------------------------------------

    use crate::proptest_strategies::{
        arb_hundredths, arb_inference_inputs, arb_person_candidate, arb_supporting_repo,
    };

    fn is_eligible(claim: &RepoClaim) -> bool {
        claim.predicate == crate::EMBODIES_PHILOSOPHY
            && matches!(
                claim.relationship,
                AuthorRelationship::You | AuthorRelationship::SubscribedPeer
            )
    }

    proptest! {
        /// DDD-9: confidence = min(29, 15 + 5(k−1), max) — never above the
        /// speculative cap nor the strongest support, monotone in k.
        #[test]
        fn inferred_confidence_is_capped_bounded_by_support_and_monotone_in_breadth(
            k in 1usize..=40,
            more in 0usize..=10,
            max in arb_hundredths(),
        ) {
            let confidence = inferred_confidence(k, max);
            let expected = 29u32.min(15 + 5 * (k as u32 - 1)).min(max.value());
            prop_assert_eq!(confidence.value(), expected);
            prop_assert!(confidence.value() <= 29 && confidence <= max);
            prop_assert!(inferred_confidence(k + more, max) >= confidence);
        }

        /// D-9: a candidate without provenance is unrepresentable — no repos,
        /// or any repo citing no claim, is rejected; otherwise it builds with
        /// the DDD-9 confidence over its supporting repos.
        #[test]
        fn person_candidate_requires_provenance_in_every_supporting_repo(
            repos in proptest::collection::vec(arb_supporting_repo(), 0..4),
            hollow_at in proptest::option::of(0usize..4),
        ) {
            let mut repos = repos;
            let mut seen = BTreeSet::new();
            repos.retain(|r| seen.insert(r.repo_subject.to_ascii_lowercase()));
            if let Some(i) = hollow_at.filter(|i| *i < repos.len()) {
                repos[i].claims.clear();
            }
            let hollow = repos.is_empty() || repos.iter().any(|r| r.claims.is_empty());
            let built = PersonCandidate::new("github:BurntSushi", "org.openlore.philosophy.x", repos.clone());
            if hollow {
                prop_assert_eq!(built, Err(CandidateError::EmptyProvenance));
            } else {
                let candidate = built.expect("provenance present");
                prop_assert_eq!(candidate.support().len(), repos.len());
                prop_assert_eq!(
                    candidate.confidence(),
                    inferred_confidence(repos.len(), candidate.max_supporting_confidence())
                );
            }
        }

        /// ADR-064 §3/§5 codec: parsing the encoded evidence yields exactly the
        /// cited claims in canonical order (bare authors); each repo closes
        /// with its person-specific commits URL; no entry carries a comma.
        #[test]
        fn provenance_round_trips_through_evidence(candidate in arb_person_candidate()) {
            let evidence = encode_provenance(&candidate);
            let cited: Vec<CitedClaim> = candidate
                .support()
                .iter()
                .flat_map(|repo| repo.claims.iter().map(|c| c.cited.clone()))
                .collect();
            prop_assert_eq!(parse_provenance(&evidence), cited.clone());
            prop_assert_eq!(evidence.len(), cited.len() + candidate.support().len());
            prop_assert!(cited.iter().all(|c| !c.author_did.contains('#')));
            prop_assert!(evidence.iter().all(|e| !e.contains(',')));
            let repo_folded: Vec<String> = candidate.support().iter().map(|r| r.repo_subject.to_ascii_lowercase()).collect();
            prop_assert!(repo_folded.windows(2).all(|w| w[0] <= w[1]));
            let commits: Vec<&String> = evidence.iter().filter(|e| e.starts_with("https://github.com/")).collect();
            prop_assert_eq!(commits.len(), candidate.support().len());
            let suffix = format!("/commits?author={}", candidate.login());
            prop_assert!(commits.iter().all(|url| url.ends_with(&suffix)));
        }

        /// Numbering is deterministic: any permutation of links and claims
        /// yields the identical numbered candidate list.
        #[test]
        fn numbering_is_invariant_under_input_order(
            ((links, claims), (shuffled_links, shuffled_claims)) in arb_inference_inputs()
                .prop_flat_map(|(links, claims)| (
                    Just((links.clone(), claims.clone())),
                    (Just(links).prop_shuffle(), Just(claims).prop_shuffle()),
                )),
        ) {
            prop_assert_eq!(
                infer_person_candidates(&links, &claims),
                infer_person_candidates(&shuffled_links, &shuffled_claims)
            );
        }

        /// DDD-7 eligibility: every cited claim is an `embodiesPhilosophy`
        /// claim by me or a subscribed peer on a repo linked to the person;
        /// every such claim is cited by the matching candidate.
        #[test]
        fn candidates_cite_exactly_the_eligible_linked_claims(
            (links, claims) in arb_inference_inputs(),
        ) {
            let candidates = infer_person_candidates(&links, &claims);
            let cited: BTreeSet<(String, String, String)> = candidates
                .iter()
                .flat_map(|c| c.support().iter().flat_map(move |repo| repo.claims.iter().map(move |s| (
                    c.person_subject().to_ascii_lowercase(),
                    c.philosophy().to_string(),
                    s.cited.cid.clone(),
                ))))
                .collect();
            let expected: BTreeSet<(String, String, String)> = links
                .iter()
                .flat_map(|link| claims.iter()
                    .filter(|claim| is_eligible(claim) && claim.repo_subject.eq_ignore_ascii_case(&link.repo_subject))
                    .map(move |claim| (
                        link.person_subject.to_ascii_lowercase(),
                        claim.philosophy.clone(),
                        claim.cid.clone(),
                    )))
                .collect();
            prop_assert_eq!(cited, expected);
        }
    }

    // --- the inference report (US-CPI-002 AC1-AC4; D-2 / D-7 / KPI-CPI-2) ---

    proptest! {
        /// Unscoped, the report's candidates ARE the inferred candidates; the
        /// footer names exactly the linked repos (case-folded, once each) that
        /// carry zero eligible claims — sorted, never one that supports.
        #[test]
        fn report_footer_names_exactly_the_linked_repos_without_signed_claims(
            (links, claims) in arb_inference_inputs(),
        ) {
            let report = infer_people_report(&links, &claims, None);
            prop_assert_eq!(&report.candidates, &infer_person_candidates(&links, &claims));
            let expected: BTreeSet<String> = links
                .iter()
                .map(|l| subject_key(&l.repo_subject))
                .filter(|repo| !claims.iter().any(|c| is_eligible(c) && subject_key(&c.repo_subject) == *repo))
                .collect();
            let named: Vec<String> = report.repos_without_signed_claims.iter().map(|r| subject_key(r)).collect();
            prop_assert_eq!(named.iter().cloned().collect::<BTreeSet<_>>(), expected);
            prop_assert!(named.windows(2).all(|w| w[0] < w[1]), "sorted, once each");
        }

        /// Scoping to a person keeps exactly that person's candidates (any
        /// letter case) and names only that person's unsupported repos; every
        /// candidate keeps complete provenance and each cited claim keeps its
        /// OWN author (D-7 — a peer's claim is never re-attributed).
        #[test]
        fn a_person_scoped_report_keeps_only_that_persons_attributed_candidates(
            (links, claims) in arb_inference_inputs(),
            person in proptest::sample::select(&crate::proptest_strategies::PERSON_POOL[..]),
        ) {
            let scoped = infer_people_report(&links, &claims, Some(&person.to_ascii_uppercase().replacen("GITHUB:", "github:", 1)));
            let expected: Vec<PersonCandidate> = infer_person_candidates(&links, &claims)
                .into_iter()
                .filter(|c| subject_key(c.person_subject()) == subject_key(person))
                .collect();
            prop_assert_eq!(&scoped.candidates, &expected);
            let persons_repos: BTreeSet<String> = links
                .iter()
                .filter(|l| subject_key(&l.person_subject) == subject_key(person))
                .map(|l| subject_key(&l.repo_subject))
                .collect();
            let unsupported: BTreeSet<String> = persons_repos
                .into_iter()
                .filter(|repo| !claims.iter().any(|c| is_eligible(c) && subject_key(&c.repo_subject) == *repo))
                .collect();
            let named: BTreeSet<String> = scoped.repos_without_signed_claims.iter().map(|r| subject_key(r)).collect();
            prop_assert_eq!(named, unsupported);
            let author_of: BTreeSet<(String, String)> = claims.iter().map(|c| (c.cid.clone(), bare_did(&c.author_did).to_string())).collect();
            for candidate in &scoped.candidates {
                prop_assert!(!candidate.support().is_empty());
                for repo in candidate.support() {
                    prop_assert!(!repo.claims.is_empty());
                    for claim in &repo.claims {
                        prop_assert!(author_of.contains(&(claim.cited.cid.clone(), claim.cited.author_did.clone())));
                    }
                }
            }
        }
    }

    // --- cross-repo overlap (US-CPI-001 AC4) -------------------------------

    /// The people a repo's links record, as a selection would rank them.
    fn people_of(repo: &str, links: &[ContributionLink]) -> Vec<RankedContributor> {
        links
            .iter()
            .filter(|l| subject_key(&l.repo_subject) == subject_key(repo))
            .map(|l| RankedContributor {
                login: l.person_subject.trim_start_matches("github:").to_string(),
                github_user_id: l.github_user_id,
                rank: l.rank,
                contributions: l.contributions,
            })
            .collect()
    }

    /// Overlap as case-folded `(person, other repo)` keys.
    fn overlap_keys(shared: &[SharedContributor]) -> BTreeSet<(String, String)> {
        shared
            .iter()
            .map(|s| {
                (
                    subject_key(&format!("github:{}", s.login)),
                    subject_key(&format!("github:{}", s.other_repo)),
                )
            })
            .collect()
    }

    proptest! {
        /// Every recorded person of the scraped repo who is linked to ANOTHER
        /// repo (subjects compared case-folded) is surfaced with that repo —
        /// once — and the scraped repo itself never appears as "other".
        #[test]
        fn overlap_surfaces_exactly_the_other_repos_each_person_is_linked_to(
            links in proptest::collection::vec(crate::proptest_strategies::arb_contribution_link(), 0..10),
            current in proptest::sample::select(vec![
                "github:BurntSushi/ripgrep", "github:rust-lang/regex", "github:dtolnay/serde",
            ]),
        ) {
            let people = people_of(current, &links);
            let person_keys: BTreeSet<String> = people
                .iter()
                .map(|p| subject_key(&p.person_subject()))
                .collect();
            let expected: BTreeSet<(String, String)> = links
                .iter()
                .filter(|l| subject_key(&l.repo_subject) != subject_key(current))
                .filter(|l| person_keys.contains(&subject_key(&l.person_subject)))
                .map(|l| (subject_key(&l.person_subject), subject_key(&l.repo_subject)))
                .collect();

            let shared = shared_contributors(current, &people, &links);

            prop_assert_eq!(overlap_keys(&shared), expected.clone());
            prop_assert_eq!(shared.len(), expected.len(), "one line per (person, repo)");
        }

        /// Overlap is symmetric: P is shown under A → B exactly when P is
        /// shown under B → A.
        #[test]
        fn overlap_is_symmetric_between_two_scraped_repos(
            links in proptest::collection::vec(crate::proptest_strategies::arb_contribution_link(), 0..10),
        ) {
            let (repo_a, repo_b) = ("github:BurntSushi/ripgrep", "github:rust-lang/regex");
            let persons_towards = |from: &str, to: &str| -> BTreeSet<String> {
                overlap_keys(&shared_contributors(from, &people_of(from, &links), &links))
                    .into_iter()
                    .filter(|(_, other)| *other == subject_key(to))
                    .map(|(person, _)| person)
                    .collect()
            };
            prop_assert_eq!(persons_towards(repo_a, repo_b), persons_towards(repo_b, repo_a));
        }

        /// Case never matters: re-casing every recorded subject yields the
        /// same case-folded overlap.
        #[test]
        fn overlap_ignores_subject_case(
            links in proptest::collection::vec(crate::proptest_strategies::arb_contribution_link(), 0..10),
        ) {
            let current = "github:BurntSushi/ripgrep";
            let upper: Vec<ContributionLink> = links
                .iter()
                .map(|l| ContributionLink {
                    repo_subject: l.repo_subject.to_ascii_uppercase().replacen("GITHUB:", "github:", 1),
                    person_subject: l.person_subject.to_ascii_uppercase().replacen("GITHUB:", "github:", 1),
                    ..l.clone()
                })
                .collect();
            let people = people_of(current, &links);
            prop_assert_eq!(
                overlap_keys(&shared_contributors(current, &people, &links)),
                overlap_keys(&shared_contributors(current, &people, &upper))
            );
        }
    }
}
