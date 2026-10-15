# Wave Decisions: owner-only-signing (DESIGN)

- **Mode**: propose (autonomous subagent). Every decision lists 2-3 options and a recommendation.
  **The user approved every decision and assumption as proposed on 2026-10-14** (incl. A-11 =
  persist imported candidates, the I-9 exception with the 7-day window, and the repo-file
  definition of admin); each is marked ACCEPTED (user, 2026-10-14).
- **Architect**: Morgan (nw-solution-architect). **Date**: 2026-10-14.
- **Scope**: application. **Paradigm**: functional Rust (ADR-007). **Style**: unchanged — hexagonal
  modular monolith (ADR-009), three disjoint composition roots (ADR-023/072).
- **Builds on** github-did-identity-link (GDL, ADR-084/085, Accepted). The GDL changes this design
  needs (`upstream-changes.md` UC-1..UC-8) were applied to the GDL design and ADR-085 on 2026-10-14.
- **No DISCUSS wave.** Requirements derive from the user's statement; recorded as assumptions.
- **ADRs**: ADR-086 (authority model + enforcement), ADR-087 (authority proofs). Both Accepted
  (2026-10-14). C4 and components: `architecture-design.md`.

## The requirement and its interpretation

User (verbatim): "it should be possible for someone to import claims into the database without
being the owner of an account but only the owner of the account can sign those claims. If possible,
also make it so only an owner or admin of a repo can sign claims about the repo but let anyone
import a repo and generate unsigned claims."

Interpretation used (accepted, A-1):

- **Import** = scrape/infer: anyone gets unsigned candidates about any public `github:` subject.
- **Sign** = any OpenLore act that produces a record attributed to your DID for publication: CLI
  sign (keychain), review-app self-attested publish, and `claim publish` of a not-yet-published
  local claim.
- **Owner of an account** = the DID proven (ADR-084 link + live bio check) to control the GitHub
  account `github:<login>`.
- **Owner or admin of a repo** = the DID linked to the GitHub user that owns a personal repo, **or**
  a DID listed by the repo's maintainers in `.github/openlore-authority` (any repo, the only public
  route for org repos). This is broader than GitHub's "admin" role (see OOS-D5).
- **Not in scope**: subjects outside the `github:` scheme; counter-claims and retractions (OOS-D11).

**Fact correction for the user**: today the CLI does **not** store unsigned repo candidates.
`scrape github owner/repo` recomputes and prints them each run; only contribution links (person ↔
repo) are persisted. "Import into the database" literally needs OOS-D12 option B.

## Prior-wave reading

| | Artifact | Used for |
|---|---|---|
| ✓ | `docs/product/architecture/brief.md` | Inventory, roots, I-1..I-12 (I-9 offline sign), GDL entry, SSOT discipline |
| ✓ | `docs/adrs/` (latest ADR-085) | ADR-008, 015, 017, 022, 023, 063, 064, 071, 076, 084, 085; new ADRs start at 086 |
| ✓ | `docs/feature/github-did-identity-link/design/*`, ADR-084/085 | Link wire rule, `verified` check, association/contested rule, check store v7, C-GDL-1..10, per-verb capability table |
| ✓ | `docs/feature/contributor-philosophy-inference/feature-delta.md` | D-1 (signed inferred claims), D-2 (signed inputs only), D-5 append-only, OD-CPI-1, R-1/R-2 |
| ✓ | `docs/feature/bluesky-claim-review-app/design/wave-decisions.md` + code | DD-9 repo-level only; scan = `list_owned_repos` (`type=owner`) of the verified login; publish path `routes/publish.rs`, `PublishPlan` |
| ✓ | `docs/feature/openlore-github-scraper/design/wave-decisions.md` | WD-49 human gate, WD-51 public-data-only, I-SCR-6 single publish path |
| ✓ | `docs/product/kpi-contracts.yaml` | KPI pattern; nothing registered here (DISCUSS/DEVOPS own registration) |
| ✓ | `lexicons/org/openlore/claim.json` | Free-string subject, `references` types, optional signature — no field for "kind" |
| ✓ | Code: `crates/cli/src/verbs/{claim_add,claim_counter,claim_retract,claim_publish,sign_batch,scrape_github,scrape_person,infer_people}.rs` | Seven composers + publish; counter/retract copy the original subject |
| ✓ | Code: `crates/ports/src/lib.rs` `GithubPort`, `crates/adapter-github/src/client.rs` | Methods today; `/repos` body already read in harvest; optional PAT |
| ✓ | Code: `crates/review-domain/src/ownership.rs`, `crates/openlore-review-app/src/{scan.rs,routes/publish.rs}` | Bio matcher; scan of owned repos; publish has no repo re-check |
| ✓ | Code: `crates/appview-domain/src/lib.rs`, `crates/claim-domain/src/provenance.rs`, `crates/adapter-duckdb/src/schema*.rs` | Indexer row shape, provenance verdict, migrations up to v6 (GDL proposes v7) |
| ⊘ | DISCUSS / DISCOVER artifacts for this feature | Do not exist |
| ⊘ | `lexicons/org/openlore/philosophy.json` | Philosophy records are not `github:` subjects |

## Assumptions (accepted by the user 2026-10-14)

| ID | Assumption | If wrong |
|---|---|---|
| A-1 | Interpretation above (import vs sign; `github:` scope). Confirmed by the user 2026-10-14. | Re-scope |
| A-2 | Person authority = ADR-084 identity link + this reader's `verified` check; no one else may sign about a person, including the authors of claims today. Confirmed by the user 2026-10-14. | OOS-D4 |
| A-3 | For repos, "admin" may be approximated by "listed in `.github/openlore-authority` by someone with push access". **Residual risk R-1**: any collaborator with push access can list their own DID with no identity proof; CODEOWNERS blocks this only if branch protection requires code-owner review, which readers cannot see. Reader label is `repo-listed`, not "admin". Confirmed by the user 2026-10-14. | OOS-D5 option B (OAuth) |
| A-4 | Third-party claims stay visible as labelled opinions; they are not deleted or hidden. | OOS-D2/D9 |
| A-5 | Anyone may still counter any claim. | OOS-D11 |
| A-6 | Signing freshness window = 7 days of the latest local authority check. | One constant |
| A-7 | Existing dogfood claims in PDSes and the live index that are third-party reading as `third-party` is acceptable. Confirmed by the user 2026-10-14. | OOS-D3 |
| A-8 | Contributor-philosophy inference becomes candidates-only for anyone but the person themself. Confirmed by the user 2026-10-14. | CPI D-1 stays; requirement not met for people |
| A-9 | GitHub's collaborator-permission endpoint needs push access to the repo (documented for the collaborators endpoints; unverified for `/permission`). | Re-open OOS-D5 option C as signing-time only |
| A-10 | `GET /repos/{o}/{r}` for a renamed/transferred repo answers a 301 to `/repositories/{id}`; the adapter follows one same-host redirect. Partly unverified. | Renames read `repo_not_found` until re-scraped under the new name |
| A-11 | "Import into the database" means persisted (OOS-D12 B). | Drop slice 5 |
| A-12 | The design does not rely on whether collaborators can hold admin on personal repos (unverified). The Owner tier recognizes only the owner account; any other admin needs a file listing. | None |
| A-14 | Person authority follows the current login: after a GitHub rename, claims about the old login read `third-party` and the person re-signs under the new login (known gap G-2). | Pin the user id in claim evidence (ADR-087 Alternatives) |
| A-13 | Counter-claims add weight to the disputed pairing today (the scoring feed has no reference column). Inferred from code, to confirm with a fixture (UC-20). | If already excluded elsewhere, ADR-086 §2's weight rule is a no-op for counters |

## Key Decisions

### OOS-D1 — Scope and exemptions — ACCEPTED (user, 2026-10-14)
Recommended: authority applies to `github:<login>` and `github:<owner>/<repo>` assertions only.
Non-weight kinds — `retracts` claims, `counters` claims, ADR-084 identity links — need no authority
**and never count in scoring** (any subject, any author), so a spoofed `counters` reference cannot
buy weight (review C-1). `supersedes`/`corrects` need authority (they are new assertions).
| Option | Trade-off |
|---|---|
| **A. `github:` subjects; exempt retract/counter/link** (recommended) | Matches the requirement; keeps dispute and withdrawal open (counter/retract copy the subject, so without exemption they would be blocked). |
| B. Also gate counters | Owners become indisputable; contradicts ADR-015. |
| C. All subjects | No authority source exists for non-GitHub subjects. |

### OOS-D2 — What happens to "any DID asserts anything" — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Keep the claim model; derive `Attested(Owner\|RepoListed) / Unattested / NotChecked` per claim in readers; OpenLore composers refuse to sign non-attested `github:` claims** (recommended) | No wire change; honest labels; peers decide; works for already-published data. Composer refusal only binds OpenLore tools (ATProto cannot stop other clients). |
| B. Replace: readers hide every non-attested claim | Closest literal reading; erases the opinion layer and published data from view. |
| C. New wire field `kind: attestation\|opinion` | Self-declared, still needs a proof; Lexicon + CID + OAuth consent change. |
Sources: ADR-008, ADR-015, ADR-022, ADR-071; ATProto repo model.

### OOS-D3 — Already-published third-party claims (PDSes + live index) — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Same rule for old and new (grandfathered by label); no cutoff, no purge** (recommended) | Nothing lost or rewritten; a pre-feature claim by a real owner reads `Attested`. Dogfood third-party claims read `third-party`. |
| B. Purge/hide in the index | Needs indexer GitHub capability or a rule it cannot evaluate; destroys data. |
| C. Date cutoff on `composedAt` | Forgeable (author-asserted). |

### OOS-D4 — Person authority proof — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Author's ADR-084 link to the account + this reader's `verified` check; subject login ∈ logins observed for that id** (recommended) | Consent (the DID owner published the link), numeric id pin, rename-safe, re-checkable by any reader. Requires GDL slice 1. |
| B. Bio check alone (no link record) | No GDL dependency, but associates DID and account from a bio scan without published consent (contradicts GDL A-2). |
| C. GitHub OAuth by the signer | Not re-checkable by readers; new OAuth app and token storage. |

### OOS-D5 — Repo authority proof — ACCEPTED (user, 2026-10-14)
Concrete GitHub evaluation in ADR-087 §Context.
| Option | Trade-off |
|---|---|
| **A. Two public tiers: `Owner` (`GET /repos` `owner.type=User` ∧ `owner.id` = author's verified linked id) and `RepoListed` (`.github/openlore-authority` on the default branch lists the DID; label `repo-listed`)** (recommended) | Both re-checkable by any reader anonymously; WD-51 holds; covers personal and org repos; doubles as delegation for collaborators. "Admin" = "listed by someone with push access" — residual risk R-1 (A-3). Every observation reads both `/repos` and the file, so it does not depend on who checked (review H-2). |
| B. Signer's OAuth token, `permissions.admin` | Exact admin, org repos included; signing-time only (readers trust the signer), new GitHub OAuth app + CLI device flow, leaves WD-51. |
| C. `GET /repos/{o}/{r}/collaborators/{u}/permission` | Exact role; caller needs push access (A-9) — arbitrary readers cannot re-check. |
Rejected outright: org membership role (not public), topics (cannot hold `:`), description/README.
File location sub-choice: **`.github/openlore-authority`** (recommended; `.github/` is GitHub's
repo-meta folder, CODEOWNERS can guard it) vs root `OPENLORE` (more visible, clutters root) vs
`.well-known/openlore` (no meaning on GitHub).

### OOS-D6 — Authority over time — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Current authority, re-checked; former owners' claims read `Unattested (no longer owner)`; nothing auto-retracted** (recommended) | Honest about transfers and removed admins; append-only kept. |
| B. Authority at signing time | Unprovable later (GitHub has no public history of ownership/permissions). |
| C. Auto-retract on loss | System signs on the user's behalf (CPI D-5, C-GDL-6). |

### OOS-D7 — Signing gate vs offline signing (I-9) — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Gate from the latest local authority check, `Attested` and ≤ 7 days old; online verbs refresh in-invocation; offline verbs refuse with the verify command** (recommended) | Sign itself stays network-free, but a `github:` assertion without a fresh check is refused offline: an **accepted exception to I-9**, recorded in ADR-086 §5 and the brief's I-9 row (review H-10). Staleness bounded to 7 days for the signer (readers re-check anyway). |
| B. Live check at every sign | Freshest; breaks I-9 for `github:` claims; `infer people` turns online. |
| C. No gate; reader-side only | Not what the user asked. |

### OOS-D8 — Where the gate runs — recommended (follows from D2/D7)
Sign sites (every inline `IdentityPort::sign` today): `claim_add.rs`, `claim_counter.rs` (exempt
permit), `claim_retract.rs` (exempt permit), `sign_batch.rs` (`scrape github --sign`, `scrape person
--sign`, `infer people --sign`) — all moved behind a NEW `sign_permitted` helper;
`philosophy_add.rs` (philosophy records, not claims) is out of scope and allowlisted by name.
Publish sites: `publish_signed_claim` takes a permit (covers `claim add`, the batch and `claim
publish <cid>`; for `claim publish`, "never published" = `claims.published_at IS NULL`); review-app
`execute_publish` via a permitted `PublishPlan` (extends `owner_can_publish`). The review-app share
draft quotes only `Attested` claims. One pure gate (`may_sign` → `SigningPermit`, evidence with
private fields built only from adapter-decoded observations). No override flag.

### OOS-D9 — Reader treatment and scoring — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Label everywhere; `score_attested` weights only assertions that are non-`github:` or Attested; non-weight kinds (counter, retract, sameAs) never weighted; names what it skipped; `--include-unattested` adds unattested assertions as a separate per-author section** (recommended) | The rule means something to peers; transparent; one pure choke point; also stops counters adding weight (A-13). Weighted views shrink until `authority verify` runs. |
| B. Label only; count everything | No surprise; a foreign client bypasses the rule for every reader. |
| C. Hide unattested claims | Loses the opinion layer and disputes. |
Consequence (accepted): ADR-022's cross-author triangulation becomes mostly owner + listed admins.

### OOS-D10 — Indexer and network search — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Indexer unchanged; CLI renders index rows "authority not checked here"; local check on demand** (recommended) | Keeps C-GDL-8 and ADR-023; no DTO change. |
| B. Indexer verifies authority | GitHub capability + shared rate budget in the indexer root. |
| C. Indexer classifies by shape only (author has an indexed `sameAs` to the subject login/owner) | Cheap hint, but squattable without the GitHub side; later optional slice. |

### OOS-D11 — Counter-claims and retractions — ACCEPTED (user, 2026-10-14)
Recommended: anyone may counter any claim (owner's included), shown-not-applied as today; only the
author may retract (ADR-008), never refused for lack of authority. Alternative: owner-only counters
(rejected: indisputable owners); counters counted in scoring (rejected: ADR-015 shown-not-applied).

### OOS-D12 — Unsigned import and hand-off — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| A. Keep today: candidates printed per run, contribution links stored; refusal message names who can sign and the exact command | Zero storage change; "import into the database" not literal. |
| **B. A + persist imported candidates locally: migration v8 `imported_candidates` (insert-only, never federated, never fed to inference or scoring); `openlore candidates list [--subject]` offline** (recommended, separate last slice) | Literal reading of the requirement; offline review of what you imported. One table, one read verb. |
| C. Publish unsigned "proposal" records the owner can adopt | Network hand-off, but re-creates third-party claims under a new name and needs a new record type. |
Hand-off (all options): the refusal and the candidate list print "can be signed by: `<owner login>`
(after `openlore identity link github <login>`) or a DID listed in `.github/openlore-authority`",
and the command the owner runs (`openlore scrape github <owner/repo> --sign N`). Candidates are
deterministic from public data, so the owner re-derives them; no data transfer is needed.

### OOS-D13 — Review app — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. Publish-time authority check in the request (fresh bio read + `read_repo_authority`); keep scanning owned repos only; prompt to publish the identity link so readers see claims as `owner`** (recommended) | Closes the transfer-after-scan gap at +1 request per publish request + 2 per repo (3 for one repo); no new OAuth scope. |
| B. A + "import any public repo" into private suggestions, marked "not yours to publish" | Literal "anyone can import" in the app; more UI, more GitHub budget. Later slice if wanted. |
| C. No change (scan-time ownership only) | Repo transferred after the scan still publishes. |

### OOS-D14 — Where the logic lives — ACCEPTED (user, 2026-10-14)
| Option | Trade-off |
|---|---|
| **A. New pure module `claim-domain::authority` beside GDL's `identity_link`; `GithubPort::read_repo_authority`; v8 `repo_authority_checks` + split read/write ports in `adapter-duckdb`** (recommended) | No new crate; all readers already depend on `claim-domain`. |
| B. New crate `authority-domain` | Clean boundary; one more pure-core arm for a module that needs `identity_link`. |
| C. In `scraper-domain` | `scoring`/`viewer-domain` would depend on the scraper context. |

### OOS-D15 — Enforcement — recommended (no user decision)
TYPE: `SigningPermit` and `Attested` with private fields (built only by `classify`/`may_sign`);
`sign_permitted`, `publish_signed_claim` and `PublishPlan` execution take a permit; split
read/write check ports; `classify` requires provenance-verified claims. STRUCTURAL
(`xtask check-arch`): `sign_only_with_permit` (`IdentityPort::sign` only in `sign_permitted` and
allowlisted `philosophy_add.rs`), `authority_evidence_only_from_adapters`,
`repo_authority_checks_insert_only`,
`authority_readers_hold_no_write_handle`, `scoring_callers_use_score_attested`, existing indexer
boundary (no `adapter-github` in `openlore-indexer`). BEHAVIORAL: AT fixtures (brief "For
Acceptance Designer"), incl. spoofed-kind and observation-order cases. Properties target the
derivations (`kind`, `requirement`, `classify` against the ADR-087 rule table), not only the
`score_attested` definition (review H-9).

## Architecture Summary

- **Pure core** `claim-domain::authority` (NEW module): `requirement(claim) → Exempt | Required(
  Person | Repo)`, `classify(claim, author, link_view, repo_observations) → Authority`,
  `may_sign(draft, signer, evidence, now, window) → SigningPermit | SigningRefusal`, hand-off text
  data. Consumes GDL's `identity_link` projection and matcher.
- **Scoring**: `score_attested(feed, view)` is the only entry for the CLI and viewer; the feed
  gains reference types; non-weight kinds are never weighted.
- **Ports**: EXTEND `GithubPort::read_repo_authority`; NEW `RepoAuthorityCheckReadPort` /
  `RepoAuthorityCheckWritePort`; (slice 5) `ImportedCandidatePort` read/write.
- **Adapters**: `adapter-github` (+ contents of one fixed path, redirect follow); `adapter-duckdb`
  v8 (`repo_authority_checks`, slice 5 `imported_candidates`).
- **CLI**: NEW `openlore authority verify`; gate in every composer + `claim publish`; labels in
  `graph query`, `infer people`, `scrape` views, viewer; `--include-unattested`; (slice 5)
  `openlore candidates list`.
- **Review app**: publish-time authority check; identity-link prompt.
- **Indexer**: unchanged. **Lexicon**: unchanged. **New crates**: 0.

## Technology Stack

No new crate in `Cargo.lock`. Reused: `reqwest` (MIT/Apache-2.0), `duckdb` (MIT), `proptest`
(MIT/Apache-2.0). Decoding the contents API's base64 body: `base64` (MIT/Apache-2.0) is already in
`Cargo.lock` transitively; `adapter-github` would take it as a direct dependency (one `Cargo.toml`
line, `cargo deny` unchanged). Alternative: the `application/vnd.github.raw+json` media type
returns raw bytes, but then a second metadata read is needed to refuse symlinks/submodules.

## Constraints Established

- C-OOS-1: No OpenLore composer signs or publishes a `github:` assertion without a
  `SigningPermit` from an `Attested` check ≤ 7 days old (fresh in-invocation for online verbs).
- C-OOS-2: Retraction and counter-claims are never refused for lack of authority.
- C-OOS-3: Authority proofs use only public GitHub data; any reader can re-check them.
- C-OOS-4: Authority is reader-local, current, never federated; checks are insert-only.
- C-OOS-5: Nothing is deleted, hidden on the wire, retracted or re-signed on anyone's behalf.
- C-OOS-6: Scoring weights only assertions that are non-`github:` or Attested; counters,
  retractions and identity links are never weighted; unattested assertions are shown separately and
  per author, never merged.
- C-OOS-7: Unsigned import stays open to everyone; imported candidates are never federated and never
  feed inference or scoring.
- C-OOS-8: The indexer gains no GitHub capability.
- C-OOS-9: Signing never calls the network. Accepted I-9 exception: a `github:` assertion without
  a fresh (≤ 7 days) local authority check is refused offline (ADR-086 §5; brief I-9 row).

## Slices (indicative; DISTILL/DELIVER own the roadmap)

No slice ships without GDL slice 1: from slice 1 on, every `github:` signing path is gated, so person and
personal-repo signing needs a published link. The only GDL-independent path is `RepoListed` (file
listing), which arrives in slice 2.

0. Prerequisite: GDL slice 1 (identity link + v7 checks).
1. Walking skeleton: `claim-domain::authority` + person gate in `claim add` / `infer people --sign`
   (own login signs; other login refused with hand-off; counter/retract exempt).
2. Repo authority: `read_repo_authority`, v8 `repo_authority_checks`, `authority verify`, gate in
   `scrape github/person --sign`, `claim add`, `claim publish`.
3. Readers: labels (CLI, viewer, search), `score_attested` + `--include-unattested`.
4. Review app: publish-time check, identity-link prompt.
5. (If A-11 holds) `imported_candidates` + `candidates list`.

## KPIs (registered in `docs/product/kpi-contracts.yaml` 2026-10-14)

- KPI-OOS-1 (guardrail): 0 `github:` assertions signed/published by any OpenLore composer
  without a permit — CI property + ATs over all composers.
- KPI-OOS-2 (guardrail): the weight never changes when non-weight kinds (incl. spoofed counters)
  or unattested `github:` assertions are added to a feed — CI property over `kind`/`classify`; CLI
  and viewer ATs. Consistent with KPI-GDL-2 (`sameAs` never weighted).
- KPI-OOS-3 (guardrail): 0 retractions/counters refused for authority — AT.
- KPI-OOS-4 (leading): ≥ 1 repo claim `repo-listed` via an authority file in an org repo (signed
  from the CLI; the review app cannot reach org repos in v1) and ≥ 1 `owner` claim, each confirmed
  by a second reader.

## Upstream Changes

See `upstream-changes.md`: GDL display-only stance extended (UC-1), contested ids grant authority
(UC-2), GDL capability table rows (UC-3), GDL slices become prerequisites (UC-4), CPI D-1 narrowed
(UC-9), ADR-022 default narrowed (UC-11), scraper/review-app gating (UC-12/13).

## External Integrations (handoff to platform-architect)

GitHub REST: `GET /repos/{o}/{r}` (consumed today), `GET /repos/{o}/{r}/contents/.github/openlore-authority`
(new), plus GDL's `/users/{u}` and `/user/{id}`. Contract tests recommended for GitHub REST — a
**required** recorded-fixture contract test in CI (fixtures listed in ADR-087 Earned Trust) and the
optional scheduled live smoke. No consumer-driven Pact is possible against GitHub.

## Review

Iteration 1 (nw-solution-architect-reviewer): REJECTED_PENDING_REVISIONS — 1 critical, 8 high
(reviewer counted H-6 as medium), 8 medium, 2 low. Revisions:

| Issue | Fix |
|---|---|
| C-1 exemption from claim content + exempt rows counted in weight (counters already weighted today) | Non-weight kinds (counter, retract, sameAs) need no authority **and are never weighted**; feed gains reference types; spoof ATs/properties; UC-16/17 corrected, UC-20 + A-13 added |
| H-1 `sameAs` in formula vs GDL drop | One formula: weight-eligible = Assertion ∧ (non-`github:` ∨ Attested); KPI-OOS-2 aligned with KPI-GDL-2 |
| H-2 observation depended on who checked | File always read; `not_read` state removed; observation-order AT |
| H-3 "only admin of a personal repo" | Corrected; A-12 added; Owner tier = owner account only |
| H-4/H-6 "admin-listed" overstated | Label `repo-listed`; CODEOWNERS not enforcement; residual risk R-1 in A-3 |
| H-5 budget ignored harvest baseline | Recomputed (≈ 10 GETs/repo, ≈ 6 repos/hour anonymous); exhaustion behaviour; `GITHUB_TOKEN` recommended |
| H-7 incomplete sign/publish inventory | All five sign sites named; new `sign_permitted`; permit on `publish_signed_claim`; `philosophy_add` scoped out; share draft covered |
| H-8 forgeable evidence | `Attested` private; `authority_evidence_only_from_adapters` rule |
| H-9 definitional properties | Properties over `kind`/`requirement`/`classify` against the rule table |
| H-10 I-9 silently amended | Explicit proposed exception (ADR-086 §5, brief I-9 row, WS-10 noted) |
| M-1 redirect policy | New same-host ≤ 1 redirect policy listed (ADR-087 §4) |
| M-2 existing tests | Affected AT files listed (ADR-086 Consequences) |
| M-3 share flow | Share draft quotes Attested only |
| M-4 name reuse | Recorded as known gap G-1 with the correct rejection reason |
| M-5 GDL link-check write on scrape | Row retained in the capability table |
| M-6 sequencing | DISTILL gate and GDL dependency explicit in Slices |
| M-7 "never published" | `claims.published_at IS NULL` |
| M-8 contract shapes | Bounded-change universes declared for scrape and publish rows |
| L-1 review-app gate | Extends `owner_can_publish`; org repos CLI-only in v1 |
| L-2 observability | Rate budget via `AuthReport`; opt-in refusal counter |

Iteration 2: CONDITIONALLY APPROVED — all iteration-1 issues resolved (H-3 partial, M-4 recorded
as gap G-1); 0 critical; 2 new high, 3 medium, 1 low, all fixed in text as the reviewer's
condition for DISTILL:

| Issue | Fix |
|---|---|
| N-1 (high) person authority kept on a reused old login | Subject login must equal the **latest** check's `login_seen`; old-login claims read `Unattested(LoginNotCurrent)`; gap G-2 + A-14 |
| N-2 (high) scrape `--sign` promised a refresh it could not write | `--sign` rows hold link-check write for the signer's own link; ADR-087 §6 states it |
| N-3 file state fabricated for `repo_not_found` | `authority_file` NULL iff `repo_not_found` (table CHECK) |
| N-4 review-app cost figures disagreed | One figure: +1 per publish request + 2 per repo |
| N-5 unverified GitHub fact in rationale | Removed; A-12 says the design does not rely on it |
| N-6 UC order, dangling pointer | Reordered; this section |

No third review iteration (max 2).
