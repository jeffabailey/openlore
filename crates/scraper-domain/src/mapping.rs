//! The signal->predicate mapping — the `jobs.yaml ::
//! J-004.signal_predicate_mapping` SSOT, embedded at compile time and parsed
//! purely.
//!
//! ## SSOT discipline (WD-67 / I-SCR-5)
//!
//! The mapping is the single source of truth in `docs/product/jobs.yaml`. This
//! crate embeds a snapshot of that block via [`EMBEDDED_MAPPING_YAML`]
//! (`include_str!`, a compile-time include — NOT filesystem I/O, so the
//! pure-core rule I-2 holds). The `mapping_matches_ssot` test re-extracts the
//! block from `jobs.yaml` and asserts the embedded snapshot has not diverged.
//! There is therefore exactly ONE authoritative mapping and one verified copy
//! — never a divergent hardcode.
//!
//! ## Field-naming bridge (data-models.md §"NOTE on the SSOT field naming")
//!
//! In `jobs.yaml` each entry's `predicate:` field carries a *philosophy NSID*
//! (`org.openlore.philosophy.*`). In a [`CandidateClaim`](ports::CandidateClaim)
//! this becomes the `object` (the philosophy being embodied), while the
//! relation verb (the candidate's `predicate`) defaults to
//! [`EMBODIES_PHILOSOPHY`]. [`MappingEntry`] records the SSOT value as `object`
//! so the parse never silently mis-assigns fields.

use ports::SignalKind;
use serde::Deserialize;

/// The relation verb stamped on every derived candidate. The SSOT's
/// `predicate:` value is the *philosophy* (the candidate's `object`); the
/// relation is always "embodies a philosophy" in slice-02
/// (data-models.md §"NOTE on the SSOT field naming").
pub const EMBODIES_PHILOSOPHY: &str = "embodiesPhilosophy";

/// The signal->predicate mapping snapshot, embedded from the `jobs.yaml`
/// SSOT at compile time. A pure `include_str!` — NO filesystem I/O at runtime
/// (preserves I-2). Verified against `jobs.yaml` by `mapping_matches_ssot`.
pub const EMBEDDED_MAPPING_YAML: &str = include_str!("signal_predicate_mapping.yaml");

/// Failure modes of [`load_mapping`](crate::load_mapping).
///
/// Errors are values (railway-oriented; nw-fp-domain-modeling §8). A
/// hand-written [`Display`](std::fmt::Display) keeps the pure crate free of a
/// `thiserror` dependency it does not otherwise need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingError {
    /// The embedded YAML failed to parse, or an entry's `signal` description
    /// does not resolve to a known [`SignalKind`]. Carries a human-readable
    /// detail.
    MalformedEntry(String),
    /// An entry's `object` (the `org.openlore.philosophy.*` string) does not
    /// resolve to a SEEDED philosophy (AC-007.2 / KPI-PV-6). No drift string
    /// may enter the mapping — every proposed object must `philosophy
    /// show`-resolve in the seed vocabulary. Names the offending object.
    UnknownPhilosophy {
        /// The unseeded object string that failed vocabulary resolution.
        object: String,
    },
}

impl std::fmt::Display for MappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MappingError::MalformedEntry(detail) => {
                write!(f, "malformed signal->predicate mapping entry: {detail}")
            }
            MappingError::UnknownPhilosophy { object } => {
                write!(
                    f,
                    "mapping object {object} is not a seeded philosophy \
                     (KPI-PV-6: no orphan philosophy strings)"
                )
            }
        }
    }
}

impl std::error::Error for MappingError {}

/// One parsed mapping entry: a [`SignalKind`] resolves to a philosophy NSID
/// (the candidate `object`) at the mapping default confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct MappingEntry {
    /// The harvested-signal kind this entry maps from.
    pub signal_kind: SignalKind,
    /// The philosophy NSID (`org.openlore.philosophy.*`) — the candidate's
    /// `object`. (The SSOT names this field `predicate`; see the module docs.)
    pub object: String,
    /// The mapping default confidence — always `0.25` in slice-02 (WD-52).
    pub default_confidence: f64,
}

/// The typed parse of the signal->predicate mapping SSOT.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalPredicateMapping {
    /// One entry per recognized [`SignalKind`].
    pub entries: Vec<MappingEntry>,
}

impl SignalPredicateMapping {
    /// The entry whose `signal_kind` matches `kind`, if any. `derive_candidates`
    /// uses this to look up the predicate/object/confidence for a harvested
    /// signal; a signal whose kind has no mapping entry produces no candidate.
    pub fn entry_for(&self, kind: SignalKind) -> Option<&MappingEntry> {
        self.entries.iter().find(|e| e.signal_kind == kind)
    }
}

// -----------------------------------------------------------------------------
// YAML wire shape — flat DTO mirroring the SSOT block (persistence-ignorance:
// nw-fp-domain-modeling §10; the DTO is serde-friendly, the domain type is not)
// -----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MappingDto {
    signal_predicate_mapping: Vec<EntryDto>,
}

#[derive(Debug, Deserialize)]
struct EntryDto {
    signal: String,
    predicate: String,
    default_confidence: f64,
}

/// Parse the embedded signal->predicate mapping snapshot (SSOT).
///
/// PURE: no filesystem read (the YAML is embedded via `include_str!` by the
/// caller — typically [`EMBEDDED_MAPPING_YAML`]); this is a pure parse +
/// validation pipeline: deserialize the wire DTO, then resolve and validate each
/// entry via [`parse_entry`].
pub fn load_mapping(embedded_yaml: &str) -> Result<SignalPredicateMapping, MappingError> {
    let dto: MappingDto = serde_yaml_ng::from_str(embedded_yaml)
        .map_err(|e| MappingError::MalformedEntry(format!("yaml parse failed: {e}")))?;

    let entries = dto
        .signal_predicate_mapping
        .into_iter()
        .map(parse_entry)
        .collect::<Result<Vec<_>, MappingError>>()?;

    Ok(SignalPredicateMapping { entries })
}

/// Resolve and validate one wire-shape [`EntryDto`] into a domain [`MappingEntry`].
///
/// Two validations, both errors-as-values (railway-oriented):
/// - the free-text `signal` description must resolve to a typed [`SignalKind`],
///   else [`MappingError::MalformedEntry`];
/// - the `object` (named `predicate` in the SSOT; see the module docs) must be a
///   SEEDED philosophy (AC-007.1/007.2, KPI-PV-6) — resolved against the real
///   seed vocabulary. An object that does not `philosophy show`-resolve is a
///   drift string and is rejected by NAME with [`MappingError::UnknownPhilosophy`]
///   — no orphan philosophy string can enter a mapping.
fn parse_entry(entry: EntryDto) -> Result<MappingEntry, MappingError> {
    let signal_kind = signal_kind_for_description(&entry.signal)
        .ok_or_else(|| MappingError::MalformedEntry(entry.signal.clone()))?;
    if lexicon::philosophy::find(&entry.predicate).is_none() {
        return Err(MappingError::UnknownPhilosophy { object: entry.predicate });
    }
    Ok(MappingEntry {
        signal_kind,
        object: entry.predicate,
        default_confidence: entry.default_confidence,
    })
}

/// Resolve a free-text SSOT `signal` description to a typed [`SignalKind`].
///
/// The SSOT descriptions are the canonical, human-edited prose; this match is
/// the ONE place that binds prose to the bounded `SignalKind` enum. A future
/// SSOT edit that adds a signal kind extends both this match and `SignalKind`.
fn signal_kind_for_description(description: &str) -> Option<SignalKind> {
    match description {
        "Dependency manifest pins exact versions (Cargo.lock committed, == pins)" => {
            Some(SignalKind::DependencyManifestPinned)
        }
        "Docs directory present + README > 200 lines + doc-comment density high" => {
            Some(SignalKind::DocsPresentAndSubstantial)
        }
        "Test-to-source file ratio > 0.5 OR CI runs a test matrix" => {
            Some(SignalKind::TestRatioOrCiMatrix)
        }
        "Tags follow semver + CHANGELOG present" => Some(SignalKind::SemverAndChangelog),
        "Primary language is Rust OR memory-safety language + no unsafe blocks" => {
            Some(SignalKind::MemorySafetyLanguage)
        }
        _ => None,
    }
}

// =============================================================================
// In-crate unit tests — seeded-vocabulary validation (slice-28 / US-PV-007)
// =============================================================================
//
// Layer 2 (pure core, no I/O) per nw-tdd-methodology Layered Test Discipline.
//
// US-PV-007 ("Scraper proposes seeded philosophies", job J-004) closes the
// vocabulary loop: every object the mapping proposes MUST be a KNOWN (seeded)
// philosophy, so a scraped candidate always `philosophy show`-resolves and no
// drift string (`org.openlore.philosophy.mystery`) can ever be minted.
//
//   - AC-007.1: the mapping references seeded philosophy records (single source);
//     every proposed object is a known philosophy.
//   - AC-007.2: a signal with no seeded philosophy is EXPLICIT — no drift string.
//   - KPI-PV-6: scrape → every proposed object `philosophy show`-resolves
//     (0 orphan philosophy strings).
//
// RED TODAY (the primary test below): `load_mapping` validates each entry's
// free-text `signal` description against `SignalKind`, but does NOT yet validate
// that the entry's `object` (the `org.openlore.philosophy.*` string) is a SEEDED
// philosophy. A mapping with a VALID signal but a DRIFT object therefore parses
// `Ok` today — the missing vocabulary validation is the RED cause. The test
// references only the EXISTING `load_mapping` signature (asserts `.is_err()`, no
// not-yet-existing symbol), so it COMPILES now and FAILS at runtime →
// MISSING_FUNCTIONALITY, never BROKEN.
//
// The guardrail pins KPI-PV-6 for the SHIPPED SSOT (all 5 mapping objects are
// seeded today) via `lexicon` — a pure crate (no I/O), added as a dev-dependency
// so the guardrail resolves each object against the real seed vocabulary. DELIVER
// promotes `lexicon` to a normal dependency for the production validation.

#[cfg(test)]
mod philosophy_vocabulary_tests {
    use super::*;

    /// A VALID signal description reused verbatim from the SSOT — so this entry
    /// resolves cleanly to `SignalKind::MemorySafetyLanguage` and is NOT rejected
    /// for a malformed signal. The rejection under test must come from the DRIFT
    /// OBJECT, not the signal.
    const VALID_SSOT_SIGNAL: &str =
        "Primary language is Rust OR memory-safety language + no unsafe blocks";

    // -------------------------------------------------------------------------
    // PRIMARY RED (MISSING_FUNCTIONALITY) — AC-007.2 / KPI-PV-6.
    //
    // A mapping whose object is NOT a seeded philosophy must be REJECTED. Today
    // `load_mapping` has no vocabulary check, so it returns `Ok` for the drift
    // object `org.openlore.philosophy.mystery` → this assertion FAILS (RED). The
    // signal is valid, so the failure is unambiguously the missing seeded-object
    // validation — not a malformed-signal side effect.
    // -------------------------------------------------------------------------
    #[test]
    fn load_mapping_rejects_object_not_in_seeded_vocabulary() {
        let drift_yaml = format!(
            "signal_predicate_mapping:\n  - signal: \"{VALID_SSOT_SIGNAL}\"\n    \
             predicate: org.openlore.philosophy.mystery\n    default_confidence: 0.25\n"
        );
        let result = load_mapping(&drift_yaml);
        assert!(
            result.is_err(),
            "a mapping whose object is not a SEEDED philosophy must be rejected \
             (AC-007.2 / KPI-PV-6: no drift string like \
             `org.openlore.philosophy.mystery`); got {result:?}"
        );
    }

    // -------------------------------------------------------------------------
    // ERROR-VALUE CONTRACT — AC-007.2 / KPI-PV-6.
    //
    // Rejection is a value that must NAME the offending object: an operator
    // reading the error has to see WHICH drift string failed vocabulary
    // resolution. Pins the `Display` arm added in slice-28 so it can never
    // silently degrade to an empty/opaque message.
    // -------------------------------------------------------------------------
    #[test]
    fn unknown_philosophy_display_names_the_offending_object() {
        let drift = "org.openlore.philosophy.mystery";
        let rendered = MappingError::UnknownPhilosophy {
            object: drift.to_string(),
        }
        .to_string();
        assert!(
            rendered.contains(drift),
            "the UnknownPhilosophy rejection must name the offending object \
             {drift} (KPI-PV-6: an orphan philosophy string is identified, not \
             opaque); got {rendered:?}"
        );
    }

    // -------------------------------------------------------------------------
    // GUARDRAIL (GREEN-today, no-regression) — AC-007.1 / KPI-PV-6.
    //
    // The SHIPPED SSOT mapping parses, AND every object it proposes resolves in
    // the seeded philosophy vocabulary (`lexicon::philosophy::find`). Passes
    // today because all 5 SSOT objects are seeded; guards against a future SSOT
    // edit introducing an orphan philosophy string.
    // -------------------------------------------------------------------------
    #[test]
    fn every_ssot_mapping_object_resolves_in_seeded_vocabulary() {
        let mapping = load_mapping(EMBEDDED_MAPPING_YAML)
            .expect("the shipped SSOT mapping must parse (all signals + objects known)");
        for entry in &mapping.entries {
            assert!(
                lexicon::philosophy::find(&entry.object).is_some(),
                "SSOT mapping object {} does not resolve to a seeded philosophy \
                 (KPI-PV-6: 0 orphan philosophy strings)",
                entry.object
            );
        }
    }
}
