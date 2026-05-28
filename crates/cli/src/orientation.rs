//! `orientation` — once-per-install orientation-message gating
//! (slice-03; ADR-016 + data-models.md §"OrientationState — identity.toml
//! extensions").
//!
//! Three slice-03 verbs emit a once-per-install orientation message the
//! FIRST time they succeed: `peer pull`, `graph query --federated`, and
//! `claim counter`. The "have we shown it yet?" bit is persisted in the
//! slice-01 `~/.config/openlore/identity.toml` file under a new
//! `[federation]` section:
//!
//! ```toml
//! [federation]
//! first_pull_completed_at = "2026-05-27T10:14:32Z"
//! first_federated_query_completed_at = "2026-05-27T10:15:08Z"
//! first_counter_claim_completed_at = "2026-05-28T09:42:11Z"
//! ```
//!
//! Semantics (data-models.md):
//!
//! - **Absence (or empty value) of a key** ⇒ "the orientation message
//!   MUST fire on the next invocation of that verb." This is the
//!   `should_fire == true` state.
//! - On success of the operation, the corresponding key is written with
//!   the current UTC timestamp; failure to write is logged, not fatal.
//! - The keys are local-only (no telemetry). Deleting `identity.toml`
//!   resets all three.
//!
//! ## Pure-vs-effect split (ADR-009 / nw-fp-hexagonal-architecture)
//!
//! The "which keys have fired?" decision is PURE: it reads a parsed
//! [`FederationOrientation`] snapshot and answers `should_fire(...)`. The
//! effect side ([`load`] / [`mark_completed`]) performs the filesystem
//! read/write at the boundary. This keeps the gating logic unit-testable
//! without touching the disk.
//!
//! SCAFFOLD: false  (orientation state is real + unit-tested; the verb
//! call-sites that consult it land per-scenario in Phases 03-05.)

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The three once-per-install orientation milestones.
///
/// A choice type rather than three booleans so the call-site names the
/// milestone it is gating, and so a future fourth milestone is a single
/// variant + match-arm addition the compiler verifies exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrientationMilestone {
    /// First-ever successful `peer pull`.
    FirstPull,
    /// First-ever successful `graph query --federated`.
    FirstFederatedQuery,
    /// First-ever successful `claim counter`.
    FirstCounterClaim,
}

/// On-disk shape of the `[federation]` section in `identity.toml`.
///
/// Each field is the RFC3339 UTC timestamp of the first successful
/// invocation of the corresponding verb, or `None`/absent if it has
/// never fired. `skip_serializing_if = "Option::is_none"` keeps the file
/// minimal (an untouched install has no timestamp keys at all) and
/// preserves byte-stability for the slice-01 sections.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationOrientation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_pull_completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_federated_query_completed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_counter_claim_completed_at: Option<String>,
}

impl FederationOrientation {
    /// PURE gating decision: should the orientation message for
    /// `milestone` fire on this invocation?
    ///
    /// `true` ⇔ the corresponding key is absent OR empty (never recorded).
    /// Per data-models.md, an empty string is treated identically to a
    /// missing key so a hand-edited `identity.toml` that blanks a value
    /// re-arms the orientation.
    pub fn should_fire(&self, milestone: OrientationMilestone) -> bool {
        let slot = match milestone {
            OrientationMilestone::FirstPull => &self.first_pull_completed_at,
            OrientationMilestone::FirstFederatedQuery => &self.first_federated_query_completed_at,
            OrientationMilestone::FirstCounterClaim => &self.first_counter_claim_completed_at,
        };
        match slot {
            None => true,
            Some(value) => value.trim().is_empty(),
        }
    }

    /// PURE state transition: return a new snapshot with `milestone`
    /// recorded as completed at `timestamp`. Immutable — the caller
    /// persists the returned value; the receiver is unchanged.
    #[must_use]
    pub fn with_completed(&self, milestone: OrientationMilestone, timestamp: String) -> Self {
        let mut next = self.clone();
        match milestone {
            OrientationMilestone::FirstPull => next.first_pull_completed_at = Some(timestamp),
            OrientationMilestone::FirstFederatedQuery => {
                next.first_federated_query_completed_at = Some(timestamp)
            }
            OrientationMilestone::FirstCounterClaim => {
                next.first_counter_claim_completed_at = Some(timestamp)
            }
        }
        next
    }
}

/// Minimal mirror of the on-disk `identity.toml` carrying ONLY the
/// `[federation]` section plus a flatten-capture of every other key.
///
/// We deliberately do NOT depend on `verbs::init::IdentityToml` here:
/// this loader must preserve the slice-01 sections it does not understand
/// (handle / did / schema_version) byte-for-byte on rewrite. The
/// `#[serde(flatten)]` `rest` map captures unknown top-level keys so a
/// round-trip through [`load`] + [`mark_completed`] never drops them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct IdentityTomlWithFederation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    federation: Option<FederationOrientation>,
    #[serde(flatten)]
    rest: toml::Table,
}

/// EFFECT: read the `[federation]` orientation snapshot from
/// `identity_path`. A missing file or a missing `[federation]` section
/// both yield the default (all milestones un-fired) — a fresh install has
/// never shown any orientation message.
pub fn load(identity_path: &Path) -> Result<FederationOrientation> {
    if !identity_path.exists() {
        return Ok(FederationOrientation::default());
    }
    let text = fs::read_to_string(identity_path)
        .with_context(|| format!("read {}", identity_path.display()))?;
    let parsed: IdentityTomlWithFederation = toml::from_str(&text)
        .with_context(|| format!("parse TOML at {}", identity_path.display()))?;
    Ok(parsed.federation.unwrap_or_default())
}

/// EFFECT: record `milestone` as completed at `timestamp` in the
/// `[federation]` section of `identity_path`, preserving every other
/// section. Atomic temp+rename so a crash never leaves a partial file.
///
/// If the file does not exist yet, it is created with only the
/// `[federation]` section (the slice-01 `init` verb normally creates the
/// file first; this path is defensive).
pub fn mark_completed(
    identity_path: &Path,
    milestone: OrientationMilestone,
    timestamp: String,
) -> Result<()> {
    let mut doc: IdentityTomlWithFederation = if identity_path.exists() {
        let text = fs::read_to_string(identity_path)
            .with_context(|| format!("read {}", identity_path.display()))?;
        toml::from_str(&text)
            .with_context(|| format!("parse TOML at {}", identity_path.display()))?
    } else {
        IdentityTomlWithFederation::default()
    };

    let current = doc.federation.unwrap_or_default();
    doc.federation = Some(current.with_completed(milestone, timestamp));

    write_atomic(identity_path, &doc)
}

/// Atomic write: serialize → temp sibling → fsync → rename.
fn write_atomic(path: &Path, doc: &IdentityTomlWithFederation) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create config dir {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(doc).with_context(|| "serialize identity.toml")?;
    let tmp = path.with_extension("toml.tmp");
    {
        let mut f = fs::File::create(&tmp)
            .with_context(|| format!("create temp {}", tmp.display()))?;
        f.write_all(text.as_bytes())
            .with_context(|| format!("write temp {}", tmp.display()))?;
        f.sync_all()
            .with_context(|| format!("fsync temp {}", tmp.display()))?;
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// A missing key reads as "should fire" for every milestone — a fresh
    /// install has never shown any orientation message. This is the
    /// load-bearing default the verb call-sites depend on.
    #[test]
    fn missing_keys_fire_for_every_milestone() {
        let orientation = FederationOrientation::default();
        for milestone in [
            OrientationMilestone::FirstPull,
            OrientationMilestone::FirstFederatedQuery,
            OrientationMilestone::FirstCounterClaim,
        ] {
            assert!(
                orientation.should_fire(milestone),
                "a missing key must fire for {milestone:?}"
            );
        }
    }

    /// An empty-string value is treated identically to a missing key
    /// (data-models.md): re-arms the orientation.
    #[test]
    fn empty_value_re_arms_the_orientation() {
        let orientation = FederationOrientation {
            first_pull_completed_at: Some("   ".to_string()),
            ..Default::default()
        };
        assert!(
            orientation.should_fire(OrientationMilestone::FirstPull),
            "an empty/whitespace value must re-arm the orientation"
        );
    }

    /// Once a milestone is recorded, it no longer fires — AND the other
    /// milestones are unaffected (state-delta: only the targeted slot
    /// flips; the rest stay armed).
    #[test]
    fn recording_one_milestone_silences_only_that_milestone() {
        let before = FederationOrientation::default();
        let after = before.with_completed(
            OrientationMilestone::FirstFederatedQuery,
            "2026-05-27T10:15:08Z".to_string(),
        );

        assert!(
            !after.should_fire(OrientationMilestone::FirstFederatedQuery),
            "the recorded milestone must NOT fire again"
        );
        assert!(
            after.should_fire(OrientationMilestone::FirstPull),
            "an unrelated milestone must remain armed (unchanged)"
        );
        assert!(
            after.should_fire(OrientationMilestone::FirstCounterClaim),
            "an unrelated milestone must remain armed (unchanged)"
        );
        // The original snapshot is immutable.
        assert!(
            before.should_fire(OrientationMilestone::FirstFederatedQuery),
            "with_completed must not mutate the receiver"
        );
    }

    /// EFFECT round-trip: loading a missing file yields the default
    /// (everything armed); after mark_completed the milestone is silenced
    /// AND the slice-01 sections are preserved byte-for-byte.
    #[test]
    fn load_then_mark_completed_round_trips_and_preserves_other_sections() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("identity.toml");

        // Seed a slice-01-shaped identity.toml (no [federation] yet).
        fs::write(
            &path,
            "schema_version = 1\nhandle = \"jeff.test\"\ndid = \"did:plc:test-jeff\"\n",
        )
        .expect("seed identity.toml");

        // A fresh load sees no [federation] ⇒ everything fires.
        let loaded = load(&path).expect("load");
        assert!(loaded.should_fire(OrientationMilestone::FirstPull));

        // Record the first pull.
        mark_completed(
            &path,
            OrientationMilestone::FirstPull,
            "2026-05-27T10:14:32Z".to_string(),
        )
        .expect("mark_completed");

        // Re-load: the pull no longer fires; the federated-query milestone
        // still does.
        let reloaded = load(&path).expect("reload");
        assert!(
            !reloaded.should_fire(OrientationMilestone::FirstPull),
            "first_pull must be silenced after mark_completed"
        );
        assert!(
            reloaded.should_fire(OrientationMilestone::FirstFederatedQuery),
            "an untouched milestone must remain armed across a reload"
        );

        // The slice-01 sections survive the rewrite (no data loss).
        let text = fs::read_to_string(&path).expect("read back");
        assert!(text.contains("handle = \"jeff.test\""), "handle preserved:\n{text}");
        assert!(text.contains("did = \"did:plc:test-jeff\""), "did preserved:\n{text}");
        assert!(
            text.contains("first_pull_completed_at = \"2026-05-27T10:14:32Z\""),
            "federation key written:\n{text}"
        );
    }
}
