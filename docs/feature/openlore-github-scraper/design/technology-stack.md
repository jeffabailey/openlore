# Technology Stack — openlore-github-scraper (slice-02) — DELTA from slice-01

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Extends**: `docs/feature/openlore-foundation/design/technology-stack.md`

Slice-02 introduces ZERO new HTTP-transport crates. The GitHub client reuses
the workspace `reqwest` (rustls + webpki-roots) already pulled in by
`adapter-atproto-pds`. The only candidate NEW production dependencies are a
pure YAML parser for `scraper-domain` (to parse the embedded signal->predicate
mapping snapshot) — and that is it.

## GitHub client crate choice (the one real DESIGN call)

| Option | Verdict | Rationale |
|--------|---------|-----------|
| **`reqwest` + `serde` directly (CHOSEN)** | **SELECTED** | `reqwest 0.12` (rustls-tls-webpki-roots, json) is ALREADY in the workspace (`adapter-atproto-pds`, ADR-004). Reusing it adds ZERO new transport dependency and ZERO new `cargo deny` surface (I-11; DISCUSS handoff explicitly prefers this). The GitHub public-read surface is small and bounded (a handful of REST endpoints, or one GraphQL POST); hand-rolling typed structs with `serde` over `reqwest` is straightforward and keeps the dependency footprint minimal. License: reqwest Apache-2.0/MIT; rustls ISC/MIT/Apache-2.0 — all already on the allowlist. |
| `octocrab` (typed GitHub client) | REJECTED | A typed GitHub client is ergonomic but pulls a LARGE transitive tree (its own HTTP stack, full GitHub API model, auth machinery) for a 5-signal bounded surface — resume-driven complexity vs requirement. It would also add a NEW `cargo deny` license/advisory surface that must be vetted, contradicting the I-11 / DISCUSS preference to reuse the workspace client. Reconsider in slice-04 if the harvest surface grows substantially (deep contributor triangulation). Note: octocrab is MIT-licensed, so license is not the blocker — footprint + new supply-chain surface is. |
| `github-rs` / other GitHub SDKs | REJECTED | Less maintained than octocrab; same footprint objection; no advantage over `reqwest`+`serde` for a bounded surface. |

**Decision: `reqwest` + `serde` directly in `adapter-github`** (ADR-019). REST
vs GraphQL per signal is a DELIVER call (Q-DELIVER-2): GraphQL (one POST to
`https://api.github.com/graphql`) minimizes round-trips for the bounded signal
set and is friendlier to the anon rate budget; REST is simpler to fixture.
Either is permitted; both are public-only.

## Production crates — slice-02 surface

### Reused workspace crates (no version bump)

| Crate (already in slice-01/03) | New use in slice-02 | License | Justification |
|--------------------------------|---------------------|---------|---------------|
| `reqwest` (rustls-tls-webpki-roots, json) | HTTPS to the GitHub public API (REST and/or GraphQL); optional `Authorization: token <PAT>` header | Apache-2.0 / MIT (reqwest); ISC/MIT/Apache-2.0 (rustls) | Already the workspace HTTP client per ADR-004; reusing it avoids a new `cargo deny` surface (I-11). webpki-roots ships the Mozilla CA bundle (no system CA dependency). |
| `tokio` | Async runtime for the GitHub harvest (per-target sequential) | MIT | Already used per ADR-004. |
| `async-trait` | `GithubPort` is async (network); same pattern as `PdsPort` | MIT | Already used per ADR-009. |
| `serde` / `serde_json` | Deserialize GitHub REST/GraphQL JSON responses into typed signal structs | MIT / Apache-2.0 | Already used. |
| `thiserror` | The new `GithubError` enum + new `ProbeRefusalReason` variants | MIT / Apache-2.0 | Already used per ADR-007. |
| `tracing` | New `health.startup.refused{reason: github.*}` events + the KPI-SCR-1 / KPI-SCR-5 instrumentation hooks | MIT | Already used per ADR-010. The token value is NEVER included in any tracing field (WD-54 / no-token-leak probe). |
| `clap` | The new `scrape github <target> [--sign N[,N...]]` verb + `--sign` flag | MIT / Apache-2.0 | Already used; clap groups the sugar verb cleanly. |
| `claim-domain` | `scraper-domain` reuses the claim value shape; `cli` reuses canonicalize/compute_cid/sign for the sign step | MIT / Apache-2.0 (workspace) | Pure dependency; reused unchanged. |
| `lexicon` | `scraper-domain` references the `org.openlore.philosophy.*` NSID vocabulary | MIT / Apache-2.0 (workspace) | Pure dependency; reused unchanged. |
| `ports` | hosts the new `GithubPort` trait | workspace | Pure. |
| `url` | `GithubPort` signatures + endpoint construction (if DELIVER uses `url::Url`) | MIT / Apache-2.0 | Already a workspace dependency (added in slice-03 for peer endpoints). |

### NEW production dependency — pure YAML parser (scraper-domain)

`scraper-domain` needs to parse the embedded `jobs.yaml` signal->predicate
mapping snapshot. This is the ONLY genuinely new production dependency.

| Crate | License | Maintenance | Purpose |
|-------|---------|-------------|---------|
| `serde_yaml` (or `serde_yml` fork, or `serde_norway`) | MIT / Apache-2.0 | `serde_yaml` is widely used but in maintenance-only mode (archived by dtolnay 2024); the actively-maintained drop-in forks `serde_yml` / `serde_norway` are alternatives. DELIVER picks the maintained option (Q-DELIVER-1). | Pure parse of the embedded mapping YAML snapshot into `SignalPredicateMapping`. NO I/O (the YAML is embedded via `include_str!`); preserves the pure-core rule (I-2). |

Alternative considered: hand-parse the mapping as a `const`/Rust literal in
`scraper-domain` (no YAML dependency at all). REJECTED for slice-02 because the
SSOT is YAML in `jobs.yaml`; parsing the embedded YAML snapshot keeps a single
authoritative format and lets `mapping_matches_ssot` compare apples to apples.
If the maintained-fork situation is unsatisfactory, DELIVER MAY instead embed
the mapping as a small generated Rust table (xtask codegen from `jobs.yaml`) —
either way the SSOT is `jobs.yaml` and `mapping_matches_ssot` guards drift.

This dependency is added to `scraper-domain`'s `Cargo.toml`. It is a PURE
dependency (no I/O, no global state), so it does not violate the pure-core
isolation rule (I-2). `xtask check-arch` MUST whitelist `scraper-domain` and
its YAML-parse dependency alongside `serde` as permitted pure-core dependencies
(WD-65).

`adapter-github` adds NO new production dependency beyond what the workspace
already pins (it composes `reqwest` + `serde` + `tokio` + `tracing`).

## PAT handling

- The optional PAT is read from the `GITHUB_TOKEN` environment variable
  (WD-54; env-only for slice-02 per OD-SCR-2 / WD-63 — config-file support
  deferred).
- It is held ONLY in `adapter-github` (effect shell). `scraper-domain` (pure)
  never sees it.
- It is sent as an `Authorization: token <PAT>` header on harvest requests
  ONLY. It is NEVER logged, echoed, written to a claim, or published
  (no-token-leak probe + the contract-test assertion).
- No new secret-handling crate is added: the token is a plain env-var string
  passed to a request header (US-SCR-004 Technical Notes). It does NOT go
  through the OS keychain (that is for the signing key, ADR-002; the PAT is a
  lower-stakes, ephemeral, per-invocation credential).

## Test-only / dev-dependency additions (slice-02)

| Crate | License | Purpose |
|-------|---------|---------|
| `wiremock` (or equivalent HTTP mock; DELIVER's call) | MIT / Apache-2.0 | Mock the GitHub public API in `adapter-github` integration tests + the probe gold-tests: the public-reachability stub, the private-refusal (404) fixture, the rate-limit (403) fixture, and the rejected-token (401) fixture. Added to `test-support` dev-deps (or `adapter-github` dev-deps). Coordinated with DEVOPS for the live-vs-recorded fixture split (Q-DELIVER-6). |

## License compliance

All slice-02 dependencies are MIT or Apache-2.0 (or dual / ISC for rustls). The
one new production crate (`serde_yaml` or a maintained fork) is MIT/Apache-2.0.
The existing `cargo deny check licenses` allowlist from slice-01
(MIT OR Apache-2.0 OR BSD-3-Clause OR Unicode-DFS-2016) covers them ALL without
change (I-11). DELIVER MUST run `cargo deny check` after adding the YAML parser
and confirm no new advisory/source surface (ADR-019 acceptance criterion).

## Versioning policy

Per slice-01: pin MAJOR.MINOR in `Cargo.toml`; let `Cargo.lock` resolve PATCH.
Slice-02 does NOT bump any slice-01/03 dependency's MAJOR.MINOR. The GitHub
client is the existing `reqwest 0.12`. The new YAML parser is pinned at its
latest stable MAJOR.MINOR by DELIVER (Q-DELIVER-1).

## Supply chain (inherited)

- `cargo deny check advisories | bans | sources | licenses` runs in CI on every
  commit (I-11). The new YAML parser must be vetted by `cargo deny`; reusing
  `reqwest` adds no new transport surface to vet.
- Reproducible builds via committed `Cargo.lock`.
- No prebuilt binary dependencies (rustls statically linked; no openssl-sys —
  reusing the workspace reqwest config preserves this).

## Rejected alternatives

| Alternative | Rejected because |
|-------------|------------------|
| `octocrab` typed GitHub client | Large transitive tree + new `cargo deny` surface for a 5-signal bounded read; reuse the workspace `reqwest` per I-11 (above). |
| A new dedicated HTTP client (`hyper` direct, `ureq`, etc.) | The workspace already standardizes on `reqwest`+rustls (ADR-004); a second HTTP stack is supply-chain bloat. |
| Putting the PAT in the OS keychain (like the signing key) | The signing key is the high-stakes identity credential (ADR-002). The PAT is a lower-stakes, ephemeral GitHub read credential; env-var is the zero-friction, well-understood mechanism (WD-54) and avoids coupling GitHub auth to the keychain adapter. |
| A new `scraper-types` crate for `Signal`/`CandidateClaim` | Premature; the types live in `scraper-domain` (their natural home — the pure derivation crate). No third crate needed. |
| Hardcoding the signal->predicate mapping as a Rust literal in `scraper-domain` | Would risk divergence from the `jobs.yaml` SSOT (WD-53). Embedding the YAML snapshot + `mapping_matches_ssot` keeps a single authoritative format. (A generated Rust table from `jobs.yaml` via xtask codegen is an acceptable DELIVER alternative — SSOT still `jobs.yaml`.) |

## Summary

Slice-02's technology stack is the slice-01/03 stack PLUS one pure YAML parser
for `scraper-domain`. The GitHub client is the workspace `reqwest` (no new
transport, no new `cargo deny` surface). No version bumps. No new operational
concerns beyond the optional `GITHUB_TOKEN` env var. The cost-lowering thesis is
validated on the same technology surface that proved the walking skeleton.
