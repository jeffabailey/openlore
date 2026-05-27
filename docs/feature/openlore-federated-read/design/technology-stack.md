# Technology Stack — openlore-federated-read (slice-03) — DELTA from slice-01

- **Wave**: DESIGN
- **Date**: 2026-05-27
- **Architect**: Morgan
- **Extends**: `docs/feature/openlore-foundation/design/technology-stack.md`

Slice-03 introduces ZERO new external crates as production dependencies.
Every new capability extends existing slice-01 crates' use of their
already-pinned dependencies.

## Production crates — slice-03 surface

| Crate (already in slice-01) | New use in slice-03 | License | Justification |
|---|---|---|---|
| `atrium-api` | Peer `listRecords` + `getRecord` calls (read paths against arbitrary peer PDSes); PLC DID document fetches | MIT / Apache-2.0 (dual) | Already the ATProto client of record per ADR-004; the same crate handles authenticated and unauthenticated XRPC calls. Peer reads are unauthenticated. |
| `atrium-crypto` | Per-claim signature verification against peer DID-doc keys at pull time | MIT / Apache-2.0 | Already used for own-claim signing/verification (ADR-002). Symmetric use for peer claims. |
| `duckdb` (Rust crate) | New tables (`peer_subscriptions`, `peer_claims`, `peer_claim_references`, `peer_claim_evidence`) in the same single-file store | MIT | Already used per ADR-001. No version bump required for the new tables; the migration SQL is plain DDL. |
| `reqwest` / `rustls` | HTTPS to peer PDSes + PLC directory | Apache-2.0 / MIT (reqwest); ISC/MIT/Apache-2.0 (rustls) | Already used per ADR-004. Peer-PDS reads are HTTPS GETs/POSTs identical in shape to own-PDS reads. |
| `ciborium` | Re-serialize peer records to canonical CBOR for CID recomputation | Apache-2.0 / MIT | Already used per ADR-006. CID recomputation is symmetric for own and peer claims. |
| `cid` / `multihash` | Build CIDv1 from sha2-256(canonical CBOR) of peer records | MIT (cid), Apache-2.0 / MIT (multihash) | Already used per ADR-006. |
| `serde` / `serde_json` | Deserialize peer records; serialize new `reason` field with `skip_serializing_if` | MIT / Apache-2.0 (serde), MIT / Apache-2.0 (serde_json) | Already used. The `reason` field's `skip_serializing_if = "Option::is_none"` behavior is built-in serde. |
| `chrono` (or whatever slice-01 chose for UTC timestamps) | `subscribed_at`, `fetched_at`, `removed_at` timestamps | MIT / Apache-2.0 | Already used. |
| `clap` | New verb definitions (`peer add`, `peer pull`, `peer remove`, `claim counter`) + `--federated`, `--reason`, `--purge` flags | MIT / Apache-2.0 | Already used per slice-01. clap groups sub-verbs cleanly; no derive-macro version bump required. |
| `keyring` | Unchanged (counter-claim signing reuses the slice-01 key from OS keychain) | Apache-2.0 / MIT | Already used per ADR-002. |
| `tokio` | Async runtime for peer-PDS reads (per-peer sequential; per ADR-016 no concurrency in slice-03) | MIT | Already used per ADR-004. |
| `thiserror` | New error variants on existing `*Error` enums (`StorageError`, `IdentityError`, `PdsError`, `ClaimError`) + new `PeerStorageError` enum | MIT / Apache-2.0 | Already used per ADR-007. |
| `async-trait` | NEW peer methods on `PdsPort` are async; new `PeerStoragePort` is sync (no async-trait needed for it) | MIT | Already used per ADR-009. |
| `tracing` | New `health.startup.refused{reason: ...peer...}` event variants for slice-03 probe failures; `claim.counter.published` event for KPI-FED-3 | MIT | Already used per ADR-010. |

## NEW production dependency — Unicode normalization

ONE new crate is required:

| Crate | License | Last release | Maintenance | Purpose |
|---|---|---|---|---|
| `unicode-normalization` | MIT / Apache-2.0 | actively maintained by the Servo project; widely depended on in the Rust ecosystem (>50 million downloads/month per crates.io) | Yes — official Servo crate; >10 contributors; releases regular | NFC normalization for the counter-claim `--reason` text per ADR-015. Used by `claim-domain::normalize_reason`. |

Alternative considered: `icu_normalizer` (more comprehensive Unicode
support but heavier dependency footprint; the ICU4X family pulls in
larger dependency trees). Rejected because slice-03's normalization need
is NFC-only — `unicode-normalization` is the smaller, focused choice.

This new dependency is added to the `claim-domain` crate's
`Cargo.toml`. It is a PURE dependency (no I/O, no global state), so it
does not violate ADR-009's pure-core isolation rule. `xtask check-arch`
needs to whitelist this crate alongside `serde` as a permitted
pure-core dependency.

## Test-only / dev-dependency additions (slice-03)

| Crate | License | Purpose |
|---|---|---|
| `wiremock` (or equivalent HTTP mock crate; DELIVER's call) | MIT / Apache-2.0 | Used by integration tests to mock peer PDS responses (the adversarial-peer fixture for KPI-FED-6 + the `peer_pds_unreachable` fixture). Added to `test-support` crate's dev-deps. |

The adversarial-peer fixture is handed off to DEVOPS per the outcome-kpis
DEVOPS handoff section; DEVOPS configures the CI matrix to run with the
fixture wired into the acceptance suite.

## License compliance

All new dependencies are MIT or Apache-2.0 (or dual). No new license
considerations; the `cargo deny check licenses` allowlist from slice-01
(MIT OR Apache-2.0 OR BSD-3-Clause OR Unicode-DFS-2016) covers them all
without change.

## Versioning policy

Per slice-01: pin MAJOR.MINOR in `Cargo.toml`; let `Cargo.lock` resolve
PATCH. Slice-03 does NOT bump any slice-01 dependency's MAJOR.MINOR
unless DELIVER discovers a specific need (in which case it bumps and
documents the reason in `wave-decisions.md`).

## Supply chain (inherited)

- `cargo deny check advisories | bans | sources | licenses` runs in CI on
  every commit (slice-01 I-11). No changes for slice-03; the new
  `unicode-normalization` crate is well-known and on the existing
  allow-list patterns.
- Reproducible builds via `Cargo.lock` committed.
- No prebuilt binary dependencies (rustls statically linked; no openssl-sys).

## Rejected alternatives

| Alternative | Rejected because |
|---|---|
| New crate `adapter-peer-store` with separate sled/sqlite backend | Per WD-19 + alternatives-considered.md Choice 2 Option C: premature; slice-03 has no evidence that peer storage needs a different backend; adds a new adapter to maintain. |
| Switching to `icu_normalizer` for NFC | Heavier dependency footprint; slice-03 only needs NFC. |
| Using `tower` / `tower-http` for peer-PDS HTTP middleware (retry, rate-limit) | Premature; pull is sequential and rate-limits are deferred per task brief. atrium-api's built-in retry suffices. |
| Adding `pact-stub-server` or `pact_consumer` crates for contract tests | DEVOPS's call (per architecture's external-integration handoff); not a slice-03 production dependency. Will likely extend the slice-01 Pact suite that DEVOPS already owns. |
| New crate `federation-types` or `peer-types` for slice-03 type definitions | Premature; slice-03 type additions slot into existing crates (`ports`, `lexicon`, `claim-domain`) without bloating any of them. |

## Summary

Slice-03's technology stack is the slice-01 stack PLUS one well-known
Rust crate (`unicode-normalization`). No version bumps. No new build
tooling. No new operational concerns. The federation thesis is being
validated on the same technology surface that proved the walking
skeleton.
