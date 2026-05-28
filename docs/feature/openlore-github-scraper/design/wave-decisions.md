# Wave Decisions — DESIGN — openlore-github-scraper (slice-02)

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Inherits from**: DISCUSS WD-46..WD-58 (feature-delta.md); WD-1..WD-13 + ADR-001..012 (slice-01); WD-26..WD-45 + ADR-013..016 (slice-03)
- **Format**: WD-XX entries (continuing the sequence after WD-58); one decision per row; rationale + status + locks downstream

## DESIGN-wave decisions

| # | Decision | Rationale | Status | Locks |
|---|----------|-----------|--------|-------|
| WD-59 | Slice-02 is a TWO-CRATE additive EXTENSION of slice-01, not a re-architecture. It adds `scraper-domain` (PURE) + `adapter-github` (EFFECT) and extends `ports` + `cli` + `xtask` in place; production crate count 8 -> 10. | The brief explicitly scopes slice-02 to "adds `adapter-github` + `scraper-domain`" (WD-57). The pure/effect split (WD-56) CANNOT be honored without two crates. This is the first crate addition since slice-01 (slice-03 added zero per WD-26). | LOCKED | DELIVER ships exactly two new crates; introducing a third during DELIVER requires returning to DESIGN. |
| WD-60 | **OD-SCR-1 RESOLVED: verb shape is the sugar verb `openlore scrape github <target> [--sign N[,N...]]`** (NOT a `claim add --from-github` flag). | Symmetric with the slice-03 sugar verbs (`peer pull`, `claim counter`); discoverable; keeps `claim add` focused on hand-authoring; the `--sign` continuation reuses the slice-01 pipeline. Confirms the WD-50 DISCUSS default. | LOCKED per ADR-017. | DELIVER implements the sugar verb; ADR-017 amends ADR-003/ADR-013. |
| WD-61 | **`GithubPort` is a NEW port, NOT an extension of an existing port.** It is the only new port. | GitHub is a wholly different external system from ATProto — no method shape, auth model, rate-limit semantics, or failure surface is shared with `PdsPort`/`IdentityPort`. Unlike slice-03 (where peer reads genuinely WERE ATProto XRPC, so `PdsPort` extension was right per WD-28), folding GitHub harvest into `PdsPort` would conflate two unrelated trust boundaries. A new port keeps the boundary honest and gives `adapter-github` its own probe contract. | LOCKED per ADR-019. | DELIVER implements `GithubPort` as a distinct trait in `ports` with its own `probe()` (I-4). |
| WD-62 | **OD-SCR-3 RESOLVED: `derived-from` provenance is DISPLAY-ONLY in slice-02** (NOT a signed-payload field). | Confirms the WD-58 / OD-SCR-3 DISCUSS default. Display-only is the smaller, safer change: NO Lexicon change this slice, the signed payload is byte-identical to a hand-authored claim, CID stability holds with zero new CID path (I-6/I-10/I-SCR-7), and CID-stable-when-absent is trivially satisfied (the field is never present). Reversible via a future ADR if a federation reason to persist provenance emerges (would then mirror the slice-03 `reason` optional-field treatment, WD-32/ADR-015). | LOCKED per ADR-018. | DELIVER renders provenance as a display-only line; NO field added to `org.openlore.claim`; `lexicon` crate UNCHANGED. |
| WD-63 | **OD-SCR-2 RESOLVED: PAT is read from the `GITHUB_TOKEN` env var ONLY in slice-02** (config-file support deferred). | Confirms the WD-54 / OD-SCR-2 DISCUSS default. Env-var is the zero-friction minimum and the well-understood mechanism for CI/dev tokens. Config-file token support is deferred to a later slice if a multi-account need emerges. The PAT is an effect-shell credential held only in `adapter-github`; it does NOT use the OS keychain (that is for the signing key, ADR-002). | LOCKED per ADR-019. | DELIVER reads `GITHUB_TOKEN` from env; DISTILL provides the token to acceptance fixtures via env. |
| WD-64 | **OD-SCR-4 RESOLVED: contributor (user) targets harvest a BOUNDED cross-repo aggregate in slice-02**; deep cross-repo triangulation is deferred to slice-04 (scoring-graph). | Confirms the OD-SCR-4 DISCUSS default. Deep triangulation is a scoring concern with its own JTBD and its own slice. `harvest_user` returns a bounded aggregate (capped page walk; cap is a DELIVER call). The candidate-list footer + story-map set the expectation explicitly. | LOCKED. | DELIVER implements `harvest_user` with a bounded page cap (Q-DELIVER-4); slice-04 revisits for triangulation. |
| WD-65 | **`scraper-domain` is added to the `xtask check-arch` pure-core set; its pure YAML-parse dependency is whitelisted.** `adapter-github` is registered as an effect adapter wired only by `cli`. | The pure/effect split (WD-56 / ADR-007) MUST be machine-enforced for the new crates exactly as for the existing ones (I-1/I-2/I-3). `scraper-domain`'s only non-`serde` dependency is a pure YAML parser (no I/O); it joins the allowlist alongside `serde` (mirrors slice-03 whitelisting `unicode-normalization`). | LOCKED. | DELIVER extends `xtask check-arch` allowlist + adapter-isolation rules; `check-arch` MUST pass for both new crates. |
| WD-66 | **The candidate->compose pre-fill reuses `VerbClaimAdd` + `VerbClaimPublish` internals via function call; NO parallel publish path** (preserves ADR-003 + WD-22). `cli::CandidatePrefill` is the ONLY bridge from a `CandidateClaim` to a signed claim. | Preserves the single-publish-path invariant and the human-gate (WD-49): the scraper has no signing key and no publish code path; the ONLY way a candidate becomes a claim is the human signing it through the slice-01 pipeline. Mirrors how slice-03's `VerbClaimCounter` reuses `VerbClaimPublish` (WD-33). | LOCKED per ADR-017. | DELIVER MUST NOT introduce a parallel publish path; code review + cli probe + the `scraper_reuses_slice01_publish_path` gate enforce this. |
| WD-67 | **The signal->predicate mapping is EMBEDDED from the `jobs.yaml` SSOT at build time** (`include_str!` + a pure parse), with a `mapping_matches_ssot` build-time test asserting no drift. Read-at-runtime is rejected. | Embedding keeps `scraper-domain` PURE (no filesystem I/O at runtime; I-2 holds). A build-time include + drift test honors WD-53 (single SSOT, no divergent hardcode) without violating the pure-core rule. A generated Rust table via xtask codegen from `jobs.yaml` is an acceptable DELIVER alternative (SSOT still `jobs.yaml`). | LOCKED. | DELIVER embeds the snapshot; `mapping_matches_ssot` MUST pass; runtime filesystem reads from `scraper-domain` are forbidden by `check-arch`. |
| WD-68 | **The three DESIGN-wave ADRs (017, 018, 019) are accepted with this DESIGN-wave handoff**; no further DESIGN iterations required pending peer review. | Each ADR has 2+ alternatives considered, carries the DISCUSS locks, and includes an Earned Trust section translating to concrete probe contracts. Slice-02 is a straightforward additive extension of slice-01 on the proven technology surface; the only novel risk (GitHub-can-lie-about-access) is addressed by the `adapter-github` probe (architecture-design §6.3). | LOCKED pending Atlas (solution-architect-reviewer) approval. | Reviewer may flag issues for an iteration-2 pass. |

## Decisions DEFERRED to DELIVER

| # | Question | Default for DELIVER | Why deferred |
|---|----------|---------------------|--------------|
| Q-DELIVER-1 | Pure YAML parser crate + version pin for `scraper-domain` (`serde_yaml` vs maintained fork `serde_yml`/`serde_norway`); or xtask-codegen a Rust table instead | Pick the actively-maintained drop-in fork; pin MAJOR.MINOR per slice-01 policy; OR codegen if the fork situation is unsatisfactory | Maintenance status of the YAML-parser ecosystem is a DELIVER-time fact; SSOT is `jobs.yaml` either way (`mapping_matches_ssot` guards drift). |
| Q-DELIVER-2 | REST vs GraphQL per signal in `adapter-github` | GraphQL (one POST; fewer round-trips, friendlier to the anon rate budget) where it cleanly covers a signal; REST where simpler to fixture | atrium-style typed-client tradeoffs vs hand-rolled serde are a DELIVER implementation detail; ADR-019 permits either (both public-only). |
| Q-DELIVER-3 | `Signal` / `CandidateClaim` type placement: `scraper-domain` (default) vs `ports` | `scraper-domain` (the pure derivation crate is their natural home; `ports` references them for `GithubPort` signatures) | Both placements keep both crates pure; the choice is about the `ports <-> scraper-domain` dependency direction. Crafter confirms which reads cleaner. |
| Q-DELIVER-4 | `harvest_user` page-walk cap (bounded aggregate per WD-64) | A small fixed cap (e.g. first N public repos by recency); document it in the candidate-list footer | The exact cap is an empirical tuning call against the anon/PAT rate budget; deep triangulation is slice-04. |
| Q-DELIVER-5 | Exact batch-skip gesture (Ctrl-C-per-candidate vs explicit "skip" input) | Whichever cleanly satisfies "skip one without aborting the rest"; DISTILL's acceptance test asserts the behavior, not the keystroke | Product contract is the behavior; the gesture is a UX detail crafter + DISTILL settle. |
| Q-DELIVER-6 | `wiremock` (or equivalent) version pin + the live-vs-recorded GitHub fixture split | Coordinate with DEVOPS; recorded fixtures for CI, real public GitHub for the production probe | Hand-off is to DEVOPS (per outcome-kpis.md); DELIVER consumes the fixtures once DEVOPS provides them. |
| Q-DELIVER-7 | Exact candidate-list + progress-block line format | Match the `data-models.md` "scrape verb output format" sketch; satisfy DISTILL's asserted lines | DISTILL's acceptance tests assert specific lines; DELIVER fills in the format that satisfies them. |

## OD-SCR resolutions (consolidated)

The DISCUSS-wave Open Decisions (OD-SCR-1..4) are resolved by this DESIGN wave:

| Open Decision | DISCUSS default | DESIGN resolution |
|---------------|-----------------|-------------------|
| OD-SCR-1 (verb shape) | sugar verb `scrape github` | **WD-60 LOCKED**: sugar verb `scrape github <target> [--sign N[,N...]]` (ADR-017) |
| OD-SCR-2 (PAT config surface) | env-var only | **WD-63 LOCKED**: `GITHUB_TOKEN` env-var only; config-file deferred (ADR-019) |
| OD-SCR-3 (`derived-from` storage) | display-only | **WD-62 LOCKED**: display-only; NO Lexicon change (ADR-018) |
| OD-SCR-4 (contributor depth) | bounded aggregate | **WD-64 LOCKED**: bounded aggregate; deep triangulation -> slice-04 |

## ADR proposals (this DESIGN wave)

| ADR | Title | Status | Replaces / amends |
|-----|-------|--------|-------------------|
| ADR-017 | CLI Verb Contract Amendment — `scrape github` Sugar Verb + `--sign` Continuation | Accepted (proposed) | Amends ADR-003 + ADR-013 |
| ADR-018 | Candidate-Claim Model + Signal->Predicate Mapping Contract + Display-Only Provenance | Accepted (proposed) | Extends ADR-005 (provenance forward-compat) + ADR-007 (pure derivation) |
| ADR-019 | GitHub Adapter — New `GithubPort`, `reqwest` Reuse, Rate-Limit + Optional-PAT Policy, Public-Data-Only Probe | Accepted (proposed) | Extends ADR-004 (HTTP client) + ADR-009 (probe contract) |

(ADR numbering continues sequentially after ADR-016, the highest to date.)

## Inherited locks summary (do NOT relitigate)

| Source | Locks |
|--------|-------|
| Slice-01 | All ADR-001..012; WD-1..WD-13; the 12 cross-feature invariants in `docs/product/architecture/brief.md` |
| Slice-03 | ADR-013..016; WD-26..WD-45 (especially WD-22 single-publish-path, the ADR-013 verb-amendment precedent, the WD-32/ADR-015 optional-field CID-stability precedent) |
| Slice-02 DISCUSS | WD-46..WD-58 (feature-delta.md) + OD-SCR-1..4 (now resolved above) |
| Slice-02 DESIGN | WD-59..WD-68 (this file) + ADR-017..019 |

### Hard constraints honored (from the DISCUSS handoff)

- **WD-49 human-gate**: scraper proposes, human signs; no auto-sign, no
  auto-publish, no signing key in the scraper (architecture: `adapter-github`
  holds no storage/identity/pds reference; `CandidatePrefill` is the only
  bridge; I-SCR-1).
- **WD-51 public-data-only**: `adapter-github` calls only public endpoints;
  private/non-existent refused; probe step 2 asserts it (I-SCR-2).
- **WD-52 confidence 0.25 numeric, never auto-inflate**: `scraper-domain`
  stamps 0.25; buckets display-only (I-SCR-3).
- **WD-53 mapping SSOT**: embedded from `jobs.yaml`; `mapping_matches_ssot`
  (WD-67; I-SCR-5).
- **WD-55 nothing persisted unsigned**: `scrape` without `--sign` writes zero
  rows / zero PDS / zero files (`scraper_never_persists_unsigned`).
- **WD-56 pure/effect split**: `scraper-domain` PURE (check-arch passes, WD-65);
  `adapter-github` effect with `probe()` within 250ms (I-4/I-5).
- **WD-57 two new crates**: count 8 -> 10; brief Component Inventory updated at
  finalize (handoff note).
- **I-6/I-10 CID stability**: no new CID path; provenance display-only (WD-62;
  I-SCR-7).

## Brief SSOT update (finalize-time deliverable — NOT done now)

Per slice-03 precedent, the brief's Component Inventory + CLI surface updates
are LEFT TO FINALIZE (not edited during DESIGN). At finalize, the brief gains:

- Two Component Inventory rows: `crates/scraper-domain` (pure core; "candidate
  derivation from harvested signals; no I/O"; slice-02) and
  `crates/adapter-github` (effect; "implements `GithubPort` over the GitHub
  public API; optional PAT; probe"; slice-02).
- Production crate count: **8 -> 10**.
- One CLI surface row: `openlore scrape github <target> [--sign N[,N...]]`
  (slice-02; ADR-017).
- A "shipped slice extensions" note for slice-02 (mirroring the slice-03 entry).

This note is the handoff record; the actual edit happens at finalize.

## Handoff

This file is the canonical DESIGN-wave decision record. It is consumed by:

- **Atlas (solution-architect-reviewer)** for peer review iteration 1.
- **DISTILL (nw-acceptance-designer)** for resolving any `# confirm` flags in
  the gherkin scenarios against the WD-59..WD-68 + ADR-017..019 decisions
  (especially verb shape WD-60, provenance display-only WD-62, the skip gesture
  Q-DELIVER-5, and the PAT env-only WD-63).
- **DEVOPS (nw-platform-architect)** for instrumentation planning (KPI-SCR-1 +
  KPI-SCR-5), the GitHub public-endpoint allowlist contract test (KPI-SCR-4),
  and the GitHub stub fixtures.
- **DELIVER (nw-software-crafter)** for implementation; the Q-DELIVER-1..7
  deferred decisions are crafter's call.
