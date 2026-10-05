# Technology Stack: bluesky-claim-review-app

> **Status: IMPLEMENTED** (delivered 2026-10-04, steps 01-01..03-05). Copied from
> `docs/feature/bluesky-claim-review-app/design/` at finalize; history in
> `docs/evolution/bluesky-claim-review-app-evolution.md`.

All choices are open source and inside the `deny.toml` license allowlist (ADR-012). **Reused**
means it is already in the workspace and adds no new supply-chain surface.

| Concern | Choice | License | Status | Rationale / alternatives |
|---|---|---|---|---|
| Language / paradigm | Rust 1.91, functional core and effect shell | MIT/Apache | Reused (ADR-007) | Reuses the pure cores (CID, scraper mapping, provenance) |
| Async runtime | `tokio` | MIT | Reused (ADR-004) | — |
| HTTP server | `hyper` 1.x + `hyper-util`, hand-rolled router | MIT | Reused (`adapter-http-viewer`) | `axum`/`actix-web` stay banned (ADR-072) |
| HTML | `maud` 0.27 (pure) + vendored htmx 2.0.4 (0BSD) | MIT / 0BSD | Reused (ADR-029/031) | Server-rendered, works without JS, strict CSP (no CDN) |
| HTTP client | `reqwest` 0.12, `rustls-tls-webpki-roots` | MIT/Apache | Reused | native-tls stays forbidden (`openssl-sys` ban) |
| ATProto OAuth | `atrium-oauth` 0.1.x + `atrium-identity` + `atrium-api`, `default-features=false` | MIT | **New** (ADR-073) | Supports confidential `private_key_jwt`, DPoP nonces, refresh and revoke. Rejected: `jacquard-oauth` (MPL-2.0, not allowlisted). Fallback: `atproto-oauth` (Gerakines, MIT). Hand-rolled rejected for time to market. **SPIKE-2** |
| JOSE / ECDSA (transitive) | `jose-*`, `p256`, `elliptic-curve` (RustCrypto) | MIT/Apache | New (transitive) | ES256 client assertions and DPoP |
| Secret encryption | `chacha20poly1305` (XChaCha20-Poly1305) | Apache/MIT | **New** (ADR-074) | Pure RustCrypto. Rejected: `ring` AEAD (already transitive through rustls, but a less ergonomic nonce API; acceptable fallback) |
| Randomness | `rand` / `getrandom` (OS RNG) | MIT/Apache | Transitive | Session ids, nonces, CSRF tokens |
| Grapheme counting | `unicode-segmentation` | MIT/Apache | New (pure) | Bluesky's 300-grapheme post limit |
| Private store | DuckDB 1.3 (bundled), separate `review-app.duckdb` | MIT | Reused engine (ADR-074) | Rejected: SQLite (second engine), Postgres (operations cost) |
| GitHub | existing `adapter-github` + one fine-grained PAT (public read) | — | Reused (ADR-076) | Rejected: per-user GitHub OAuth (later), GitHub App |
| Identity resolution | `adapter-atproto-did` (resolve-only) and atrium's resolver inside OAuth | MIT | Reused / new | — |
| Logging | `tracing` + `tracing-subscriber` (JSON) | MIT | Reused / new feature | Field allowlist, no secrets |
| Container base | `gcr.io/distroless/cc-debian12` (arm64) | Apache-2.0 | New (DEVOPS) | DuckDB needs libstdc++. Rejected: `scratch`/musl (DuckDB's C++ on musl is a build risk) |
| Image registry | GHCR (`ghcr.io/jeffabailey/…`) | Hosted service; free for public repos | New (DEVOPS) | Same account as the repo, so no extra credentials for public pulls |
| Reverse proxy / TLS | Caddy 2.8 (existing container) | Apache-2.0 | Reused (ADR-075) | Exact-host site through the module import hook |
| Hosting | Existing EC2 t4g.micro (tofu-aws-pds) | — | Reused (ADR-075) | $0 extra. Fallback: t4g.small (+$6.13/month) |
| Secrets | AWS SSM Parameter Store SecureString | AWS service | Reused pattern | Client JWK, data key, GitHub PAT |
| CI | GitHub Actions; the build runs on `ubuntu-24.04-arm` | — | Reused + new job | The existing `ci.yml --workspace` covers the new crates |
| Architecture enforcement | `cargo xtask check-arch` / `check-probes`, `cargo deny` | — | Reused + 6 rule deltas | See `component-boundaries.md` §5 |
| Contract tests | Recorded-fixture contracts in `test-support` (`pact_consumer`, MIT, optional) + nightly live smoke | MIT | New (DISTILL/DEVOPS) | PDS XRPC, OAuth authorization server, PLC/handle, GitHub |

## Dependency risk notes

- **`atrium-oauth` 0.1.x**: pre-1.0 with a slow cadence (last release 2026-03-26). Mitigations:
  pin the exact version; wrap it behind `OAuthPort` so a swap to `atproto-oauth` stays local to one
  adapter; SPIKE-2 runs `cargo deny check` on the full tree.
- **Granular OAuth scopes**: their rollout status is unverified for late 2026 (SPIKE-3). Scopes are
  configuration.
- **Memory on t4g.micro**: SPIKE-4 measures app RSS during a 10-repo scan alongside the PDS.
