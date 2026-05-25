# ADR-004: Async Runtime = Tokio (with rustls TLS)

- **Status**: Accepted
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect) — auto-mode default; revisit if user disagrees
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

The CLI performs two kinds of I/O:

- **Local I/O**: DuckDB queries, keychain reads, filesystem reads/writes.
  Most local I/O is synchronous in nature (DuckDB's Rust client is sync; the
  keychain libraries are sync).
- **Network I/O**: ATProto PDS calls (identity resolution, record write/read).
  The dominant Rust ATProto client (`atrium-api`) is built on `tokio` and
  `reqwest`.

We need an async runtime to drive the ATProto client, but most of the rest of
the CLI is happy being sync. The runtime choice locks us in for the lifetime
of the project; switching later is expensive.

## Decision

**Use `tokio` as the async runtime; use `rustls` (pure-Rust TLS) as the TLS
backend.**

- The CLI's `main` runs `tokio::runtime::Runtime::new()` and blocks on the
  async entry point; sync code (DuckDB, keychain) runs via
  `tokio::task::spawn_blocking` when called from async contexts, or directly
  in sync contexts when no async is needed.
- `reqwest` (transitively from `atrium-api`) is configured to use `rustls` via
  the `rustls-tls-webpki-roots` feature.
- The `pds-port` trait MUST expose async methods; the `storage-port` and
  `identity-port` traits MAY be sync (most adapters are sync internally).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **`async-std`** | Smaller community; `atrium-api` (the de-facto ATProto Rust client) is built on tokio; using `async-std` would force a runtime-compat shim. |
| **`smol`** | Lighter weight but same ecosystem-compat issue; atrium and most production HTTP clients target tokio. |
| **No async runtime; use blocking HTTP client (`ureq`)** | Viable for slice-01's narrow needs but rules out atrium-api. We would have to write our own ATProto client from XRPC primitives — significant scope expansion, low value. |
| **`native-tls` (OpenSSL system dep)** | Portability hazard on macOS (LibreSSL vs OpenSSL versioning), Linux (no system OpenSSL guaranteed in minimal containers), Windows-on-WSL2 (mixed). rustls is pure-Rust, statically linked, single binary. |

## Consequences

### Positive

- `atrium-api` is usable out of the box.
- Single statically-linked binary; no `libssl` runtime dependency.
- `tokio` is the most mature Rust async runtime; ecosystem support is broadest.

### Negative

- The CLI carries a full tokio runtime even for sync-only commands (e.g.,
  `openlore graph query`). **Mitigation**: use `tokio::runtime::Builder::new_current_thread()`
  for the CLI; avoid the multi-threaded runtime overhead since the workload is
  single-user, low-concurrency.
- `rustls` does not honor system trust stores by default. **Mitigation**: use
  `webpki-roots` (Mozilla-curated CA bundle compiled in). Acceptable for slice-01
  (the only HTTPS endpoint is the user's PDS, typically a public TLS cert from
  a major CA). Document in slice-02+ if private-CA peers become a use case.

### Earned Trust

The `pds-adapter` `probe()` MUST verify TLS is honored end-to-end. The probe
issues a `com.atproto.server.describeServer` XRPC call against the configured
PDS endpoint and confirms:

1. TLS handshake completes with no warning.
2. Returned DID matches the configured user's PDS DID.
3. Refuses to start with `health.startup.refused{reason: pds.tls_handshake_failed | pds.did_mismatch}` on either failure.
