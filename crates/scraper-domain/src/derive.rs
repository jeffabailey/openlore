//! `derive_candidates` — the PURE slice-02 derivation (J-004b load-bearing
//! surface; the mutation-test target of the slice).
//!
//! Maps each harvested [`Signal`](ports::Signal) to a candidate via the
//! [`SignalPredicateMapping`], collapsing multiple signals for ONE predicate
//! into ONE [`CandidateClaim`](ports::CandidateClaim) that lists all
//! contributing signals (US-SCR-002 Example 4). Every candidate is stamped with
//! the mapping default confidence `0.25` (WD-52 / I-SCR-3) and — by routing
//! construction through [`CandidateClaim::try_new`](ports::CandidateClaim::try_new)
//! — is guaranteed to name at least one source signal (I-SCR-4).
//!
//! ## Functional discipline
//!
//! Pure: values in, values out; no I/O, no mutation of inputs, no clock/RNG.
//! Determinism is structural — candidates come out in first-appearance order of
//! their predicate among the input signals, so identical inputs always yield
//! identical output (the determinism property, component-boundaries.md).

use ports::{CandidateClaim, Signal};

use crate::mapping::{SignalPredicateMapping, EMBODIES_PHILOSOPHY};

/// Derive auditable candidate claims from already-harvested public GitHub
/// signals via the signal->predicate mapping.
///
/// - `subject` is the resolved `github:<owner>/<repo>` or `github:<user>`
///   target string (the `github_target` shared artifact; the caller resolves
///   it once via `adapter-github`).
/// - Each candidate's `object` is the philosophy NSID from the mapping; its
///   `predicate` is the relation verb [`EMBODIES_PHILOSOPHY`].
/// - Multiple signals mapping to the SAME predicate COLLAPSE into one candidate
///   listing all contributing signals; that candidate's `evidence` carries each
///   contributing signal's `source_url`.
/// - Signals whose kind has no mapping entry are silently dropped (not an
///   error). Zero matching signals -> empty `Vec` (US-SCR-002 Example 2).
///
/// Output order is the first-appearance order of each predicate among
/// `signals`, making the derivation deterministic.
pub fn derive_candidates(
    subject: &str,
    signals: &[Signal],
    mapping: &SignalPredicateMapping,
) -> Vec<CandidateClaim> {
    group_signals_by_predicate(signals, mapping)
        .into_iter()
        .map(|group| build_candidate(subject, group))
        .collect()
}

/// A predicate (philosophy NSID) and the ordered signals that produced it.
struct PredicateGroup {
    object: String,
    confidence: f64,
    signals: Vec<Signal>,
}

/// Group the mappable signals by their predicate, preserving first-appearance
/// order of both predicates and the signals within each predicate. Signals
/// with no mapping entry are dropped here (the only place a signal can vanish).
fn group_signals_by_predicate(
    signals: &[Signal],
    mapping: &SignalPredicateMapping,
) -> Vec<PredicateGroup> {
    let mut groups: Vec<PredicateGroup> = Vec::new();
    for signal in signals {
        let Some(entry) = mapping.entry_for(signal.kind) else {
            continue; // signal kind not in the SSOT mapping — dropped, not an error
        };
        match find_group(&mut groups, &entry.object) {
            Some(group) => group.signals.push(signal.clone()),
            None => groups.push(PredicateGroup {
                object: entry.object.clone(),
                confidence: entry.default_confidence,
                signals: vec![signal.clone()],
            }),
        }
    }
    groups
}

/// The existing group for `object`, if one was already started.
fn find_group<'a>(
    groups: &'a mut [PredicateGroup],
    object: &str,
) -> Option<&'a mut PredicateGroup> {
    groups.iter_mut().find(|g| g.object == object)
}

/// Assemble one collapsed candidate from a predicate group. Routes through
/// [`CandidateClaim::try_new`]; the group is non-empty by construction (it
/// exists only because at least one signal joined it), so the non-empty
/// invariant holds and the `expect` is unreachable in practice.
fn build_candidate(subject: &str, group: PredicateGroup) -> CandidateClaim {
    let evidence = group
        .signals
        .iter()
        .map(|s| s.source_url.clone())
        .collect::<Vec<_>>();
    CandidateClaim::try_new(
        subject.to_string(),
        EMBODIES_PHILOSOPHY.to_string(),
        group.object,
        evidence,
        group.confidence,
        group.signals,
    )
    .expect("a predicate group always has >=1 contributing signal (I-SCR-4)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{load_mapping, EMBEDDED_MAPPING_YAML};
    use ports::SignalKind;

    /// SC-4 collapse coverage, preserved as a DIRECT pure-`derive_candidates`
    /// unit (RGSD-6): three DISTINCT `DocsPresentAndSubstantial` signals
    /// (distinct `value`s + distinct `source_url`s) all map to the single
    /// `documentation-first` predicate and COLLAPSE into exactly ONE
    /// `CandidateClaim` whose `source_signals()` lists all THREE contributors
    /// (US-SCR-002 Ex 4 / auditability I-SCR-4).
    ///
    /// Real detection can NEVER emit three same-kind signals (`detect_signals`
    /// fires each kind at most once), so this behavior is unreachable through
    /// the CLI once the `signals[]` scaffold is gone — it lives here, at the
    /// pure derivation, independent of the scaffold. This lets STEP 2 delete the
    /// SC-4 acceptance scenario safely.
    #[test]
    fn three_same_predicate_signals_collapse_into_one_candidate_listing_all_three() {
        let mapping =
            load_mapping(EMBEDDED_MAPPING_YAML).expect("embedded SSOT mapping must parse");
        let subject = "github:some-org/well-documented";

        let signals = vec![
            Signal {
                kind: SignalKind::DocsPresentAndSubstantial,
                value: "docs/ directory present".to_string(),
                source_url: "https://github.com/some-org/well-documented/tree/main/docs"
                    .to_string(),
            },
            Signal {
                kind: SignalKind::DocsPresentAndSubstantial,
                value: "README 412 lines (> 200)".to_string(),
                source_url: "https://github.com/some-org/well-documented/blob/main/README.md"
                    .to_string(),
            },
            Signal {
                kind: SignalKind::DocsPresentAndSubstantial,
                value: "doc-comment density high (0.34)".to_string(),
                source_url: "https://github.com/some-org/well-documented".to_string(),
            },
        ];

        let candidates = derive_candidates(subject, &signals, &mapping);

        assert_eq!(
            candidates.len(),
            1,
            "three same-predicate signals must collapse into exactly ONE candidate (I-SCR-4)"
        );
        let candidate = &candidates[0];
        assert_eq!(
            candidate.object, "org.openlore.philosophy.documentation-first",
            "the collapsed candidate must be grouped under the documentation-first predicate"
        );
        assert_eq!(
            candidate.source_signals().len(),
            3,
            "the collapsed candidate must list all THREE contributing signals (auditability I-SCR-4)"
        );
        // The auditable detail lives in each signal's `value` — all three must
        // survive the collapse verbatim.
        let values: Vec<&str> = candidate
            .source_signals()
            .iter()
            .map(|s| s.value.as_str())
            .collect();
        assert!(values.contains(&"docs/ directory present"));
        assert!(values.contains(&"README 412 lines (> 200)"));
        assert!(values.contains(&"doc-comment density high (0.34)"));
    }
}
