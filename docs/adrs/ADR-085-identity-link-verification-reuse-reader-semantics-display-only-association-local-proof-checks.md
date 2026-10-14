# ADR-085: Identity-Link Verification Reuse and Reader Semantics — One Pure Proof in `claim-domain`, Association Only After Your Own Check, Local Never-Federated Proof Checks

- **Status**: Proposed (2026-10-14; revised after review iteration 1). **Conditional**: the
  Decision text describes the recommended options of GDL-D6..D14
  (`docs/feature/github-did-identity-link/design/wave-decisions.md`). It binds only once the user
  confirms them; DISTILL must not start before GDL-D1..D5 (ADR-084) are confirmed.
- **Date**: 2026-10-14
- **Deciders**: Morgan (nw-solution-architect), propose mode; Jeff Bailey to confirm
- **Feature**: github-did-identity-link (DESIGN)
- **Companion**: ADR-084 (wire contract)
- **Amends (behavior-preserving)**: ADR-076 §4 (the pure bio matcher moves to `claim-domain`;
  `review-domain` delegates). **Amends (additive)**: ADR-022 (the scorer drops `sameAs` claims).
- **References**: ADR-007, ADR-009, ADR-014/022 (anti-merging), ADR-023/072 (disjoint roots),
  ADR-060 (retraction rule), ADR-063, ADR-071 (provenance + origin), ADR-074 (purge precedent),
  ADR-076, ADR-078, ADR-082 (separate purge port precedent)

## Context

ADR-084 fixes what a link looks like. This ADR fixes who checks it, where the checking code
lives, and what every reader does with a link.

- The bio proof exists once, in `crates/review-domain/src/ownership.rs` (pure: `did_tokens`,
  `ownership_verdict`, `prove_ownership`, `VerifiedOwnership`). The CLI does not depend on
  `review-domain`, which also carries maud views and the review/consent context.
- `check-arch` forbids adapter→adapter dependencies; only the three roots wire adapters, with
  disjoint capability sets. The CLI and the review app both wire `adapter-github`.
- `GithubPort::read_person` takes a **login**; there is no id-based read. A renamed account cannot
  be told apart from a re-registered login without one.
- The indexer holds no GitHub capability (ADR-023/078). `infer people` is offline by design.
- Anti-merging (ADR-014/022): every aggregate decomposes to `(author_did, claim_cid)`.
- `scoring::score` groups by (subject, object) with no predicate filter. It is called from the
  CLI (`graph_query.rs`, two sites) and the viewer (`adapter-http-viewer` `/score`), all over
  `StoragePort::query_attributed_for_scoring`.
- The wire rule (ADR-084 §3) proves only the DID side. Any DID owner can publish a `sameAs` naming
  any GitHub account. Readers must not turn that into an association.

## Decision

1. **One pure proof, in `claim-domain::identity_link`** (new module, no new crate): the DID-token
   matcher hoisted verbatim from `review-domain::ownership`, a `GithubObservation { login, user_id,
   bio }` value, `check_link`, the ADR-084 codec, `project`, and `link_view` (§4).
   `review-domain::prove_ownership` keeps its signature and delegates; its property tests are the
   regression guardrail.
2. **Capability-typed compose and fresh reads.** `LinkDraft` is built only from `ProvenLink`, and
   `ProvenLink` only from a passing `check_link` against an observation read **within the same
   command invocation or HTTP request, with no cache** ("fresh"). Both the CLI verb and the review
   app's publish handler perform that read themselves; the review app's stored `github_links` row
   is never the proof. GitHub may serve a profile ~60 s stale (ADR-076 §6); that window is an
   accepted limitation, and the next check shows the truth.
3. **Each root performs its own GitHub reads through its own wiring.** `GithubPort` is EXTENDED
   with `read_person_by_id(id)` (one `GET /user/{id}`) in `adapter-github`. No adapter depends on
   another. The indexer gets **no** GitHub capability.
4. **Check algorithm** (pure decision over at most two reads):
   - Read `/users/{signed or last-seen login}`. GitHub may answer a renamed login with a redirect
     (A-10, partly unverified): the adapter follows at most one same-host `api.github.com`
     redirect, and `login_seen` is always the `login` in the final profile body. Same id → match
     the bio.
   - Login 404, or a different id (re-registered) → read `/user/{id}`. Found → match the bio of that
     profile (a rename; `login_seen` = the current login). 404 → `account_not_found`.
   - Rate limit / outage on either read → **no check is recorded**; state unchanged.
   - Recorded verdicts (one label set, ADR-076 spelling): `verified`, `did_missing`,
     `different_did`, `no_bio`, `account_not_found`. `identity_changed` is not recorded: the id
     read resolves it into one of the above.
5. **Reader projection** (`link_view`, pure). Input: projected links (each from a claim whose
   ADR-071 provenance verdict is already established — `project` takes the verified claim *and*
   its `Provenance`, so a self-attested `sameAs` from a non-`AuthorPds` origin never projects),
   ADR-060 lineages, and the reader's latest local checks. Keyed by **(subject DID, numeric id)**.
   - `Lifecycle = Active | Retracted | Superseded` (same-author markers only).
   - `ProofCheck = Verified { at, login_seen } | Disproved { reason, at } | NotChecked`.
   - **Association rule**: a GitHub id is *associated* with a DID **only** when that DID's Active
     link has a `Verified` latest check **by this reader** and no other DID's Active link to the
     same id is also `Verified`. Two or more `Verified` DIDs (the bio names several) → `Contested`,
     none associated.
   - `NotChecked` and `Disproved` links are rendered as **assertions** ("did:… claims github:x —
     not checked / proof gone"), never as associations, and they never make a verified link
     Contested. A squatter's link therefore cannot hijack or grief a genuine one.
   - One DID may hold several Active links; each is judged alone.
   - Every rendered line shows the check age ("verified 12 days ago"). Age alone never
     downgrades the state; past 30 days the line adds "re-check with `openlore identity verify`".
6. **Readers: display-only association.** `infer people` / `scrape github <user>` / `identity
   list` show the projection offline from the last local check. `graph query`, `search`, the
   indexer and the viewer show links as ordinary claims (a viewer linked-identity line is a later
   slice). **Scoring choke point: `scoring::score` itself drops every claim whose predicate is
   `sameAs`** before grouping (contract shape: *unbounded preservation* — for every feed `F`,
   `score(F) == score(F \ {rows with predicate sameAs})`, and no other row is altered). One rule
   inside the pure scorer covers all current and future callers (CLI and viewer).
7. **Local proof checks, never federated, read/write split.**
   - Table (migration **v7**) `identity_link_checks`: **insert-only**, primary key `check_id`
     (a DuckDB sequence, so equal timestamps never collide), indexed on (`subject_did`,
     `github_user_id`); "latest" is a read query (max `checked_at`, then max `check_id`). Growth
     is one row per explicit check or person scrape of a linked login; `identity forget` bounds it.
   - `IdentityLinkCheckReadPort` (`latest_checks(filter)`), `IdentityLinkCheckWritePort`
     (`record_check` — insert only), `IdentityLinkCheckPurgePort` (`purge_subject(did)` — the only
     DELETE, ADR-082 precedent). The adapter implements `probe`.
   - Per-verb capabilities (composition root injects the narrowest handle):

     | Verb / handler | Check read | Check write | Check purge | `IdentityPort::sign` + publish | `GithubPort` |
     |---|---|---|---|---|---|
     | `identity list`, `infer people` | ✓ | — | — | — | — |
     | `scrape github <user>` | ✓ | ✓ (published links for that login only) | — | existing `--sign` only | existing |
     | `identity verify` | ✓ | ✓ | — | — | ✓ |
     | `identity link` | ✓ | ✓ (on confirm only) | — | ✓ (on confirm only) | ✓ |
     | `identity forget --did <did>` | — | — | ✓ | — | — |
     | review app publish/retract link | — (no CLI store) | — | — | create-only `UserRepoWritePort` | ✓ |
   - `identity link` without confirmation is **plan-returning**: it reads GitHub, builds the plan
     and renders the preview, and writes nothing. On confirm it writes exactly {one check row, the
     signed claim via the single publish path}.
   - Retention: checks are a local cache of observations. `identity forget --did <did>` removes one
     DID's history; nothing else deletes.
8. **Federation**: links travel as ordinary claims through the unchanged peer-pull, indexer and
   self-attested paths. Each reader proves the GitHub side itself (`identity verify`). Checks are
   never published; a peer's check never counts for you. Index/search results label links "GitHub
   side not checked here".

## Alternatives Considered

| Topic | Alternative | Verdict |
|---|---|---|
| Proof home | **CLI depends on `review-domain`** | Rejected: drags maud page views and the review/consent context into the CLI. |
| Proof home | **New pure crate `identity-link-domain`** | Viable; a sixth pure-core `check-arch` arm for ~200 lines, while every reader already depends on `claim-domain`. Revisit if other identity providers arrive. |
| Proof home | **`scraper-domain`** | Rejected: `scoring`/`viewer-domain` would depend on it only for `project`; splits one rule set. |
| Association | **Associate any Active link; Contested on any two Active links** | Rejected (review H-2): an unproven squatter link would associate by default and grief verified links. |
| Association | **Opt-in union flag** (`--include-linked` with per-subject breakdown) | Deferred: needs its own ADR-022-style decomposition. |
| Association | **Canonical merged subject** | Rejected: silent merging (ADR-014/022). |
| Scoring exclusion | **Filter in each caller / in the SQL feed** | Rejected: three call sites today, more later; SQL filter puts a domain rule in an adapter. The pure scorer is the one choke point. |
| Rename detection | **Login read only; treat 404 as disproved** | Rejected (review H-3): a renamed account would show as disproved. |
| Federation | **Indexer re-verifies bios** | Rejected: GitHub capability and a shared rate budget in the indexer root. |
| Federation | **Trust the signature alone** | Rejected: a DID owner could link an account they do not control. |
| Checks | **Latest-wins upsert** | Rejected (review H-1): an UPDATE that loses history; insert-only + read query is simpler to enforce. |
| Checks | **No persistence** | Rejected: network in offline verbs. |
| Checks | **Columns on `contribution_links`** | Rejected: different aggregate. |
| Invalidation | **Auto-retract when the proof disappears** | Rejected: append-only invariant (CPI D-5, ADR-008). |

## Consequences

- **Positive**: one proof rule for three call sites; no new crate; no adapter→adapter edge; no
  indexer capability change; offline verbs stay offline; squatting cannot create or block an
  association; one scoring rule covers every caller.
- **Negative**: a link's GitHub-side status is reader-local and can be stale; readers may disagree
  (by design). One migration, three small port traits, one `GithubPort` method (one more endpoint
  on the adapter allowlist). A behavior-preserving refactor of the shipped review app.
- **Enforcement** (three layers): TYPE — private `ProvenLink`/`LinkDraft` constructors; split
  read/write/purge traits; `project` requires a `Provenance`. STRUCTURAL (`xtask check-arch`) —
  `identity_link_checks_insert_only` (no `UPDATE`/`DELETE` on the table outside
  `adapter-duckdb/src/identity_link_purge.rs`), `identity_link_purge_only_in_forget_verb`,
  `identity_link_readers_hold_no_write_handle` (the `identity list` / `infer people` modules name
  no write/purge port and no `IdentityPort`), `identity_link_compose_only_via_proven_link`, and the
  existing indexer capability boundary. BEHAVIORAL — AT fixtures (brief "For Acceptance
  Designer").

## Earned Trust

| Probe | Runs | Pass criterion | On failure |
|---|---|---|---|
| Check-store adapter `probe()` | CLI startup wiring (wire → probe → use), every command | In a transaction: insert the same sentinel key at two `checked_at` values, read back the later one as latest, roll back | CLI refuses to start (`health.startup.refused`), as every adapter today |
| `adapter-github` probe (existing: rate limit, no-token-leak) | CLI and review-app startup | Unchanged | Unchanged (startup refused) |
| GitHub profile shape (`login`, `id`, `bio` on `/users/{u}` and `/user/{id}`) | **Required** CI contract test over recorded fixtures `crates/test-support/fixtures/github/users_profile.json` and `user_by_id.json`; optional scheduled live smoke | Both shapes decode; id endpoint returns the same shape | CI red. At runtime a shape error is `GithubUnavailable`: `identity link` fails closed (no publish), `identity verify` records nothing, offline reads unaffected |
| Codec round-trip | CI property test | encode → canonicalize → CID → decode → project = same DID, login, id; any mutation of subject, author, predicate, id URI or provenance → `None` | CI red |

GitHub lies in the `FakeGithub` catalogue: profile ~60 s stale; login 404 after rename (resolved by
id read); login re-registered by another id; deleted account (404 on both); bios with several DIDs,
a prefix-colliding DID, an uppercase DID, trailing punctuation; 403 rate limit (records nothing).
Clock: `checked_at` from `ClockPort`; a check dated in the future renders as `NotChecked`.
