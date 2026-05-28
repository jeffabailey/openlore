# ADR-019: GitHub Adapter — New `GithubPort`, `reqwest` Reuse, Rate-Limit + Optional-PAT Policy, Public-Data-Only Probe

- **Status**: Accepted
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-51/WD-54/WD-56 locks from Luna (nw-product-owner) for openlore-github-scraper
- **Feature**: openlore-github-scraper (slice-02)
- **Extends**: ADR-004 (async runtime + HTTP client) + ADR-009 (hexagonal ports + probe contract). Both remain in force.

## Context

slice-02 adds one external integration: the GitHub public API. DISCUSS locked:

- **WD-51**: public-data-only (no surveillance); private/non-existent targets
  refused; the target is the SUBJECT of a claim, never a controller.
- **WD-54**: optional PAT via `GITHUB_TOKEN`; works unauthenticated for small
  targets; the token is NEVER logged/claimed/published.
- **WD-56**: `adapter-github` is the EFFECT shell behind a new `GithubPort`,
  with a `probe()` per ADR-009 I-4 within the 250ms budget (I-5).

DESIGN owns: whether the GitHub surface is a new port or an extension; the HTTP
client choice; the rate-limit + PAT handling policy; and the probe contract
(including how the probe demonstrates the public-data-only guarantee against an
environment that can LIE about access — a private repo 404s like a missing one).

GitHub is the highest-risk boundary in the slice: it is external, rate-limited,
and can mislead about access (a private repo and a missing repo can both return
404; a rate-limited response can look like a transport error).

## Decision

### 1. `GithubPort` is a NEW port (not an extension)

GitHub is a wholly different external system from ATProto. No method shape,
auth model, rate-limit semantics, or failure surface is shared with `PdsPort` /
`IdentityPort`. Unlike slice-03 (where peer reads genuinely WERE ATProto XRPC,
so `PdsPort` extension was correct per WD-28), folding GitHub harvest into
`PdsPort` would conflate two unrelated trust boundaries and give `adapter-github`
no place to own its distinct probe. `GithubPort` is a new trait in `ports`:

```rust
#[async_trait]
pub trait GithubPort {
    fn probe(&self) -> ProbeOutcome;
    async fn resolve_target(&self, target: &str) -> Result<TargetKind, GithubError>;
    async fn harvest_repo(&self, owner: &str, repo: &str) -> Result<Vec<Signal>, GithubError>;
    async fn harvest_user(&self, user: &str) -> Result<Vec<Signal>, GithubError>;
}
```

`adapter-github` holds NO `StoragePort`/`IdentityPort`/`PdsPort` reference — by
construction it CANNOT sign or publish (the human-gate at the architecture
layer; WD-49).

### 2. HTTP client = the workspace `reqwest` (no new transport)

`adapter-github` uses the workspace `reqwest 0.12` (rustls-tls-webpki-roots,
json) already pulled in by `adapter-atproto-pds` (ADR-004). This adds ZERO new
transport dependency and ZERO new `cargo deny` surface (I-11; DISCUSS handoff
preference). REST vs GraphQL per signal is a DELIVER call (Q-DELIVER-2); both
are public-only. `octocrab` was rejected for footprint + a new supply-chain
surface (see `technology-stack.md`).

### 3. Public-data-only policy (WD-51)

- `adapter-github` calls ONLY public GitHub endpoints (the allowlist:
  `GET /repos/{owner}/{repo}`, `/contents/{path}`, tags/releases, languages,
  `/users/{user}`, `/users/{user}/repos`, or GraphQL equivalents). No
  authenticated-private endpoint is EVER reachable.
- `resolve_target` REFUSES private/non-existent targets: a 404/403 on a repo ->
  `GithubError::NotPublic` ("scraper only reads public data") or
  `GithubError::NotFound` ("not found"); the verb exits non-zero with zero
  candidates.
- The public-endpoint allowlist is the subject of a contract test (KPI-SCR-4
  release-gate; DEVOPS).

### 4. Optional-PAT policy (WD-54)

- The PAT is read from the `GITHUB_TOKEN` env var ONLY (WD-63; config-file
  deferred). When present, sent as `Authorization: token <PAT>` for the higher
  rate budget (5000/hr). When absent, harvest runs unauthenticated (anon 60/hr).
- The token is held ONLY in `adapter-github`; `scraper-domain` (pure) never sees
  it. It is NEVER logged, echoed, written to a claim, or published. It does NOT
  use the OS keychain (that is the signing key, ADR-002; the PAT is a
  lower-stakes ephemeral read credential).
- A rejected token (401) -> `GithubError::TokenRejected` with a remediation hint
  that does NOT echo the token value; the probe fast-fails a stale token at
  startup.

### 5. Rate-limit policy

- `adapter-github` reads `X-RateLimit-Remaining` / `X-RateLimit-Limit` headers;
  reports the remaining budget when authenticated.
- On anon budget exhaustion (403 rate-limit) -> `GithubError::RateLimited`; the
  verb surfaces a `set GITHUB_TOKEN for higher limits (5000/hour)` remediation
  and renders NO partial candidate list (a partial list would mislead the user
  into thinking they saw everything — US-SCR-004 AC).
- Retry-with-backoff is NOT used for rate limits (the budget is exhausted, not
  transient). Transient transport errors MAY use a bounded retry (DELIVER's
  call); rate-limit is a hard stop with remediation.

### 6. Probe contract (Earned Trust against a lying environment)

`adapter-github` ships a `probe()` within the 250ms budget (I-5) that exercises
the SPECIFIC GitHub lies (principle 12), not just a happy-path fetch:

1. **Public reachability**: `resolve_target` against a stable PUBLIC fixture
   returns `TargetKind::Repo` within budget.
2. **Private refusal**: `resolve_target` against a known-private/inaccessible
   fixture returns `GithubError::NotPublic` (NOT a silent empty harvest). This
   is the load-bearing KPI-SCR-4 probe — it catches the "private repo 404s like
   a missing one, so we harvested nothing and called it success" lie.
3. **Auth-mode**: a set-but-rejected `GITHUB_TOKEN` (401) refuses to start
   (`GithubTokenRejected`); an accepted token reports the budget.
4. **Rate-limit-header presence**: assert the budget-reporting path parses the
   headers (catches a GitHub response-shape change).
5. **No-token-leak**: assert the token value never appears in any structured
   probe event or log line.

On any refusal the probe emits `health.startup.refused{reason: github.*}` and
the system refuses to start (exit 2). Under `--offline` the probe is skipped but
`scrape` refuses to run (it requires network).

## Alternatives Considered

| Option | Rejection rationale |
|--------|---------------------|
| **Extend `PdsPort` with GitHub harvest methods** | Rejected (WD-61). GitHub is not ATProto; no method shape / auth / rate-limit / failure surface is shared. Slice-03 extended `PdsPort` for PEER reads because those WERE ATProto XRPC (WD-28) — that reasoning does NOT apply to GitHub. A new port keeps the trust boundary honest and gives `adapter-github` its own probe. |
| **`octocrab` typed GitHub client** | Rejected (technology-stack.md). Large transitive tree + a NEW `cargo deny` surface for a 5-signal bounded read; the DISCUSS handoff + I-11 prefer reusing the workspace `reqwest`. Reconsider in slice-04 if the harvest surface grows. |
| **Retry-with-backoff on rate-limit** | Rejected. A 403 rate-limit means the budget is EXHAUSTED, not transient; retrying wastes time and still fails. The honest response is a hard stop with a `set GITHUB_TOKEN` remediation and no partial list. (Transient transport errors MAY retry — different case.) |
| **Render a partial candidate list when the rate limit hits mid-harvest** | Rejected (US-SCR-004 AC). A partial list misleads the user into thinking they saw the full proposal set; refusing with a remediation is the honest choice. |
| **Store the PAT in the OS keychain (like the signing key)** | Rejected (WD-54 / technology-stack.md). The signing key is the high-stakes identity credential (ADR-002); the PAT is a low-stakes ephemeral GitHub read credential. Env-var is the zero-friction, well-understood mechanism and avoids coupling GitHub auth to the keychain adapter. |
| **Make the probe a happy-path public fetch only** | Rejected (principle 12). A probe that passes against only a reachable public repo does NOT demonstrate the public-data-only guarantee. The probe MUST exercise the private-refusal lie (step 2) and the rate-limit/token lies — the catalogued substrate lies of the GitHub environment. |

## Consequences

### Positive

- One new port with a clear, single-external-system boundary and its own probe.
- Zero new HTTP transport / `cargo deny` surface (reuses workspace `reqwest`).
- The public-data-only guarantee is empirically demonstrated at startup by the
  private-refusal probe, not just asserted in docs.
- The PAT is an isolated effect-shell credential; the pure derivation never sees
  it.

### Negative

- Crate count reaches 10 (the informal ~10 cap in ADR-009's Revisit Trigger).
  **Mitigation**: the two new crates are justified by the pure/effect split
  (WD-56/57); no meta-crate grouping is warranted yet. If slice-04/05 push the
  count higher, ADR-009's Revisit Trigger fires and adapters may be grouped.
- The probe makes real (or stubbed) GitHub calls at startup. **Mitigation**:
  the 250ms budget (I-5) bounds it; CI uses stubbed fixtures; production uses
  read-only public endpoints; under `--offline` it is skipped (and `scrape`
  refuses to run offline anyway).

### Earned Trust

`adapter-github`'s probe is itself the Earned-Trust contract for the GitHub
boundary. Three-layer enforcement (ADR-009) extends to it:

1. **Subtype (compile-time)**: `GithubPort` declares `fn probe(&self) ->
   ProbeOutcome`; rustc refuses an `impl` lacking it.
2. **Structural (pre-commit AST hook)**: `xtask check-probes` /
   `scripts/check-probes.sh` asserts a non-stub `probe()` body in
   `impl GithubPort for AdapterGithub`.
3. **Behavioral (CI gold-test runner)**: the probe exercises the GitHub
   substrate lies (private-repo-404, rate-limit-403, rejected-token-401) via
   stubbed fixtures — a probe passing against ONLY a public happy-path fetch is
   a CI failure.

The contract-test allowlist (DEVOPS) is the cross-cutting Earned-Trust answer to
"what if a future endpoint addition touches a private path?" — it fails CI on
any off-allowlist endpoint (KPI-SCR-4).

## Revisit Trigger

- Crate count grows beyond ~10 (slice-04/05) — ADR-009's meta-crate grouping
  trigger fires.
- The harvest surface grows substantially (deep contributor triangulation in
  slice-04) — reconsider `octocrab` vs hand-rolled `reqwest`+serde.
- A config-file PAT need emerges (multi-account) — extend WD-63 with an ADR.
- GitHub deprecates an endpoint on the public allowlist — the contract test
  fails and the allowlist + harvest paths are updated.
