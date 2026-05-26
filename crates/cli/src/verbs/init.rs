//! `init` — bootstrap identity + DuckDB, idempotent on re-run (WS-1).
//!
//! Contract (from `tests/acceptance/walking_skeleton.rs::walking_skeleton_init_creates_identity_duckdb_and_is_idempotent`):
//!
//! - First run: creates `$XDG_CONFIG_HOME/openlore/identity.toml`, opens
//!   or creates the DuckDB file at `$XDG_DATA_HOME/openlore/openlore.duckdb`
//!   with the slice-01 schema (via DuckDbStorageAdapter::open), prints
//!   `OpenLore initialized for <did>` to stdout, exits 0.
//! - Re-run: detects existing identity.toml, prints
//!   `already initialized for <did>`, exits 0.
//!
//! The verb does NOT contact the PDS in slice-01. Real did:plc resolution
//! is slice-03; for now the DID is taken from the `OPENLORE_DID`
//! environment variable (set by tests; explicit error if absent in
//! production).
//!
//! ## Pure-vs-effect split (ADR-009)
//!
//! The verb's "decide what to do" logic is pure: given the
//! `IdentityToml` snapshot, decide whether to bootstrap or report-already.
//! The "do it" side performs the filesystem write (atomic temp+rename)
//! and the DuckDB open. The storage adapter's `open` already runs the
//! schema migration as part of construction, so the verb itself does not
//! need to invoke migrations explicitly.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::wiring::Wiring;

/// Argument struct for the `init` verb (mirrors the clap subcommand).
pub struct InitArgs {
    pub handle: String,
    pub app_password: String,
}

/// Outcome of one `init` invocation. The exit code + stdout chunk the
/// dispatcher emits.
pub struct InitOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Contents of `<config>/identity.toml`. Pure data; the file is the
/// persistence layer for "the user has already run `init`". Field shape
/// is locked at slice-01: the handle the user provided, the resolved
/// DID, and a schema version so future migrations stay forward-compatible.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityToml {
    pub schema_version: i32,
    pub handle: String,
    pub did: String,
}

impl IdentityToml {
    /// Current schema version for the identity.toml file. Bumped when
    /// the on-disk shape changes; slice-01 is version 1.
    pub const CURRENT_SCHEMA_VERSION: i32 = 1;
}

/// Run the init verb. Idempotent on re-invocation: detects existing
/// identity.toml and short-circuits to the "already initialized"
/// message without rewriting state.
pub fn run(wiring: &Wiring, args: &InitArgs) -> Result<InitOutcome> {
    let identity_path = wiring.paths.identity_toml();
    let did = wiring.identity.author_did().0.clone();

    if identity_path.exists() {
        // Idempotency path. Read the persisted identity to confirm the
        // DID matches what the IdentityPort exposes; mismatch is a
        // hard error (the user re-ran init under a different DID).
        let persisted = read_identity_toml(&identity_path)
            .with_context(|| format!("reading {}", identity_path.display()))?;
        if persisted.did != did {
            return Err(anyhow!(
                "identity.toml at {} records did={} but IdentityPort reports did={}; \
                 refusing to overwrite — remove the config dir manually to re-bootstrap",
                identity_path.display(),
                persisted.did,
                did
            ));
        }
        return Ok(InitOutcome {
            exit_code: 0,
            stdout: format!("already initialized for {did}\n"),
        });
    }

    // Bootstrap path. Storage adapter has already been constructed (via
    // Wiring::production), which ran the schema migration and created
    // the data dir + claims/ subdir. All that remains is writing the
    // identity TOML atomically.
    let toml_doc = IdentityToml {
        schema_version: IdentityToml::CURRENT_SCHEMA_VERSION,
        handle: args.handle.clone(),
        did: did.clone(),
    };
    write_identity_toml(&identity_path, &toml_doc)
        .with_context(|| format!("writing {}", identity_path.display()))?;

    Ok(InitOutcome {
        exit_code: 0,
        stdout: format!("OpenLore initialized for {did}\n"),
    })
}

/// Read + parse the identity TOML. Errors carry context for the
/// dispatcher's tracing event.
fn read_identity_toml(path: &Path) -> Result<IdentityToml> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let parsed: IdentityToml = toml::from_str(&text)
        .with_context(|| format!("parse TOML at {}", path.display()))?;
    Ok(parsed)
}

/// Write the identity TOML atomically: serialize → write to a temp
/// sibling → fsync → rename. POSIX guarantees rename(2) is atomic on
/// the same filesystem, so a crash mid-write never leaves a partial
/// file at the canonical path.
fn write_identity_toml(path: &Path, toml_doc: &IdentityToml) -> Result<()> {
    // Ensure the parent (config dir) exists.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create config dir {}", parent.display()))?;
    }

    let text = toml::to_string_pretty(toml_doc)
        .with_context(|| "serialize identity.toml")?;
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

    /// Roundtrip property: IdentityToml serializes + deserializes
    /// losslessly. Pins the on-disk contract.
    #[test]
    fn identity_toml_roundtrips_through_toml_serialization() {
        let original = IdentityToml {
            schema_version: 1,
            handle: "jeff.test".to_string(),
            did: "did:plc:test-jeff".to_string(),
        };
        let text = toml::to_string_pretty(&original).expect("serialize");
        let parsed: IdentityToml = toml::from_str(&text).expect("deserialize");
        assert_eq!(parsed, original);
    }

    /// Schema version is pinned at 1 for slice-01. Bumping this is a
    /// migration event; failing this test loudly signals that intent.
    #[test]
    fn identity_toml_schema_version_is_one_at_slice_01() {
        assert_eq!(IdentityToml::CURRENT_SCHEMA_VERSION, 1);
    }
}
