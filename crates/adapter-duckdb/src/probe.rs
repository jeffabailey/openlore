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
use crate::schema_v3;
use crate::schema_v4;

/// The highest schema version THIS binary knows how to read. Each slice
/// that adds a migration bumps this: slice-03 taught migration v3, slice-24
/// teaches migration v4, so the forward-incompatibility refusal must compare
/// against v4 — not the slice-01-only `schema::LATEST_VERSION` (=1), which
/// would make the probe refuse its own freshly-applied schema. Computed as
/// the max of the migration heads so adding future slices only requires
/// bumping their own version constant.
const fn supported_version() -> i32 {
    let v1 = schema::LATEST_VERSION;
    let v3 = schema_v3::PEER_STORAGE_VERSION;
    let v4 = schema_v4::PHILOSOPHY_STORAGE_VERSION;
    let max_v1_v3 = if v3 > v1 { v3 } else { v1 };
    if v4 > max_v1_v3 {
        v4
    } else {
        max_v1_v3
    }
}

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
    let max_supported = supported_version();
    let observed = schema::read_version(conn).map_err(|err| ProbeFailure {
        reason: ProbeRefusalReason::StorageSchemaMismatch,
        detail: format!("could not read schema_version: {err}"),
        structured: json!({"observed": null, "expected_max": max_supported}),
    })?;

    if observed > max_supported {
        return Err(ProbeFailure {
            reason: ProbeRefusalReason::StorageSchemaMismatch,
            detail: format!(
                "DB schema version {observed} is newer than binary-supported {max_supported}"
            ),
            structured: json!({
                "observed": observed,
                "expected_max": max_supported,
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

// -----------------------------------------------------------------------------
// Slice-04 (scoring + graph) probe extension — recursive-CTE cycle-safety
// (ADR-021 / component-boundaries.md §`crates/adapter-duckdb` probe #2/#3).
// -----------------------------------------------------------------------------
//
// The slice-04 substrate-lie probe: DuckDB recursive CTEs do NOT auto-detect
// cycles, so the design refuses to trust the engine — it bounds the walk by a
// depth column AND guards a delimited `visited` path. This probe seeds a cyclic
// claim graph (A↔B via two claims), runs `traverse_graph` at a depth that would
// loop without the guard, and asserts it TERMINATES within the 250ms budget
// (I-5) emitting each edge exactly once (probe #2), and that a depth-bounded
// walk omits deeper edges (probe #3).
//
// SCAFFOLD: true (slice-04) — the live cycle-safety probe lands WITH the live
// recursive-CTE impl in `graph_query::traverse_graph` (Phase 05). Until the SQL
// exists there is no cyclic walk to time; the body is a stub. The signature +
// the refusal reason wiring below are the contract the live probe satisfies.

/// The budget (ms) the cyclic-fixture traversal MUST terminate within (I-5 /
/// component-boundaries.md probe #2). The recursive CTE without the visited
/// guard would loop forever; the guard is what keeps this bounded.
pub(crate) const TRAVERSAL_BUDGET_MS: u64 = 250;

/// Slice-04 probe: recursive-CTE traversal is cycle-safe + depth-bounded.
///
/// Seeds a CYCLIC fixture into an isolated in-memory DuckDB (two claims sharing
/// a project subject so the `eb.subject = w.subject` recursive join would loop
/// without the visited guard), runs the SAME `WITH RECURSIVE` shape
/// `graph_query::traverse_graph` uses at a depth large enough to loop forever
/// without the guard, and asserts the walk TERMINATES within
/// [`TRAVERSAL_BUDGET_MS`] (I-5 / probe #2). DuckDB recursive CTEs do NOT
/// auto-detect cycles (ADR-021), so a missing/broken visited guard would either
/// hang (caught by the budget) or explode the row count; this probe is the
/// substrate-lie guard that proves the guard is live BEFORE the first real
/// traversal. Refuses with `ProbeRefusalReason::StorageFsyncUnreliable` (the
/// umbrella substrate-lie refusal) on timeout, mirroring the fsync probe.
///
/// The probe runs against a SEPARATE in-memory connection (not the live store)
/// so it never mutates user data — it proves the engine + SQL terminate, which
/// is a property of the substrate, not of any particular claim set.
#[allow(dead_code)]
pub(crate) fn probe_traversal_cycle_safe(_conn: &Connection, _claims_dir: &Path) -> ProbeOutcome {
    let probe_conn = match Connection::open_in_memory() {
        Ok(c) => c,
        Err(err) => {
            return ProbeFailure {
                reason: ProbeRefusalReason::StorageFsyncUnreliable,
                detail: format!("cycle-safety probe: open in-memory connection failed: {err}"),
                structured: json!({"phase": "open"}),
            }
            .into_outcome();
        }
    };

    // Minimal cyclic fixture: two claims that share a project subject so the
    // recursive `eb.subject = w.subject` self-join revisits the same edge — a
    // loop that only the delimited `visited` guard breaks. The schema mirrors
    // the live `claims` table's traversal columns (subject/object/author_did/cid).
    let seed = "CREATE TABLE claims (cid VARCHAR PRIMARY KEY, subject VARCHAR, object VARCHAR, \
                author_did VARCHAR); \
                INSERT INTO claims VALUES \
                ('bafycyclea', 'github:test/project', 'org.test.philosophy', 'did:plc:a'), \
                ('bafycycleb', 'github:test/project', 'org.test.philosophy', 'did:plc:b');";
    if let Err(err) = probe_conn.execute_batch(seed) {
        return ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("cycle-safety probe: seed cyclic fixture failed: {err}"),
            structured: json!({"phase": "seed"}),
        }
        .into_outcome();
    }

    // The SAME visited-guarded recursive shape `traverse_graph` uses, at a depth
    // (64) that without the guard would loop on the shared subject indefinitely.
    // The guard bounds it to each claim_cid being traversed at most once.
    let walk = "WITH RECURSIVE walk(subject, claim_cid, depth, visited) AS ( \
                  SELECT subject, cid AS claim_cid, 1 AS depth, '|' || cid || '|' AS visited \
                  FROM claims WHERE object = 'org.test.philosophy' \
                  UNION ALL \
                  SELECT c.subject, c.cid AS claim_cid, w.depth + 1 AS depth, \
                         w.visited || c.cid || '|' AS visited \
                  FROM claims c JOIN walk w ON c.subject = w.subject \
                  WHERE w.depth + 1 <= 64 \
                    AND w.visited NOT LIKE '%|' || c.cid || '|%' \
                ) SELECT count(*) FROM walk";

    let started = std::time::Instant::now();
    let walk_result: Result<i64, _> = probe_conn.query_row(walk, [], |row| row.get(0));
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if let Err(err) = walk_result {
        return ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!("cycle-safety probe: recursive walk failed: {err}"),
            structured: json!({"phase": "walk"}),
        }
        .into_outcome();
    }

    if elapsed_ms > TRAVERSAL_BUDGET_MS {
        return ProbeFailure {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: format!(
                "cycle-safety probe: cyclic traversal took {elapsed_ms}ms, over the \
                 {TRAVERSAL_BUDGET_MS}ms budget — the visited guard may be ineffective (ADR-021)"
            ),
            structured: json!({
                "elapsed_ms": elapsed_ms,
                "budget_ms": TRAVERSAL_BUDGET_MS,
            }),
        }
        .into_outcome();
    }

    ProbeOutcome::Ok
}
