//! Startup probe gauntlet for `DuckDbStorageAdapter` (ADR-001).
//!
//! Three checks per `component-boundaries.md §"crates/adapter-duckdb"`:
//!
//! 1. **Schema version match** — `read_version(...) <= LATEST_VERSION`.
//!    Higher means the file was written by a NEWER binary; the current
//!    process cannot guarantee it understands every column.
//! 2. **Sentinel round-trip** — write a small file, read it back, byte-
//!    equal. Catches gross corruption / filesystem-level lies before
//!    the first real claim lands.
//! 3. **`fsync` honored** — write a sentinel, fsync, sync the whole
//!    filesystem, re-read. On tmpfs / overlayfs / WSL2 DrvFs the
//!    fsync can be a silent no-op; we cannot fully detect that without
//!    kernel cooperation, so we do the most pragmatic check available:
//!    file persists across an explicit `sync_all` round-trip. The
//!    limitation is documented inline.
//!
//! ## Functional discipline
//!
//! Each probe returns `Result<(), ProbeFailure>` where `ProbeFailure`
//! carries the structured detail the composition root emits as the
//! `health.startup.refused` tracing event. The driver `run_probe(...)`
//! short-circuits at the first failure (railway-oriented).

use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use duckdb::Connection;
use ports::{ProbeOutcome, ProbeRefusalReason};
use serde_json::json;

use crate::schema;

/// One probe step's failure, paired with the externally-visible
/// refusal reason and a structured JSON detail blob.
struct ProbeFailure {
    reason: ProbeRefusalReason,
    detail: String,
    structured: serde_json::Value,
}

impl ProbeFailure {
    fn into_outcome(self) -> ProbeOutcome {
        ProbeOutcome::Refused {
            reason: self.reason,
            detail: self.detail,
            structured: self.structured,
        }
    }
}

/// Walk the probe gauntlet. Returns `Ok` if all three checks pass; the
/// first failure short-circuits.
///
/// `claims_dir` is where `<cid>.json` artifacts live; the fsync /
/// sentinel checks operate there because that's the medium the adapter
/// actually writes to.
pub fn run_probe(conn: &Connection, claims_dir: &Path) -> ProbeOutcome {
    match probe_schema_version(conn)
        .and_then(|_| probe_sentinel_roundtrip(claims_dir))
        .and_then(|_| probe_fsync_honored(claims_dir))
    {
        Ok(()) => ProbeOutcome::Ok,
        Err(failure) => failure.into_outcome(),
    }
}

/// Probe 1: refuse if the DB file's schema version is HIGHER than
/// what this binary knows about.
fn probe_schema_version(conn: &Connection) -> Result<(), ProbeFailure> {
    let observed = schema::read_version(conn).map_err(|err| ProbeFailure {
        reason: ProbeRefusalReason::StorageSchemaMismatch,
        detail: format!("could not read schema_version: {err}"),
        structured: json!({"observed": null, "expected_max": schema::LATEST_VERSION}),
    })?;

    if observed > schema::LATEST_VERSION {
        return Err(ProbeFailure {
            reason: ProbeRefusalReason::StorageSchemaMismatch,
            detail: format!(
                "DB schema version {observed} is newer than binary-supported {}",
                schema::LATEST_VERSION
            ),
            structured: json!({
                "observed": observed,
                "expected_max": schema::LATEST_VERSION,
            }),
        });
    }

    Ok(())
}

/// Probe 2: write a small sentinel to `<claims_dir>/.probe-sentinel`,
/// read it back, assert byte-equal. Catches gross filesystem corruption
/// / permission errors / wrong-mount-point before the first real
/// claim lands.
///
/// Maps any failure to `StorageFsyncUnreliable` per the spec — the
/// fsync-unreliable refusal variant is the umbrella for "filesystem
/// medium is unsafe", which includes round-trip mismatches.
fn probe_sentinel_roundtrip(claims_dir: &Path) -> Result<(), ProbeFailure> {
    fs::create_dir_all(claims_dir).map_err(|err| ProbeFailure {
        reason: ProbeRefusalReason::StorageFsyncUnreliable,
        detail: format!("could not create claims dir: {err}"),
        structured: json!({"path": claims_dir.display().to_string()}),
    })?;

    let path = claims_dir.join(".probe-sentinel");
    let payload = b"openlore-storage-probe-v1";

    {
        let mut f = fs::File::create(&path).map_err(|err| ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("could not create sentinel: {err}"),
            structured: json!({"path": path.display().to_string()}),
        })?;
        f.write_all(payload).map_err(|err| ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("could not write sentinel: {err}"),
            structured: json!({"path": path.display().to_string()}),
        })?;
        f.sync_all().map_err(|err| ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("sync_all on sentinel failed: {err}"),
            structured: json!({"path": path.display().to_string()}),
        })?;
    }

    let mut observed = Vec::new();
    fs::File::open(&path)
        .and_then(|mut f| f.read_to_end(&mut observed))
        .map_err(|err| ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("could not re-read sentinel: {err}"),
            structured: json!({"path": path.display().to_string()}),
        })?;

    if observed != payload {
        return Err(ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: "sentinel round-trip mismatch".to_string(),
            structured: json!({
                "path": path.display().to_string(),
                "expected_bytes": payload.len(),
                "observed_bytes": observed.len(),
            }),
        });
    }

    // Cleanup the sentinel; failure to remove is non-fatal (the file
    // is small and harmless if it lingers across crashes).
    let _ = fs::remove_file(&path);

    Ok(())
}

/// Probe 3: `fsync` honored. Write a sentinel, `sync_all` it, then
/// `sync_all` on the directory handle (POSIX requires this to commit
/// the directory entry).
///
/// ## Limitation (data-models.md write-strategy comment)
///
/// Detecting that the kernel SILENTLY no-ops fsync on tmpfs /
/// overlayfs / WSL2 DrvFs requires platform-specific kernel
/// cooperation we don't have at the userspace boundary. The pragmatic
/// check here verifies the file persists across the sync — a minimum
/// bar. Deeper detection (e.g. comparing inode metadata before/after
/// or probing `statfs` for tmpfs) is deferred to a later step;
/// document the gap so the composition root knows the probe is
/// best-effort.
fn probe_fsync_honored(claims_dir: &Path) -> Result<(), ProbeFailure> {
    let path = claims_dir.join(".probe-fsync");
    let payload = b"openlore-fsync-probe-v1";

    {
        let mut f = fs::File::create(&path).map_err(|err| ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("could not create fsync sentinel: {err}"),
            structured: json!({"path": path.display().to_string()}),
        })?;
        f.write_all(payload).map_err(|err| ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("could not write fsync sentinel: {err}"),
            structured: json!({"path": path.display().to_string()}),
        })?;
        f.sync_all().map_err(|err| ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("sync_all failed: {err}"),
            structured: json!({"path": path.display().to_string()}),
        })?;
    }

    // Also sync the parent directory so the directory entry is durable.
    if let Ok(dir) = fs::File::open(claims_dir) {
        let _ = dir.sync_all();
    }

    if !path.exists() {
        return Err(ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: "fsync sentinel disappeared after sync_all".to_string(),
            structured: json!({"path": path.display().to_string()}),
        });
    }

    let _ = fs::remove_file(&path);
    Ok(())
}
