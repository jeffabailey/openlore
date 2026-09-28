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

use claim_domain::{
    bare_did, is_self_retracted, is_superseded_by_author, ClaimLineage, ClaimReference,
    ReferenceType,
};
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
    /// The claim's typed references (retracts / counters / supersedes /
    /// corrects) — the reference graph DDD-7 eligibility reads.
    pub references: Vec<ClaimReference>,
}

impl RepoClaim {
    /// The borrowed reference-graph view the shared `claim-domain`
    /// withdrawal rules read.
    fn lineage(&self) -> ClaimLineage<'_> {
        ClaimLineage {
            author_did: &self.author_did,
            cid: &self.cid,
            references: &self.references,
        }
    }
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
            references: unsigned.references.clone(),
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
    /// Cite a claim; the author DID is stored bare (fragment stripped) by the
    /// SAME rule `claim publish` mints at-uris with (Q-CPI-D3).
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
    let eligible = supporting_claims(repo_claims);
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

/// A person the inference is ABOUT, named `github:<login>` (D-8) — the only
/// form a new surface accepts; validated once at the edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSubject(String);

/// Why a named person was refused (D-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSubjectError {
    pub given: String,
}

impl std::fmt::Display for PersonSubjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a person is named as github:<login> (e.g. github:BurntSushi), not `{}`",
            self.given
        )
    }
}

impl std::error::Error for PersonSubjectError {}

impl PersonSubject {
    /// Accept exactly `github:<login>` (login: ASCII letters, digits and
    /// hyphens, not starting with a hyphen); refuse anything else.
    pub fn parse(raw: &str) -> Result<Self, PersonSubjectError> {
        match raw.strip_prefix("github:") {
            Some(login) if is_github_login(login) => Ok(Self(raw.to_string())),
            _ => Err(PersonSubjectError {
                given: raw.to_string(),
            }),
        }
    }

    /// The `github:<login>` subject.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_github_login(login: &str) -> bool {
    !login.is_empty()
        && !login.starts_with('-')
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The DDD-13 filters of one `infer people` run, applied BEFORE numbering
/// (UC-8) so the list and `--sign` number the same candidates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InferenceFilter {
    /// Scope to one person (compared case-insensitively).
    pub person: Option<PersonSubject>,
    /// Keep only candidates supported by at least this many repos.
    pub min_repos: usize,
}

/// One of MY signed claims, as the already-signed / STRONGER classification
/// (DDD-8) sees it: its evidence carries the cited AT-URIs (ADR-064 §5) and
/// its `composed_at` orders several current claims for one pair (Q-CPI-D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnClaim {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub author_did: String,
    pub cid: String,
    pub evidence: Vec<String>,
    /// RFC3339 UTC as the one clock port writes it, so lexical order is
    /// chronological.
    pub composed_at: String,
    pub references: Vec<ClaimReference>,
}

/// An inferred (person, philosophy) I already signed: shown, never numbered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlreadySigned {
    pub candidate: PersonCandidate,
    /// The CID of my standing adherence claim for the pair.
    pub cid: String,
}

/// Why a numbered candidate is proposed (DDD-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateStatus {
    /// No standing adherence of mine for the pair.
    New,
    /// My latest standing INFERRED claim for the pair leaves a currently
    /// supporting repo uncited; signing adds `supersedes <cid>` to a NEW claim
    /// and leaves that claim unchanged (D-5).
    Stronger { supersedes: String },
}

/// A numbered candidate and why it is proposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberedCandidate {
    pub candidate: PersonCandidate,
    pub status: CandidateStatus,
}

/// One `infer people` run as pure data: the numbered candidates, the
/// inferred pairs I already signed (unnumbered, DDD-8), plus the linked repos
/// that contributed nothing because no signed philosophy claim is about them
/// (D-2 / KPI-CPI-3 footer) — so the render needs no second query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceReport {
    pub candidates: Vec<NumberedCandidate>,
    pub already_signed: Vec<AlreadySigned>,
    pub repos_without_signed_claims: Vec<String>,
    /// My standing INFERRED claims (in scope) some of whose cited support no
    /// longer holds (DDD-8 / UC-5) — flagged only, never acted on (D-5).
    pub weakened: Vec<WeakenedClaim>,
}

/// Why one supporting claim my signed inference cites no longer supports it
/// (DDD-8 / UC-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeakenedSupport {
    /// Its own author retracted it (the shared claim-domain rule).
    Retracted,
    /// It is still cached but its author is no longer a subscribed peer (D-2).
    NoLongerEligible,
    /// No claim with its CID is in my local store any more.
    MissingLocally,
}

/// One of my standing inferred claims whose cited support weakened: the
/// count of distinct supporting claims it cites and, per reason, how many of
/// them no longer support it. The claim itself is untouched (D-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakenedClaim {
    pub cid: String,
    pub person_subject: String,
    pub philosophy: String,
    pub cited: usize,
    pub weakened: BTreeMap<WeakenedSupport, usize>,
}

/// Infer the report under `filter`, classifying each candidate against my
/// own standing adherence claims (DDD-8). Person scoping narrows the links
/// first, so the footer names only the scoped person's repos.
pub fn infer_people_report(
    links: &[ContributionLink],
    repo_claims: &[RepoClaim],
    own_claims: &[OwnClaim],
    filter: &InferenceFilter,
) -> InferenceReport {
    let person = filter.person.as_ref().map(PersonSubject::as_str);
    let scoped_links: Vec<ContributionLink> = links
        .iter()
        .filter(|link| is_in_scope(link, person))
        .cloned()
        .collect();
    let signed = standing_adherences(own_claims);
    let no_standing: Vec<&OwnClaim> = Vec::new();
    let (already_signed, candidates) = infer_person_candidates(&scoped_links, repo_claims)
        .into_iter()
        .filter(|candidate| candidate.support().len() >= filter.min_repos)
        .fold(
            (Vec::new(), Vec::new()),
            |(mut already, mut numbered), candidate| {
                let standing = signed
                    .get(&pair_key(
                        candidate.person_subject(),
                        candidate.philosophy(),
                    ))
                    .unwrap_or(&no_standing);
                match classify(&candidate, standing) {
                    Classification::New => numbered.push(NumberedCandidate {
                        candidate,
                        status: CandidateStatus::New,
                    }),
                    Classification::AlreadySigned(cid) => already.push(AlreadySigned {
                        cid: cid.to_string(),
                        candidate,
                    }),
                    Classification::Stronger {
                        supersedes,
                        still_signed,
                    } => {
                        already.extend(still_signed.iter().map(|cid| AlreadySigned {
                            cid: (*cid).to_string(),
                            candidate: candidate.clone(),
                        }));
                        numbered.push(NumberedCandidate {
                            candidate,
                            status: CandidateStatus::Stronger {
                                supersedes: supersedes.to_string(),
                            },
                        });
                    }
                }
                (already, numbered)
            },
        );
    InferenceReport {
        candidates,
        already_signed,
        repos_without_signed_claims: repos_without_signed_claims(&scoped_links, repo_claims),
        weakened: weakened_claims(&signed, repo_claims, person),
    }
}

/// DDD-8 / UC-5: my standing inferred claims in scope whose cited support
/// weakened, by pair then CID. Read-only — nothing is retracted, countered or
/// re-signed (D-5).
fn weakened_claims(
    standing: &BTreeMap<(String, String), Vec<&OwnClaim>>,
    repo_claims: &[RepoClaim],
    person: Option<&str>,
) -> Vec<WeakenedClaim> {
    let lineages: Vec<ClaimLineage<'_>> = repo_claims.iter().map(RepoClaim::lineage).collect();
    let person_key = person.map(subject_key);
    standing
        .iter()
        .filter(|((person_subject, _), _)| person_key.as_ref().is_none_or(|p| p == person_subject))
        .flat_map(|(_, claims)| {
            let mut claims = claims.clone();
            claims.sort_by(|a, b| a.cid.cmp(&b.cid));
            claims
        })
        .filter_map(|claim| weakened_claim(claim, repo_claims, &lineages))
        .collect()
}

/// `claim`'s weakened support, counted per reason over its DISTINCT cited
/// claims — `None` when every cited claim still supports it (or it cites
/// none, i.e. it was authored by hand).
fn weakened_claim(
    claim: &OwnClaim,
    repo_claims: &[RepoClaim],
    lineages: &[ClaimLineage<'_>],
) -> Option<WeakenedClaim> {
    let cited: BTreeSet<String> = parse_provenance(&claim.evidence)
        .into_iter()
        .map(|cited| cited.cid)
        .collect();
    let weakened = cited
        .iter()
        .filter_map(|cid| weakening_of(cid, repo_claims, lineages))
        .fold(BTreeMap::new(), |mut by_reason, reason| {
            *by_reason.entry(reason).or_insert(0) += 1;
            by_reason
        });
    (!weakened.is_empty()).then(|| WeakenedClaim {
        cid: claim.cid.clone(),
        person_subject: claim.subject.clone(),
        philosophy: claim.object.clone(),
        cited: cited.len(),
        weakened,
    })
}

/// Why the cited claim `cid` no longer supports an inference, resolved by
/// content address against the read: absent → missing locally; retracted by
/// its own author (the shared claim-domain rule) → retracted; held only from
/// authors who are no longer active (DDD-7 eligibility, D-2) → no longer
/// eligible; otherwise it still supports.
fn weakening_of(
    cid: &str,
    repo_claims: &[RepoClaim],
    lineages: &[ClaimLineage<'_>],
) -> Option<WeakenedSupport> {
    let rows: Vec<&RepoClaim> = repo_claims
        .iter()
        .filter(|claim| claim.cid == cid)
        .collect();
    if rows.is_empty() {
        Some(WeakenedSupport::MissingLocally)
    } else if rows
        .iter()
        .any(|row| is_self_retracted(&row.lineage(), lineages))
    {
        Some(WeakenedSupport::Retracted)
    } else if !rows.iter().any(|row| is_by_active_author(row)) {
        Some(WeakenedSupport::NoLongerEligible)
    } else {
        None
    }
}

/// How one candidate stands against my standing claims for its pair.
enum Classification<'a> {
    New,
    AlreadySigned(&'a str),
    Stronger {
        supersedes: &'a str,
        still_signed: Vec<&'a str>,
    },
}

/// DDD-8 / UC-4 / Q-CPI-D4: no standing claim → NEW; any hand-authored one
/// (cites no claim AT-URI) → already signed, never STRONGER; otherwise the
/// latest `composed_at` inferred claim is STRONGER-superseded when a repo
/// supporting the candidate now is cited by none of its claim AT-URIs — the
/// others stay listed as already signed.
fn classify<'a>(candidate: &PersonCandidate, standing: &[&'a OwnClaim]) -> Classification<'a> {
    let Some(smallest) = standing.iter().map(|claim| claim.cid.as_str()).min() else {
        return Classification::New;
    };
    if standing.iter().any(|claim| is_hand_authored(claim)) {
        return Classification::AlreadySigned(smallest);
    }
    let Some(latest) = standing
        .iter()
        .max_by(|a, b| (&a.composed_at, &a.cid).cmp(&(&b.composed_at, &b.cid)))
    else {
        return Classification::New;
    };
    if !has_uncited_supporting_repo(candidate, latest) {
        return Classification::AlreadySigned(smallest);
    }
    let mut still_signed: Vec<&str> = standing
        .iter()
        .filter(|claim| claim.cid != latest.cid)
        .map(|claim| claim.cid.as_str())
        .collect();
    still_signed.sort_unstable();
    Classification::Stronger {
        supersedes: latest.cid.as_str(),
        still_signed,
    }
}

/// UC-4: an adherence citing no claim AT-URI was authored by hand.
fn is_hand_authored(claim: &OwnClaim) -> bool {
    parse_provenance(&claim.evidence).is_empty()
}

/// Some repo supporting the candidate NOW has none of its supporting claims
/// cited by `signed` (claims matched by content address).
fn has_uncited_supporting_repo(candidate: &PersonCandidate, signed: &OwnClaim) -> bool {
    let cited: BTreeSet<String> = parse_provenance(&signed.evidence)
        .into_iter()
        .map(|cited| cited.cid)
        .collect();
    candidate.support().iter().any(|repo| {
        !repo
            .claims
            .iter()
            .any(|claim| cited.contains(&claim.cited.cid))
    })
}

/// DDD-14 change summary: how many numbered (unsigned) inferred candidates
/// the `after` report proposes for a (person, philosophy) pair the `before`
/// report did not — the scrape hint's count. Candidates present in both runs,
/// and candidates a run dropped, count nothing; identical reports give 0.
pub fn new_inferred_candidate_count(before: &InferenceReport, after: &InferenceReport) -> usize {
    let proposed_before = numbered_pair_keys(before);
    numbered_pair_keys(after)
        .difference(&proposed_before)
        .count()
}

/// The join keys of a report's numbered candidates.
fn numbered_pair_keys(report: &InferenceReport) -> BTreeSet<(String, String)> {
    report
        .candidates
        .iter()
        .map(|numbered| {
            pair_key(
                numbered.candidate.person_subject(),
                numbered.candidate.philosophy(),
            )
        })
        .collect()
}

/// The (person key, philosophy) join key of an adherence pair.
fn pair_key(person_subject: &str, philosophy: &str) -> (String, String) {
    (subject_key(person_subject), philosophy.to_string())
}

/// DDD-8: my STANDING adherence claims by pair — any adherence I signed
/// (from an inference or by hand) that is no retraction marker and that I have
/// neither retracted nor superseded (the shared claim-domain rules).
fn standing_adherences(own_claims: &[OwnClaim]) -> BTreeMap<(String, String), Vec<&OwnClaim>> {
    let lineages: Vec<ClaimLineage<'_>> = own_claims.iter().map(OwnClaim::lineage).collect();
    own_claims
        .iter()
        .filter(|claim| claim.predicate == ADHERES_TO_PHILOSOPHY)
        .filter(|claim| !is_retraction_marker(&claim.references))
        .filter(|claim| {
            let lineage = claim.lineage();
            !is_self_retracted(&lineage, &lineages) && !is_superseded_by_author(&lineage, &lineages)
        })
        .fold(BTreeMap::new(), |mut by_pair, claim| {
            by_pair
                .entry(pair_key(&claim.subject, &claim.object))
                .or_insert_with(Vec::new)
                .push(claim);
            by_pair
        })
}

fn is_retraction_marker(references: &[ClaimReference]) -> bool {
    references
        .iter()
        .any(|reference| reference.ref_type == ReferenceType::Retracts)
}

impl OwnClaim {
    fn lineage(&self) -> ClaimLineage<'_> {
        ClaimLineage {
            author_did: &self.author_did,
            cid: &self.cid,
            references: &self.references,
        }
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
    let supported: BTreeSet<String> = supporting_claims(repo_claims)
        .into_iter()
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

/// UC-6 / DDD-6 read side: every subject spelling the effect shell must read
/// with the exact-match federated query so the case-insensitive join sees all
/// claims about each linked repo — the links' own spellings plus every stored
/// subject that case-folds to a linked repo.
pub fn repo_subjects_to_read<'a>(
    links: &[ContributionLink],
    stored_subjects: impl IntoIterator<Item = &'a str>,
) -> BTreeSet<String> {
    let linked: BTreeSet<String> = links
        .iter()
        .map(|link| subject_key(&link.repo_subject))
        .collect();
    let stored_spellings = stored_subjects
        .into_iter()
        .filter(|subject| linked.contains(&subject_key(subject)))
        .map(str::to_string);
    links
        .iter()
        .map(|link| link.repo_subject.clone())
        .chain(stored_spellings)
        .collect()
}

/// DDD-7: the claims that may support an inference — each judged against the
/// whole read (retraction markers and successors arrive in the same subject
/// read, DDD-6).
fn supporting_claims(repo_claims: &[RepoClaim]) -> Vec<&RepoClaim> {
    let lineages: Vec<ClaimLineage<'_>> = repo_claims.iter().map(RepoClaim::lineage).collect();
    repo_claims
        .iter()
        .filter(|claim| supports_inference(claim, &lineages))
        .collect()
}

/// DDD-7: an `embodiesPhilosophy` claim by me or an ACTIVE peer, that is no
/// retracts/counters marker, and that its own author has neither retracted
/// (ADR-060 D-RF-D3, shared rule) nor superseded.
fn supports_inference(claim: &RepoClaim, lineages: &[ClaimLineage<'_>]) -> bool {
    let lineage = claim.lineage();
    is_philosophy_claim(claim)
        && is_by_active_author(claim)
        && !is_retraction_or_counter_marker(claim)
        && !is_self_retracted(&lineage, lineages)
        && !is_superseded_by_author(&lineage, lineages)
}

fn is_philosophy_claim(claim: &RepoClaim) -> bool {
    claim.predicate == crate::EMBODIES_PHILOSOPHY
}

/// Me, or a peer I am subscribed to NOW (D-2) — never a former peer's cache.
fn is_by_active_author(claim: &RepoClaim) -> bool {
    matches!(
        claim.relationship,
        AuthorRelationship::You | AuthorRelationship::SubscribedPeer
    )
}

/// A marker copies its target's subject/predicate/object but asserts nothing
/// new — it is never itself support.
fn is_retraction_or_counter_marker(claim: &RepoClaim) -> bool {
    claim.references.iter().any(|reference| {
        matches!(
            reference.ref_type,
            ReferenceType::Retracts | ReferenceType::Counters
        )
    })
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

    /// DDD-7 oracle, straight from the rule's text over the whole claim set:
    /// an `embodiesPhilosophy` claim by me or an ACTIVE peer that is not a
    /// retracts/counters marker, and that its OWN author has neither retracted
    /// nor superseded.
    fn supports(claim: &RepoClaim, all: &[RepoClaim]) -> bool {
        let withdrawn_by_author = |ref_type: ReferenceType| {
            all.iter().any(|other| {
                other.author_did == claim.author_did
                    && other
                        .references
                        .iter()
                        .any(|r| r.ref_type == ref_type && r.cid.0 == claim.cid)
            })
        };
        claim.predicate == crate::EMBODIES_PHILOSOPHY
            && matches!(
                claim.relationship,
                AuthorRelationship::You | AuthorRelationship::SubscribedPeer
            )
            && !claim.references.iter().any(|r| {
                matches!(
                    r.ref_type,
                    ReferenceType::Retracts | ReferenceType::Counters
                )
            })
            && !withdrawn_by_author(ReferenceType::Retracts)
            && !withdrawn_by_author(ReferenceType::Supersedes)
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
        /// claim by me or an ACTIVE peer on a repo linked to the person (any
        /// letter case), is no retracts/counters marker, and is neither
        /// retracted nor superseded by its own author; every such claim is
        /// cited by the matching candidate. A third party's counter or
        /// retract never hides it.
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
                    .filter(|claim| supports(claim, &claims) && claim.repo_subject.eq_ignore_ascii_case(&link.repo_subject))
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
            let report = infer_people_report(&links, &claims, &[], &InferenceFilter::default());
            prop_assert_eq!(numbered(&report), infer_person_candidates(&links, &claims));
            prop_assert!(report.candidates.iter().all(|n| n.status == CandidateStatus::New));
            let expected: BTreeSet<String> = links
                .iter()
                .map(|l| subject_key(&l.repo_subject))
                .filter(|repo| !claims.iter().any(|c| supports(c, &claims) && subject_key(&c.repo_subject) == *repo))
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
            let filter = InferenceFilter {
                person: Some(PersonSubject::parse(&person.to_ascii_uppercase().replacen("GITHUB:", "github:", 1)).expect("pool persons are valid")),
                min_repos: 0,
            };
            let scoped = infer_people_report(&links, &claims, &[], &filter);
            let expected: Vec<PersonCandidate> = infer_person_candidates(&links, &claims)
                .into_iter()
                .filter(|c| subject_key(c.person_subject()) == subject_key(person))
                .collect();
            prop_assert_eq!(numbered(&scoped), expected);
            let persons_repos: BTreeSet<String> = links
                .iter()
                .filter(|l| subject_key(&l.person_subject) == subject_key(person))
                .map(|l| subject_key(&l.repo_subject))
                .collect();
            let unsupported: BTreeSet<String> = persons_repos
                .into_iter()
                .filter(|repo| !claims.iter().any(|c| supports(c, &claims) && subject_key(&c.repo_subject) == *repo))
                .collect();
            let named: BTreeSet<String> = scoped.repos_without_signed_claims.iter().map(|r| subject_key(r)).collect();
            prop_assert_eq!(named, unsupported);
            let author_of: BTreeSet<(String, String)> = claims.iter().map(|c| (c.cid.clone(), bare_did(&c.author_did).to_string())).collect();
            for candidate in numbered(&scoped).iter() {
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

    proptest! {
        /// UC-6 read side: the subjects to read are every linked spelling
        /// plus every stored spelling of a linked repo (case-folded join),
        /// nothing else; re-deriving from its own output changes nothing.
        #[test]
        fn repo_subjects_to_read_are_every_spelling_of_a_linked_repo(
            (links, claims) in arb_inference_inputs(),
            extra in proptest::collection::vec("github:[a-zA-Z]{1,3}/[a-zA-Z]{1,3}", 0..4),
        ) {
            let stored: Vec<String> = claims.iter().map(|c| c.repo_subject.clone()).chain(extra).collect();
            let to_read = repo_subjects_to_read(&links, stored.iter().map(String::as_str));
            let linked: BTreeSet<String> = links.iter().map(|l| subject_key(&l.repo_subject)).collect();
            let expected: BTreeSet<String> = links
                .iter()
                .map(|l| l.repo_subject.clone())
                .chain(stored.iter().filter(|s| linked.contains(&subject_key(s))).cloned())
                .collect();
            prop_assert_eq!(&to_read, &expected);
            prop_assert_eq!(repo_subjects_to_read(&links, to_read.iter().map(String::as_str)), to_read);
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

    // --- filters, already-signed pairs and the person form (US-CPI-002
    //     AC5/AC6/AC7; DDD-8 / DDD-13 / D-8 / UC-8) ---

    /// One of my own claims about a person, drawn over the inference pools:
    /// an adherence (or not), possibly my retraction/supersession of another.
    fn arb_own_claims() -> impl Strategy<Value = Vec<OwnClaim>> {
        proptest::collection::vec(
            (
                proptest::sample::select(&crate::proptest_strategies::PERSON_POOL[..]),
                prop_oneof![Just(ADHERES_TO_PHILOSOPHY), Just("usesLanguage")],
                proptest::sample::select(vec![
                    "org.openlore.philosophy.memory-safety",
                    "org.openlore.philosophy.test-driven",
                ]),
                "bafyown[a-z2-7]{6}",
            ),
            0..5,
        )
        .prop_flat_map(|rows| {
            let n = rows.len();
            (
                Just(rows),
                proptest::collection::vec(
                    proptest::option::of((
                        prop_oneof![
                            Just(ReferenceType::Retracts),
                            Just(ReferenceType::Supersedes)
                        ],
                        0..n.max(1),
                    )),
                    n,
                ),
            )
        })
        .prop_map(|(rows, references)| {
            let cids: Vec<String> = rows.iter().map(|r| r.3.clone()).collect();
            rows.into_iter()
                .zip(references)
                .map(
                    |((person, predicate, philosophy, cid), reference)| OwnClaim {
                        subject: person.to_string(),
                        predicate: predicate.to_string(),
                        object: philosophy.to_string(),
                        author_did: "did:plc:me".to_string(),
                        // Hand-authored shape: cites no claim AT-URI (UC-4).
                        evidence: vec!["https://example.org/why".to_string()],
                        composed_at: "2026-09-01T09:00:00Z".to_string(),
                        references: reference
                            .filter(|(_, target)| cids[*target] != cid)
                            .map(|(ref_type, target)| ClaimReference {
                                ref_type,
                                cid: claim_domain::Cid(cids[target].clone()),
                            })
                            .into_iter()
                            .collect(),
                        cid,
                    },
                )
                .collect()
        })
    }

    /// Oracle: my standing adherence pairs — an adherence claim that is no
    /// retraction marker and that I have neither retracted nor superseded.
    fn standing_pairs(own: &[OwnClaim]) -> BTreeSet<(String, String)> {
        let withdrawn = |claim: &OwnClaim| {
            own.iter().any(|other| {
                other.references.iter().any(|r| {
                    r.cid.0 == claim.cid
                        && matches!(
                            r.ref_type,
                            ReferenceType::Retracts | ReferenceType::Supersedes
                        )
                })
            })
        };
        own.iter()
            .filter(|c| c.predicate == ADHERES_TO_PHILOSOPHY)
            .filter(|c| {
                !c.references
                    .iter()
                    .any(|r| r.ref_type == ReferenceType::Retracts)
            })
            .filter(|c| !withdrawn(c))
            .map(|c| (subject_key(&c.subject), c.object.clone()))
            .collect()
    }

    /// The numbered candidates of a report, without their status.
    fn numbered(report: &InferenceReport) -> Vec<PersonCandidate> {
        report
            .candidates
            .iter()
            .map(|n| n.candidate.clone())
            .collect()
    }

    fn pair_of(candidate: &PersonCandidate) -> (String, String) {
        (
            subject_key(candidate.person_subject()),
            candidate.philosophy().to_string(),
        )
    }

    proptest! {
        /// UC-8 / DDD-8 / DDD-13: filters and the already-signed exclusion
        /// apply BEFORE numbering — the numbered list is exactly the
        /// unfiltered inference, in its order, kept when in scope, supported
        /// by ≥ min-repos repos and not already signed by me; the in-filter
        /// signed ones are listed apart, each with a CID of mine for that pair.
        #[test]
        fn filters_and_signed_pairs_apply_before_numbering(
            (links, claims) in arb_inference_inputs(),
            own in arb_own_claims(),
            person in proptest::option::of(proptest::sample::select(&crate::proptest_strategies::PERSON_POOL[..])),
            min_repos in 0usize..4,
        ) {
            let filter = InferenceFilter {
                person: person.map(|p| PersonSubject::parse(p).expect("pool persons are valid")),
                min_repos,
            };
            let report = infer_people_report(&links, &claims, &own, &filter);
            let signed = standing_pairs(&own);
            let in_filter: Vec<PersonCandidate> = infer_person_candidates(&links, &claims)
                .into_iter()
                .filter(|c| person.is_none_or(|p| subject_key(c.person_subject()) == subject_key(p)))
                .filter(|c| c.support().len() >= min_repos)
                .collect();
            let (expected_signed, expected_numbered): (Vec<_>, Vec<_>) = in_filter
                .into_iter()
                .partition(|c| signed.contains(&pair_of(c)));

            prop_assert_eq!(numbered(&report), expected_numbered);
            let shown: Vec<&PersonCandidate> = report.already_signed.iter().map(|a| &a.candidate).collect();
            prop_assert_eq!(shown, expected_signed.iter().collect::<Vec<_>>());
            for already in &report.already_signed {
                prop_assert!(own.iter().any(|c| c.cid == already.cid
                    && (subject_key(&c.subject), c.object.clone()) == pair_of(&already.candidate)));
            }
        }

        /// D-8: `github:<login>` is the only accepted person form; anything
        /// else is refused with an error naming that form.
        #[test]
        fn a_person_is_accepted_only_as_github_login(
            login in "[A-Za-z0-9][A-Za-z0-9-]{0,20}",
            other in "[A-Za-z0-9:_./ -]{0,24}",
        ) {
            let named = format!("github:{login}");
            prop_assert_eq!(PersonSubject::parse(&named).map(|p| p.as_str().to_string()), Ok(named));
            let bare = PersonSubject::parse(&login);
            prop_assert!(bare.is_err());
            prop_assert!(bare.unwrap_err().to_string().contains("github:<login>"));
            let is_github_login = other
                .strip_prefix("github:")
                .is_some_and(|l| !l.is_empty() && !l.starts_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
            prop_assert_eq!(PersonSubject::parse(&other).is_ok(), is_github_login);
        }
    }

    // --- the scrape's before/after change summary (US-CPI-004; DDD-14) ---

    fn numbered_pairs(report: &InferenceReport) -> BTreeSet<(String, String)> {
        report
            .candidates
            .iter()
            .map(|n| pair_of(&n.candidate))
            .collect()
    }

    proptest! {
        /// A scrape that changed no inference input reports nothing new.
        #[test]
        fn an_unchanged_inference_reports_no_new_candidates(
            (links, claims) in arb_inference_inputs(),
            own in arb_own_claims(),
        ) {
            let report = infer_people_report(&links, &claims, &own, &InferenceFilter::default());
            prop_assert_eq!(new_inferred_candidate_count(&report, &report.clone()), 0);
        }

        /// The hint counts exactly the numbered pairs the run did not propose
        /// before — never one it already proposed, never one it dropped.
        #[test]
        fn only_candidates_absent_before_are_counted_as_new(
            (links_before, claims_before) in arb_inference_inputs(),
            (links_added, claims_added) in arb_inference_inputs(),
            own in arb_own_claims(),
        ) {
            let everything = InferenceFilter::default();
            let before = infer_people_report(&links_before, &claims_before, &own, &everything);
            let links_after: Vec<ContributionLink> = links_before.iter().chain(&links_added).cloned().collect();
            let claims_after: Vec<RepoClaim> = claims_before.iter().chain(&claims_added).cloned().collect();
            let after = infer_people_report(&links_after, &claims_after, &own, &everything);
            let expected = numbered_pairs(&after).difference(&numbered_pairs(&before)).count();
            prop_assert_eq!(new_inferred_candidate_count(&before, &after), expected);
        }
    }

    // --- STRONGER vs already signed (US-CPI-004; DDD-8 / UC-4 / Q-CPI-D4) ---

    /// My adherence claim for `candidate`'s pair, citing the supporting claims
    /// of the repos `cited_repos` selects (bit i = repo i; 0 cites no claim —
    /// a hand-authored shape), composed on September `day`.
    fn my_claim_for(
        candidate: &PersonCandidate,
        cited_repos: u8,
        day: u8,
        cid: String,
    ) -> OwnClaim {
        let evidence = candidate
            .support()
            .iter()
            .enumerate()
            .filter(|(i, _)| cited_repos & (1 << (i % 8)) != 0)
            .flat_map(|(_, repo)| repo.claims.iter().map(|c| c.cited.at_uri()))
            .chain(std::iter::once(format!(
                "https://github.com/o/r/commits?author={}",
                candidate.login()
            )))
            .collect();
        OwnClaim {
            subject: candidate.person_subject().to_string(),
            predicate: ADHERES_TO_PHILOSOPHY.to_string(),
            object: candidate.philosophy().to_string(),
            author_did: "did:plc:me".to_string(),
            cid,
            evidence,
            composed_at: format!("2026-09-{day:02}T09:00:00Z"),
            references: Vec::new(),
        }
    }

    /// My claims drawn over the inferred candidates: `(which candidate, which
    /// repos it cites, which day)`, each with a distinct CID.
    fn my_claims(candidates: &[PersonCandidate], picks: &[(usize, u8, u8)]) -> Vec<OwnClaim> {
        if candidates.is_empty() {
            return Vec::new();
        }
        picks
            .iter()
            .enumerate()
            .map(|(k, (which, cited_repos, day))| {
                my_claim_for(
                    &candidates[which % candidates.len()],
                    *cited_repos,
                    *day,
                    format!("bafymine{k}"),
                )
            })
            .collect()
    }

    fn cites_a_claim(claim: &OwnClaim) -> bool {
        claim.evidence.iter().any(|e| e.starts_with("at://"))
    }

    fn mine_for<'a>(own: &'a [OwnClaim], candidate: &PersonCandidate) -> Vec<&'a OwnClaim> {
        own.iter()
            .filter(|c| (subject_key(&c.subject), c.object.clone()) == pair_of(candidate))
            .collect()
    }

    fn numbered_status<'a>(
        report: &'a InferenceReport,
        candidate: &PersonCandidate,
    ) -> Option<&'a CandidateStatus> {
        report
            .candidates
            .iter()
            .find(|n| pair_of(&n.candidate) == pair_of(candidate))
            .map(|n| &n.status)
    }

    proptest! {
        /// UC-4: a pair I signed by hand (a claim citing no claim AT-URI) is
        /// already signed and never STRONGER, however support grows; and no
        /// STRONGER ever supersedes a hand-authored claim.
        #[test]
        fn a_claim_citing_no_at_uri_is_never_superseded(
            (links, claims) in arb_inference_inputs(),
            picks in proptest::collection::vec((0usize..16, any::<u8>(), 1u8..=28), 0..6),
        ) {
            let inferred = infer_person_candidates(&links, &claims);
            let own = my_claims(&inferred, &picks);
            let report = infer_people_report(&links, &claims, &own, &InferenceFilter::default());
            for candidate in &inferred {
                let mine = mine_for(&own, candidate);
                if mine.iter().any(|c| !cites_a_claim(c)) {
                    prop_assert_eq!(numbered_status(&report, candidate), None);
                    prop_assert!(report.already_signed.iter().any(|a| pair_of(&a.candidate) == pair_of(candidate)));
                }
            }
            for numbered in &report.candidates {
                if let CandidateStatus::Stronger { supersedes } = &numbered.status {
                    let superseded = own.iter().find(|c| &c.cid == supersedes);
                    prop_assert!(superseded.is_some_and(cites_a_claim), "supersedes a claim citing support");
                }
            }
        }

        /// DDD-8 / Q-CPI-D4: with only inferred claims for a pair, the pair is
        /// STRONGER exactly when a repo supporting it now is cited by none of
        /// the LATEST (composed_at) claim's AT-URIs; the STRONGER supersedes
        /// that latest claim and every other one stays listed as already
        /// signed. Otherwise the pair is already signed, never numbered.
        #[test]
        fn stronger_supersedes_the_latest_claim_when_support_has_an_uncited_repo(
            (links, claims) in arb_inference_inputs(),
            picks in proptest::collection::vec((0usize..16, any::<u8>(), 1u8..=28), 0..6),
        ) {
            let inferred = infer_person_candidates(&links, &claims);
            // Every claim cites at least its first supporting repo: inferred only.
            let picks: Vec<(usize, u8, u8)> = picks.into_iter().map(|(w, repos, day)| (w, repos | 1, day)).collect();
            let own = my_claims(&inferred, &picks);
            let report = infer_people_report(&links, &claims, &own, &InferenceFilter::default());
            for candidate in &inferred {
                let mine = mine_for(&own, candidate);
                let listed: BTreeSet<&str> = report
                    .already_signed
                    .iter()
                    .filter(|a| pair_of(&a.candidate) == pair_of(candidate))
                    .map(|a| a.cid.as_str())
                    .collect();
                let Some(latest) = mine.iter().max_by_key(|c| (c.composed_at.clone(), c.cid.clone())) else {
                    prop_assert_eq!(numbered_status(&report, candidate), Some(&CandidateStatus::New));
                    continue;
                };
                let cited: BTreeSet<&str> = latest.evidence.iter().filter_map(|e| e.rsplit('/').next()).collect();
                let uncited_repo = candidate
                    .support()
                    .iter()
                    .any(|repo| repo.claims.iter().all(|c| !cited.contains(c.cited.cid.as_str())));
                if uncited_repo {
                    prop_assert_eq!(
                        numbered_status(&report, candidate),
                        Some(&CandidateStatus::Stronger { supersedes: latest.cid.clone() })
                    );
                    let others: BTreeSet<&str> = mine.iter().filter(|c| c.cid != latest.cid).map(|c| c.cid.as_str()).collect();
                    prop_assert_eq!(listed, others);
                } else {
                    prop_assert_eq!(numbered_status(&report, candidate), None);
                    prop_assert_eq!(listed.len(), 1);
                }
            }
        }
    }

    // --- SUPPORT WEAKENED (US-CPI-004 AC3/AC4; DDD-8 / UC-5; D-5) ---

    /// UC-5 oracle, straight from the rule's text, for one cited CID over the
    /// whole read: absent → missing locally; retracted by its own author →
    /// retracted; only cached from a peer I no longer follow → no longer
    /// eligible; otherwise it still supports (not weakened).
    fn expected_weakening(cid: &str, all: &[RepoClaim]) -> Option<WeakenedSupport> {
        let rows: Vec<&RepoClaim> = all.iter().filter(|c| c.cid == cid).collect();
        let retracted_by_author = |row: &RepoClaim| {
            all.iter().any(|other| {
                other.author_did == row.author_did
                    && other
                        .references
                        .iter()
                        .any(|r| r.ref_type == ReferenceType::Retracts && r.cid.0 == row.cid)
            })
        };
        let active = |row: &&RepoClaim| {
            matches!(
                row.relationship,
                AuthorRelationship::You | AuthorRelationship::SubscribedPeer
            )
        };
        if rows.is_empty() {
            Some(WeakenedSupport::MissingLocally)
        } else if rows.iter().any(|row| retracted_by_author(row)) {
            Some(WeakenedSupport::Retracted)
        } else if !rows.iter().any(active) {
            Some(WeakenedSupport::NoLongerEligible)
        } else {
            None
        }
    }

    /// My inferred claim `bafymine<k>` citing, per pick, either the read's
    /// claim at that index or (past the end) a CID no store row has.
    fn my_claim_citing(claims: &[RepoClaim], picks: &[usize], k: usize) -> OwnClaim {
        let evidence = picks
            .iter()
            .map(|pick| match claims.get(*pick) {
                Some(claim) => CitedClaim::new(&claim.author_did, &claim.cid).at_uri(),
                None => CitedClaim::new("did:plc:gone", &format!("bafyabsent{pick}")).at_uri(),
            })
            .chain(std::iter::once(
                "https://github.com/o/r/commits?author=someone".to_string(),
            ))
            .collect();
        OwnClaim {
            subject: "github:someone".to_string(),
            predicate: ADHERES_TO_PHILOSOPHY.to_string(),
            object: "org.openlore.philosophy.memory-safety".to_string(),
            author_did: "did:plc:me".to_string(),
            cid: format!("bafymine{k}"),
            evidence,
            composed_at: format!("2026-09-{:02}T09:00:00Z", k + 1),
            references: Vec::new(),
        }
    }

    proptest! {
        /// DDD-8 / UC-5: each of my standing inferred claims is flagged
        /// exactly when some cited supporting claim is retracted, no longer
        /// eligible, or missing locally — with the distinct cited count and a
        /// correct count per reason; a claim whose every cited claim still
        /// supports it (or that cites none) is never flagged.
        #[test]
        fn weakened_support_counts_each_cited_claim_by_its_reason(
            (links, claims) in arb_inference_inputs(),
            cites in proptest::collection::vec(proptest::collection::vec(0usize..14, 0..6), 0..4),
        ) {
            let own: Vec<OwnClaim> = cites
                .iter()
                .enumerate()
                .map(|(k, picks)| my_claim_citing(&claims, picks, k))
                .collect();
            let report = infer_people_report(&links, &claims, &own, &InferenceFilter::default());
            for mine in &own {
                let cited: BTreeSet<String> =
                    parse_provenance(&mine.evidence).into_iter().map(|c| c.cid).collect();
                let expected: BTreeMap<WeakenedSupport, usize> = cited
                    .iter()
                    .filter_map(|cid| expected_weakening(cid, &claims))
                    .fold(BTreeMap::new(), |mut by_reason, reason| {
                        *by_reason.entry(reason).or_insert(0) += 1;
                        by_reason
                    });
                let flagged = report.weakened.iter().find(|w| w.cid == mine.cid);
                if expected.is_empty() {
                    prop_assert_eq!(flagged, None);
                } else {
                    prop_assert!(flagged.is_some(), "weakened claim {} is flagged", mine.cid);
                    let flagged = flagged.unwrap();
                    prop_assert_eq!(flagged.cited, cited.len());
                    prop_assert_eq!(&flagged.weakened, &expected);
                    prop_assert_eq!(flagged.person_subject.as_str(), mine.subject.as_str());
                }
            }
        }
    }
}
