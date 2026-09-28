//! `people` — the pure person-inference area of the scraper domain
//! (contributor-philosophy-inference DDD-1 / DDD-3 / ADR-063 §1). Values in,
//! values out; no I/O.
//!
//! - `selection` — the ranked top-N human contributors of a repo scrape;
//! - `overlap` — cross-repo overlap and possible renames over the links;
//! - `confidence` — the DDD-9 inferred confidence in hundredths;
//! - `candidate` / `provenance` — the person candidate and its ADR-064
//!   `evidence[]` encoding;
//! - `inference` — links + signed repo claims → numbered candidates;
//! - `subject` — the validated `github:<login>` person subject;
//! - `report` / `classify` / `weakened` — one `infer people` run:
//!   filters, NEW / already signed / STRONGER, weakened support.

mod candidate;
mod classify;
mod confidence;
mod inference;
mod overlap;
mod provenance;
mod report;
mod selection;
mod subject;
mod weakened;

#[cfg(test)]
mod tests;

pub use candidate::{CandidateError, PersonCandidate, RepoClaim, SupportingClaim, SupportingRepo};
pub use confidence::{confidence_arithmetic, inferred_confidence, Hundredths};
pub use inference::{infer_person_candidates, repo_subjects_to_read};
pub use overlap::{possible_renames, shared_contributors, SharedContributor};
pub use provenance::{encode_provenance, parse_provenance, CitedClaim};
pub use report::{
    infer_people_report, new_inferred_candidate_count, AlreadySigned, CandidateStatus,
    InferenceFilter, InferenceReport, NumberedCandidate, OwnClaim,
};
pub use selection::{
    contributor_count_for, is_bot, select_contributors, ContributorCountError,
    ContributorSelection, DEFAULT_CONTRIBUTOR_COUNT, MAX_CONTRIBUTOR_COUNT,
};
pub use subject::{PersonSubject, PersonSubjectError};
pub use weakened::{WeakenedClaim, WeakenedSupport};

/// The person-level predicate an inferred adherence claim carries (ADR-064 §2).
pub const ADHERES_TO_PHILOSOPHY: &str = "adheresToPhilosophy";

/// A `github:` subject without its scheme (matched case-insensitively), e.g.
/// `github:BurntSushi` → `BurntSushi`; any other subject is returned as is.
fn strip_github_prefix(subject: &str) -> &str {
    subject
        .get(..GITHUB_PREFIX.len())
        .filter(|head| head.eq_ignore_ascii_case(GITHUB_PREFIX))
        .map_or(subject, |_| &subject[GITHUB_PREFIX.len()..])
}

const GITHUB_PREFIX: &str = "github:";

/// The join key of a (person, philosophy) adherence pair: the case-folded
/// person subject, then the philosophy id.
type PairKey = (String, String);

/// The [`PairKey`] of `person_subject` adhering to `philosophy`.
fn pair_key(person_subject: &str, philosophy: &str) -> PairKey {
    (subject_key(person_subject), philosophy.to_string())
}

/// A `github:` subject's case-folded join key (logins are case-insensitive).
fn subject_key(subject: &str) -> String {
    subject.to_ascii_lowercase()
}
