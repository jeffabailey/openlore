# Wave Decisions: github-did-identity-link (DESIGN)

- **Mode**: propose (autonomous subagent). Every decision below lists 2-3 options and a
  recommendation. **The user approved every decision and assumption as proposed on 2026-10-14**;
  each is marked ACCEPTED (user, 2026-10-14).
- **Revised by ADR-086/087 (owner-only-signing)**: a verified link is also the proof of signing
  authority over `github:` subjects (UC-1..UC-8 of `docs/feature/owner-only-signing/design/upstream-changes.md`,
  applied here, in `architecture-design.md` and in ADR-085).
- **Architect**: Morgan (nw-solution-architect). **Date**: 2026-10-14.
- **Scope**: application. **Paradigm**: functional Rust (ADR-007). **Style**: unchanged — hexagonal
  modular monolith (ADR-009), three disjoint composition roots (ADR-023/072).
- **No DISCUSS wave.** Requirements are derived from OD-CPI-1, the shipped bio-DID proof (ADR-076)
  and the claim model (ADR-005/008/064/071). They are recorded as assumptions A-1..A-9 for a later
  DISCUSS pass. Prior-wave changes: `upstream-changes.md`.
- **ADRs**: ADR-084 (wire contract), ADR-085 (verification reuse and reader semantics). Both
  Accepted (2026-10-14). C4 and component detail: `architecture-design.md`.

## Prior-wave reading

| | Artifact | Used for |
|---|---|---|
| ✓ | `docs/product/architecture/brief.md` | Component inventory, roots, I-1..I-12, SSOT discipline (ADRs flat in `docs/adrs/`) |
| ✓ | `docs/adrs/` (latest ADR-083; ADR-063/064 Proposed) | ADR-008, 022, 059, 063, 064, 070, 071, 073, 076, 078 |
| ✓ | `docs/feature/contributor-philosophy-inference/feature-delta.md` | OD-CPI-1 (l.834), shared artifact `person_subject` (l.166), out of scope (l.792), DESIGN DDD-1..16 |
| ✓ | `docs/feature/bluesky-claim-review-app/design/wave-decisions.md` + code (`review-domain/src/ownership.rs`, `openlore-review-app/src/routes/github.rs`, `ports/src/review.rs` `GithubLinkPort::record_verified_link`) | The bio-DID proof, re-verification gate, numeric id pin, create-only scopes (SPIKE-3) |
| ✓ | `crates/ports/src/identity_resolve.rs`, `crates/adapter-atproto-did` | DID→PDS/handle resolution (reused for display only) |
| ✓ | `crates/cli/src/verbs/infer_people.rs`, `render/people.rs` (`candidate_renames`, `possible_renames`) | Rename flagging by numeric id |
| ✓ | `crates/scoring/src/score.rs` | Groups by (subject, object), no predicate filter → exclusion needed |
| ✓ | `docs/product/kpi-contracts.yaml` | KPI-CPI-1..5 pattern; no GDL KPIs registered (DISCUSS/DEVOPS own registration) |
| ✓ | `lexicons/org/openlore/claim.json` | Free-string subject/predicate/object, `evidence: uri[]`, ref types |
| ⊘ | `lexicons/org/openlore/philosophy.json` | Not relevant (no philosophy object) |
| ⊘ | DISCUSS / DISCOVER artifacts for this feature | Do not exist |
| ⊘ | `docs/product/architecture/adr-*.md` | Repo convention is `docs/adrs/ADR-NNN-*.md`; followed |

## Assumptions (accepted by the user 2026-10-14)

| ID | Assumption | Basis | If wrong |
|---|---|---|---|
| A-1 | The job: "let readers know my GitHub contributions and my OpenLore claims are the same person, without anyone else being able to say so." | OD-CPI-1 | Re-scope |
| A-2 | Linking is **opt-in, by the DID owner only**, and never inferred (no link from a bio scan, a contribution link, or a rename flag). | OD-CPI-1 "opt-in, human-signed"; CPI R-2 surveillance risk | Hard constraint — confirmed by the user 2026-10-14 |
| A-3 | The CLI user (P-001) is the first user; review-app users second. | CPI personas; the review app already holds verified links | Swap slice order |
| A-4 | One DID may link several GitHub accounts; one GitHub account should be linked by at most one DID (more = Contested). | People have work/personal accounts; impersonation risk | Change projection only |
| A-5 | Keeping the DID in the GitHub bio is acceptable to users; removing it makes readers show "proof gone". | ADR-076 already requires it in the review app | Consider gist proof (GDL-D4 option C) |
| A-6 | Display-only association is enough for v1; nobody needs a merged person score yet. Revised by ADR-086/087 (owner-only-signing): no merging still, but a verified link is also the proof of signing authority. | CPI out-of-scope "changes to scoring"; ADR-022 | ADR-085 deferred union flag |
| A-7 | GitHub's `https://api.github.com/user/<id>` is a stable identifier URI for the account. Confirmed by the user 2026-10-14. | Long-standing public endpoint (not in GitHub's "versioned" docs) | Accept `github-id:<n>` in evidence — a change to the ADR-084 §3 reader rule, not a free-string swap |
| A-10 | GitHub `GET /user/{id}` returns the current profile (same shape as `/users/{login}`) for a renamed account and 404 for a deleted one; after a rename the old login on `/users/{login}` either redirects to the new login or 404s, unless someone re-registers it (then a different id). **Partly unverified** — the check algorithm handles redirect, 404 and different id. | Observed GitHub behavior; not externally verified in this wave | Pin with recorded fixtures (`user_by_id.json`) and the scheduled live smoke; if wrong, renames show as `account_not_found` until re-linked |
| A-8 | A link is an identity fact: confidence fixed at 10000 bp, not editable. | ADR-070 | Make editable (no other change) |
| A-9 | Published links are acceptable as public, permanent records (retraction withdraws, does not erase). | ATProto semantics; ADR-008 | Gate behind an extra confirmation |

## Key Decisions

Status legend: **ACCEPTED (user, 2026-10-14)** = the recommendation, approved as proposed.

### GDL-D1 — Claim shape: existing record vs new lexicon — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. `org.openlore.claim` with predicate `sameAs`** (recommended) | Zero Lexicon/CID/validator change; review app needs no new OAuth scope (`repo:org.openlore.claim?action=create` already granted, SPIKE-3); every reader, retraction and both provenance modes work unchanged. Cost: shape is a free-string convention (like ADR-064). |
| B. New record `org.openlore.identityLink` | Typed fields; but new OAuth scope + re-consent for every review-app user, a second record path in peer pull/indexer/viewer/peer_claims, and old peers ignore it. |
| C. DID document `alsoKnownAs` (PLC op) | ATProto-native, but needs a PLC rotation-key operation users on bsky.social cannot do; OpenLore holds no PLC key. |

Rationale: the simplest option that keeps consent and federation on paths already verified.
Source: ADR-005, ADR-064 §Alternatives, ADR-071, ADR-073, bluesky-claim-review-app SPIKE-3.

### GDL-D2 — Field layout and predicate name — ACCEPTED (user, 2026-10-14)
Recommended: `subject` = author's bare DID; `predicate` = `sameAs`; `object` = `github:<login>`;
`evidence` = [`https://github.com/<login>`, `https://api.github.com/user/<id>`]; confidence 10000;
`supersedes` only for a re-link after rename (ADR-084 §2).
| Option | Trade-off |
|---|---|
| **A. subject = DID, object = `github:<login>`, `sameAs`** (recommended) | Consent rule is a single equality (subject == author). Matches OD-CPI-1 wording; schema.org `sameAs` is exactly "profile of the same entity". |
| B. subject = `github:<login>`, object = DID | Fits ADR-064 subject convention so `graph query --subject github:x` lists it; but every reader sees a claim *about* a GitHub account by another party, and scoring pairs it under the person. |
| C. Predicate `controlsAccount` instead of `sameAs` | Directional and literally what is proven; departs from OD-CPI-1 vocabulary. A one-constant switch. |

### GDL-D3 — Who may assert a link — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Only the DID owner (author == subject); third-party `sameAs` claims are ordinary claims, never projected as links** (recommended) | Consent by construction; no doxxing vector; nothing new to moderate. |
| B. Allow third-party assertions as a weaker "asserted by X" link state | Lets others connect identities without consent — contradicts A-2 and CPI R-2. |
| C. Require a counter-signature from both sides on the wire | GitHub accounts cannot sign; collapses into D4. |

### GDL-D4 — Proof carried and how the GitHub side is proven — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. DID side = record provenance (app-signed or self-attested); GitHub side = live bio check (ADR-076 matcher) + numeric id pinned in evidence; composing refuses without a passing check** (recommended) | Reuses the shipped proof; any peer can re-check; renames detectable. Proof is only as current as the reader's last check. |
| B. Embed a bio snapshot / verification timestamp in the record | Looks self-contained but is just the author's statement; forgeable; stale forever. |
| C. Keybase-style signed gist on GitHub | Stronger, survives bio edits; adds a GitHub write the user must do, a second fetch shape and a new rate cost. Revisit if A-5 fails. |

### GDL-D5 — Retraction and invalidation — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Retract = shipped ADR-008 `retracts` by the same author (CLI `claim retract`, review-app `RetractPlan`); a vanished bio proof is shown as `Disproved`, never auto-retracted** (recommended) | Append-only invariant kept; the owner stays in control; readers are honest about state. |
| B. Auto-retract when a check fails | System signs on the user's behalf (violates CPI D-5); a 60 s stale profile could retract a valid link. |
| C. Links expire after N days without re-proof | Silent decay; needs a clock contract on the wire (CID-relevant). |

### GDL-D6 — Where linking is initiated — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Both: CLI first (`openlore identity link github <login>`, `identity verify`, `identity list`), review app second ("Publish this link" on the verified GitHub step)** (recommended) | One pure core, two thin shells; review-app users already have a verified link. |
| B. CLI only | Smallest; review-app users (no CLI, no app key) cannot link. |
| C. Review app only | Reuses its UI, but CLI users would need OAuth sign-in to link their own DID. |

### GDL-D7 — How verification is reused without adapter→adapter deps — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Hoist the pure matcher into `claim-domain::identity_link`; `review-domain` delegates; each root wires its own `adapter-github`** (recommended) | No new crate; every reader already depends on `claim-domain`; shipped review-app proptests guard the refactor. |
| B. New pure crate `identity-link-domain` | Clean boundary; a sixth pure-core `check-arch` arm for ~200 lines. Revisit if other identity providers arrive. |
| C. CLI depends on `review-domain` | Drags maud views and the review context into the CLI. |

### GDL-D8 — How readers treat a linked pair — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Display-only association everywhere; `scoring::score` itself drops every `sameAs` claim (one choke point for the CLI and viewer callers; unbounded preservation: `score(F) == score(F \ sameAs rows)`); no subject merging** (recommended) | Keeps ADR-014/022 anti-merging; smallest change; honest. |
| B. Opt-in read-time union (`--include-linked`) with per-subject breakdown | Useful later; needs its own decomposition design. Deferred. |
| C. Canonical merged subject | Silent merging; contested links get resolved by guessing. Rejected. |

Revised by ADR-086/087 (owner-only-signing): association stays display-only, but a `verified` link
is also the proof of signing authority over `github:<login>` (current login) and over repos owned
by that account. The `sameAs` drop is expressed via `authority::kind` in `score_attested` (same
outcome).

### GDL-D9 — Renames and duplicates — ACCEPTED (user, 2026-10-14)
Recommended: key every link by (subject DID, numeric GitHub id). A check reads `/users/{login}`;
on 404 or a different id it reads `/user/{id}` (NEW `GithubPort::read_person_by_id`) and matches
that profile's bio — a renamed account stays `verified` with `login_seen` = the new login; a deleted
one is `account_not_found` (ADR-085 §4, A-10). The signed login is historical; a re-link MAY
`supersede`. Duplicates (same DID, same id, several Active claims): show one, newest `composedAt`,
list CIDs.
| Option | Trade-off |
|---|---|
| **A. Id-keyed with an id-based read on rename** (recommended) | Correct for renames and re-registrations; one more endpoint on the adapter allowlist; ≤ 2 requests per check. |
| B. Login read only; 404 → disproved | No port change, but every rename looks like a lost proof. |
| C. Key by login | Breaks on rename; a re-registered login inherits the link. |

### GDL-D14 — When a link counts as an association (squatting, conflicts) — ACCEPTED (user, 2026-10-14)
The wire rule proves only the DID side, so any DID owner can publish a `sameAs` naming any GitHub
id.
| Option | Trade-off |
|---|---|
| **A. Associate only when this reader's latest check is `Verified`; unchecked/disproved links render as assertions ("did:… claims github:x — not checked / proof gone"), never as associations, and never make a verified link Contested; `Contested` only when ≥ 2 DIDs are `Verified` for one id (the bio names both)** (recommended) | A squatter can neither hijack nor grief; costs one `identity verify` before a peer's link shows as associated. |
| B. Associate any Active link; label `NotChecked` | Zero extra requests, but the squatter's DID is associated with the victim by default. |
| C. Contested on any two Active links | Simple, but any stranger can block a genuine link. |

Revised by ADR-086/087 (owner-only-signing): the rule governs display only. For signing authority
each DID with its own `Verified` check counts, even when the id is Contested.

### GDL-D10 — Local proof-check store — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. DuckDB v7 `identity_link_checks`, insert-only (PK includes `checked_at`; latest = read query); split ports `IdentityLinkCheckReadPort` / `IdentityLinkCheckWritePort` / `IdentityLinkCheckPurgePort`; narrowest handle per verb; `identity forget --did` is the only delete; never federated** (recommended) | Offline verbs show status; `scrape github <user>` refreshes at zero extra GitHub requests; read-only verbs hold no write capability; local erasure possible. One migration, three small traits. |
| B. No persistence; check on every display | Network in offline verbs; spends rate budget per render. |
| C. Columns on `contribution_links` | Wrong aggregate (repo↔person observations). |

Revised by ADR-086/087 (owner-only-signing): v7 stays; owner-only-signing adds v8
(`repo_authority_checks`, `imported_candidates`); DELIVER may fold both into one migration. The
per-verb capability table gains rows (ADR-085 §7). `openlore authority verify` re-checks links plus
repo authority; `identity verify` is unchanged.

### GDL-D11 — Federation: how peers verify an ingested link — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Links federate as ordinary claims; DID side verified by the unchanged provenance paths; GitHub side re-checked by each reader (`identity verify`); index/search label "GitHub side not checked here"** (recommended) | No new capability in the indexer; trust is never transitive. |
| B. Indexer re-verifies bios | Gives the indexer root a GitHub capability and a shared rate budget across all DIDs. |
| C. Trust the signature as full proof | A DID owner could link an account they do not control. |

Revised by ADR-086/087 (owner-only-signing): index rows of every `github:` assertion are labelled
"authority not checked here"; the indexer still has no GitHub capability.

### GDL-D12 — Privacy and consent — ACCEPTED (user, 2026-10-14)
Recommended: (1) a link exists only when its DID owner runs the link verb or presses "Publish this
link"; (2) bio DIDs seen by `scrape github <user>` only refresh checks for **already published**
links — they never create, suggest or display an unpublished association; (3) the preview states
that the record is public and that retraction withdraws but does not erase; (4) no link data in
telemetry or KPI counters beyond aggregate counts.
Alternative: show "bio names did:… (not linked)" in the person view — informative, but it is
inference from scraped data (CPI R-2). Rejected for v1.

### GDL-D13 — Enforcement — recommended (no user decision needed)
Type: `ProvenLink`/`LinkDraft` private constructors; split read/write/purge check traits; `project`
requires a `Provenance`. Structural (`xtask check-arch`): `identity_link_checks_insert_only`,
`identity_link_purge_only_in_forget_verb`, `identity_link_readers_hold_no_write_handle`,
`identity_link_compose_only_via_proven_link` (the `sameAs` constant is referenced only in
`claim-domain::identity_link` and the scorer's exclusion), existing indexer capability boundary.
Behavioral: AT fixtures (see brief "For Acceptance Designer").
*Amended 2026-10-15 (DELIVER, Q-RM-2/Q-RM-4): see "DELIVER amendments" below for the rule
definitions as planned.*

### Freshness rule (part of GDL-D4/D6)
"Fresh" = the profile is read within the same CLI invocation or the same review-app HTTP request,
with no cache. The review app's publish handler performs its own `read_person` (and the id read if
needed); the stored `github_links` row is never the proof. GitHub may serve a profile ~60 s stale
(ADR-076 §6): a link can publish against a bio edited less than a minute ago; the next check shows
`Disproved`. Accepted limitation.

## Architecture Summary

- **Pure core** (`claim-domain::identity_link`, new module): hoisted DID-token matcher,
  `GithubObservation`, `check_link → ProofCheck`, `ProvenLink` → `LinkDraft` → `UnsignedClaim`
  codec, `project(&claim, &Provenance) → Option<IdentityLink>`, `link_view` projection (lifecycle ×
  proof check × association/contested, keyed by DID + numeric id). `scoring::score` drops `sameAs`.
- **Ports** (`crates/ports`): NEW sync `IdentityLinkCheckReadPort`, `IdentityLinkCheckWritePort`,
  `IdentityLinkCheckPurgePort`; EXTEND `GithubPort` with `read_person_by_id`. REUSED unchanged:
  `GithubPort::read_person`, `IdentityPort`, `StoragePort`, single publish path,
  `UserRepoWritePort`.
- **Adapters**: `adapter-duckdb` EXTEND (v7 migration + check adapter on the shared connection,
  purge in its own file). `adapter-github` EXTEND (`GET /user/{id}` on the allowlist).
- **Driving surfaces**: CLI NEW `openlore identity link github <login>` (plan-returning preview;
  writes only on confirm), `identity verify [--did | --github]`, `identity list [--did]`,
  `identity forget --did <did>`; EXTEND `infer people` / `scrape github <user>` render and
  `scrape github <user>` (records a check for already-published links). Review app EXTEND: "Publish
  this link" + "Retract this link" via `PlanKind::IdentityLink`.
- **Readers**: viewer/search/indexer unchanged (links are ordinary claims); viewer linked-identity
  line is an optional later slice. Per-verb capability table: ADR-085 §7.
- **Zero new crates. Zero Lexicon change. One migration (v7). Three small port traits + one
  `GithubPort` method.**

## Technology Stack

No new dependency. Reused: `reqwest` (MIT/Apache-2.0) in `adapter-github`; `duckdb` (MIT);
`proptest` (MIT/Apache-2.0); `atrium-oauth` (MIT) in the review app unchanged.

## Constraints Established

- C-GDL-1: Only the DID owner creates a link (subject == bare author DID); third-party `sameAs`
  claims never associate identities.
- C-GDL-2: No OpenLore composer produces a link without a passing bio check of a fresh profile
  read: the identity verbs via `ProvenLink` (type-enforced); every other composer refuses
  predicate `sameAs` (one pure guard in the shared compose path) **except `claim counter`**,
  whose output (author ≠ subject DID) is never projected as a link. Foreign-client `sameAs`
  records render as unverified assertions only.
  *Amended 2026-10-14 (user, DISTILL): `claim counter` exempted (DISTILL decision D-b).*
- C-GDL-3: Links are keyed by numeric GitHub id; logins are display-only.
- C-GDL-4: Association is display-only and requires this reader's own `Verified` check; the scorer
  drops every `sameAs` claim; no subject merging anywhere. Revised by ADR-086/087
  (owner-only-signing): a `Verified` link is also the proof of signing authority (per DID).
- C-GDL-5: Proof checks are local observations, insert-only, never federated; the only delete is
  the explicit `identity forget --did <did>`.
- C-GDL-9: Read-only verbs hold no check-write, purge or signing handle.
- C-GDL-10: "Fresh" proof = read in the same invocation/request, no cache.
- C-GDL-6: The system never retracts, supersedes or creates a link on the user's behalf.
- C-GDL-7: Offline verbs (`infer people`) stay offline; status comes from the last local check.
- C-GDL-8: The indexer root gains no GitHub capability.

## Slices (indicative; DISTILL/DELIVER own the roadmap)

1. Pure core + `identity link`/`identity verify` in the CLI + v7 check store (walking skeleton:
   prove → sign → publish → re-read → project).
2. Readers: person view / `infer people` linked-identity line, scoring exclusion, contested/rename
   display.
3. Review app: publish/retract the verified link (self-attested), refactor `prove_ownership` to
   delegate.
4. (Optional) viewer subject page line.

Revised by ADR-086/087 (owner-only-signing): slice 1 is a prerequisite of owner-only-signing slice
1; slice 3 must land before review-app claims can read `owner`.

## KPIs (registered in `docs/product/kpi-contracts.yaml` 2026-10-14)

- KPI-GDL-1 (guardrail): 0 links projected where subject ≠ author or evidence login ≠ object login;
  0 `sameAs` records produced by OpenLore composers without a passing check; 0 associations
  without this reader's `Verified` check — CI property + AT fixtures.
- KPI-GDL-2 (guardrail): weighted output identical with and without `sameAs` claims in the feed —
  CI property on `scoring::score` plus AT fixtures through both the CLI `graph query --weighted`
  and the viewer `/score` route.
- KPI-GDL-3 (guardrail): 100% of links whose bio proof is gone show `Disproved` on the next check —
  AT fixture.
- KPI-GDL-4 (leading): ≥ 1 dogfood link published and verified by a second reader.

## Upstream Changes

See `upstream-changes.md`: OD-CPI-1 deferral realized; ADR-064 §1 extended (numeric id is signed
in the link record, still not in adherence claims); ADR-022 amended (scoring exclusion); ADR-076 §4
pure matcher relocated (behavior-preserving); review-app DD-9 ("person-level claims come later")
partly realized by the link record; review-app `github_links` "one per DID" stays private-state only.

## External Integrations (handoff to platform-architect)

GitHub REST `GET /users/{user}` (already consumed) and `GET /user/{id}` (new). Contract tests
recommended for GitHub REST — a **required** recorded-fixture contract test in CI over
`crates/test-support/fixtures/github/users_profile.json` and `user_by_id.json` (shape `login`,
`id`, `bio`), plus the optional scheduled live smoke already recommended by
contributor-philosophy-inference. No consumer-driven Pact is possible against GitHub.

## DELIVER amendments (2026-10-15, orchestrator rulings on the slice-1 roadmap review)

| ID | Ruling | Amends |
|---|---|---|
| Q-RM-1 | `claim_record_json`, `reference_type` and `CLAIM_COLLECTION` **move together** from `crates/review-domain/src/plans.rs` into `claim-domain` (beside `identity_link`); `review-domain::plans` re-exports/delegates (`review_domain::plans::{claim_record_json, CLAIM_COLLECTION}` keep resolving for `tests/acceptance/review_app_core.rs`). Nothing is copied. claim-domain had no `CLAIM_COLLECTION`; the separate copies in `lexicon`, `adapter-atproto-*`, `scraper-domain` and `test-support` predate this feature and are untouched. Reason: CORE-4's `sut_encode_link` binds with ONE call (DD-GDL-16) and claim-domain cannot depend on review-domain. | UC-4 (second behaviour-preserving hoist) |
| Q-RM-2 | One file per identity verb: `crates/cli/src/verbs/{identity_link,identity_verify,identity_list,identity_forget}.rs` (house pattern), so the module-scoped check-arch rules can tell them apart. `identity_link_readers_hold_no_write_handle`: `identity_list.rs` names no check-write/purge port, no `IdentityPort`, no `GithubPort`; `infer_people.rs` names no check write or purge port — its `--sign` path holds signing by design. | `architecture-design.md` component table + Reuse Analysis; ADR-085 §7 / Enforcement |
| Q-RM-3 | Future-dated checks are **ignored**; the latest non-future check governs (a link with only future checks is `NotChecked`). The earlier "renders as NotChecked" text would let one clock-skewed row suppress every real check. Matches DISTILL Q-GDL-D4 and the CORE-5 oracle. | ADR-085 §5 and Earned Trust "Clock" line |
| Q-RM-4 | `identity_link_compose_only_via_proven_link` = the `sameAs` literal and the link-predicate constant appear ONLY in `claim-domain::identity_link`. `crates/cli/src/verbs/identity_link.rs` builds the claim through `LinkDraft` and obtains the predicate through the `identity_link` API, never by naming it (slice 2 decides how the scorer exclusion obtains it, through the same API). The identity verb bypasses the shared compose step (`build_unsigned_claim`, which holds the reserved-predicate guard) and uses only signing + the single publish path. *Restated 2026-10-15 (orchestrator, roadmap review R4).* | GDL-D13; ADR-085 Enforcement; `walking-skeleton.md` |
| Q-RM-5 | Link reads: `claim-domain::identity_link` exposes the link predicate through its API (e.g. `link_predicate()`); readers (`identity list`, `identity verify`) use it to select candidate rows from storage, and `project()` decides what is a link. *Ruled 2026-10-15 (orchestrator).* | GDL-D13 (compatible with Q-RM-4) |
| F14 | **Deferred**: the ADR-084 Earned Trust peer-pull fixture (publish a link to `FakePds`, re-read it through `peer pull`, CID == rkey, projection intact) moves to slice 2 (readers), where IR-1/IR-2 already drive `peer pull` of links. Slice 1 covers the codec at layer 2 (CORE-4) and the app-signed wire record at the PDS (WS-GDL-1). *Orchestrator, 2026-10-15.* | ADR-084 Earned Trust / Consequences |

## Review

Iteration 1 (nw-solution-architect-reviewer): NEEDS_REVISION — 2 critical, 5 high, 5 medium,
1 low. Revisions:

| Issue | Fix |
|---|---|
| C-1 one check port gave read-only verbs write capability | Split read/write/purge traits; per-verb capability table (ADR-085 §7); `identity_link_readers_hold_no_write_handle` rule |
| C-2 scoring exclusion only in `cli`; viewer `/score` uncovered | Exclusion moved into `scoring::score` (one choke point), declared unbounded-preservation; KPI-GDL-2 covers CLI and viewer |
| H-1 "append-only" vs "latest-wins upsert" | Insert-only table, `checked_at` in PK, latest = read query; probe = double insert + ordered read-back |
| H-2 squatter links associate/grief | New GDL-D14: association only after this reader's `Verified` check; unchecked/disproved = assertions; Contested only among Verified |
| H-3 renames undetectable with login-only read | `GithubPort::read_person_by_id`; check algorithm (ADR-085 §4); A-10 + fixture |
| H-4 "fresh" undefined for the review app | Freshness rule C-GDL-10; publish handler reads itself; staleness window accepted and tested |
| H-5 ADRs read as settled | ADR-084/085 status marked conditional; A-2, A-7 flagged for explicit confirmation; DISTILL gated on GDL-D1..D5 |
| M-1 probe failure behavior | Earned Trust table in ADR-085 (when/criterion/on-failure); GitHub shape CI test required |
| M-2 origin not an input to `project` | `project(&claim, &Provenance)`; negative property for non-AuthorPds self-attested |
| M-3 verdict label drift | One label set, ADR-076 spelling; `identity_changed` resolved by the id read, not recorded |
| M-4 no local erasure | `identity forget --did` via `IdentityLinkCheckPurgePort` (only delete) |
| M-5 C4 gaps; preview wrote a check | Viewer + ports added to C4; `identity link` preview is plan-returning, writes only on confirm |
| L-1 A-7 fallback | Restated as an ADR-084 §3 rule change |

Iteration 2: APPROVED_WITH_NOTES — all 13 iteration-1 issues resolved; 0 critical; 1 new high
(N-1), fixed in text as the reviewer's condition for DISTILL:

| Issue | Fix |
|---|---|
| N-1 (high) generic composers could sign a `sameAs` with no bio check | Every non-identity composer refuses predicate `sameAs` via one pure guard (ADR-084 §4); C-GDL-2 and KPI-GDL-1 restated |
| N-2 renamed-login redirect unspecified | Adapter follows ≤ 1 same-host redirect; `login_seen` from the final body; A-10 marked partly unverified; redirect fixture |
| N-3 Verified never expires | Lines show check age; "re-check" hint after 30 days; no automatic downgrade |
| N-4 evidence login could differ from object login | Equality added to the ADR-084 §3 rule and negative property |
| N-5 PK collision on equal timestamps | `check_id` sequence PK; growth noted |
| N-6 ADR-064 pointer, numbering | Done 2026-10-14 on acceptance: ADR-064 points to ADR-084; numbering left stable to keep references valid |

No third review iteration (max 2).

## Slice 4 (viewer linked-identity line)

DESIGN 2026-10-19 · Morgan · propose mode. Detail, wording table, C4 L3 and scenarios VL-1..19:
`slice-4-viewer-linked-identity.md`. No DISCUSS story existed; US-VL-1..3 recorded as UC-11.
Status: **PROPOSED** (awaiting the user).

| ID | Decision | Options (recommended first) | Rationale |
|---|---|---|---|
| D-VL-1 | Which pages show the line | **A. Pages about one person: `/score?contributor`, `/project?subject=github:…`, claim detail author, search author groups** · B. Every DID everywhere (tables, edges, peers) · C. `/score` only | A covers each place a person is the topic; B repeats long lines in every table row; C misses GitHub persons and search |
| D-VL-2 | Table rows and lists | **A. No line; their DIDs already link to `/score`** · B. A short badge per row | A keeps tables scannable and needs no second wording; a badge would be a second, shorter wording to keep honest |
| D-VL-3 | Data source | **A. The same lineage read as `identity list` (`read_listed_links`), moved from `cli` into `ports` and called by both** · B. Copy the read into the viewer adapter · C. CLI snapshots the lines at `openlore ui` start | A: one read, so retraction, U-19 and U-39 rules cannot drift (slice-2 lesson B2). B duplicates. C is stale and adds a startup failure mode |
| D-VL-4 | Capability | **A. Inject the three existing read-only traits (check read, candidate read, lineage read) as optional handles on `ViewerServer`** · B. Add link reads to `StoreReadPort` · C. A new `LinkedIdentityReadPort` | A matches ADR-085 §7 ("viewer: check read only"), no new port; B bloats the store trait and every fake; C is a new port for no new capability |
| D-VL-5 | Wording | **A. The `identity list` line, character for character; DID and account become links; the command in the 30-day hint is a `<code>` element; parity pinned by a test (VL-2, layer-2 property)** · B. Hoist one sentence builder into `claim-domain` · C. New viewer wording | A: two media (text, HTML with links), four sentences; a shared segment type costs more than one parity test. C splits the vocabulary |
| D-VL-6 | GitHub person match | **A. Last-seen login, case-insensitive (U-6 fallback); the viewer has no account ids for a page** · B. Add a contribution-link read to get ids | A needs no new read. Limit: until the reader re-checks, a renamed account's line stays on its old login's page, even if another account now uses that login (VL-17). B is a later option |
| D-VL-7 | Failure and probe | **A. Any link read failure → the whole region says "Linked identities could not be read from your local store."; startup runs a read-only probe (one `read_listed_links` call), never the check port's insert-and-rollback `probe()`; a refusal injects no handles, emits `viewer.identity_links.probe`, shows the notice on all four pages, never refuses the viewer** · B. Omit the region silently · C. Refuse startup · D. Call `IdentityLinkCheckReadPort::probe()` | A is honest and never shows "not checked" when the check read failed. B hides a failure. C takes down the viewer for a secondary view (ADR-054 precedent). D writes (rolled back) from the read-only viewer |
| D-VL-8 | Network | **A. None: the viewer never asks GitHub; lines come from the reader's last local check; a legend says so** · B. Re-check on page load | A keeps C-GDL-7/C-GDL-9 and the read-only viewer; B spends rate budget and needs the check write handle |
| D-VL-9 | ADR | **A. None.** No new port, crate, route, migration or cross-crate dependency (`viewer-domain` and `adapter-http-viewer` already depend on `claim-domain`); one amendment line in ADR-085 §6 | — |
| D-VL-10 | Search link-row label in the viewer | **A. Label link rows "authority not checked here", same constant semantics as the CLI (U-7: link rows only)** · B. Leave unlabelled | ADR-085 §8 asks every search surface to label; the viewer's `/search` had no label |
| D-VL-11 | Enforcement | `identity_link_readers_hold_no_write_handle` extends to `crates/adapter-http-viewer/src/**`; the new fault-seam token joins the release-build guard; link rows selected only through `is_reserved_predicate` | Rules without enforcement erode |

Constraints added: **C-VL-1** the viewer holds no check write or purge handle; **C-VL-2** the
viewer makes no GitHub request to show a line; **C-VL-3** a page never changes a standing (decided
over all active links first).

D-VL-3 detail: the moved read returns a typed `ports::LinkReadError` (no `anyhow` in `ports`).
D-VL-6 limit: login reuse after a rename shows the old account's line until the reader re-checks
(VL-17).

Review (nw-solution-architect-reviewer). Iteration 1: NEEDS_REVISION — 1 critical, 2 high,
4 medium, 1 low. All addressed:

| Issue | Fix |
|---|---|
| S4-1 (critical) the check port's `probe()` inserts (rolled back); the viewer is read-only | Read-only startup probe (one `read_listed_links`); the viewer never calls `probe()` (D-VL-7) |
| S4-2 (high) `read_listed_links` returns `anyhow`; `ports` has none | Typed `LinkReadError` in `ports`; cli converts with `?` |
| S4-3 (high) contract shapes mislabelled | Moved read = effect-read, bounded; viewer handler = effect shell, read-only, universe stated |
| S4-4 (medium) probe-refused rendering vs VL-12 | Notice on all four pages when no handles; VL-12 only when reads succeed; VL-14 |
| S4-5 (medium) new wording; parity test home | Legend, notice, heading = viewer-only constants; parity property in `cli` tests |
| S4-6 (medium) login reuse on GitHub person page | Recorded as a known limit; VL-17 |
| S4-7 (medium) scenario gaps | VL-14..VL-19; VL-9 rows per read |
| S4-8 (low) no cost bound | Per-request query count and a ~200-candidate follow-up threshold |

Iteration 2: APPROVED_WITH_NOTES. 0 critical. S4-1, S4-2 and S4-4..S4-8 resolved; S4-3 partly
resolved (note only: the shapes are stated as effect-read and effect shell, not in the
three-name taxonomy, because neither component changes state). One new high, N-1: the D-VL-6
limit contradicted the known-limit paragraph. Fixed in the text: D-VL-6 now states the
stale-line limit, and the line's guarantee is "the account last checked, under the login last
seen". No third iteration (max 2).

Open questions for the user: (1) also show the line on `/peers` (one per followed peer)? (2) a
dedicated `/identities` page mirroring `identity list`? (3) wording of the legend and the
unreadable notice (proposed above).
