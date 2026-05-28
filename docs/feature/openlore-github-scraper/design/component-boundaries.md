# Component Boundaries — openlore-github-scraper (slice-02) — DELTA from slice-01

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Style**: Hexagonal + Modular Monolith (ADR-009, inherited)
- **Paradigm**: Functional-leaning Rust (ADR-007, inherited)
- **Extends**: `docs/feature/openlore-foundation/design/component-boundaries.md`

This document specifies ONLY the component-boundary deltas for slice-02.
Slice-01's `claim-domain`, `lexicon`, `ports`, `adapter-duckdb`,
`adapter-atproto-did`, `adapter-atproto-pds`, `adapter-system-clock`, and
`cli` crates (plus the slice-03 in-place extensions) are inherited unchanged.
Slice-02 ADDS two crates and EXTENDS `ports` + `cli` + `xtask`.

## Crate layout (slice-02 additions in bold)

```
openlore/                          # workspace root
  crates/
    claim-domain/                  # PURE — unchanged (reused for the sign step)
    lexicon/                       # PURE — unchanged (NO Lexicon change; WD-62)
    ports/                         # PURE — extended (NEW GithubPort trait + TargetKind)
    adapter-duckdb/                # EFFECT — unchanged (slice-01 store write reused)
    adapter-atproto-did/           # EFFECT — unchanged (slice-01 signing reused)
    adapter-atproto-pds/           # EFFECT — unchanged (slice-01 publish reused)
    adapter-system-clock/          # EFFECT — unchanged
    scraper-domain/                # **NEW — PURE** (candidate derivation; no I/O)
    adapter-github/                # **NEW — EFFECT** (GitHub API; probe; optional PAT)
    cli/                           # DRIVER — extended (scrape github verb + pre-fill)
  xtask/                           # extended (two new crates in check-arch + check-probes)
```

Production crate count: 8 -> **10** (the FIRST crate addition since slice-01;
slice-03 added zero per WD-26). This sits AT the informal ~10 cap noted in
ADR-009's Revisit Trigger; ADR-019 documents that the two new crates are
justified by the pure/effect split (WD-56/57) and that no meta-crate grouping
is warranted yet. The brief's Component Inventory gains two rows at finalize
(handoff note; not edited now, per slice-03 precedent).

## Component contract deltas

### `crates/scraper-domain` (NEW — PURE)

**Responsibility**: derive auditable candidate claims from already-harvested
GitHub signals via the signal->predicate mapping. PURE: takes values in,
returns values out; NO I/O. This is the J-004b load-bearing surface and the
mutation-test target of the slice.

**Public surface (sketch — exact shape is DELIVER's)**:

```rust
/// A public GitHub artifact or measurable property harvested by adapter-github.
/// Carries enough to (a) name itself in a candidate's source-signal line and
/// (b) be mapped to a predicate by the SSOT mapping.
pub struct Signal {
    pub kind: SignalKind,        // typed; matches a mapping entry
    pub value: String,           // human-readable detail ("test ratio 0.61", "Cargo.lock committed")
    pub source_url: String,      // the public GitHub URL that evidences the signal (-> candidate evidence)
}

pub enum SignalKind {
    DependencyManifestPinned,    // Cargo.lock committed / == pins
    DocsPresentAndSubstantial,   // docs/ + README > 200 lines + doc-comment density
    TestRatioOrCiMatrix,         // test/source ratio > 0.5 OR CI test matrix
    SemverAndChangelog,          // semver tags + CHANGELOG present
    MemorySafetyLanguage,        // Rust OR memory-safe lang + no unsafe
    // extensible — bounded by the SSOT mapping; adapter need not harvest more
}

/// A PROPOSAL derived purely from one or more Signals. In-memory ONLY;
/// never persisted as-is, never signed without the human's gesture.
pub struct CandidateClaim {
    pub subject: String,             // github:<owner>/<repo> or github:<user>
    pub predicate: String,           // e.g. "embodiesPhilosophy" (the relation)
    pub object: String,              // the philosophy NSID from the mapping (org.openlore.philosophy.*)
    pub evidence: Vec<String>,       // public GitHub URL(s) from the contributing signal(s)
    pub confidence: f64,             // LOAD-BEARING: always the mapping default (0.25); never auto-inflated
    pub source_signals: Vec<Signal>, // LOAD-BEARING: non-empty; names the exact signal(s) that produced it
}

pub struct SignalPredicateMapping { /* typed parse of the jobs.yaml SSOT */ }

pub enum MappingError { MalformedEntry(String), DivergedFromSsot }

/// The PURE derivation. Maps each Signal to a candidate via the mapping;
/// collapses multiple signals for one predicate into ONE candidate that lists
/// all contributing signals (US-SCR-002 Example 4); stamps default_confidence.
pub fn derive_candidates(signals: &[Signal], mapping: &SignalPredicateMapping) -> Vec<CandidateClaim>;

/// Parse the embedded jobs.yaml signal->predicate mapping snapshot (SSOT).
/// No filesystem read (embedded via include_str! by DELIVER); pure parse.
pub fn load_mapping(embedded_yaml: &str) -> Result<SignalPredicateMapping, MappingError>;
```

**Forbidden dependencies** (per I-2, enforced by `xtask check-arch`):
`tokio`, `reqwest`, `duckdb`, `keyring`, `atrium-api`, `std::fs`, `std::net`,
`std::process`, `std::env`, `std::time::SystemTime`, any `adapter-*` crate.
**Permitted**: `claim-domain` + `lexicon` (both pure) for the claim shape /
predicate vocabulary; `serde` + a pure YAML parser (`serde_yaml` or
equivalent) for the mapping parse (pure, no I/O). The pure-core allowlist in
`xtask check-arch` MUST be extended to whitelist `scraper-domain` and its pure
YAML-parse dependency (WD-65).

**Probe responsibilities** (pure crate — property/unit tests, not a runtime
probe):

- Property: `derive_candidates` is deterministic (same signals + mapping ->
  same candidates, same order).
- Property: every produced `CandidateClaim.source_signals` is NON-EMPTY
  (auditability invariant; KPI-SCR-3).
- Property: every produced `CandidateClaim.confidence == 0.25` (the mapping
  default; no auto-inflation; WD-52).
- Unit: multiple signals mapping to one predicate collapse into ONE candidate
  listing all contributing signals (US-SCR-002 Example 4).
- Unit: zero matching signals -> empty `Vec` (not an error; US-SCR-002
  Example 2).
- Build-time: `mapping_matches_ssot` — the embedded mapping snapshot equals
  `docs/product/jobs.yaml :: J-004.signal_predicate_mapping` (WD-53; no
  divergent hardcode).

### `crates/adapter-github` (NEW — EFFECT)

**Responsibility**: implement `GithubPort` over the GitHub PUBLIC REST/GraphQL
API using the workspace `reqwest` (rustls). Resolve a target (repo vs user),
refuse private/non-existent targets, harvest the bounded signal set, read the
optional `GITHUB_TOKEN`, detect rate limits, and ship a `probe()` within the
250ms budget (I-5). The PAT is an effect-shell credential held ONLY here.

**Public surface**: `pub struct AdapterGithub; #[async_trait] impl GithubPort for AdapterGithub { ... }`

**Forbidden dependencies** (per I-1/I-3): other `adapter-*` crates;
`adapter-github` holds NO reference to `StoragePort`, `IdentityPort`, or
`PdsPort` (by construction it CANNOT sign or publish — the human-gate at the
architecture layer). **Permitted**: `ports`, `scraper-domain` (for the `Signal`
type returned by harvest), `reqwest` (workspace), `tokio`, `async-trait`,
`serde`/`serde_json`, `thiserror`, `tracing`.

**Probe responsibilities** (per ADR-019 + ADR-009 I-4/I-5; see
architecture-design §6.3):

1. Public reachability: `resolve_target` against a stable PUBLIC fixture
   returns `TargetKind::Repo` within the 250ms budget.
2. Private refusal: `resolve_target` against a known-private/inaccessible
   fixture returns `GithubError::NotPublic` (NOT a silent empty harvest;
   KPI-SCR-4).
3. Auth-mode: if `GITHUB_TOKEN` is set and rejected (401), refuse to start
   (`GithubTokenRejected`); if accepted, report the rate budget.
4. Rate-limit-header presence: assert the budget-reporting path parses the
   headers.
5. No-token-leak: assert the token value never appears in any structured probe
   event or log line.

### `crates/ports` (PURE) — extension

**Slice-02 additions to public surface**:

```rust
#[async_trait::async_trait]
pub trait GithubPort {
    fn probe(&self) -> ProbeOutcome;

    /// Disambiguate `owner/repo` (Repo) vs `user` (User); REFUSE private /
    /// non-existent targets. Public-data-only (WD-51).
    async fn resolve_target(&self, target: &str) -> Result<TargetKind, GithubError>;

    /// Harvest the bounded public-signal set for a repo. Returns already-fetched
    /// Signals ready for the pure derive_candidates.
    async fn harvest_repo(&self, owner: &str, repo: &str) -> Result<Vec<Signal>, GithubError>;

    /// Harvest a BOUNDED cross-repo aggregate for a user/contributor target
    /// (deep triangulation deferred to slice-04 per WD-64).
    async fn harvest_user(&self, user: &str) -> Result<Vec<Signal>, GithubError>;
}

pub enum TargetKind {
    Repo { owner: String, repo: String },
    User { user: String },
}

pub enum GithubError {
    NotFound { target: String },          // 404 — target named in error; zero candidates
    NotPublic { target: String },         // private/inaccessible — "scraper only reads public data"
    RateLimited { authenticated: bool },  // 403 rate budget exhausted — "set GITHUB_TOKEN" remediation
    TokenRejected,                        // 401 — stale/invalid PAT; value NEVER echoed
    Network(String),                      // offline / transport — "scrape requires network"
    ApiShape(String),                     // unexpected response shape (contract drift)
}

// ProbeRefusalReason gains slice-02 variants:
pub enum ProbeRefusalReason {
    // ... slice-01 + slice-03 variants ...
    GithubPublicApiUnreachable,
    GithubPrivateNotRefused,              // probe step 2 failed — a private target was NOT refused (KPI-SCR-4)
    GithubTokenRejected,
    GithubRateLimitHeadersMissing,
}
```

NOTE on type placement: this design places `Signal` + `CandidateClaim` in
`scraper-domain` and references them from `ports`' `GithubPort` signatures
(`ports` depends on `scraper-domain`; both pure, so I-1/I-2 hold). DELIVER may
instead place `Signal` in `ports` if that dependency direction proves cleaner
(Q-DELIVER-3). Either way both crates stay pure.

**Forbidden dependencies** (unchanged): `GithubPort` is async (network) so
`#[async_trait]` is permitted (same justification as `PdsPort`). Traits may
reference `lexicon`, `claim-domain`, and (for `Signal`/`CandidateClaim`)
`scraper-domain` types — all pure.

**Probe responsibilities**: none (traits don't probe; implementations do).

### `crates/cli` (DRIVER) — extension

**Slice-02 additions to responsibility**: parse the `scrape github <target>
[--sign N[,N...]]` verb via clap; extend `Wiring` to construct `AdapterGithub`
(reading `GITHUB_TOKEN` from env) alongside slice-01 adapters; extend
`ProbeGauntlet` to include `GithubPort.probe()` (skipped under `--offline`,
which is incompatible with `scrape`); implement `VerbScrapeGithub`,
`CandidateRenderer`, `CandidatePrefill`, and `SelectionParser`; reuse
`VerbClaimAdd` + `VerbClaimPublish` internals UNCHANGED for the sign step.

**Public surface addition**: `pub fn main() -> ExitCode` is the existing entry
point; clap subcommand definitions extend internally. No new public symbols.

**Forbidden dependencies** (unchanged): none — `cli` is the composition root
(the ONLY crate that may wire `adapter-github` into `GithubPort`, per I-3).

**Probe responsibilities** (slice-02 additions, per ADR-017):

1. `scrape github <target>` WITHOUT `--sign` writes ZERO `author_claims` rows,
   makes ZERO PDS calls, writes ZERO `claims/<cid>.json` files (the human-gate
   storage probe; KPI-SCR-2).
2. `scrape github <target>` prints the public-data banner BEFORE any harvest.
3. The compose preview reached from a candidate contains the literal "not as
   truth" (inherited I-7).
4. The publish-success message reached from a sign-from-scraper claim mentions
   the retract command (inherited I-8).
5. An out-of-range / duplicate `--sign` index is rejected BEFORE any compose
   begins, naming the offending indices (US-SCR-003 / US-SCR-005).
6. `--sign N` with no edits produces a signed claim whose fields equal the
   candidate's proposed values byte-for-byte, confidence 0.25 (no
   auto-inflation; `candidate_confidence_no_autoinflate`).
7. Offline `scrape` exits non-zero with a "requires network" message and
   renders no partial list.

### `xtask/` (workspace member) — extension

**Slice-02 additions to responsibility**:

- Extend `check-arch` to cover the two new crates: `scraper-domain` is PURE
  (no I/O deps; whitelist `scraper-domain` + its pure YAML-parse dep in the
  pure-core allowlist, WD-65); `adapter-github` is an effect adapter wired ONLY
  by `cli` (I-3) and depending on no other `adapter-*` (I-1).
- Extend `check-probes` to assert `impl GithubPort for <Adapter>` blocks
  contain a non-stub `probe()` body (same mechanism as slice-01/03 for other
  ports).

**Public surface**: `cargo xtask check-arch` (extended crate set),
`cargo xtask check-probes` (extended trait set). Run from CI on every commit.

### Unchanged crates

`claim-domain`, `lexicon`, `adapter-duckdb`, `adapter-atproto-did`,
`adapter-atproto-pds`, `adapter-system-clock` are UNCHANGED. The
sign-from-scraper path reuses `claim-domain` (canonicalize/compute_cid/sign),
`adapter-duckdb` (StoragePort write), `adapter-atproto-did` (IdentityPort
sign), and `adapter-atproto-pds` (PdsPort publish) exactly as a hand-authored
claim does. `lexicon` is unchanged because provenance is display-only (WD-62)
— NO new field on `org.openlore.claim`.

## Cross-component invariants — slice-02 additions (enforced)

| # | Invariant | Enforced by |
|---|-----------|-------------|
| I-SCR-1 (human-gate) | The scraper PROPOSES; the human SIGNS. `scrape github` without `--sign` produces ZERO signed/persisted/published claims. The ONLY path from a `CandidateClaim` to a `SignedClaim` is `CandidatePrefill -> VerbClaimAdd` with the human's signing gesture. | Architecture (`adapter-github` holds no storage/identity/pds ref) + `xtask check-arch` + acceptance gate `scraper_never_persists_unsigned` |
| I-SCR-2 (public-data-only) | `adapter-github` calls ONLY public GitHub endpoints; private/non-existent targets are refused. | `adapter-github` probe step 2 + the public-endpoint allowlist contract test (DEVOPS) + acceptance gate `scraper_only_reads_public_data` |
| I-SCR-3 (confidence-no-autoinflate) | Every candidate's confidence is the mapping default (0.25); no candidate is proposed above 0.3; sign-time confidence equals the proposal UNLESS the human edits it. | `scraper-domain` property test + acceptance gate `candidate_confidence_no_autoinflate` |
| I-SCR-4 (candidate-names-signal) | Every `CandidateClaim.source_signals` is NON-EMPTY; every rendered candidate names the exact public signal(s) that produced it. | `scraper-domain` property test + acceptance gate `candidate_names_source_signal` |
| I-SCR-5 (mapping-SSOT) | `scraper-domain` consumes the `jobs.yaml :: J-004.signal_predicate_mapping` SSOT; no divergent hardcode. | build-time test `mapping_matches_ssot` (WD-53) |
| I-SCR-6 (single-publish-path) | The sign-from-scraper claim publishes via the SAME `VerbClaimPublish` internals as a hand-authored claim; no parallel publish path. | code review + cli probe #6 + acceptance gate `scraper_reuses_slice01_publish_path` |
| I-SCR-7 (CID-stability / provenance display-only) | The signed-from-scraper claim's canonical payload is byte-identical to a hand-authored claim with identical fields; `derived-from` provenance is display-only and never reaches `canonicalize`. | `claim-domain` CID gold test (unchanged) + the absence of any Lexicon change (WD-62) |

These extend the 12 cross-feature invariants in
`docs/product/architecture/brief.md`; they are slice-02-scoped and do not need
promotion (the brief's I-1..I-12 already cover the meta-invariants — pure-core
isolation, probe contract, single-publish-path — which slice-02 inherits
unchanged). If a future slice needs one of I-SCR-1..7 enforced cross-feature,
promote it to the brief in the same commit as the generalizing ADR.

## Annotation for software-crafter (DELIVER)

```markdown
## Architecture Enforcement (slice-02 additions)

Style: Hexagonal + Modular Monolith (inherited from slice-01)
Language: Rust
Tools (slice-01 + slice-03 + slice-02 additions):
  - cargo-deny (license + bans) — assert the GitHub client adds no new license
    surface (reqwest is already in the workspace); see technology-stack.md
  - cargo xtask check-arch — adds scraper-domain to the PURE set (whitelist its
    pure YAML-parse dep); adds adapter-github as an effect adapter wired only by cli
  - cargo xtask check-probes — adds GithubPort impl checks
  - scripts/check-probes.sh — unchanged pre-commit hook; picks up the new trait

Rules to enforce (additions to slice-01/03):
- scraper-domain MUST NOT depend on tokio, reqwest, duckdb, keyring, or any I/O
  crate (I-2); it MAY depend on claim-domain, lexicon, serde, and a pure YAML parser
- adapter-github is wired into GithubPort ONLY by the cli crate (I-3)
- adapter-github MUST NOT depend on adapter-duckdb / adapter-atproto-* (I-1) and
  MUST NOT hold a StoragePort/IdentityPort/PdsPort reference (human-gate at the
  architecture layer)
- every impl GithubPort for <Adapter> block MUST contain a non-stub probe() body
- NO new field on org.openlore.claim (provenance is display-only; WD-62)
- the signal->predicate mapping is embedded from the jobs.yaml SSOT; mapping_matches_ssot
  build-time test MUST pass (no divergent hardcode)
```

## Annotation for acceptance-designer (DISTILL)

```markdown
## Slice-02 Observable Contracts (additions to slice-01/03)

### Human-gate — scrape without --sign
Every acceptance test driving `openlore scrape github <target>` WITHOUT --sign MUST assert:
- A public-data-only banner is printed BEFORE any harvest
- ZERO rows are written to author_claims
- ZERO PDS writes occur
- ZERO claims/<cid>.json files are written
- A numbered candidate list is rendered with a footer stating nothing is a claim
  until the user signs it
(gate: scraper_never_persists_unsigned)

### Auditability — candidate list
Every acceptance test driving the candidate list MUST assert:
- Every candidate names the exact public signal(s) that produced it
- Every candidate's confidence is 0.25 displayed as "speculative"
- No candidate is proposed above 0.3
- Multiple signals for one predicate collapse into ONE candidate listing all signals
(gates: candidate_names_source_signal, candidate_confidence_no_autoinflate)

### Public-data-only
Every acceptance test driving a private/non-existent target MUST assert:
- Exit code is non-zero
- A "scraper only reads public data" (private) OR "not found" (404) message
- ZERO candidates produced; NO private endpoint called
(gate: scraper_only_reads_public_data)

### Sign-from-scraper reuses the slice-01 pipeline
Every acceptance test driving `--sign N` MUST assert:
- The compose preview contains the literal "not as truth" (inherited I-7)
- The two-prompt contract holds (compose+sign, then SEPARATE publish [Y/n])
- With no edits, the signed claim's fields equal the candidate's proposed values
  byte-for-byte; confidence stays 0.25 (no auto-inflation)
- A display-only derived-from line names the source signal (NOT a signed-payload field)
- The publish success message mentions the retract command (inherited I-8)
- The claim publishes via the SAME VerbClaimPublish path as a hand-authored claim
(gates: scraper_reuses_slice01_publish_path, candidate_confidence_no_autoinflate)

### --sign selection list (batch)
Every acceptance test driving `--sign N,M,...` MUST assert:
- Each candidate gets its OWN compose preview + individual signing gesture
- A running "(k of M signed)" progress line
- NO "sign all without review" affordance
- One candidate can be skipped without aborting the rest; summary reports signed/skipped
- Duplicate or out-of-range indices are rejected BEFORE any compose, naming the offenders
- A single index behaves identically to single-candidate sign (US-SCR-003)

### PAT / rate-limit (US-SCR-004)
- Authenticated harvest reports authenticated status + remaining rate budget
- The token value NEVER appears in any output line, claim, or log
- Anon rate-limit exhaustion exits non-zero with a "set GITHUB_TOKEN" remediation
  and renders NO partial candidate list
- A rejected token (401) exits non-zero without echoing the token value

Reference: feature-delta.md WD-46..WD-58, wave-decisions.md WD-59..WD-66,
ADR-017..019, design's sections 5.1 + 5.2.
```

## Annotation for platform-architect (DEVOPS)

```markdown
## External Integrations Requiring Contract Tests (slice-02 additions)

- GitHub Public API (REST and/or GraphQL over HTTPS, READ paths):
  - Consumed endpoints (public-only allowlist): GET /repos/{owner}/{repo},
    GET /repos/{owner}/{repo}/contents/{path}, tags/releases, languages,
    GET /users/{user}, GET /users/{user}/repos (or GraphQL equivalents)
  - Recommended: extend the existing Pact-style contract suite with consumer-driven
    contracts for the GitHub read paths; replay against recorded public-response fixtures.
  - PUBLIC-ENDPOINT ALLOWLIST ASSERTION (KPI-SCR-4 release-gate): a contract test
    MUST assert adapter-github calls ONLY allowlisted public endpoints; NO
    authenticated-private endpoint is reachable.
  - Rate-limit fixture (403 + rate-limit headers) and rejected-token fixture (401)
    to exercise the remediation paths (US-SCR-004) + the no-token-leak assertion.
  - DEVOPS owns the wiremock/recorded GitHub fixtures (per outcome-kpis.md handoff).

## Earned Trust Telemetry Hooks (slice-02 additions)

- New probe failure reasons emitted via tracing health.startup.refused events:
  - github.public_api_unreachable
  - github.private_not_refused        (KPI-SCR-4 — the load-bearing trust event)
  - github.token_rejected
  - github.rate_limit_headers_missing

- KPI instrumentation (handed off per outcome-kpis.md DEVOPS section):
  - scrape.to_sign.duration_seconds histogram from `scrape github` to first
    successful sign (KPI-SCR-1; per target-type bucket: small repo / large repo / user)
  - scrape.candidate.signed{target, predicate, fields_edited, proposed_confidence,
    signed_confidence} on each sign-from-scraper (KPI-SCR-5 edit rate)
  - GitHub public-endpoint allowlist contract fixture in the CI acceptance suite
    (KPI-SCR-4 release-gate)
  - Verify the optional GITHUB_TOKEN path in CI uses a least-privilege fixture token
    (or recorded fixture) and that the token never appears in logs.
```
