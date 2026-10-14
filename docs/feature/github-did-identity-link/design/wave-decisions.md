# Wave Decisions: github-did-identity-link (DESIGN)

- **Mode**: propose (autonomous subagent; the user was not asked). Every decision below lists 2-3
  options and a recommendation. The recommendation is the working default so the documents are
  complete. Items marked **PROPOSED — awaiting user** need the user's confirmation.
- **Architect**: Morgan (nw-solution-architect). **Date**: 2026-10-14.
- **Scope**: application. **Paradigm**: functional Rust (ADR-007). **Style**: unchanged — hexagonal
  modular monolith (ADR-009), three disjoint composition roots (ADR-023/072).
- **No DISCUSS wave.** Requirements are derived from OD-CPI-1, the shipped bio-DID proof (ADR-076)
  and the claim model (ADR-005/008/064/071). They are recorded as assumptions A-1..A-9 for a later
  DISCUSS pass. Prior-wave changes: `upstream-changes.md`.
- **ADRs**: ADR-084 (wire contract), ADR-085 (verification reuse and reader semantics). Both
  Proposed. C4 and component detail: `architecture-design.md`.

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

## Assumptions (to confirm in a later DISCUSS pass)

| ID | Assumption | Basis | If wrong |
|---|---|---|---|
| A-1 | The job: "let readers know my GitHub contributions and my OpenLore claims are the same person, without anyone else being able to say so." | OD-CPI-1 | Re-scope |
| A-2 | Linking is **opt-in, by the DID owner only**, and never inferred (no link from a bio scan, a contribution link, or a rename flag). | OD-CPI-1 "opt-in, human-signed"; CPI R-2 surveillance risk | Proposed hard constraint — **user must confirm explicitly** |
| A-3 | The CLI user (P-001) is the first user; review-app users second. | CPI personas; the review app already holds verified links | Swap slice order |
| A-4 | One DID may link several GitHub accounts; one GitHub account should be linked by at most one DID (more = Contested). | People have work/personal accounts; impersonation risk | Change projection only |
| A-5 | Keeping the DID in the GitHub bio is acceptable to users; removing it makes readers show "proof gone". | ADR-076 already requires it in the review app | Consider gist proof (GDL-D4 option C) |
| A-6 | Display-only association is enough for v1; nobody needs a merged person score yet. | CPI out-of-scope "changes to scoring"; ADR-022 | ADR-085 deferred union flag |
| A-7 | GitHub's `https://api.github.com/user/<id>` is a stable identifier URI for the account. **User must confirm explicitly.** | Long-standing public endpoint (not in GitHub's "versioned" docs) | Accept `github-id:<n>` in evidence — a change to the ADR-084 §3 reader rule, not a free-string swap |
| A-10 | GitHub `GET /user/{id}` returns the current profile (same shape as `/users/{login}`) for a renamed account and 404 for a deleted one; after a rename the old login on `/users/{login}` either redirects to the new login or 404s, unless someone re-registers it (then a different id). **Partly unverified** — the check algorithm handles redirect, 404 and different id. | Observed GitHub behavior; not externally verified in this wave | Pin with recorded fixtures (`user_by_id.json`) and the scheduled live smoke; if wrong, renames show as `account_not_found` until re-linked |
| A-8 | A link is an identity fact: confidence fixed at 10000 bp, not editable. | ADR-070 | Make editable (no other change) |
| A-9 | Published links are acceptable as public, permanent records (retraction withdraws, does not erase). | ATProto semantics; ADR-008 | Gate behind an extra confirmation |

## Key Decisions

Status legend: **PROPOSED — awaiting user** = recommendation is the working default.

### GDL-D1 — Claim shape: existing record vs new lexicon — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. `org.openlore.claim` with predicate `sameAs`** (recommended) | Zero Lexicon/CID/validator change; review app needs no new OAuth scope (`repo:org.openlore.claim?action=create` already granted, SPIKE-3); every reader, retraction and both provenance modes work unchanged. Cost: shape is a free-string convention (like ADR-064). |
| B. New record `org.openlore.identityLink` | Typed fields; but new OAuth scope + re-consent for every review-app user, a second record path in peer pull/indexer/viewer/peer_claims, and old peers ignore it. |
| C. DID document `alsoKnownAs` (PLC op) | ATProto-native, but needs a PLC rotation-key operation users on bsky.social cannot do; OpenLore holds no PLC key. |

Rationale: the simplest option that keeps consent and federation on paths already verified.
Source: ADR-005, ADR-064 §Alternatives, ADR-071, ADR-073, bluesky-claim-review-app SPIKE-3.

### GDL-D2 — Field layout and predicate name — PROPOSED — awaiting user
Recommended: `subject` = author's bare DID; `predicate` = `sameAs`; `object` = `github:<login>`;
`evidence` = [`https://github.com/<login>`, `https://api.github.com/user/<id>`]; confidence 10000;
`supersedes` only for a re-link after rename (ADR-084 §2).
| Option | Trade-off |
|---|---|
| **A. subject = DID, object = `github:<login>`, `sameAs`** (recommended) | Consent rule is a single equality (subject == author). Matches OD-CPI-1 wording; schema.org `sameAs` is exactly "profile of the same entity". |
| B. subject = `github:<login>`, object = DID | Fits ADR-064 subject convention so `graph query --subject github:x` lists it; but every reader sees a claim *about* a GitHub account by another party, and scoring pairs it under the person. |
| C. Predicate `controlsAccount` instead of `sameAs` | Directional and literally what is proven; departs from OD-CPI-1 vocabulary. A one-constant switch. |

### GDL-D3 — Who may assert a link — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. Only the DID owner (author == subject); third-party `sameAs` claims are ordinary claims, never projected as links** (recommended) | Consent by construction; no doxxing vector; nothing new to moderate. |
| B. Allow third-party assertions as a weaker "asserted by X" link state | Lets others connect identities without consent — contradicts A-2 and CPI R-2. |
| C. Require a counter-signature from both sides on the wire | GitHub accounts cannot sign; collapses into D4. |

### GDL-D4 — Proof carried and how the GitHub side is proven — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. DID side = record provenance (app-signed or self-attested); GitHub side = live bio check (ADR-076 matcher) + numeric id pinned in evidence; composing refuses without a passing check** (recommended) | Reuses the shipped proof; any peer can re-check; renames detectable. Proof is only as current as the reader's last check. |
| B. Embed a bio snapshot / verification timestamp in the record | Looks self-contained but is just the author's statement; forgeable; stale forever. |
| C. Keybase-style signed gist on GitHub | Stronger, survives bio edits; adds a GitHub write the user must do, a second fetch shape and a new rate cost. Revisit if A-5 fails. |

### GDL-D5 — Retraction and invalidation — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. Retract = shipped ADR-008 `retracts` by the same author (CLI `claim retract`, review-app `RetractPlan`); a vanished bio proof is shown as `Disproved`, never auto-retracted** (recommended) | Append-only invariant kept; the owner stays in control; readers are honest about state. |
| B. Auto-retract when a check fails | System signs on the user's behalf (violates CPI D-5); a 60 s stale profile could retract a valid link. |
| C. Links expire after N days without re-proof | Silent decay; needs a clock contract on the wire (CID-relevant). |

### GDL-D6 — Where linking is initiated — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. Both: CLI first (`openlore identity link github <login>`, `identity verify`, `identity list`), review app second ("Publish this link" on the verified GitHub step)** (recommended) | One pure core, two thin shells; review-app users already have a verified link. |
| B. CLI only | Smallest; review-app users (no CLI, no app key) cannot link. |
| C. Review app only | Reuses its UI, but CLI users would need OAuth sign-in to link their own DID. |

### GDL-D7 — How verification is reused without adapter→adapter deps — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. Hoist the pure matcher into `claim-domain::identity_link`; `review-domain` delegates; each root wires its own `adapter-github`** (recommended) | No new crate; every reader already depends on `claim-domain`; shipped review-app proptests guard the refactor. |
| B. New pure crate `identity-link-domain` | Clean boundary; a sixth pure-core `check-arch` arm for ~200 lines. Revisit if other identity providers arrive. |
| C. CLI depends on `review-domain` | Drags maud views and the review context into the CLI. |

### GDL-D8 — How readers treat a linked pair — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. Display-only association everywhere; `scoring::score` itself drops every `sameAs` claim (one choke point for the CLI and viewer callers; unbounded preservation: `score(F) == score(F \ sameAs rows)`); no subject merging** (recommended) | Keeps ADR-014/022 anti-merging; smallest change; honest. |
| B. Opt-in read-time union (`--include-linked`) with per-subject breakdown | Useful later; needs its own decomposition design. Deferred. |
| C. Canonical merged subject | Silent merging; contested links get resolved by guessing. Rejected. |

### GDL-D9 — Renames and duplicates — PROPOSED — awaiting user
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

### GDL-D14 — When a link counts as an association (squatting, conflicts) — PROPOSED — awaiting user
The wire rule proves only the DID side, so any DID owner can publish a `sameAs` naming any GitHub
id.
| Option | Trade-off |
|---|---|
| **A. Associate only when this reader's latest check is `Verified`; unchecked/disproved links render as assertions ("did:… claims github:x — not checked / proof gone"), never as associations, and never make a verified link Contested; `Contested` only when ≥ 2 DIDs are `Verified` for one id (the bio names both)** (recommended) | A squatter can neither hijack nor grief; costs one `identity verify` before a peer's link shows as associated. |
| B. Associate any Active link; label `NotChecked` | Zero extra requests, but the squatter's DID is associated with the victim by default. |
| C. Contested on any two Active links | Simple, but any stranger can block a genuine link. |

### GDL-D10 — Local proof-check store — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. DuckDB v7 `identity_link_checks`, insert-only (PK includes `checked_at`; latest = read query); split ports `IdentityLinkCheckReadPort` / `IdentityLinkCheckWritePort` / `IdentityLinkCheckPurgePort`; narrowest handle per verb; `identity forget --did` is the only delete; never federated** (recommended) | Offline verbs show status; `scrape github <user>` refreshes at zero extra GitHub requests; read-only verbs hold no write capability; local erasure possible. One migration, three small traits. |
| B. No persistence; check on every display | Network in offline verbs; spends rate budget per render. |
| C. Columns on `contribution_links` | Wrong aggregate (repo↔person observations). |

### GDL-D11 — Federation: how peers verify an ingested link — PROPOSED — awaiting user
| Option | Trade-off |
|---|---|
| **A. Links federate as ordinary claims; DID side verified by the unchanged provenance paths; GitHub side re-checked by each reader (`identity verify`); index/search label "GitHub side not checked here"** (recommended) | No new capability in the indexer; trust is never transitive. |
| B. Indexer re-verifies bios | Gives the indexer root a GitHub capability and a shared rate budget across all DIDs. |
| C. Trust the signature as full proof | A DID owner could link an account they do not control. |

### GDL-D12 — Privacy and consent — PROPOSED — awaiting user
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
  predicate `sameAs` (one pure guard in the shared compose path). Foreign-client `sameAs` records
  render as unverified assertions only.
- C-GDL-3: Links are keyed by numeric GitHub id; logins are display-only.
- C-GDL-4: Association is display-only and requires this reader's own `Verified` check; the scorer
  drops every `sameAs` claim; no subject merging anywhere.
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

## Proposed KPIs (not registered; DISCUSS/DEVOPS to confirm)

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
| N-6 ADR-064 pointer, numbering | Deferred: amend ADR-064 with a pointer when ADR-084 is accepted; numbering left stable to keep references valid |

No third review iteration (max 2).
