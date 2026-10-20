# Wave Decisions — DISTILL — owner-only-signing

- **Designer**: Quinn (nw-acceptance-designer). **Date**: 2026-10-20. **Mode**: subagent; no
  question asked; every open point is recorded below.
- `[lang-mode] rust` · `[policy-mode] inherit` (one section appended to
  `docs/architecture/atdd-infrastructure-policy.md`) · `[port-mode] inherit`
  (`tests/common/state_delta.rs`).
- **DISCUSS** (revision 1): `discuss/user-stories.md` now exists (commit 680f56b, written in
  parallel); its S-OOS-1..9 IDs match the ones DISTILL derived, and every scenario carries
  `@s-oos-N` (`test-scenarios.md` §3).
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
| DWD-OOS-2 | **Hand-off state (house rule D-d) and scaffold policy (revised in iteration 1, review H-3).** Every scenario is `#[ignore = "DISTILL owner-only-signing <ID>: pending DELIVER …"]`, the walking skeleton included; the default run is green; DELIVER enables WS-OOS-1 first. **Rule**: a scenario that passes today AND must keep passing after DELIVER runs ACTIVE, even when it is only vacuous until the gate lands — it then catches the first over-refusing or over-wiring commit. Active: PS-3 (KPI-OOS-3), PS-7 (contested account still signs; verified green), IC-5 (imported candidates never feed inference). RD-5 was re-checked and stays ignored: its Given needs `authority verify`. |
| DWD-OOS-3 | **Wording pins** (DISTILL-owned; DELIVER may change a pin with a one-line edit in `support/authority.rs`): labels `owner` / `repo-listed` / `third-party` / `authority not checked`, index label `authority not checked here`; refusal fragments `Refusal::phrase()` (e.g. "you have no identity link to", "is owned by the organization", "is not a valid authority file", "authority was last checked", "authority has not been checked", "GitHub could not be asked", "GitHub serves this repo as"); hand-off lead "Can be signed by" + the owner login + `openlore identity link github <login>` and/or `.github/openlore-authority`; repo hand-off command `openlore scrape github <owner/repo> --sign`; re-check command `openlore authority verify --subject <s>`; "You can sign"; footer "N claims not counted: a third-party, b authority not checked"; section "Unattested claims, by author"; "No imported candidates"; "No GitHub claims to re-check"; review app "not counted: authority not proven"; dispute annotation "counters <cid> by <did>" (shipped `render/federated.rs`); budget refusal "retry after" (review F-13). Assertions are substring-based. |
| DWD-OOS-4 | **Exit codes.** A refused signature exits non-zero (`claim add`, `claim publish`, and `scrape … --sign` / `infer people --sign` when the selected candidate is refused — candidates are still listed). Mirrors the journeys' "--sign N out of range → exit non-zero". |
| DWD-OOS-5 | **Someone else's account reads `NoLink`.** ADR-087 §1 has no "not your account" reason: a person claim about a login none of the signer's links names is `Unattested(NoLink)`; a link whose latest check sees another login is `LoginNotCurrent`. WS-OOS-1 row 2, PS-2 row 3, PS-5, RA-2 use `NoLink`. |
| DWD-OOS-6 | **Renamed repo**: signing under the old name is refused naming the new name (RA-8), the shipped `SigningRefusal::RepoRenamed` behaviour of the shared gate (ADR-089 §2) applied to the CLI. |
| DWD-OOS-7 | **Fail closed on subjects**: a `github:` subject whose scheme differs only in letter case, or whose rest is neither a login nor `owner/repo`, is never Exempt (CORE-OOS-2). |
| DWD-OOS-8 | **`authority verify --subject <s>` records an observation even with no local claim about `<s>`** (DESIGN flow "re-check with `openlore authority verify --subject github:me/tool`" before a first offline signature). Used by RA-7/8/10/15, RD-1/7. |
| DWD-OOS-9 | **No Tier B.** `classify` / `may_sign` are pure projections of (claim, latest checks, now): FORALL properties over generated states (CORE-OOS-3/4) cover the space without a command protocol; the observation-order journey is RA-9 (layer 3). |
| DWD-OOS-10 | **Layer-2 seam** (identity_link_core / RRA DWD-RRA-7 precedent): `owner_only_signing_core.rs` holds view types, ADR oracles and `todo!()` `sut_*` bindings — RED, not BROKEN, with zero production scaffolding. The CORE-OOS-5 oracle side calls the REAL `scoring::score`. DELIVER binds one property per step and may move it into `crates/claim-domain/tests`. |
| DWD-OOS-11 | **Test double extended additively** (`crates/test-support`, test-only): `AuthorityFile`, `set_authority_file`, `redirect_repo_to_another_host`, `exhaust_budget_at_authority_reads`, `authority_file_reads_of` + one unit test (passes). No shipped suite reads the new path. |
| DWD-OOS-12 | **Mandate 8 at layer 3 (superseded in part by D-e/R1 below: the store universe is SECONDARY only).** Read-only and refused paths assert the fail-closed universe `capture_authority_universe` (house store slots + `identity_link_checks` + `repo_authority_checks` + `imported_candidates` count) unchanged; signing paths assert the bounded delta (claims +1, claim files). Import verbs legitimately record observations, so their refusal rows assert `assert_nothing_signed` (claims, claim files, PDS) and pin the recorded observation separately where it matters (RA-1, RA-4, RA-6). Layer-4 rows (viewer, review app) use traditional assertions. |
| DWD-OOS-13 | **Review-app share (SA-1)**: "Attested" in the app (no CLI check store, C-OOS-4) is the owner tier holding NOW — the share request re-reads the bio and each quoted repo's owner, as publish does (ADR-089 §4). **Ruling P-OOS-2 accepted 2026-10-20** (U-OOS-4 resolved); SA-1 is an ordinary DELIVER-pending scenario. |
| DWD-OOS-14 | **Window**: inclusive, `0 ≤ age ≤ 7 days` of the OLDER check used (person: link check; owner repo: max(link, repo); listed repo: repo); a future-dated check is missing. Pinned at minute granularity (PS-4, RA-10, CORE-OOS-4x). |
| DWD-OOS-15 | **Wire-shaped Givens.** Peer claims arrive through the real `peer add/pull` path (`PeerPds`); index rows through the real ingest + `openlore-indexer serve`; the only pre-feature claim comes from the checked-in frozen pre-v7 store (GDL fixture, reused — no new fixture). No scenario inserts into `repo_authority_checks` directly. |
| DWD-OOS-17 | **One walking skeleton** (review F-09): the feature is one rule (who may sign) applied at many composers; slice 1 is its thinnest end-to-end proof. RA-1 (own repo, import → sign → publish) is the second headline journey and is tagged `@driving_adapter`, not `@walking_skeleton`, so DELIVER keeps one "enable first" scenario. |
| DWD-OOS-18 | **One followed peer per store** (house harness: `peer pull` resolves every subscription with only the latest peer's seam). Third-party, repo-listed and not-checked claims all come from Rachel; RD-4/RD-5 put the attested claim on Jeff's side. |
| DWD-OOS-16 | **Mandate-12, Rust adaptation.** CM-I-1: typed domain module = `support/authority.rs` (`Subject`, `RepoRef`, `Authority`, `Refusal`, `CanSign`, `AuthorityFile`, instants, repo constants) + GDL's `Account`. CM-I-2: steps take `Subject` / `RepoRef` / `Account` / `Refusal` / `CanSign`, never raw logins or phrases. CM-I-3: scenario bodies call vocabulary steps; the only control flow is table iteration and a `match` on the row's expected outcome. CM-I-4: 5.84× (below). |


## Review iteration 1 (2026-10-20) — orchestrator rulings and DISTILL decisions

Four reviewers rejected revision 0. The orchestrator ruled R1-R11; the coordinator then relayed the
user's acceptance of P-OOS-1..3 (design `wave-decisions.md` "Rulings P-OOS-1..3 — ACCEPTED",
9288ea0) and the ADR-087 §6 budget correction (a600ad3). Each is recorded here; the finding →
fix table is in `acceptance-review.md` § Review iteration 1.

| ID | Decision | Pinned by |
|---|---|---|
| D-e / R1 | **Observation order (GDL D-e, adopted exactly).** PRIMARY Thens observe through driving ports: exit status; CLI output (`authority verify`, `candidates list`, `graph query`, `identity list`, refusal text); PDS wire JSON; viewer and review-app pages; the GitHub double's request log. `WhatJeffSees` (`graph query --subject … --federated`, `identity list`, PDS bodies) is the port view behind `assert_jeff_sees_no_change`, `assert_nothing_signed_or_published`, `assert_signed_and_kept_locally`. The store universe is renamed `guardrail_*` and labelled SECONDARY everywhere it remains; slots renamed to port-meaningful names (`authority.repo_observations`, `candidates.imported.count`). RA-12's primary evidence is CLI behaviour (both pre-feature claims listed; a second open prints the identical output); schema versions are a secondary guard. | every scenario; `support/authority.rs` |
| R2 | **GitHub request-log assertions stay.** Requests to GitHub are an external side effect with a rate budget, observed at the driven-port boundary (GDL D-VL-T13). Only user-meaningful counts are asserted ("asked GitHub nothing", "once for each repo and once for its authority file"); the exact redirect policy belongs to the adapter-github recorded-fixture test (U-OOS-8). | RA-1, RA-4, RA-14, PS-4, RA-10, IC-1 |
| DWD-OOS-19 / R3 | **Sign-time GitHub outage**: an online verb (`scrape person/github … --sign`) tries to refresh in the same invocation; if GitHub cannot answer (rate limited, unreachable) the refresh records nothing and the gate falls back to the latest local check exactly as offline — permit iff Attested and ≤ 7 days, else refuse naming `openlore authority verify --subject <s>`. **Upstream note for DESIGN**: recorded in ADR-086 §5 / ADR-087 §6 by a600ad3. Double: `rate_limit_profile_after` / `drop_profile_after`. | PS-9 (person path, PS-4's Given), RA-13 rows 3-5 (repo path) |
| R3′ (a600ad3) | **Budget exhaustion at `--sign` falls back too**: RA-6 is now a two-row table — never re-checked → refused with "retry after" AND the re-check command; re-checked on 1 October → signed on 3 October. RA-16 (`authority verify`, not a signing verb) is unaffected: it records nothing and says GitHub could not be asked. | RA-6, RA-16 |
| DWD-OOS-20 / R4 | **Token budget**: RA-16 table — anonymous budget exhausted at the authority read; token configured and exhausted; token rejected (401). Each: "GitHub could not be asked", nothing recorded (port view unchanged + guardrail), token value absent from stdout/stderr (`scrape_auth.rs` no-leak pattern via `assert_token_value_absent`; the double's `reject_token`). | RA-16 |
| DWD-OOS-21 / R5 | **Frozen v7 store** generated from current main (head v7) with the pre-v7 regen procedure: EXPORT DATABASE dump + home + golden `identity list` at 3 Oct (`tests/fixtures/owner-only-signing-v7-store/`). One linked account, link checks dated 20 Sep (older than 7 days) and 1 Oct (latest). The `identity_link_check_ids` sequence is exported at its current value (START 9); the regenerator self-checks that a new check after rebuild does not collide. **No `.duckdb` file is committed**, so the `*.duckdb` ignore rule needs no b780d68-style exception. Loader `LinkWorld::with_frozen_store_from` (generalises the pre-v7 loader). | RA-12b, `regenerate_the_frozen_v7_store_fixture` |
| R6 | RA-11a gains a network-off row (Forge 5); RA-10 gains listed-repo 7 d / 7 d + 1 min rows (Forge 8). **Forge 4 (downgrade to an older binary) — out of scope**: existing project policy (stores are forward-only; no downgrade path is supported for any migration). **Forge 6 (acceptance build time) — DELIVER action**: measure the acceptance build after slice 1; if over budget, consolidate `owner_only_signing_core.rs` into a `claim-domain` test target. | RA-10, RA-11a |
| R7 | **Spec-ambiguity gates** — superseded by the accepted rulings: P-OOS-1 (U-OOS-7) → RD-11 (`--include-unattested` without `--weighted` exits 2, "--include-unattested requires --weighted", no results); P-OOS-2 (U-OOS-4) → SA-1 aligned; P-OOS-3 (U-OOS-6) → IC-4 aligned (each candidate once, as of the latest import). SA-1 and IC-4 drop `@spec-ambiguity` and are ordinary `#[ignore]` DELIVER-pending scenarios. Nothing else depended on U-OOS-7 (RD-3 uses both flags). | RD-11, SA-1, IC-4 |
| R8 | **Review-app publish is already covered** by shipped review-app-repos-and-accounts tests (`no_request_publishes_a_suggestion_about_a_repo_the_owner_doesnt_own`, AP-1/2/4, `review_app_publish_permit_alone.rs`); the identity-link prompt is GDL slice 3 (`review_app_identity_link.rs`). Cited in `test-scenarios.md` §3/§4 for KPI-OOS-1 and S-OOS-8; not duplicated. | traceability |
| R9 | IC-5 (imported candidates never feed `infer people`, `@c-oos-7`, ACTIVE). Enforcement mapping: C-OOS-8 ← `xtask check-arch` `INDEXER_FORBIDDEN_DEPS` (DELIVER adds `adapter-github`) + RD-7; C-OOS-4 ← `repo_authority_checks_insert_only` / `authority_readers_hold_no_write_handle` (DESIGN) + shipped `identity_link_checks_insert_only` + RA-4/RA-16 at the port. `@s-oos-N` on every scenario doc comment (IDs unchanged). | IC-5, `test-scenarios.md` §4 |
| R10 | PS-3 asserts the published wire body (counter → Rachel's CID, about `github:rachelk`; retraction → Jeff's CID, about `github:jeffbailey`; author Jeff); count is secondary. PS-7 un-ignored (green). RD-5 stays ignored (GIVEN-blocked). RA-3/RA-5 Examples reworded in user terms; refusal fragments live only in the wording pins (RA-3 doc maps "not listed" → `OrgOwnedNotListed`, "not a usable authority file" → `FileInvalid`). Contract shapes: CLI read scenarios → `unbounded-preservation`, mutating → `bounded-change`; layer-2 properties → `pure-function`. CORE-OOS-4 oracle restated as the decision table `GateRow` G1-G8 from ADR-086 §5 / ADR-087 §7, anchored by CORE-OOS-4x; layer-2 CM-A exception documented in the core module doc. L-1: `authority verify` is ONE command for person and repo subjects because one gate needs both check kinds (ADR-087 §6, DESIGN "challenged" row); `identity verify` stays link-only. L-2: a peer publishing twice into one store is verified working (`a_link_its_peer_author_retracted_no_longer_shows_or_is_rechecked`, green), so RD-4 keeps its "withdrawn" row. | PS-3, PS-7, RA-3, RA-5, core |
| R11 | Completeness summary and counts recomputed (below). | — |

### Mandate-8 waiver (explicit)

Mandate 8 asks every state-mutating test at layers 1-3 to assert through a port-exposed
universe. These scenarios run at layers 3-4 and observe through driving ports first (D-e / R1).
The secondary guardrail (`guardrail_store_unchanged`, `guardrail_store_delta`,
`guardrail_nothing_signed`) reads `claims` / `peer_claims` row counts, claim files, PDS record
count, `contribution_links`, the DESIGN-pinned `identity_link_checks` columns, the v8
`repo_authority_checks` rows (timestamps and ids excluded) and the `imported_candidates` count.
Reading those tables directly is a deliberate waiver following house precedent
(`store_universe_slots` / `capture_store_universe` in `support/people.rs`, GDL's
`capture_link_universe`). It is fail-closed (any unexpected change fails) and never the only
evidence for a Then. Known exception: RA-12's RED today comes from the v8 guardrail, because its
primary Thens (pre-feature claims listed, second open identical) already hold — they are the
regression half of the upgrade contract.

### Walking-skeleton strategy

Architecture of Reference defaults via the project policy (RRA "Strategy B" equivalent): REAL
`openlore` binary, `adapter-github` over HTTP, `adapter-duckdb`, filesystem (and the REAL viewer,
indexer and review app where a scenario drives them); FAKE (driven-external only)
`FakeGithubAccounts`, `FakeGithub` (contributors), `FakePds`, `PeerPds`, `FakeIdentity`, the
ATProto network and ingest doubles. One skeleton, WS-OOS-1 (`walking-skeleton.md`), clean
environment; stale-check / offline / proof-gone / GitHub-failing environments are focused
scenarios (PS-2, PS-4, PS-8, RA-5, RA-6, RA-10).

## Test Coverage Summary (recomputed in iteration 1, R11)

| Slice | Example scenario fns (rows) | Layer-2 | Status |
|---|---|---|---|
| 1 — person gate (WS-OOS-1, PS-2..PS-9) | 9 (20) | CORE-OOS-1, 2, 4, 4x | new; PS-3, PS-7 active |
| 2 — repo authority (RA-1..RA-16, 11a/11b, 12b) | 18 (50) | CORE-OOS-3 | new |
| 3 — readers (RD-1..RD-11) | 11 (17) | CORE-OOS-5 | new |
| 4 — review app | 1 (1) — SA-1 share draft | — | publish-time check + link prompt already shipped (R8) |
| 5 — imported candidates (IC-1..IC-5) | 5 (5) | — | new; IC-5 active |
| **Total** | **44 (93 rows)** | **6** | **50 test fns** (+1 fixture regenerator) |

- Walking skeleton: 1 (WS-OOS-1). Non-happy: 30 / 44 example scenario fns (68 %); strict refusal / nothing-changed row share ≈ 48 / 93 (52 %). `@security`: 20. Active guards: PS-3, PS-7, IC-5.
- Tables: WS ×2, PS-2 ×3, PS-3 ×2, PS-4 ×4, PS-5 ×2 + 1 supersedes case, PS-8 ×2, PS-9 ×3, RA-2 ×3, RA-3 ×11, RA-5 ×3, RA-6 ×2, RA-8 ×2, RA-10 ×6, RA-11a ×2, RA-11b ×3, RA-12b ×3, RA-13 ×5, RA-16 ×3, RD-4 ×6, RD-7 ×2. Proptest (256 cases) on every unbounded pure rule (CORE-OOS-1..5).
- `@kpi`: KPI-OOS-1 (WS-OOS-1, PS-2, PS-5, PS-6, RA-1, RA-2, RA-11a/b, CORE-OOS-4 + shipped review-app publish tests, R8), KPI-OOS-2 (RD-2, RD-3, RD-4, RD-6, RD-10, IC-2, CORE-OOS-5), KPI-OOS-3 (PS-3), KPI-OOS-4 (RA-1, RA-3, RA-4).
- **Step reuse (CM-I-4, informational)**: 411 vocabulary invocations / 67 distinct functions = **6.13×**.

### AT-completeness (15 items): **14 of 14 applicable pass; C7c not applicable (documented) — COMPLETE**

Revision 0 claimed 14/15 while one item (C5a) was open; the accurate statement then was 13/14
applicable. RD-11 (ruling P-OOS-1) closes C5a.

| Item | | Evidence |
|---|---|---|
| C1a empty/zero | ✓ | PS-2 (no link), RA-10 (never observed), RA-14 (no GitHub claims), IC-3 (nothing imported) |
| C1b boundaries | ✓ | window 0 / 7 d / 7 d + 1 min / −1 min for person (PS-4, CORE-OOS-4x) and listed repo (RA-10); 4 KiB exactly vs over (RA-3) |
| C2a state machine documented | ✓ | module docs: person tier (`owner_only_signing.rs`), repo observation + permit (`owner_only_signing_repos.rs`), gate table `GateRow` (core) |
| C2b illegal event per state | ✓ | sign with no link / proof gone / stale / future check / old login / old repo name / repo not found; publish a pre-feature third-party claim (online and offline) |
| C3 0/1/many | ✓ | links 0/1/2 DIDs on one account; listed DIDs 0..3 (CORE-OOS-3); imports 0/1/2; weightless rows 1..5 |
| C4a apply twice | ✓ | RA-11b re-publish, RA-12 open twice, RA-12b first open of a v7 store, RA-9 repeated observation, IC-4 re-import (P-OOS-3) |
| C4b inverse without prerequisite | ✓ | RA-14, PS-3 withdraw after authority is gone |
| C5a mode combinations | ✓ | `--sign` × subject kind × verb; online × offline (incl. RA-11a); fresh × stale; GitHub answers × fails at sign time (PS-9, RA-13); `--weighted` × `--include-unattested` incl. the unweighted combination (RD-11) |
| C5b flag orthogonality | ✓ | RD-3, PS-6 |
| C6a malformed input | ✓ | malformed / case-variant subjects (CORE-OOS-2), authority-file lies (RA-3, CORE-OOS-3), spoofed `counters` (RD-4) |
| C6b each declared error | ✓ | every `Refusal` at layer 3; NotListed at layer 2; usage error exit 2 (RD-11) |
| C6c closed error set | ✓ | CORE-OOS-3, CORE-OOS-4 (`GateRow` total) |
| C7a degraded resource | ✓ | network off (PS-4, RA-10, RA-11a, RA-12b, IC-1), rate limit / unreachable / cross-host (RA-5, PS-9, RA-13), budget and token (RA-6, RA-16) |
| C7b interruption | ✓ | RA-6 (budget runs out between harvest and authority read), PS-9 (refresh fails after the import read) |
| C7c concurrency | N/A | DESIGN states no concurrency contract for signing or checks (insert-only; deterministic latest-row tie-break) |

Completeness audit log: `(owner-only-signing, all, 0, NONE)`.

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
| U-OOS-4 | **RESOLVED 2026-10-20 — ruling P-OOS-2 accepted**: the share preview re-reads bio + each quoted repo's owner as publish does; quotes only permitted claims; others listed "not counted: authority not proven". SA-1 aligned. | resolved |
| U-OOS-5 | `kind` precedence when one claim carries both `retracts` and `counters` (or a `sameAs` predicate plus a marker) is unspecified; harmless for gate and weight (all non-weight), so CORE-OOS-1 asserts the exact kind only for single-marker claims. | DESIGN (text) |
| U-OOS-6 | **RESOLVED 2026-10-20 — ruling P-OOS-3 accepted**: each (subject, predicate, object) once, with the latest import's confidence, evidence and time; de-duplication in the read query. IC-4 aligned. | resolved |
| U-OOS-7 | **RESOLVED 2026-10-20 — ruling P-OOS-1 accepted**: exits 2 with "--include-unattested requires --weighted" and prints no results. RD-11 added. | resolved |
| U-OOS-8 | **Tracked DELIVER blocker (review F-07)**: the REQUIRED recorded-fixture contract test of `/repos` + contents shapes (ADR-087 Earned Trust fixtures `repo_owner_user.json`, `repo_owner_org.json`, `repo_renamed_redirect.json`, `authority_file.json`, `authority_file_symlink.json`) and the `repo_authority_checks` `probe()` are adapter tests owned by DELIVER, not acceptance. DELIVER adds them in `crates/adapter-github` with `read_repo_authority`; untracked when slice 2 starts = blocker. | DELIVER |
| U-OOS-9 | `FakeGithubAccounts::harvested_repos` / `scrape_reads_of` count any `/repos/{o}/{r}/…` read as a harvest. If DELIVER adds the authority-file read to the review app's publish path (ADR-089 §1 says `read_repo_authority` = `read_repo` + file), RRA oracles that compare harvested repos would see it; exclude `contents/.github/openlore-authority` there when that happens. | DELIVER |
| U-OOS-10 | Opt-in telemetry counter `claim.sign.refused.authority` (and the review app's `claim.publish.refused.authority`) has no acceptance scenario: observability, not KPI-gated; DELIVER unit-tests it. | DELIVER |
| U-OOS-12 | **Two driving-port details uncovered at layer 3** (review F-24): `authority verify --mine` and a sign-time refusal of a `supersedes`/`corrects` claim on someone else's `github:` subject (layer 2 only, CORE-OOS-2). DELIVER adds one row each to RA-4 / PS-2 when the verb lands. **Supersedes half moved to PS-5** (orchestrator ruling, DELIVER step 01-05): `claim add` carries no reference arguments, so the only verb that signs `supersedes` is `infer people --sign`. PS-5 now holds a case where a STRONGER inference about BurntSushi supersedes one Jeff signed while he could attest the account, after he withdrew the link, and is refused with `NoLink` and the hand-off. The `authority verify --mine` row stays with RA-4. | DELIVER |
| U-OOS-13 | **Contested accounts — the second DID's half** (review F-03): each store signs only as Jeff (`TestEnv` identity), so "Rachel, also verified for jeffbailey, signs about it" cannot be driven at layer 3; CORE-OOS-3 generates both DIDs verified for one id and requires Owner for each. PS-7's Given re-checks every link in Jeff's store (`identity verify` without a filter), Rachel's included. | record |
| U-OOS-14 | **Environments** (review F-19/F-20): `with-pre-commit` is N/A for a CLI-local feature; `with-stale-config` maps to the stale-check / proof-gone / pre-feature-store states in PS-2, PS-4, RA-10, RA-11a, RA-12, RD-8. Elevator pitches were authored in DISTILL (no DISCUSS): DISCUSS→DISTILL verbatim continuity is unverifiable until a DISCUSS exists (WARN). | record |
| U-OOS-15 | **PS-9 successful-refresh rows (2026-10-20, after DELIVER step 01-06)**: PS-9 covered only GitHub failing at sign time, so deleting the sign-time refresh in `scrape person --sign` left every test green. Two rows were added with GitHub answering at 21 October against a 1 October check. In the `jeffbailey` row, signing succeeds, `identity list` shows the check as "verified today", exactly one new check is recorded (secondary guardrail), and Jeff's profile is read twice: once by the import and once by the re-check. In the `BurntSushi` row, signing is refused with `NoLink`, identities read as before, no check is recorded, and Jeff's profile is never read. New steps: `github_serves_the_profile_of` and `profile_reads_of` in `support/authority.rs`. Mutation check: deleting the refresh call makes PS-9 fail at the primary Then (the signing is refused). | record |
| U-OOS-11 | **RESOLVED**: `discuss/user-stories.md` now holds S-OOS-1..9 with job ids (680f56b); IDs unchanged; scenarios tagged `@s-oos-N`. | resolved |

## Review Gate Result

Internal Sentinel cycles 1-2 approved revision 0; the four-reviewer gate then rejected it.
Review iteration 1 (this revision) addresses every finding — `acceptance-review.md` § Review
iteration 1.

Iteration 2 (re-review of ed27d3d, with design a600ad3, stories 680f56b, rulings 9288ea0):

- Review type: full review (4 reviewers), 2 iterations.
- Eclipse / PO: rejected -> revised -> **approved** (open for the user: proposed job J-011).
- Architect / SA: rejected -> revised -> **approved** (residual: guardrail call sites each beside a
  primary Then — DELIVER spot check; keep GitHub request counts trimmed).
- Forge / PA: rejected -> revised -> **approved**, on condition the active guards pass: PS-3, PS-7
  and IC-5 ran green locally at ed27d3d (9 run, 9 passed, 47 skipped). Actions: measure the
  acceptance build on the next CI run; optional sign-time token row in RA-6 (N1).
- Sentinel / AD: needs_revision -> revised -> **conditionally approved**: the Mandate-8 waiver
  (R1, GDL D-e precedent) needs the user's acknowledgement. R-2: RA-12 is red today only through
  its secondary guardrail because its primary Thens are preservation Thens that already hold; this
  is the nature of a preservation scenario, recorded as a known exception, not fixed.

Mandate-8 waiver ACKNOWLEDGED by the user (2026-10-20); Sentinel condition met. Handoff to DELIVER: ready.
