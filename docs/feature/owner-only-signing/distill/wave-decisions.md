# Wave Decisions — DISTILL — owner-only-signing

- **Designer**: Quinn (nw-acceptance-designer). **Date**: 2026-10-20. **Mode**: subagent; no
  question asked; every open point is recorded below.
- `[lang-mode] rust` · `[policy-mode] inherit` (one section appended to
  `docs/architecture/atdd-infrastructure-policy.md`) · `[port-mode] inherit`
  (`tests/common/state_delta.rs`).
- **WARN — DISCUSS artifacts missing**: stories and acceptance criteria are derived from DESIGN
  (A-1..A-14, C-OOS-1..9, KPI-OOS-1..4, slices 1-5); `test-scenarios.md` §3 lists them.
- **WARN — DEVOPS artifacts missing**: the project's existing CI acceptance stage runs these
  targets; default environment (clean; stale/offline/proof-gone states are focused scenarios).

## Prior-wave reading and reconciliation

Read: DESIGN (`architecture-design.md`, `wave-decisions.md`, `upstream-changes.md`), ADR-086,
ADR-087, ADR-088, ADR-089, `crates/claim-domain/src/authority.rs`, the brief (§ For Acceptance
Designer, I-9 row), KPI-OOS-1..4, four journeys, the RRA evolution doc's subject (checklist in
`test-scenarios.md` §1).

**Reconciliation passed — 0 blocking contradictions.** With no DISCUSS or DEVOPS wave there is no
cross-wave decision to contradict. Tensions between this DESIGN (2026-10-14) and what shipped
since are settled upstream and recorded as Upstream Issues U-OOS-1..3 below; the tests follow the
shipped decisions.

## Key Decisions

| ID | Decision |
|---|---|
| DWD-OOS-1 | **Placement.** Four suites in `crates/cli/Cargo.toml` (`owner_only_signing`, `_repos`, `_readers`, `_core`), one in `crates/openlore-review-app/Cargo.toml` (`owner_only_signing_review_app`, app-only suites live there, RRA DWD-RRA-1). Vocabulary `tests/acceptance/support/authority.rs`, registered in `support/mod.rs`. |
| DWD-OOS-2 | **Hand-off state (house rule D-d).** Every scenario is `#[ignore = "DISTILL owner-only-signing <ID>: pending DELIVER …"]`, the walking skeleton included; the default run is green. DELIVER enables WS-OOS-1 first. Exception: PS-3 (KPI-OOS-3 dispute/withdraw guard) holds today and must hold after DELIVER, so it runs as an ACTIVE regression guard (GDL IR-13/IR-17 precedent). PS-7 also holds today but is vacuous until the gate exists, so it stays ignored. |
| DWD-OOS-3 | **Wording pins** (DISTILL-owned; DELIVER may change a pin with a one-line edit in `support/authority.rs`): labels `owner` / `repo-listed` / `third-party` / `authority not checked`, index label `authority not checked here`; refusal fragments `Refusal::phrase()` (e.g. "you have no identity link to", "is owned by the organization", "is not a valid authority file", "authority was last checked", "authority has not been checked", "GitHub could not be asked", "GitHub serves this repo as"); hand-off lead "Can be signed by" + the owner login + `openlore identity link github <login>` and/or `.github/openlore-authority`; repo hand-off command `openlore scrape github <owner/repo> --sign`; re-check command `openlore authority verify --subject <s>`; "You can sign"; footer "N claims not counted: a third-party, b authority not checked"; section "Unattested claims, by author"; "No imported candidates"; "No GitHub claims to re-check"; review app "not counted: authority not proven"; dispute annotation "counters <cid> by <did>" (shipped `render/federated.rs`); budget refusal "retry after" (review F-13). Assertions are substring-based. |
| DWD-OOS-4 | **Exit codes.** A refused signature exits non-zero (`claim add`, `claim publish`, and `scrape … --sign` / `infer people --sign` when the selected candidate is refused — candidates are still listed). Mirrors the journeys' "--sign N out of range → exit non-zero". |
| DWD-OOS-5 | **Someone else's account reads `NoLink`.** ADR-087 §1 has no "not your account" reason: a person claim about a login none of the signer's links names is `Unattested(NoLink)`; a link whose latest check sees another login is `LoginNotCurrent`. WS-OOS-1 row 2, PS-2 row 3, PS-5, RA-2 use `NoLink`. |
| DWD-OOS-6 | **Renamed repo**: signing under the old name is refused naming the new name (RA-8), the shipped `SigningRefusal::RepoRenamed` behaviour of the shared gate (ADR-089 §2) applied to the CLI. |
| DWD-OOS-7 | **Fail closed on subjects**: a `github:` subject whose scheme differs only in letter case, or whose rest is neither a login nor `owner/repo`, is never Exempt (CORE-OOS-2). |
| DWD-OOS-8 | **`authority verify --subject <s>` records an observation even with no local claim about `<s>`** (DESIGN flow "re-check with `openlore authority verify --subject github:me/tool`" before a first offline signature). Used by RA-7/8/10/15, RD-1/7. |
| DWD-OOS-9 | **No Tier B.** `classify` / `may_sign` are pure projections of (claim, latest checks, now): FORALL properties over generated states (CORE-OOS-3/4) cover the space without a command protocol; the observation-order journey is RA-9 (layer 3). |
| DWD-OOS-10 | **Layer-2 seam** (identity_link_core / RRA DWD-RRA-7 precedent): `owner_only_signing_core.rs` holds view types, ADR oracles and `todo!()` `sut_*` bindings — RED, not BROKEN, with zero production scaffolding. The CORE-OOS-5 oracle side calls the REAL `scoring::score`. DELIVER binds one property per step and may move it into `crates/claim-domain/tests`. |
| DWD-OOS-11 | **Test double extended additively** (`crates/test-support`, test-only): `AuthorityFile`, `set_authority_file`, `redirect_repo_to_another_host`, `exhaust_budget_at_authority_reads`, `authority_file_reads_of` + one unit test (passes). No shipped suite reads the new path. |
| DWD-OOS-12 | **Mandate 8 at layer 3.** Read-only and refused paths assert the fail-closed universe `capture_authority_universe` (house store slots + `identity_link_checks` + `repo_authority_checks` + `imported_candidates` count) unchanged; signing paths assert the bounded delta (claims +1, claim files). Import verbs legitimately record observations, so their refusal rows assert `assert_nothing_signed` (claims, claim files, PDS) and pin the recorded observation separately where it matters (RA-1, RA-4, RA-6). Layer-4 rows (viewer, review app) use traditional assertions. |
| DWD-OOS-13 | **Review-app share (SA-1)**: "Attested" in the app (no CLI check store, C-OOS-4) is read as the owner tier holding NOW — the share request re-reads each quoted repo's owner and bio, as publish does (ADR-089 §4). Proposal pending U-OOS-4. |
| DWD-OOS-14 | **Window**: inclusive, `0 ≤ age ≤ 7 days` of the OLDER check used (person: link check; owner repo: max(link, repo); listed repo: repo); a future-dated check is missing. Pinned at minute granularity (PS-4, RA-10, CORE-OOS-4x). |
| DWD-OOS-15 | **Wire-shaped Givens.** Peer claims arrive through the real `peer add/pull` path (`PeerPds`); index rows through the real ingest + `openlore-indexer serve`; the only pre-feature claim comes from the checked-in frozen pre-v7 store (GDL fixture, reused — no new fixture). No scenario inserts into `repo_authority_checks` directly. |
| DWD-OOS-17 | **One walking skeleton** (review F-09): the feature is one rule (who may sign) applied at many composers; slice 1 is its thinnest end-to-end proof. RA-1 (own repo, import → sign → publish) is the second headline journey and is tagged `@driving_adapter`, not `@walking_skeleton`, so DELIVER keeps one "enable first" scenario. |
| DWD-OOS-18 | **One followed peer per store** (house harness: `peer pull` resolves every subscription with only the latest peer's seam). Third-party, repo-listed and not-checked claims all come from Rachel; RD-4/RD-5 put the attested claim on Jeff's side. |
| DWD-OOS-16 | **Mandate-12, Rust adaptation.** CM-I-1: typed domain module = `support/authority.rs` (`Subject`, `RepoRef`, `Authority`, `Refusal`, `CanSign`, `AuthorityFile`, instants, repo constants) + GDL's `Account`. CM-I-2: steps take `Subject` / `RepoRef` / `Account` / `Refusal` / `CanSign`, never raw logins or phrases. CM-I-3: scenario bodies call vocabulary steps; the only control flow is table iteration and a `match` on the row's expected outcome. CM-I-4: 5.84× (below). |

### Walking-skeleton strategy

Architecture of Reference defaults via the project policy (RRA "Strategy B" equivalent): REAL
`openlore` binary, `adapter-github` over HTTP, `adapter-duckdb`, filesystem (and the REAL viewer,
indexer and review app where a scenario drives them); FAKE (driven-external only)
`FakeGithubAccounts`, `FakeGithub` (contributors), `FakePds`, `PeerPds`, `FakeIdentity`, the
ATProto network and ingest doubles. One skeleton, WS-OOS-1 (`walking-skeleton.md`), clean
environment; stale-check / offline / proof-gone / GitHub-failing environments are focused
scenarios (PS-2, PS-4, PS-8, RA-5, RA-6, RA-10).

## Test Coverage Summary

| Slice | Example scenario fns (rows) | Layer-2 | Status |
|---|---|---|---|
| 1 — person gate (WS-OOS-1, PS-2..PS-8) | 8 (17) | CORE-OOS-1, 2, 4, 4x | new |
| 2 — repo authority (RA-1..RA-15, 11a/11b) | 16 (37) | CORE-OOS-3 | new |
| 3 — readers (RD-1..RD-10) | 10 (16) | CORE-OOS-5 | new |
| 4 — review app | 1 (1) — SA-1 share draft only | — | publish-time check + link prompt already shipped (U-OOS-1) |
| 5 — imported candidates (IC-1..IC-4) | 4 (4) | — | new |
| **Total** | **39 (76 rows)** | **6** | **45 test fns** |

- Walking skeleton: 1 (WS-OOS-1). Non-happy: 27 / 39 example scenario fns (69 %; rule in `test-scenarios.md` §2); strict refusal-row share ≈ 43 %. `@security`: 18. Active guard: PS-3.
- Tables (parametrize density): WS ×2, PS-2 ×3, PS-3 ×2, PS-4 ×4, PS-5 ×2, PS-8 ×2, RA-2 ×3, RA-3 ×11, RA-5 ×3, RA-8 ×2, RA-10 ×4, RA-11b ×3, RA-13 ×2, RD-4 ×6, RD-7 ×2. Proptest (256 cases) on every unbounded pure rule (CORE-OOS-1..5).
- `@kpi`: KPI-OOS-1 (WS-OOS-1, PS-2, PS-5, PS-6, RA-1, RA-2, RA-11a/b, CORE-OOS-4), KPI-OOS-2 (RD-2, RD-3, RD-4, RD-6, IC-2, CORE-OOS-5), KPI-OOS-3 (PS-3), KPI-OOS-4 (RA-1 owner, RA-3/RA-4 repo-listed; the "second reader" half is a dogfood log, not acceptance).
- **Step reuse (CM-I-4, informational)**: 304 vocabulary invocations / 52 distinct functions = **5.84×**.

### AT-completeness (15 items): **14/15 — COMPLETE**

| Item | | Evidence |
|---|---|---|
| C1a empty/zero | ✓ | PS-2 (no link), RA-10 (never observed), RA-14 (no GitHub claims), IC-3 (nothing imported) |
| C1b boundaries | ✓ | window 0 / 7 d / 7 d + 1 min / −1 min (PS-4, RA-10, CORE-OOS-4x); authority file exactly 4 KiB vs over (RA-3) |
| C2a state machine documented | ✓ | module docs: person tier (`owner_only_signing.rs`), repo observation + permit (`owner_only_signing_repos.rs`) |
| C2b illegal event per state | ✓ | sign with no link / proof gone / stale / future check / old login / old repo name / repo not found; publish a pre-feature third-party claim; re-import (IC-4) |
| C3 0/1/many | ✓ | links 0/1/2 DIDs on one account (PS-2, WS, PS-7); listed DIDs 0..3 (CORE-OOS-3); imports 0/1/2 (IC-3, IC-1); added weightless rows 1..5 (CORE-OOS-5) |
| C4a apply twice | ✓ | RA-11b re-publish idempotent, RA-12 open twice, RA-9 repeated observation, IC-4 re-import |
| C4b inverse without prerequisite | ✓ | RA-14 re-check with nothing to re-check; PS-3 withdraw after authority is gone |
| C5a mode combinations | ✗ | `--sign` × subject kind × verb, online × offline, fresh × stale, `--weighted` × `--include-unattested` covered; `--include-unattested` WITHOUT `--weighted` is unspecified (U-OOS-7, SPECIFICATION_AMBIGUITY → DESIGN) |
| C5b flag orthogonality | ✓ | RD-3 (`--include-unattested` never changes the weight), PS-6 (no `--sign` writes nothing) |
| C6a malformed input | ✓ | malformed / case-variant `github:` subjects (CORE-OOS-2), 7 malformed authority-file shapes + DID lies (RA-3, CORE-OOS-3), spoofed `counters` (RD-4, CORE-OOS-5) |
| C6b each declared error | ✓ | NoLink, LinkNotVerified, LoginNotCurrent, NotRepoOwner, OrgOwnedNotListed, FileInvalid, RepoNotFound, Renamed, CheckTooOld, NotChecked, GithubUnfinished at layer 3; NotListed at layer 2 |
| C6c closed error set | ✓ | CORE-OOS-3 (reason ∈ the ADR-087 set), CORE-OOS-4 (permit/refuse total) |
| C7a degraded resource | ✓ | network off (PS-4, RA-10, IC-1), rate limit / unreachable / cross-host (RA-5), budget exhausted (RA-6) |
| C7b interruption | ✓ | RA-6 (budget runs out between harvest and authority read: candidates listed, nothing recorded) |
| C7c concurrency | ✓ (N/A) | DESIGN states no concurrency contract for signing or checks (architecture-design.md § Data model: insert-only, latest = max `checked_at` then max `check_id` — a deterministic tie-break, not a concurrency claim). Strictly: 13/14 applicable items pass, C5a open (review F-21) |

Completeness audit log: `(owner-only-signing, C5, 1, LOW)`; all other categories 0 findings. No
`at-completeness-gap-log.md` (verdict ≥ 13).

### Shipped acceptance tests the DELIVER behaviour change will break (listed, not changed)

ADR-086 Consequences, confirmed and extended by grep on 2026-10-20. DELIVER migrates each to a
seeded-authority Given (e.g. `the_repo_holds_authority_file` + `given_jeff_verified_authority_of`
from `support/authority.rs`), a non-`github:` subject, or a changed expectation:

1. **Signing gate — shared seeding helpers** that sign own `github:` claims through `claim add`:
   `support/mod.rs` `seed_own_plus_peer_graph`, `seed_own_claims_via_cli`,
   `seed_own_claim_with_evidence`, `seed_claim_with_counter`,
   `seed_claim_two_counters_distinct_authors`, `seed_one_signed_local_claim`,
   `seed_own_claim_for_search`, `seed_local_claim_about`; `support/people.rs` `given_i_signed`,
   `given_i_signed_repo_claim`. Every caller breaks: `walking_skeleton.rs` (WS-1..WS-10, incl.
   WS-10 — see U-OOS-2), `appview_search.rs`, `claim_compose_advisory.rs`, `counter_claim.rs`,
   `cross_instance_pull.rs`, `federated_query.rs`, `federation_roundtrip.rs`, `identity_link.rs`,
   `infer_people.rs`, `infer_people_sign.rs`, `infer_people_evidence_grows.rs`,
   `philosophy_alias_triangulation.rs`, `public_card.rs`, `publish_init.rs`,
   `publish_pull_reconcile.rs`, `publish_roundtrip.rs`, `scrape_auth.rs`, `scrape_person.rs`,
   `scrape_sign.rs`, `viewer_cli_concurrent.rs`, `viewer_counter_aware_counts(_invariants).rs`,
   `viewer_counter_claim_list_flags.rs`, `viewer_counter_claim_threads(_invariants).rs`,
   `viewer_htmx(_invariants).rs`, `viewer_invariants.rs`,
   `viewer_landing_dashboard_invariants.rs`, `viewer_network_search_invariants.rs`,
   `viewer_persistent_left_nav(_invariants).rs`, `viewer_philosophies(_invariants).rs`,
   `viewer_search_full_follow_state(_invariants).rs`, `viewer_store.rs`; ADR-086 also lists
   `scrape_github.rs`, `scrape_person_command.rs`, `graph_query_explore.rs`, `indexer_ingest.rs`.
2. **`score_attested`** (peer `github:` claims without a local check stop weighing): weighted /
   score expectations in `graph_query_explore.rs`, `viewer_contributor_scoring(_invariants).rs`,
   `viewer_counter_aware_counts.rs`, `viewer_peer_counter_aware_counts.rs`,
   `viewer_counter_flags_score_surface(_invariants).rs`, `viewer_counter_flags_graph_surfaces.rs`,
   `viewer_graph_traversal(_invariants).rs`, `viewer_landing_dashboard.rs`,
   `identity_link_readers.rs` IR-14/IR-15; `identity_link_core.rs` CORE-6 re-points `sut_score`
   to `score_attested` (one line, as its doc says).
3. **Search label widening (UC-7)**: `identity_link_readers.rs` IR-16b asserts a non-link
   `github:` row is NOT labelled "authority not checked here" — RD-7 supersedes it; its assertion
   flips in DELIVER.
4. **Review-app publish suites**: already carry `FakeGithubAccounts` owner fixtures since ADR-089
   (RRA); only the share-draft change (SA-1) can move `review_app_consent_and_share.rs` SH-1/SH-3
   if a quoted repo is not owner-held at share time (it is, in those fixtures).

## Upstream Issues

| ID | Issue | routes_to |
|---|---|---|
| U-OOS-1 | **Reconciliation — slice 4 largely delivered.** Since this DESIGN, review-app-repos-and-accounts shipped ADR-088/089: `claim_domain::authority` exists with the OWNER tier only (`may_sign(signer, subject, listed_ids, RepoRead, OwnerBioRead) → SigningPermit` with private fields, `SigningRefusal { NotOwnedByListedAccount, OrgOwned, RepoGone, RepoRenamed, BioNoLongerHolds, Unfinished }`), `GithubPort::read_repo` (one `GET /repos`, ≤ 1 same-host redirect), and a `PublishPlan` that needs a permit minted from fresh in-request reads. OOS-D13 option A was superseded by the user's option B (import any public repo into unsigned private suggestions, "Not yours to publish"). The identity-link prompt is GDL slice 3 ("Publish this link", `review_app_identity_link.rs`). **Covered, not re-specified**: publish-time owner check (`review_app_repos_and_accounts_several.rs` AP-1/2/4, `review_app_publish_permit_alone.rs`), link prompt. **Remaining and specified**: share draft quotes only attested claims (UC-21, SA-1). **CLI side unchanged in scope**: person gate, `sign_permitted` + `sign_only_with_permit`, repo-listed tier via `.github/openlore-authority` + `read_repo_authority` (= `read_repo` + the file read), v8, `authority verify`, publish gate, scrape `--sign` gating, offline I-9 exception, counter/retract exemption, `score_attested` + labels, slice 5. | record (DELIVER) |
| U-OOS-2 | **ADR-086 §5 / brief I-9 row: "WS-10 keeps its scope because its subject is not a `github:` assertion" is wrong.** `tests/acceptance/walking_skeleton.rs` WS-10 (and WS-1..WS-9) compose `github:rust-lang/rust embodiesPhilosophy memory-safety` — a `github:` repo assertion. Under the gate WS-10 fails at signing, not at the PDS. DELIVER must move the slice-01 walking skeleton's subject off `github:` (keeping KPI-5 / I-9 meaning) or seed a fresh repo-listed check; the offline-with-fresh-check AT the DESIGN asked for is PS-4 / RA-10. The I-9 brief row should name the replacement test. | DESIGN (text) + DELIVER |
| U-OOS-3 | **`may_sign` name and shape collide.** The shipped review-app gate is `may_sign(signer, subject, listed, RepoRead, OwnerBioRead)` (same-request reads); DESIGN's CLI gate is `may_sign(draft, signer_did, evidence, now, window)` (local checks). DELIVER must extend the module without breaking ADR-089 (e.g. a second entry `may_sign_from_checks`, or both behind one evidence ADT) and keep `SigningPermit` minted only there. `SigningPermit.basis` needs a `RepoListed` arm. The CORE-OOS-4 binding is named for neither. | DELIVER (DESIGN note) |
| U-OOS-4 | **"Attested" in the review app's share draft is undefined** (the app keeps no CLI check store). SA-1 proposes: owner tier holds now, read fresh in the share request, as publish does (DWD-OOS-13). Cost: up to 2 GitHub reads per quoted repo per share preview. | DESIGN (ruling) |
| U-OOS-5 | `kind` precedence when one claim carries both `retracts` and `counters` (or a `sameAs` predicate plus a marker) is unspecified; harmless for gate and weight (all non-weight), so CORE-OOS-1 asserts the exact kind only for single-marker claims. | DESIGN (text) |
| U-OOS-6 | Slice 5 `imported_candidates` is insert-only; what `candidates list` shows after re-importing the same repo is unspecified. IC-4 proposes "each candidate once". | DESIGN (ruling) |
| U-OOS-7 | **C5a, SPECIFICATION_AMBIGUITY (LOW)**: `graph query --include-unattested` without `--weighted` is not defined (error? the label-only listing?). Not covered. | DESIGN |
| U-OOS-8 | **Tracked DELIVER blocker (review F-07)**: the REQUIRED recorded-fixture contract test of `/repos` + contents shapes (ADR-087 Earned Trust fixtures `repo_owner_user.json`, `repo_owner_org.json`, `repo_renamed_redirect.json`, `authority_file.json`, `authority_file_symlink.json`) and the `repo_authority_checks` `probe()` are adapter tests owned by DELIVER, not acceptance. DELIVER adds them in `crates/adapter-github` with `read_repo_authority`; untracked when slice 2 starts = blocker. | DELIVER |
| U-OOS-9 | `FakeGithubAccounts::harvested_repos` / `scrape_reads_of` count any `/repos/{o}/{r}/…` read as a harvest. If DELIVER adds the authority-file read to the review app's publish path (ADR-089 §1 says `read_repo_authority` = `read_repo` + file), RRA oracles that compare harvested repos would see it; exclude `contents/.github/openlore-authority` there when that happens. | DELIVER |
| U-OOS-10 | Opt-in telemetry counter `claim.sign.refused.authority` (and the review app's `claim.publish.refused.authority`) has no acceptance scenario: observability, not KPI-gated; DELIVER unit-tests it. | DELIVER |
| U-OOS-12 | **Two driving-port details uncovered at layer 3** (review F-24): `authority verify --mine` and a sign-time refusal of a `supersedes`/`corrects` claim on someone else's `github:` subject (layer 2 only, CORE-OOS-2). DELIVER adds one row each to RA-4 / PS-2 when the verb lands. | DELIVER |
| U-OOS-13 | **Contested accounts — the second DID's half** (review F-03): each store signs only as Jeff (`TestEnv` identity), so "Rachel, also verified for jeffbailey, signs about it" cannot be driven at layer 3; CORE-OOS-3 generates both DIDs verified for one id and requires Owner for each. PS-7's Given re-checks every link in Jeff's store (`identity verify` without a filter), Rachel's included. | record |
| U-OOS-14 | **Environments** (review F-19/F-20): `with-pre-commit` is N/A for a CLI-local feature; `with-stale-config` maps to the stale-check / proof-gone / pre-feature-store states in PS-2, PS-4, RA-10, RA-11a, RA-12, RD-8. Elevator pitches were authored in DISTILL (no DISCUSS): DISCUSS→DISTILL verbatim continuity is unverifiable until a DISCUSS exists (WARN). | record |
| U-OOS-11 | No DISCUSS wave: no story IDs or elevator pitches upstream; DISTILL's S-OOS-1..9 (`test-scenarios.md` §3) stand in. | DISCUSS (if run later) |

## Review Gate Result

Cycle 1 needs_revision (2 blockers, 5 highs) → fixed; cycle 2 **APPROVED** (0 blockers, 0 highs). Details: `acceptance-review.md`.
