# ADR-063: Contributor-Philosophy Inference — Pure Core in `scraper-domain`, Append-Only Contribution Links, Reused Claim Reads + Sign Pipeline

- **Status**: Proposed (2026-09-27)
- **Date**: 2026-09-27
- **Deciders**: Morgan (nw-solution-architect), on the USER-LOCKED D-1..D-4 + OD-CPI-1..7 (accepted
  2026-09-27) from Luna (nw-product-owner) for `contributor-philosophy-inference` (DISCUSS).
- **Feature**: contributor-philosophy-inference (slices 01-05; US-CPI-001..005; J-004d/J-004e)
- **Extends**: ADR-017 (scrape verb contract), ADR-018 (candidate-claim model), ADR-019 (GitHub
  adapter / rate limit / PAT), ADR-007 (functional Rust), ADR-009 (hexagonal modular monolith),
  ADR-014 (peer storage anti-merging), ADR-060 (self-retraction rule D-RF-D3), ADR-008 (reference
  types incl. `supersedes`).
- **Companion**: ADR-064 (the person-adherence claim WIRE contract: subject, predicate, provenance
  encoding). This ADR is the internal component architecture; ADR-064 is what peers see.

## Context

`scrape github owner/repo` must also record the repo's top-N human contributors (one public
`/contributors` page, bots excluded) as LOCAL, UNSIGNED observations that accumulate across
scrapes (D-3, D-4, OD-CPI-2/6/7). Over the accumulated links openlore proposes
`github:<login> adheresToPhilosophy X` CANDIDATES derived ONLY from signed, non-retracted
`embodiesPhilosophy` repo claims by the user or an active peer (D-2), labelled NEW / STRONGER, with
SUPPORT WEAKENED flags on already-signed inferred claims; the human signs through the SAME pipeline
as every other claim (D-1, D-5). `openlore infer people` is the primary verb (OD-CPI-4).

What the codebase already has (reuse analysis):

- `scraper-domain` is the PURE J-004 derivation crate (already under the `xtask check-arch`
  pure-core rule); its candidates are proposals, never auto-signed, conservative confidence.
- `GithubPort` / `adapter-github` already speak the public `/repos/...` allowlist with the ADR-019
  rate-limit / PAT / no-token-leak discipline and a railway `GithubError`.
- `StoragePort::query_federated_by_subject` already returns, per subject, every own + peer claim
  as a FULL `SignedClaim` (evidence + references) with a non-`Option` `author_did` and an
  `AuthorRelationship` (`You | SubscribedPeer | UnsubscribedCache`). Retraction markers copy the
  original's subject/predicate/object (`claim retract`), so they come back in the same read.
- `StoragePort::query_by_contributor` + `read_signed_claim` return all of MY claims.
- `lexicon::Claim.predicate` is a free string (no predicate allowlist exists), `evidence` is
  `Vec<String>`, `references[].type` already admits `supersedes`, and `ReferenceType::Supersedes`
  ships in `claim-domain`. `reference_rules_validate` already rejects self-reference / 2-hop cycles.
- The scraper `--sign` batch (`scrape_github.rs`) already implements parse-selection, per-candidate
  compose preview, skip gesture, two-prompt sign/publish over the single publish path — but its
  `ComposedClaim` carries no `references`.
- DuckDB migrations are forward-only, idempotent, one module per version (`schema_v3`, `schema_v4`),
  sharing ONE connection between `DuckDbStorageAdapter` and `DuckDbPeerStorageAdapter`.

## Decision

1. **Pure core = EXTEND `crates/scraper-domain`** with a `people` area (no new crate):
   contributor selection (bot rule, deterministic re-ranking, top-N cut), cross-repo overlap,
   inference (eligibility, grouping, confidence, classification), the provenance codec
   (encode evidence entries per ADR-064; parse cited claims back), and the before/after change
   summary for the end-of-scrape hint. Values in, values out; no I/O. Key types: `PersonSubject`
   / `RepoSubject` (validated newtypes; case-folded comparison key for the `github:` scheme),
   `ContributorSelection { people: ranked humans, bots_excluded }`, `PersonCandidate` (smart
   constructor guarantees NON-EMPTY supporting repos, each with NON-EMPTY supporting claims —
   the auditability invariant, mirroring `CandidateClaim::try_new`), `CandidateStatus =
   New | Stronger { supersedes: Cid, cited_repo_count }`, `SignedAdherence` with
   `WeakenedSupport { cid, reason: Retracted | NoLongerEligible | MissingLocally }`,
   `InferredConfidence` (integer hundredths + the arithmetic text), `InferenceReport`
   (numbered candidates in deterministic order, already-signed, weakened, repos without signed
   claims, possible renames).
2. **Harvest = EXTEND `GithubPort`** with one read, `list_contributors(owner, repo)`: exactly ONE
   `GET /repos/{o}/{r}/contributors?per_page=100` (anonymous contributors not requested), returning
   RAW rows (login, numeric user id, account type, contributions) including bots. The adapter
   filters nothing; the requested N (0..=100) is applied in the pure core, so "exactly N humans"
   holds even when bots occupy top slots, and the request count is exactly 1 (0 when N = 0).
   New `GithubError::ContributorsUnavailable { target, reason }` for GitHub's "contributor list too
   large" 403 and the empty-repo 204 (a named, non-fatal notice — not a rate-limit).
3. **Link storage = NEW driven port `ContributionLinkPort`** (in `crates/ports`, sync, local-DB only)
   with `probe`, `record_snapshot(repo, observed_at, ranked people)` and `list_links(filter: All |
   Person)`. It has NO delete and NO caller-driven update method — append/upsert-only by type.
   Adapter `DuckDbContributionLinkAdapter` in `crates/adapter-duckdb`, sharing the existing
   `Arc<Mutex<Connection>>` (DuckDB single-writer; `DuckDbPeerStorageAdapter` precedent).
4. **Schema v5 (`schema_v5.rs`)**, forward-only + idempotent exactly like v4: one table
   `contribution_links` — case-folded `repo_key` + `person_key` (PRIMARY KEY), display-form
   `repo_subject` + `person_subject`, `github_user_id` (BIGINT, indexed — rename detection),
   `rank` (≥1, among humans), `contributions` (≥0), `first_observed_at`, `last_observed_at`.
   `record_snapshot` = one transaction of `INSERT … ON CONFLICT (repo_key, person_key) DO UPDATE`
   refreshing user id / rank / contributions / `last_observed_at` and NEVER touching
   `first_observed_at`; nothing is ever deleted. "Stale" (not in the latest snapshot) is derived,
   not stored: `last_observed_at < max(last_observed_at)` for that repo. The adapter probe's
   binary-supported schema version is bumped to 5.
5. **Signed-claim inputs = REUSE existing `StoragePort` reads, no new method**:
   `query_federated_by_subject(repo)` per linked repo (repo claims + their retraction markers +
   relationship), and `query_by_contributor(me)` → `read_signed_claim` for my
   `adheresToPhilosophy` claims (incl. my retraction markers / superseding claims). The pure core
   applies: support = predicate `embodiesPhilosophy` ∧ relationship ∈ {You, SubscribedPeer} ∧
   not itself a marker (no `retracts`/`counters` reference) ∧ not self-retracted (ADR-060 D-RF-D3,
   hoisted as a pure helper into `claim-domain` so `appview-domain` and `scraper-domain` share ONE
   rule) ∧ not superseded by a same-author claim. Subjects join case-insensitively for `github:`.
6. **Classification + confidence (pure)**: per (person, philosophy) with k ≥ `--min-repos`
   (default 1) supporting repos → candidate; confidence = min(29, 15 + 5·(k−1),
   floor(100·max supporting confidence)) hundredths, arithmetic shown. My current (non-retracted,
   non-superseded) adherence claim for the pair: none → NEW; cites ≥1 supporting claim (ADR-064
   at-uri evidence) and the current support includes a repo it does not cite → STRONGER
   (`supersedes` that CID); otherwise → already signed (not numbered). A hand-authored adherence
   claim (cites nothing) is never STRONGER. A signed inferred claim whose cited claim is now
   retracted / from an unsubscribed peer / absent locally is flagged SUPPORT WEAKENED. Numbering
   = deterministic sort by (person key, philosophy object) over NEW ∪ STRONGER after filters.
7. **Signing = REUSE the slice-01 pipeline**: extract the scraper's `--sign` batch (selection
   parser, per-candidate prefill → compose preview → skip gesture → sign → single publish path)
   into a shared CLI helper fed by a verb-neutral prefill; EXTEND `ComposedClaim` /
   `build_unsigned_claim` with `references` (default empty ⇒ every existing CID unchanged);
   STRONGER prefill adds one `supersedes` reference, validated by the existing
   `reference_rules_validate` before signing. No inference-specific signing path.
8. **CLI (driving)**: NEW `openlore infer people [--person github:<login>] [--min-repos N]
   [--sign N[,N…]]`; EXTEND `scrape github <owner/repo> [--contributors N]` (clap-bounded 0..=100,
   rejected on a user target, before any request) with the contributors block, overlap line and
   the change hint; EXTEND `scrape github <user>` to render the person view (links + signed
   adherence + candidates, `--sign` = `infer people --person … --sign`). Existing
   `--contributor <did>` flags unchanged (help text says "claim author").
9. **Sequencing / atomicity (repo scrape)**: resolve → harvest signals → `list_contributors` (if
   N > 0) → pure select → `record_snapshot` (one tx) → render → optional `--sign` → hint
   (pure before/after inference diff; silent when zero). A rate-limit / auth / network / shape
   failure aborts BEFORE any link write (non-zero exit; no partial snapshot possible — the write is
   one transaction of an already-complete in-memory selection).

## Alternatives Considered

- **A. New pure crate `people-domain`** (vs extending `scraper-domain`). Pros: name matches the
  "person" vocabulary; smaller crates. Cons: a 2nd pure-core registration + check-arch rule for
  the SAME J-004 bounded context and the SAME candidate invariants (never auto-signed, conservative
  confidence, non-empty provenance); consumed by the same verb family. REJECTED (default EXTEND;
  no coupling problem to escape). Revisit if the people area outgrows the scraper area.
- **B. Inference in `crates/cli`**. Cheapest, but DoD #5 requires a pure core under the xtask
  pure-core rule; `cli` cannot be checked pure. REJECTED.
- **C. Inference in `crates/scoring`**. Pure and attribution-aware, but scoring is the WD-77
  weight formula SSOT for display; mixing candidate proposal into it muddies ADR-022. REJECTED.
- **D. Extend `StoragePort` with link methods** (vs a new `ContributionLinkPort`). Every
  `StoragePort` implementor/fake would have to grow link methods, and unsigned observations would
  sit on the claims port that carries the anti-merging SQL rules. REJECTED; a separate
  capability port mirrors the `PeerStoragePort` precedent (ISP).
- **E. Contribution links as signed `contributesTo` claims**. Locked out by OD-CPI-2 (noise,
  surveillance concern); recorded for completeness.
- **F. `per_page = N (+ slack)`** instead of always 100. Cannot guarantee N humans when bots
  cluster at the top; same request count. REJECTED.
- **G. New batch StoragePort read (`query_inference_inputs`)**. Fewer round-trips, but a new
  SQL surface under the anti-merging rules for a local, small workload (≤ #scraped repos reads).
  DEFERRED — revisit if `infer people` exceeds ~2 s on ~200 scraped repos.

## Consequences

- **Positive**: zero new crates; one new port + one new table; the claim pipeline, Lexicon,
  reference rules, publish path and federated reads are reused unchanged in behavior; every
  existing signed claim's CID is unchanged (`references` default empty). Signed inferred claims
  are ordinary claims, so `graph query --subject github:<login>`, peer pull, publish and scoring
  read them as-is.
- **Negative**: the scraper's "persists nothing without `--sign`" invariant narrows to "writes no
  CLAIM without `--sign`" (DISCUSS Changed Assumption 1); the `scraper_never_persists_unsigned`
  gate must assert on claim tables only. `FakeGithub` must serve `/contributors` by default
  (empty list) so the shipped SCR acceptance suite stays green. Person subjects now appear among
  a philosophy's subjects in `graph query --object` / scoring (accepted — DISCUSS Out of Scope
  says scorer reads them as-is).
- **Risk**: sparse overlap (R-1) is a product risk measured by slice-01, not an architecture one.

## Earned Trust (probe contract)

- `DuckDbContributionLinkAdapter::probe()` (LIVE in the gauntlet — NOT cfg-gated like the
  historical peer-storage slot): table present at schema ≥ 5; inside a transaction that is ROLLED
  BACK, upsert a sentinel row, upsert the SAME key again with a changed rank, read it back and
  assert rank refreshed + `first_observed_at` unchanged. This exercises the specific DuckDB lie
  (over-eager unique/index constraint checking on `ON CONFLICT DO UPDATE` within one transaction)
  that would otherwise surface only on the first real re-scrape. Refusal → structured
  `health.startup.refused`.
- `GithubPort::list_contributors` spends NO startup request (rate budget). Its substrate lies are
  a catalogued gold-fixture set in `FakeGithub`, exercised in CI: rows out of contribution order;
  `type: "Bot"`; `…[bot]` login with `type: "User"`; duplicate user ids; > N humans; 100 bots;
  204 empty repo; 403 "too large"; 403 rate-limit; 401 stale PAT; a row missing `login`/`id`
  (→ `ApiShape`, no partial write). The pure core re-sorts and de-duplicates rather than trusting
  API order.
- Three layers: TYPE (no delete method on `ContributionLinkPort`; non-empty provenance smart
  constructor), STRUCTURAL (`xtask check-arch` rule `contribution_links_append_only`: no
  `DELETE`/`DROP`/`TRUNCATE`/bare `UPDATE` against `contribution_links` in `adapter-duckdb`;
  `adapter-github` names no storage/identity port), BEHAVIORAL (the gold fixtures above + the
  KPI-CPI-4 byte-compare test).

## Revisit Triggers

- `infer people` latency > 2 s on a realistic store (→ alternative G).
- Peers need each other's links to infer (→ revisit OD-CPI-2 / signed `contributesTo`).
- A person area large enough to warrant its own crate (→ alternative A).
