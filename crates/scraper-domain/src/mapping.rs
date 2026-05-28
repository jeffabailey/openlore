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
}

impl std::fmt::Display for MappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MappingError::MalformedEntry(detail) => {
                write!(f, "malformed signal->predicate mapping entry: {detail}")
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
/// validation step. Each entry's free-text `signal` description is resolved to
/// a typed [`SignalKind`]; an unrecognized description is a
/// [`MappingError::MalformedEntry`].
pub fn load_mapping(embedded_yaml: &str) -> Result<SignalPredicateMapping, MappingError> {
    let dto: MappingDto = serde_yaml_ng::from_str(embedded_yaml)
        .map_err(|e| MappingError::MalformedEntry(format!("yaml parse failed: {e}")))?;

    let entries = dto
        .signal_predicate_mapping
        .into_iter()
        .map(|e| {
            let signal_kind = signal_kind_for_description(&e.signal)
                .ok_or_else(|| MappingError::MalformedEntry(e.signal.clone()))?;
            Ok(MappingEntry {
                signal_kind,
                object: e.predicate,
                default_confidence: e.default_confidence,
            })
        })
        .collect::<Result<Vec<_>, MappingError>>()?;

    Ok(SignalPredicateMapping { entries })
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
