//! State-delta + Universe assertion bootstrap (DD-3).
//!
//! Rust port of the universe-bound state-delta assertion pattern
//! (nw-tdd-methodology §"Delta-First Test Paradigm"). Per
//! `distill/wave-decisions.md` DD-3, this module is **lazily
//! bootstrapped** by DELIVER on the first state-mutating scenario.
//! Step 04-05 is that scenario: DuckDB is the first adapter whose
//! operations mutate user-observable state across multiple slots
//! (DB rows + filesystem artifacts), so the skeleton lands here.
//!
//! ## Why this module exists
//!
//! Many tests assert post-state properties of ONE slot
//! (`assert!(file.exists())`) while production code silently mutates
//! adjacent slots (an extra row in `schema_version`, a left-over
//! `.tmp` file). Universe-bound state-delta forces the test to
//! declare the FULL observable surface up-front, so hidden mutations
//! on adjacent slots become test failures, not silent debt.
//!
//! ## Status — skeleton
//!
//! Slice-01 only ships the API skeleton + a small predicate
//! vocabulary. Future steps (WS-7 CID stability, FR-3 at_uri
//! reconstructibility per `distill/wave-decisions.md` DD-3) MUST
//! migrate to `assert_state_delta(before, after, universe, expected)`
//! once they consume this module. New predicate factories
//! (`legacy_healed`, `idempotent_after`, `normalized_to`) land
//! when the consuming scenario needs them.
//!
//! ## Functional discipline
//!
//! - Pure functions only. No I/O. No global state.
//! - Predicates are closures returning `Result<(), String>` so
//!   callers can chain them through a railway pipeline.
//! - The `Delta` type is an immutable map; building one returns a
//!   fresh value (no mutation of inputs).

#![allow(dead_code)] // skeleton; first real consumer arrives in WS-7 / FR-3

use std::collections::{HashMap, HashSet};

/// A predicate over `(before_value, after_value)` pairs. Returns
/// `Ok(())` if the delta matches, `Err(reason)` otherwise.
pub type SlotPredicate<V> = Box<dyn Fn(&V, &V) -> Result<(), String>>;

/// The expected change to the observable state. Maps slot-name →
/// predicate. The `assert_state_delta` invariant: every slot in the
/// universe MUST appear here OR be implicitly `unchanged`.
pub struct Delta<V> {
    predicates: HashMap<String, SlotPredicate<V>>,
}

impl<V> Delta<V> {
    pub fn new() -> Self {
        Self {
            predicates: HashMap::new(),
        }
    }

    pub fn with_slot(mut self, name: impl Into<String>, predicate: SlotPredicate<V>) -> Self {
        self.predicates.insert(name.into(), predicate);
        self
    }
}

impl<V> Default for Delta<V> {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------------
// Predicate factories — the minimal vocabulary needed by slice-01
// -----------------------------------------------------------------------------

/// Predicate: the value at this slot is unchanged across the delta.
pub fn unchanged<V: PartialEq + std::fmt::Debug + 'static>() -> SlotPredicate<V> {
    Box::new(|before: &V, after: &V| {
        if before == after {
            Ok(())
        } else {
            Err(format!(
                "expected unchanged, got before={before:?} after={after:?}"
            ))
        }
    })
}

/// Predicate: the slot is set to `expected` after the action, regardless
/// of its prior value.
pub fn set_to<V>(expected: V) -> SlotPredicate<V>
where
    V: PartialEq + std::fmt::Debug + Clone + 'static,
{
    Box::new(move |_before: &V, after: &V| {
        if after == &expected {
            Ok(())
        } else {
            Err(format!(
                "expected after={:?}, got after={after:?}",
                expected
            ))
        }
    })
}

// -----------------------------------------------------------------------------
// Assertion driver
// -----------------------------------------------------------------------------

/// Assert that the observable state delta matches `expected` over the
/// declared `universe`. Implicit-unchanged: any universe slot NOT in
/// `expected.predicates` MUST be byte-equal between `before` and
/// `after`. Panics on mismatch with a structured message.
///
/// `before` / `after` are slot-name → value maps captured by the
/// caller's `capture_state(...)` helper at the appropriate observable
/// surface (port-exposed names only; see nw-tdd-methodology
/// "Layered test discipline — Universe per layer").
pub fn assert_state_delta<V>(
    before: &HashMap<String, V>,
    after: &HashMap<String, V>,
    universe: &HashSet<String>,
    expected: &Delta<V>,
) where
    V: PartialEq + std::fmt::Debug,
{
    let mut violations: Vec<String> = Vec::new();

    for slot in universe {
        let b = before
            .get(slot)
            .unwrap_or_else(|| panic!("slot {slot:?} missing from `before` snapshot"));
        let a = after
            .get(slot)
            .unwrap_or_else(|| panic!("slot {slot:?} missing from `after` snapshot"));

        match expected.predicates.get(slot) {
            Some(predicate) => {
                if let Err(reason) = predicate(b, a) {
                    violations.push(format!("slot {slot:?}: {reason}"));
                }
            }
            None => {
                // Implicit-unchanged: a slot in the universe but not in
                // `expected` MUST be byte-equal across the delta.
                if b != a {
                    violations.push(format!(
                        "slot {slot:?}: implicit-unchanged violated; before={b:?} after={a:?}"
                    ));
                }
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "state-delta assertion failed:\n  {}",
            violations.join("\n  ")
        );
    }
}

// -----------------------------------------------------------------------------
// Skeleton sanity test — confirms the API compiles and behaves on a
// trivial happy-path. Replaced/extended by the first real consumer
// (WS-7 / FR-3).
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assert_state_delta_passes_when_set_to_predicate_matches() {
        let mut before: HashMap<String, String> = HashMap::new();
        before.insert("claims.row".to_string(), "absent".to_string());
        before.insert("artifact.file".to_string(), "absent".to_string());

        let mut after: HashMap<String, String> = HashMap::new();
        after.insert("claims.row".to_string(), "present".to_string());
        after.insert("artifact.file".to_string(), "absent".to_string());

        let universe: HashSet<String> = ["claims.row", "artifact.file"]
            .into_iter()
            .map(String::from)
            .collect();

        let expected = Delta::new().with_slot("claims.row", set_to("present".to_string()));
        // artifact.file is implicit-unchanged.

        assert_state_delta(&before, &after, &universe, &expected);
    }

    #[test]
    #[should_panic(expected = "implicit-unchanged violated")]
    fn assert_state_delta_fails_on_hidden_mutation() {
        let mut before: HashMap<String, String> = HashMap::new();
        before.insert("claims.row".to_string(), "absent".to_string());
        before.insert("hidden.slot".to_string(), "original".to_string());

        let mut after: HashMap<String, String> = HashMap::new();
        after.insert("claims.row".to_string(), "present".to_string());
        // Hidden mutation an unobservant test wouldn't catch.
        after.insert("hidden.slot".to_string(), "mutated".to_string());

        let universe: HashSet<String> = ["claims.row", "hidden.slot"]
            .into_iter()
            .map(String::from)
            .collect();

        let expected = Delta::new().with_slot("claims.row", set_to("present".to_string()));
        // `hidden.slot` is implicit-unchanged — assertion MUST fail.

        assert_state_delta(&before, &after, &universe, &expected);
    }
}
