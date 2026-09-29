<!-- markdownlint-disable MD013 -->
# Evolution: contributor-philosophy-inference: record who builds each scraped repo, infer which philosophies a person holds from signed repo claims only, and let the human sign the inference with auditable provenance

> A full nWave feature (DISCUSS → DESIGN → DISTILL → DELIVER; DEVOPS not run, no new
> infrastructure). Brownfield extension of the shipped scraper (ADR-017..019). Paradigm:
> functional Rust (ADR-007). Companion design: ADR-063 (component architecture) and ADR-064
> (person-adherence wire contract). Zero new crates, zero Lexicon change, zero new
> dependencies. 15 DELIVER steps across 5 slices. **FEATURE COMPLETE** (2026-09-28).

## Summary

The user's original goal for openlore was to *"read software AND people on GitHub"*. The
scraper shipped only the software half: `scrape github owner/repo` derived repo-level
philosophy candidates, and `scrape github <user>` derived nothing at all (OD-SCR-4 deferred
the contributor aggregate, and slice-04 of the scoring graph shipped without it). This
feature closes that deferral without per-commit analysis:

- **J-004d (accumulate links):** every `scrape github owner/repo` also records the repo's
  top-N human contributors (default 30, one public `/contributors` page, bots excluded and
  named) in an append-only local `contribution_links` table. The people graph grows as a
  free by-product of scraping.
- **J-004e (infer, human signs):** `openlore infer people` proposes
  `github:<login> adheresToPhilosophy <philosophy>` candidates. They are derived **only** from
  signed, active, non-retracted, non-superseded `embodiesPhilosophy` repo claims authored by
  the user or an active peer. Each candidate shows every supporting claim with its own author,
  the person's rank, and a capped speculative confidence with its arithmetic. The user signs
  the ones they agree with (`--sign N`) through the same sign batch as the scraper.
- **Provenance travels with the claim (D-9):** the signed payload's `evidence[]` lists each
  supporting claim's AT-URI (author + CID) and the person's
  `https://github.com/<o>/<r>/commits?author=<login>` page per repo. A peer can audit the
  inference without the signer's link table, and the CID is stable on re-verify.
- **Evidence grows append-only (D-5):** a scrape that changes inference inputs ends with a
  one-line hint. Candidates are labelled NEW, STRONGER (signing adds a `supersedes` reference;
  the earlier claim stays byte-identical) or already signed. Signed inferred claims whose
  support was retracted, unsubscribed or is missing locally are flagged SUPPORT WEAKENED with
  the reason and are never acted on.
- **Read a person (US-CPI-005):** `scrape github <user>` now renders the person view (links,
  signed adherence, candidates, or guidance) from exactly one `/users/{user}` request, with
  `--sign` identical to `infer people --person github:<user> --sign`, and a "possible rename"
  flag when two logins share one GitHub user id (never merged).

### Business context

J-004 (*evaluate a person's body of work through a philosophy lens*) has been a primary job
since DISCUSS of the scraper, but it had no implementation. Stars and commit graphs say what
someone built, not how they think. This feature makes each signed repo claim pay off twice:
once for the repo, and again for every person who builds it. The ethics anxiety ("is this a
profiling tool?") is answered by construction: public data only, top-N committers only, links
local and unsigned, speculative confidence, nothing claimed without the user's signature, and
every inference shows its evidence.

## Key decisions

### Locked (DISCUSS, user)

| ID | Decision |
|---|---|
| D-1 | Inferred adherence is stored as **signed** claims. Inference proposes candidates only; nothing is auto-signed. |
| D-2 | Only **signed** repo claims (own ∪ active peers) feed inference. Unsigned scraper candidates never do. |
| D-3 | Contributors per repo = top N by commits from public `/contributors`, default 30, `--contributors N`, bots excluded. |
| D-4 | No per-commit analysis. Links accumulate in DuckDB; inference re-runs over all of them. |
| D-5 | Append-only: new/superseding candidates and weakened-support flags; the system never edits, re-signs or retracts. |
| D-6 | Public data only, at most one extra GitHub request per repo scrape. |
| D-7 | Anti-merging: every supporting claim shown with its own author DID; no "consensus" wording. |
| D-8 | New surfaces say "person". The shipped `--contributor <did>` keeps meaning claim author. |
| D-9 | Provenance travels inside the signed payload (how = OD-CPI-5). |

### Open decisions (DISCUSS recommendations, accepted by DESIGN)

| ID | Resolution |
|---|---|
| OD-CPI-1 | Person identity = `github:<login>` subject URI; GitHub's numeric user id is stored on the link so renames are detectable. DID linking deferred to a later, opt-in, human-signed claim. |
| OD-CPI-2 | Predicate `adheresToPhilosophy` (distinct from `embodiesPhilosophy`). Repo→person is an **unsigned local observation**, not a signed `contributesTo` claim. |
| OD-CPI-3 | Confidence `min(0.29, 0.15 + 0.05·(k−1), max supporting confidence)`, arithmetic displayed; `--min-repos` filter. |
| OD-CPI-4 | Canonical `openlore infer people` verb, a one-line hint on repo scrape, and the person view on user scrape. Never auto-sign. |
| OD-CPI-5 | Provenance as entries in the existing `evidence[]` (no Lexicon change, CID-stable). |
| OD-CPI-6 | Upsert per (repo, person); never delete; stale links derived for display. |
| OD-CPI-7 | `0 ≤ N ≤ 100` (one page); larger N rejected. |

### Design (DDD-1..16, ADR-063 / ADR-064)

| ID | Decision |
|---|---|
| DDD-1 | Pure inference core extends `crates/scraper-domain` (`people` area); check-arch pure. |
| DDD-2 | `GithubPort::list_contributors`: one `per_page=100` request, N applied in the pure core, **0 requests at N = 0**. |
| DDD-3 | Bot rule (`type == Bot` or `[bot]` suffix), re-rank by contributions, de-dup by user id; all pure. |
| DDD-4 | New `ContributionLinkPort` (no delete method) + `DuckDbContributionLinkAdapter` on the shared connection. |
| DDD-5 | Migration v5 `contribution_links`: case-folded PK, one-transaction upsert, never delete, stale derived. |
| DDD-6 | Signed-claim inputs via reused `query_federated_by_subject` + `query_by_contributor` / `read_signed_claim`; no new StoragePort method. |
| DDD-7 | Support eligibility including the ADR-060 D-RF-D3 self-retraction rule, **hoisted into `claim-domain`**; case-insensitive `github:` join. |
| DDD-8 | NEW / STRONGER (supersedes) / already signed / SUPPORT WEAKENED (reason); deterministic numbering. |
| DDD-9 | Confidence in integer hundredths `min(29, 15+5(k−1), floor(100·max))`. |
| DDD-10 | Provenance = `evidence[]` AT-URIs of supporting claims + `commits?author=` URLs (ADR-064 §3). |
| DDD-11 | No predicate validator change (predicate is already a free string). |
| DDD-12 | Sign through the extracted shared scraper sign batch; `ComposedClaim.references` (default empty keeps existing CIDs). |
| DDD-13 | CLI: `infer people [--person] [--min-repos] [--sign]`; `scrape github <repo> --contributors N`; `scrape github <user>` person view; no `--json`. |
| DDD-14 | Scrape sequencing: no link write before a complete harvest; too-large / empty repo = notice, exit 0. |
| DDD-15 | Live link-adapter probe (in-transaction upsert-twice, rolled back); GitHub lies as gold fixtures. |
| DDD-16 | `xtask check-arch`: `contribution_links_append_only` rule + adapter-github port guard. |

DESIGN also handed UC-1..UC-8 (edge-case clarifications) to the product owner; they were
ratified at the consolidated DISTILL gate and are now folded into the story ACs in
`feature-delta.md` (see "PO follow-up" below).

### DELIVER

- **Walking skeleton split across 01-01 / 01-02.** The thick WS (scrape → link → infer →
  sign) was more than a day, so 01-01 landed the link table and 01-02 turned WS-CPI-1 green.
- **Single `/users` fetch for the person scrape** (PO blocker at the DISTILL gate: SP-5
  asserts exactly one `/users/{user}` request). The CLI user scrape now resolves and harvests
  from one response.
- **Case-insensitive read** via `repo_subjects_to_read` plus `StoreReadPort` listing, rather
  than a new port method (see follow-ups for the scaling caveat).
- **Q-CPI-D2 resolved:** the self-retraction rule and `bare_did` were hoisted into
  `claim-domain`, one rule each, shared with `appview-domain`.
- **Q-CPI-D4 resolved:** `latest_composed` supersedes the newest `composed_at`, relying on the
  single-clock RFC3339 invariant (lexicographic order = time order).

## Steps completed (DELIVER)

Every step logged PREPARE → RED_ACCEPTANCE → RED_UNIT → GREEN → COMMIT `EXECUTED/PASS` in
`deliver/execution-log.json`; `verify_deliver_integrity`: "All 15 steps have complete DES
traces".

| Step | Outcome | Commit | Notes |
|---|---|---|---|
| 01-01 | Scrape records top-30 human contributors in an append-only link table and names excluded bots | `25c8c5b` | WS first half |
| 01-02 | Walking skeleton GREEN: `infer people --sign 1` signs an evidence-backed adherence through the shared sign batch | `6034659` | WS-CPI-1 green |
| 01-03 | `--contributors N` in 0..=100, at most one extra request (0 at N = 0), refused on a person target | `4701ce4` | UC-1, UC-3 |
| 01-04 | Commit-ranked regardless of GitHub order, cross-repo overlap shown, re-scrape never loses anyone; enforced by check-arch | `1afbdea` | DDD-16 |
| 01-05 | Failed harvest records nothing and exits non-zero; too-large / empty repo is a named notice, exit 0 | `82bcc3b` | UC-2 |
| 02-01 | Candidates list each supporting claim with its author and rank, at capped speculative confidence; unsigned repos noted | `0639d40` | 4/5 ATs already green on activation |
| 02-02 | Only active, non-retracted, non-superseded, non-marker claims support; self-retraction rule hoisted into claim-domain | `e2201cf` | |
| 02-03 | Already-signed pairs not re-proposed; `--person` / `--min-repos`; offline and read-only; surface says "person" | `4b2176c` | IP-14 |
| 03-01 | Signing keeps each supporting claim's author, re-verifies to the same CID, publishes the same evidence; confidence in [0, 1] | `ac1672a` | UC-7 |
| 03-02 | Selections validated before compose; numbering identical to the list under the same filters | `1bd6ac7` | RED_ACCEPTANCE skipped: all 4 ATs (IS-4/5/6/9) green on activation; pinned with properties |
| 04-01 | Scrape that changes inference inputs ends with a new-candidates hint; unchanged scrape is silent | `c5bf618` | |
| 04-02 | STRONGER candidate adds a `supersedes` reference; the earlier claim stays byte-identical | `a180b56` | UC-4 |
| 04-03 | SUPPORT WEAKENED with reason (retracted / unsubscribed / missing); never acted on | `31f4ff2` | UC-5 |
| 05-01 | `scrape github <user>` shows links, signed adherence and candidates, or guidance, from one `/users/{user}` request | `770efe9` | SG-3 narrowed |
| 05-02 | Signing from the person view equals `infer people --person … --sign`; possible-rename flag without merging | `c65ad5d` | Q-CPI-D7 |

Refactor: `cd3d620` (L1-L2 readability and complexity), `f9f1683` (L3-L4: `people.rs`, about
1250 production lines, split into `people/{selection,overlap,confidence,candidate,provenance,inference,subject,report,classify,weakened}.rs`
behind the same re-exports). Mutation killing tests: `1813739`.

## Mutation results

cargo-mutants 25.3.1 (`--timeout 120 -j 2`, no `--in-place`). Gate: ≥ 80% kill rate. Full
report: `docs/feature/contributor-philosophy-inference/deliver/mutation/mutation-report.md`.

| Surface | Before | After |
|---|---|---|
| `scraper-domain/src/people/*` (gated) | 127/161 = 78.9% (WARN; confidence, mod, subject < 70%) | **158/161 = 98.1%** |
| `claim-domain/src/retraction.rs` (gated) | 13/13 = 100% | 13/13 = 100% |
| `cli/src/verbs/sign_batch.rs` pure fns, `--lib` (gated; clean baseline) | 15/15 = 100% | 15/15 = 100% |
| `adapter-github` `classify_contributors` (report only) | 5/5 = 100% | 5/5 = 100% |

**Gated total: 82.0% → 98.4% (186/189), PASS.** The 3 survivors are proven equivalent (`<` vs
`<=` on keys where a tie means identical or equal-valued data), so the rate is 100% with them
excluded. 31 survivors were killed by 8 new properties and 2 examples. Three were caused by
**self-referential oracles**, where the property's expected value reused the function under
test: `is_bot` inside `distinct_human_ids`, `Hundredths::value` in the DDD-9 expected value,
and `candidate.login()` in the commits-URL suffix. All three oracles were rewritten from the
spec text. No production code changed.

## Review

- **Consolidated DISTILL gate** (DISCUSS + DESIGN + DISTILL): product-owner reviewer approved
  after one blocker fix (slice-05 "exactly one request" vs the shipped double `/users` fetch;
  SP-5 now asserts `seen_paths == ["/users/BurntSushi"]`); solution-architect reviewer approved
  with 0 findings; acceptance-designer reviewer approved (avg 9.3, 0 findings). RED gate: 0
  BROKEN, 0 WRONG_ASSERTION.
- **DELIVER adversarial review (Phase 4): APPROVED, no blockers.** Its concern that some
  properties "pass against stubs" was confirmed by mutation testing and fixed (above).
- **CI:** main green at `1813739`.

## Live demo evidence

Live-GitHub dogfood demos passed for all 5 slices. The load-bearing run (risk R-1: "real
repos rarely share top-N humans") scraped `rust-lang/regex` and `BurntSushi/ripgrep`: 3 of
each repo's top-10 contributors overlap, and with `memory-safety` signed for both repos,
`infer people` proposed **BurntSushi, tiehuis and atouchet → memory-safety**. Signing and the
person view (`scrape github BurntSushi`) worked end to end. R-1 did not materialize for
related repos in one ecosystem.

## Lessons learned

- **Thin slices over-deliver, so later ATs arrive green.** Many acceptance tests were already
  green on activation because earlier slices built more than their thin share (02-01 4/5,
  03-02 4/4, others). Instead of forcing a fake RED, those steps pinned the behaviour with
  pure-core properties. Roadmaps built from per-slice AT lists should expect this and budget
  the step as "prove and pin", not "build".
- **A thick walking skeleton should be split explicitly.** Splitting WS-CPI-1 across 01-01 and
  01-02 kept each step within a day and still turned the skeleton green on day one.
- **Circular oracles hide in property tests.** Three properties computed their expected value
  with the function under test. Mutation testing was the only thing that caught them. Write
  oracles from the spec text (DDD-3, DDD-9, ADR-064), never from the implementation.
- **Hoist a rule once it has two consumers.** The self-retraction rule and `bare_did` moved into
  `claim-domain` as single definitions, so appview and inference cannot drift.
- **Time-ordering by string is an invariant, not a given.** `latest_composed` compares RFC3339
  strings; that is correct only because one clock and one format mint them. It is documented
  as an invariant.
- **Clean up after deliberate break-runs.** Proptest seed files from intentional RED runs were
  deleted so they do not replay stale failures.

## Issues and follow-ups

- **Viewer `/scrape` still double-fetches `/users/{user}`.** The CLI person scrape now makes
  exactly one request (SP-5), but the viewer route's user path is a pre-existing surface that
  still resolves and harvests separately. Follow-up: route it through the single-fetch path.
  **Resolved 2026-09-28** by `fix-viewer-double-users-fetch` (`f7b66b4`, `9b700b3`); see
  [its evolution doc](fix-viewer-double-users-fetch-evolution.md).
- **`claim_domain::Confidence::try_new` RED-scaffold panic** is pre-existing, off the feature
  path, and untouched. Follow-up: implement or remove the scaffold.
- **`repo_subjects_to_read` scales with store size.** The case-insensitive join lists subjects
  through `StoreReadPort` rather than a dedicated query. Fine for dogfood stores; watch it as
  stores grow and add a case-folded query method if it shows up in profiles.
- **DID ↔ GitHub login linking is deferred** (OD-CPI-1): a future opt-in, human-signed
  `sameAs`-style claim.
- **The possible-rename flag appears only in the person view**, not in the `infer people`
  list. Follow-up if users need it there.
- **Deferred by design:** `--json` for `infer people` (Q-CPI-D6); a viewer "person" page;
  federating contribution links (OD-CPI-2).
- **Housekeeping not done in this finalize:** KPI-CPI-1..5 registration in
  `docs/product/kpi-contracts.yaml` and the outcome registry; marking slice briefs as shipped
  (DoD item 9); ADR-063 / ADR-064 status (left for the user to accept).

## PO follow-up

UC-1..UC-8 (DESIGN) and IP-14 (DISTILL, `--person` must be `github:<login>`) were ratified at
the DISTILL gate and are now appended as explicit ACs on US-CPI-001..005 in
`docs/feature/contributor-philosophy-inference/feature-delta.md`
(section "DELIVER / [REF] Ratified AC additions"). The original DISCUSS wording is unchanged.

## Links

- **Design:** `docs/adrs/ADR-063-contributor-philosophy-inference-pure-core-in-scraper-domain-append-only-contribution-links.md`,
  `docs/adrs/ADR-064-person-adherence-claim-wire-contract-github-login-subject-adherestophilosophy-at-uri-provenance-evidence.md`.
  Related: ADR-007 (functional Rust), ADR-008 (references / supersedes), ADR-009 (hexagonal),
  ADR-017..019 (scraper), ADR-060 (retraction rule D-RF-D3).
- **Migrated scenarios:** `docs/scenarios/contributor-philosophy-inference/walking-skeleton.md`
  (status IMPLEMENTED).
- **Product:** `docs/product/jobs.yaml` (J-004, J-004d, J-004e),
  `docs/product/journeys/infer-contributor-philosophy.yaml`.
- **Workspace** (kept): `docs/feature/contributor-philosophy-inference/`: `feature-delta.md`
  (all waves, C4 L1-L3, KPIs), `slices/slice-01..05`, `design/{wave-decisions,upstream-changes}.md`,
  `distill/{test-scenarios,acceptance-review,red-classification,walking-skeleton}.md`,
  `deliver/{roadmap.json,execution-log.json,mutation/mutation-report.md}`.
- **Code:** `crates/scraper-domain/src/people/`, `crates/claim-domain/src/retraction.rs`,
  `crates/ports` (`ContributionLinkPort`, `GithubPort::list_contributors`),
  `crates/adapter-github/src/lib.rs`, `crates/adapter-duckdb` (schema v5,
  `DuckDbContributionLinkAdapter`), `crates/cli/src/verbs/{infer_people,scrape_github,sign_batch}.rs`,
  `xtask/src/check_arch.rs`.
- **Acceptance:** `tests/acceptance/{infer_people_sign,contributor_links,infer_people,infer_people_evidence_grows,scrape_person}.rs`,
  `tests/acceptance/support/people.rs`.
- **Commits:** DISCUSS `14c5e07`, DESIGN `57e6744`, DISTILL `0b61668`, roadmap `21ce375`,
  DELIVER `25c8c5b` … `c65ad5d`, refactor `cd3d620` / `f9f1683`, mutation `1813739`.
