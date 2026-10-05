# ADR-074: Review-App Private State in a Separate, Owner-Scoped DuckDB File with AEAD-Encrypted Secrets

- **Status**: Accepted (2026-10-04) — implemented; see `docs/evolution/bluesky-claim-review-app-evolution.md` (proposed 2026-10-04)
- **Date**: 2026-10-04
- **Deciders**: Morgan (nw-solution-architect); retention and backup posture flagged for Jeff Bailey
- **Feature**: bluesky-claim-review-app (DESIGN), resolves OD-BRA-5 and the storage half of OD-BRA-2

## Context

The app keeps four kinds of private, multi-user state:

- pending, declined and approved suggestions;
- the GitHub ownership link;
- OAuth state (in-flight authorizations, token sets, DPoP keys) and browser sessions;
- short-lived publish/share plans and aggregate KPI counters.

I-BRA-1 and I-BRA-2 make pending and declined suggestions **readable only by their owner**.
NFR-BRA-2 says no long-lived credential may be exposed. Scale is under 50 users on one instance,
one process, co-located on a t4g.micro (ADR-075). The workspace already ships DuckDB 1.3
(bundled), with two stores and a precedent for a separate file per bounded context (ADR-025
`index.duckdb`).

## Decision

1. **One new DuckDB file, `review-app.duckdb`, owned by the new adapter
   `crates/adapter-review-store`.** It is not added to `adapter-duckdb`. That adapter is the CLI
   user's local store, and the CLI must never link hosted multi-user state (ADR-072 disjoint
   roots). The schema is in `docs/feature/bluesky-claim-review-app/design/data-models.md`.
   - It is a single connection behind a mutex, as in the existing adapters.
   - It sets `memory_limit='64MB'` and `threads=1` so it coexists with the PDS on 1 GiB.
2. **Owner scoping is enforced at three layers** (Earned Trust):
   - **Type.** The owner-data port is only reachable through an `OwnerScope` value, and an
     `OwnerScope` is only constructed from an authenticated session (the session table maps a
     cookie hash to a DID). No port method takes a raw DID for owner data.
   - **Structural.** A new `xtask` rule, `review_store_owner_scoped_sql`, requires that every SQL
     literal in `adapter-review-store` touching an owner table carries an `owner_did = ?`
     predicate. It also requires that `DELETE` appears only in the purge and expiry modules.
   - **Behavioral.** The adapter's `probe()` writes a canary row for synthetic DID A inside a
     transaction, reads through scope B, expects nothing back, then rolls back. If the read leaks,
     the app refuses to start. There is also an acceptance test for cross-user denial
     (NFR-BRA-1).
3. **Secrets at rest are AEAD-encrypted** with XChaCha20-Poly1305 (the `chacha20poly1305` crate,
   Apache-2.0/MIT, pure RustCrypto).
   - This covers OAuth token sets, DPoP private keys and PKCE verifiers.
   - The associated data is `owner_did ‖ column name ‖ schema version`, so a ciphertext cannot be
     swapped between rows.
   - The 256-bit data key comes from an SSM SecureString at startup and is never written into the
     DuckDB file.
   - The probe round-trips a canary ciphertext, which also catches a wrong key after rotation.
   - Browser session ids are stored only as a SHA-256 hash.
4. **Retention (OD-BRA-5)**:
   - Suggestions, declines and the GitHub link are kept until **disconnect** (US-BRA-012), which
     purges every row for the DID in one transaction.
   - Plans expire after 30 minutes. In-flight OAuth requests expire after 10 minutes. Browser
     sessions expire after 30 idle days.
   - **A declined key is never re-offered in v1**, even with new evidence. The key is
     `(owner_did, subject, predicate, object)`.
5. **KPI counters (OD-BRA-11)** are `(day, event, count)` rows with **no DID, handle or content
   column**. Durations are stored as histogram-bucket counts.
6. **No backup of `review-app.duckdb` in v1.**
   - Published claims live in users' PDSes and are unaffected.
   - Losing the volume would lose pending suggestions (recoverable by rescanning) and declines (a
     declined suggestion could be re-offered once).
   - Backing up private data would widen its exposure.
   - **This is flagged for Jeff's decision.** The alternative is a nightly snapshot encrypted to
     the existing backup RSA key, reusing the identity-backup pattern.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **SQLite (`rusqlite`, bundled)** | A better OLTP fit with a smaller footprint. Rejected for v1: it adds a second SQL engine and C build, and duplicates the existing DuckDB adapter patterns and the `xtask` SQL scanners. At under 50 users DuckDB is more than adequate. Revisit if write contention or memory shows up in SPIKE-4. |
| **Extend `adapter-duckdb` / `openlore.duckdb`** | Rejected. It would mix the CLI user's local store with hosted multi-tenant private data and break ADR-072's disjoint capability sets. |
| **Postgres (RDS or a container)** | Rejected. It adds operations cost and RAM on the t4g.micro, for no requirement at this scale. |
| **Store pending suggestions in the user's PDS as private records** | Rejected. ATProto repos are public. That breaks I-BRA-1 and D-4. |
| **Rely on EBS encryption only** | Rejected as insufficient on its own. The PDS container mounts all of `/pds`, so app-level AEAD keeps tokens opaque even to co-located processes and to any copy of the file. |

## Consequences

- **Positive**: no new database engine, and the privacy invariant is enforced at type, structure
  and runtime. Disconnect is a single transactional purge.
- **Negative**:
  - DuckDB is columnar and is used here for small OLTP. That is acceptable at this scale, but
    write latency should be watched.
  - Without a backup, KPI-BRA-4b (0 re-offered declines) can be violated after volume loss.
- **Revisit triggers**: more than 500 users; p95 write latency above 50 ms; a second app instance
  (DuckDB is single-process).
