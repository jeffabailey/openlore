# Technology Stack — openlore-foundation (slice-01)

- **Wave**: DESIGN
- **Date**: 2026-05-25
- **Architect**: Morgan
- **OSS-first**: every crate below is permissive open source (MIT or Apache-2.0
  or dual-licensed); no proprietary dependencies.

## Locked context (from DISCUSS)

- Language: **Rust** (user-locked)
- Storage: **DuckDB** for slice-01 (WD-8; ADR-001; re-opens slice-04)
- Federation: **ATProto** (user-locked)
- Identity: **existing ATProto DID + per-app derived key** (WD-12; ADR-002)

## DESIGN-wave decisions

- Async runtime: **tokio** (ADR-004)
- TLS: **rustls** with `webpki-roots` (ADR-004)
- Paradigm: **functional-leaning** (ADR-007 — Proposed)
- Architecture style: **Hexagonal + Modular Monolith** single Rust workspace
  (ADR-009)
- Lexicon namespace: **`org.openlore.*`** (ADR-005)
- Claim addressing: **CIDv1 dag-cbor sha2-256 base32 lower** (ADR-006)
- Retraction: **counter-claim referencing original CID, no hard-delete** (ADR-008)

## Per-crate rationale

Versions are MAJOR.MINOR floors; DELIVER picks PATCH and locks via `Cargo.lock`.

### Core (pure)

| Crate | Floor | License | Purpose | Rationale |
|---|---|---|---|---|
| `serde` | 1.0 | MIT/Apache-2.0 | Trait-based (de)serialization | Universal Rust convention; required for Lexicon serde models. |
| `serde_json` | 1.0 | MIT/Apache-2.0 | JSON for the canonical on-disk artifact files | The on-disk signed claim is JSON (greppable per US-002); CID is computed from CBOR canonicalization of the same logical payload. |
| `ciborium` | 0.2 | MIT/Apache-2.0 | RFC 8949 canonical CBOR encode/decode | Maintained by the `enarx` team; tracks the CBOR spec; mature alternative to the older `serde_cbor`. |
| `cid` | 0.11 | MIT | IPLD CIDv1 type + base32 encoding | Multiformats canonical Rust crate. |
| `multihash` | 0.19 | MIT | sha2-256 multihash for the CID | Pairs with `cid`. |
| `thiserror` | 1.0 | MIT/Apache-2.0 | Ergonomic error enum derivation | Idiomatic for typed error ADTs (ADR-007 paradigm). |
| `anyhow` | 1.0 | MIT/Apache-2.0 | Error propagation in the `cli` driver only | NOT used in `claim-domain` or `lexicon` (pure-core uses typed errors). Acceptable in the effect shell. |

### ATProto (effect)

| Crate | Floor | License | Purpose | Rationale |
|---|---|---|---|---|
| `atrium-api` | 0.24 | MIT/Apache-2.0 | ATProto XRPC client | De-facto Rust ATProto client; the only mature option. Risk: API still pre-1.0; pin MAJOR.MINOR and re-evaluate per release. |
| `atrium-xrpc` | 0.11 | MIT/Apache-2.0 | XRPC transport layer for atrium | Sibling of atrium-api. |
| `atrium-crypto` | 0.1 | MIT/Apache-2.0 | ATProto-compatible signing primitives (k256 / Ed25519) | Same maintainers; matches the verification-method expectations of ATProto PDS implementations. |
| `reqwest` | 0.12 | MIT/Apache-2.0 | HTTPS client (transitively from atrium) | tokio-native; rustls feature enabled (no OpenSSL system dep). |

### Storage (effect)

| Crate | Floor | License | Purpose | Rationale |
|---|---|---|---|---|
| `duckdb` | 1.0 | MIT | DuckDB Rust client | Official DuckDB Rust crate; matches the locked storage choice (ADR-001). |
| `directories` | 5.0 | MIT/Apache-2.0 | XDG paths (`~/.config/openlore`, `~/.local/share/openlore`) | Cross-platform XDG without hand-rolling the lookups; honors the persona's local-first values. |

### Identity / keys (effect)

| Crate | Floor | License | Purpose | Rationale |
|---|---|---|---|---|
| `keyring` | 3.0 | MIT/Apache-2.0 | OS keychain abstraction (macOS Keychain, Linux Secret Service, Windows Credential Manager) | Single API across the three target platforms; WSL2 fallback handled in adapter (ADR-002). |
| `ed25519-dalek` | 2.1 | BSD-3 | Ed25519 signing primitive | Most-audited Rust Ed25519 implementation. Used only inside `adapter-atproto-did` and the pure `Signer` function in `claim-domain` (which takes the key as input — no I/O). |
| `did-key` OR `did-plc-resolver` (vendor TBD) | TBD | MIT/Apache-2.0 | DID document resolution | Stack is in flux. DELIVER picks one and writes the adapter accordingly. Atrium ships some DID resolution; may be sufficient. |

### Async runtime + I/O (effect)

| Crate | Floor | License | Purpose | Rationale |
|---|---|---|---|---|
| `tokio` | 1.40 | MIT | Async runtime | ADR-004; single-threaded current-thread flavor for the CLI. |
| `tracing` | 0.1 | MIT | Structured logging (incl. `health.startup.refused` event emission) | Enables DEVOPS's observability story; structured events not strings. |
| `tracing-subscriber` | 0.3 | MIT | Init log destinations (stderr by default; JSON for `--log-json`) | DEVOPS wires for production telemetry. |

### CLI surface (driver)

| Crate | Floor | License | Purpose | Rationale |
|---|---|---|---|---|
| `clap` | 4.5 | MIT/Apache-2.0 | Argument parsing | The Rust CLI convention; derives subcommands from structs. |
| `dialoguer` | 0.11 | MIT | TTY prompts (confirm Enter, Y/n publish) | Required for the two-prompt contract (ADR-003); offers `Confirm`, `Input`, `Editor` primitives. |
| `console` | 0.15 | MIT | Color + bold for the compose preview (subtle; ADR-001 persona expects greppable text but colorized stderr framing is acceptable) | Used only for terminal styling, never embedded in the signed payload. |

### Testing + tooling (dev-deps)

| Crate / tool | Purpose | Notes |
|---|---|---|
| `proptest` | Property tests against canonicalization, CID, reference rules | Per ADR-006 + ADR-008 Earned Trust. |
| `cargo-mutants` | Mutation testing against `claim-domain` and `lexicon` | ≥95% kill rate target per ADR-006; CI gate. |
| `cargo-deny` | License + supply-chain checks | First-stage CI gate. |
| `assert_cmd` + `predicates` | CLI integration tests (drive the binary, assert stdout/stderr) | Validates the two-prompt observable contract (ADR-003). |
| `wiremock` | Mock HTTP server for `adapter-atproto-pds` integration tests | Avoids hitting a real PDS in CI; gold tests still record against real PDS periodically. |
| `cargo-nextest` | Test runner | Faster than `cargo test`; parallel execution; better output. |
| `cargo-fuzz` (libfuzzer-based) | Fuzz the canonicalization function | Slice-04+ aspiration; slice-01 can ship without if time-boxed. |

## Architecture enforcement tooling

Per ADR-009:

- **`cargo-deny`** for license/ban policy (license whitelist, advisory checks).
- **`xtask check-arch`** (a custom workspace member; not a published crate) —
  parses `cargo metadata` to enforce:
  - `claim-domain` + `lexicon` MUST NOT transitively depend on `tokio`,
    `reqwest`, `duckdb`, `keyring`.
  - `adapter-*` crates MUST NOT depend on each other.
  - Only `cli` may depend on `adapter-*`.
- **`xtask check-probes`** + **`scripts/check-probes.sh`** — AST walker
  (using the `syn` crate) over every `impl <Port> for <Adapter>` to assert
  `probe()` has a real body. Runs as pre-commit hook AND in CI.

## Rejected technologies (with rationale)

| Rejected | Reason |
|---|---|
| `openssl` (native OpenSSL via reqwest) | Replaced by rustls (ADR-004). |
| `serde_cbor` | Less actively maintained than `ciborium`. |
| `async-std` | atrium uses tokio; runtime split would force a shim (ADR-004). |
| `structopt` | Superseded by `clap` v4 derive macros. |
| `sled` (embedded KV) | Sled is functionally a key-value store; we want SQL queries (ADR-001). |
| `rusqlite` (alternative SQL embedded) | DuckDB is the user-locked choice (WD-8). SQLite is the natural alternative if DuckDB's Rust client becomes a blocker; flagged in ADR-001 Revisit Trigger. |
| `actix-web`, `axum`, `tower` | No HTTP server in slice-01; CLI only. |

## License posture

All listed crates are MIT, Apache-2.0, or BSD-3-Clause. None are LGPL or GPL.
`cargo-deny`'s license check enforces the whitelist `MIT OR Apache-2.0 OR
BSD-3-Clause OR Unicode-DFS-2016`. Any new dependency introducing a copyleft
license fails the build.

## OSS health snapshot (sampled 2026-05-25)

Per the architecture-patterns skill, OSS evaluation criteria. DELIVER may
re-snapshot before locking versions; this is the design-time picture.

| Crate | Last release | Maintenance signal |
|---|---|---|
| `atrium-api` | active (multiple 2025-2026 releases) | RISK: pre-1.0; tracking ATProto Lexicon evolution. DELIVER must pin and re-evaluate per release. |
| `duckdb` (Rust) | tracks DuckDB 1.x | Stable since DuckDB 1.0 (mid-2024); active. |
| `ciborium` | mature | RFC 8949 compliant; maintained by reputable org. |
| `cid` / `multihash` | mature | Multiformats canonical. |
| `tokio` | LTS | Industry standard. |
| `rustls` | mature | Industry standard. |
| `clap` | mature | Industry standard. |
| `keyring` | active | Some platform-specific quirks documented; ADR-002 probe catches them. |

## Crate-pin open question (for DELIVER)

The exact PATCH versions (and any version-pin conflicts in the atrium /
reqwest / rustls transitive graph) are NOT locked here. DELIVER (software-
crafter) resolves the dependency graph and commits `Cargo.lock`. If a pin
conflict forces a substantive tech change (e.g. `atrium-api` requires a
`reqwest` version that breaks `rustls` integration), come back to DESIGN.
