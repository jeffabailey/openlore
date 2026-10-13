//! Approving several suggestions at once: what a ticked card submits, and
//! which of the owner's pending suggestions a selection names. Pure; the
//! route builds and keeps one publish plan per chosen suggestion, so the
//! batch preview and confirm keep the Plan-value guarantee (ADR-074).

use ports::{Suggestion, SuggestionKey};

/// The form field each ticked card submits.
pub const SELECTION_FIELD: &str = "selected";

/// The most suggestions one batch previews and publishes.
pub const MAX_SELECTED: usize = 25;

/// The value a card's checkbox submits: subject, predicate and object,
/// space-separated (none of them contains a space).
pub fn selection_value(key: &SuggestionKey) -> String {
    format!("{} {} {}", key.subject, key.predicate, key.object)
}

/// The key a submitted value names, if it is exactly three non-empty parts.
pub fn selection_key(value: &str) -> Option<SuggestionKey> {
    let mut parts = value.split(' ');
    let (subject, predicate, object) = (parts.next()?, parts.next()?, parts.next()?);
    let complete = parts.next().is_none()
        && [subject, predicate, object]
            .iter()
            .all(|part| !part.is_empty());
    complete.then(|| SuggestionKey {
        subject: subject.to_string(),
        predicate: predicate.to_string(),
        object: object.to_string(),
    })
}

/// The owner's pending suggestions the submitted `values` name, in queue
/// order, each once, at most [`MAX_SELECTED`]. A value naming anything else
/// (someone else's, already published, malformed) selects nothing.
pub fn chosen_suggestions(pending: Vec<Suggestion>, values: &[String]) -> Vec<Suggestion> {
    let named: Vec<SuggestionKey> = values.iter().filter_map(|v| selection_key(v)).collect();
    pending
        .into_iter()
        .filter(|suggestion| named.contains(&suggestion.key))
        .take(MAX_SELECTED)
        .collect()
}

/// What confirming one plan of a batch did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanResult {
    /// The record landed at this `at://` address.
    Published(String),
    /// Nothing was written; `kept_plan` names the plan put back for Retry.
    Failed {
        reason: String,
        needs_sign_in: bool,
        kept_plan: Option<String>,
    },
    /// Its preview had expired, so it was not published.
    Expired,
    /// No longer the owner's to publish (no longer pending or visible).
    Skipped,
}

/// A confirmed batch, summed up for its result page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BatchTally {
    pub published: Vec<String>,
    pub failures: Vec<String>,
    pub retry_plan_ids: Vec<String>,
    pub expired: usize,
    /// Any write found the PDS session ended: sign in again before Retry.
    pub needs_sign_in: bool,
}

/// Sum each plan's result, in order.
pub fn tally_batch(results: Vec<PlanResult>) -> BatchTally {
    results
        .into_iter()
        .fold(BatchTally::default(), |mut tally, result| {
            match result {
                PlanResult::Published(at_uri) => tally.published.push(at_uri),
                PlanResult::Failed {
                    reason,
                    needs_sign_in,
                    kept_plan,
                } => {
                    tally.failures.push(reason);
                    tally.needs_sign_in |= needs_sign_in;
                    tally.retry_plan_ids.extend(kept_plan);
                }
                PlanResult::Expired => tally.expired += 1,
                PlanResult::Skipped => {}
            }
            tally
        })
}
