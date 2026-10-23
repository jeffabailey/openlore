# Acceptance Test Design — owner-only-signing (DISTILL)

- **Wave**: DISTILL · **Date**: 2026-10-20 (revision 1 after the four-reviewer gate, same day) · **Designer**: Quinn (nw-acceptance-designer)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (ADR-007)
- **Language**: Rust (`[lang-mode] rust`) · `[policy-mode] inherit` · `[port-mode] inherit` (`tests/common/state_delta.rs`)
- **Framework**: Rust std `#[test]` + subprocess, Gherkin in doc comments (house convention: `identity_link*.rs`, `infer_people_sign.rs`, `review_app_*.rs`); proptest at layer 2 only.
- **Skip marker**: `#[ignore = "DISTILL owner-only-signing <ID>: pending DELIVER …"]` on every scenario, the walking skeleton included (house rule D-d), EXCEPT the active guards PS-3, PS-7 and IC-5 (scaffold policy, DWD-OOS-2: a scenario that holds today and must hold after DELIVER runs active). The default run is green.
- **Inherits**: DESIGN (OOS-D1..D15, A-1..A-14, C-OOS-1..9, ADR-086/087 incl. the 2026-10-20 corrections a600ad3 and rulings P-OOS-1..3), DISCUSS `user-stories.md` (S-OOS-1..9, written in parallel; IDs unchanged). No DEVOPS (project CI acceptance stage).
- **Observation rule** (ruling R1, GDL D-e): every PRIMARY Then reads a driving port — exit status, CLI output (`authority verify`, `candidates list`, `graph query`, `identity list`, refusal text), PDS wire JSON, viewer / review-app pages, GitHub's request log. The store universe is a labelled SECONDARY guardrail (`guardrail_*`) under the Mandate-8 waiver in `wave-decisions.md`.

The `.rs` files are the scenario SSOT; this is the map. Every scenario doc comment carries `@s-oos-N` and `@contract-shape:…`.

## 1. Inputs read

| | Input | Used for |
|---|---|---|
| ✓ | `design/architecture-design.md`, `design/wave-decisions.md`, `design/upstream-changes.md` | driving ports, per-verb capabilities, flows, slices, UC-1..22 |
| ✓ | `docs/adrs/ADR-086-…`, `ADR-087-…` | authority model, rule table, Earned Trust, lie catalogue, affected ATs |
| ✓ | `docs/adrs/ADR-088-…`, `ADR-089-…`, `crates/claim-domain/src/authority.rs` | reconciliation: what slice 4 already shipped |
| ✓ | `docs/product/architecture/brief.md` (§ For Acceptance Designer, I-9 row) | must-cover list, doubles, I-9 exception |
| ✓ | `docs/product/kpi-contracts.yaml` (KPI-OOS-1..4) | `@kpi-oos-*` tags |
| ✓ | `docs/product/journeys/{scrape-propose-sign, author-and-publish-claim, infer-contributor-philosophy, explore-the-graph}.yaml` | failure modes, chained narrative |
| ✓ | `docs/architecture/atdd-infrastructure-policy.md` | mechanisms (appended: repo-authority ports) |
| ✓ | prior art: `tests/acceptance/identity_link*.rs`, `infer_people_sign.rs`, `review_app_repos_and_accounts*.rs`, `docs/feature/{github-did-identity-link, review-app-repos-and-accounts}/distill/*` | harness, style, ignore convention, layer-2 `sut_*` seam |
| ✓ | `docs/feature/owner-only-signing/discuss/user-stories.md` (revision 1) | S-OOS-1..9 with job ids; tags `@s-oos-N` |
| ⊘ | `docs/feature/owner-only-signing/devops/` | **missing** — WARN: project CI acceptance stage, default environment |

## 2. Test files

| File (`tests/acceptance/`) | Slice | Layer | Scenario fns (rows) | Registered in |
|---|---|---|---|---|
| `owner_only_signing.rs` | 1 — person gate, WS, exemptions (PS-3, PS-7 ACTIVE), offline window, sign-time GitHub failure, inference | 3 subprocess | 9 (20) | `crates/cli/Cargo.toml` |
| `owner_only_signing_repos.rs` | 2 — repo authority, `authority verify`, `scrape --sign`, `claim publish`, upgrades (pre-v7, v7), budget; 5 — imported candidates (IC-5 ACTIVE; IC-6 ignored, D35 addendum) | 3 subprocess | 18 + 6 (50 + 7) + 1 fixture regenerator | `crates/cli/Cargo.toml` |
| `owner_only_signing_readers.rs` | 3 — labels, `score_attested`, `--include-unattested`, viewer, search | 3/4 subprocess + viewer + indexer | 11 (17) | `crates/cli/Cargo.toml` |
| `owner_only_signing_core.rs` | 1-3 pure core | 2 proptest (256 cases) | 5 properties + 1 example | `crates/cli/Cargo.toml` |
| `owner_only_signing_review_app.rs` | 4 remainder — share draft (UC-21, P-OOS-2) | 4 HTTP | 1 (1) | `crates/openlore-review-app/Cargo.toml` |
| **Total** | | | **50 test fns** (44 example scenarios, 93 example rows; 6 layer-2) + 1 regenerator | |

Vocabulary: `support/authority.rs` (typed domain module, wording pins, `WhatJeffSees` port view, SECONDARY guardrails), reusing `support/identity_links.rs` (`LinkWorld`, link steps; revision 1 adds `with_frozen_store_from` / `freeze_store_into`) and `support/people.rs`.
Double (test-support, additive): `FakeGithubAccounts` serves `AuthorityFile` shapes at `/repos/{o}/{r}/contents/.github/openlore-authority`, `redirect_repo_to_another_host`, `exhaust_budget_at_authority_reads`, `authority_file_reads_of`; revision 1 adds `rate_limit_profile_after` / `drop_profile_after` (a profile served N more times, then 403 / 503). Two unit tests.
Fixture (revision 1): `tests/fixtures/owner-only-signing-v7-store/` — frozen v7 store (EXPORT DATABASE dump, no `.duckdb`), regenerated by `regenerate_the_frozen_v7_store_fixture`.

**Mix** (primary category of each example scenario fn; non-happy = error/refusal, boundary, edge, infrastructure-failure, guardrail/security): walking skeleton 1 · happy 13 · error/refusal 5 · boundary 2 · edge 9 · infrastructure-failure 4 · guardrail/security 10. **Non-happy: 30 / 44 fns (68 %)**; strict refusal-row share (rows whose Then is a refusal or "nothing written / unchanged") ≈ 48 / 93 rows (52 %). `@security` on 20 fns.

## 3. Stories (DISCUSS `user-stories.md`, IDs unchanged)

| Story | Scenarios (`@s-oos-N`) |
|---|---|
| S-OOS-1 Only the owner of a GitHub account signs about it; others get the hand-off | WS-OOS-1, PS-2, PS-5, PS-6, PS-7, PS-8, RA-2 (person row), CORE-OOS-2/3/4 |
| S-OOS-2 Anyone can always dispute; the author can always withdraw | PS-3, CORE-OOS-1/2 |
| S-OOS-3 Signing works offline from a check ≤ 7 days old, never otherwise (and online verbs fall back to it when GitHub cannot answer) | PS-4, PS-9, RA-10, RA-12b, RA-15, CORE-OOS-4/4x |
| S-OOS-4 Only the repo's owner or a DID its authority file lists signs about the repo; anyone imports | RA-1, RA-2, RA-3, RA-8, RA-13, CORE-OOS-3 |
| S-OOS-5 Anyone re-checks authority from public data; failures change nothing | RA-4, RA-5, RA-6, RA-7, RA-9, RA-14, RA-16 |
| S-OOS-6 A never-published claim publishes only with a permit; upgrades keep everything | RA-11a, RA-11b, RA-12, RA-12b |
| S-OOS-7 Readers label every `github:` assertion and weigh only attested ones | RD-1..RD-11, CORE-OOS-5 |
| S-OOS-8 The review app quotes and publishes only claims whose authority holds | SA-1; publish half already shipped (ruling R8): `review_app_repos_and_accounts_several.rs` AP-1/AP-2/AP-4, `review_app_publish_permit_alone.rs`, `no_request_publishes_a_suggestion_about_a_repo_the_owner_doesnt_own` (`crates/openlore-review-app/tests`); link prompt = GDL slice 3 `review_app_identity_link.rs` |
| S-OOS-9 Imported candidates are kept locally, never federated, never weighed, never inferred from | IC-1..IC-6 |

## 4. Traceability — scenario → decision (elevator pitch = test name in words)

| ID | Elevator pitch | Traces to |
|---|---|---|
| WS-OOS-1 | Only Jeff can sign what is said about Jeff's GitHub account (2 rows) | OOS-D4, C-OOS-1, KPI-OOS-1 |
| PS-2 | A person claim Jeff cannot attest is refused, says why and who can sign (3) | ADR-087 §1, ADR-086 §10 |
| PS-3 | Jeff can always dispute or withdraw, whatever authority he holds (2; ACTIVE; wire body asserted) | OOS-D11, KPI-OOS-3 |
| PS-4 | With the network off, Jeff signs about himself only from a check at most 7 days old (4) | ADR-086 §5, I-9, A-6 |
| PS-5 | Jeff signs an inferred adherence only about himself (2 + the supersedes case, U-OOS-12) | UC-9, A-8 |
| PS-6 | Jeff's inferred people say who can sign each candidate, and listing writes nothing | ADR-086 §10 |
| PS-7 | Jeff still signs about his account when its bio also names another identity (ACTIVE) | UC-2 |
| PS-8 | After Jeff renames his GitHub account, he signs only under the new login (2) | ADR-087 §1, G-2 |
| PS-9 | When GitHub cannot answer as Jeff signs from an import, his latest check decides (3); when it answers, his own link is re-checked first, once (2) | ruling R3, ADR-086 §5 clarification, ADR-087 §6 |
| RA-1 | Jeff imports his own repo and signs what it shows | ADR-087 §2, KPI-OOS-4 |
| RA-2 | Jeff imports anything public, but signing what he cannot attest is refused with a hand-off (3) | ADR-086 §10, OOS-D12 |
| RA-3 | An organization repo is signed only by an identity its authority file lists exactly (11) | ADR-087 §3, lie catalogue |
| RA-4 | Re-checking authority labels each peer's repo claims from one look at each repo | ADR-087 §4/§6, C-OOS-4 |
| RA-5 | When GitHub cannot answer, re-checking records nothing and changes nothing (3) | ADR-087 §4 |
| RA-6 | When GitHub's budget runs out mid-import, the candidates are listed and his latest check decides (2) | ADR-087 §6 (a600ad3) |
| RA-7 | After Jeff transfers his repo, his earlier claim reads third-party and nothing is withdrawn | ADR-086 §6, C-OOS-5 |
| RA-8 | After Jeff renames his repo, he signs only under the name GitHub serves now (2) | A-10, ADR-089 §2 |
| RA-9 | A listed identity stays listed whoever observed the repo last | review H-2 |
| RA-10 | With the network off, Jeff signs a repo claim only from fresh checks (6; listed 7 d / 7 d + 1 min) | ADR-086 §5 |
| RA-11a | A claim Jeff signed about someone else's repo before owner-only signing is never published (2: online / offline) | OOS-D3, OOS-D8 |
| RA-11b | Publishing a stored claim later asks for authority only if it was never published (3) | ADR-086 §4 |
| RA-12 | Opening a store made before owner-only signing keeps everything in it | ADR-087 §5 |
| RA-12b | Jeff's identity-link checks carry over when his store gains owner-only signing (3; frozen v7 store) | ADR-087 §5, ruling R5 |
| RA-13 | Signing from an import re-checks Jeff's own link first (5; incl. GitHub failing) | ADR-087 §6, N-2, ruling R3 |
| RA-14 | With no GitHub claims in his store, re-checking authority asks GitHub nothing | C3/C4b |
| RA-15 | A repo GitHub does not have is recorded as not found and cannot be signed | ADR-087 §4 (404) |
| RA-16 | When GitHub's budget is gone, re-checking says so, records nothing and never shows his token (3) | ADR-087 §6, ruling R4 |
| IC-1 | Jeff reviews what he imported, offline, one repo at a time | OOS-D12 B |
| IC-2 | Importing a repo keeps its candidates locally and nowhere else | C-OOS-7 |
| IC-3 | With nothing imported, the list says so | C1a |
| IC-4 | Importing the same repo twice lists each candidate once, as last imported | P-OOS-3 |
| IC-5 | Imported candidates never feed Jeff's inferred people (ACTIVE) | C-OOS-7, review PO F-05 |
| IC-6 | Reading a person keeps the candidates it suggests, each listed once (2 rows; ignored, DELIVER pending — addendum 2026-10-23) | user decision D35, U-OOS-D35, C-OOS-7, P-OOS-3 |
| RD-1 | Every claim about a repo is shown with whose word it is | ADR-086 §8 |
| RD-2 | The weighted view counts only attested claims and says what it left out | OOS-D9, KPI-OOS-2 |
| RD-3 | Jeff can see the claims left out, per author, without changing the weight | OOS-D9 |
| RD-4 | Nothing but an attested claim moves the weight (6) | KPI-OOS-2, UC-16/20 |
| RD-5 | A claim marked as a dispute is shown as a dispute | ADR-086 §2 |
| RD-6 | Jeff's viewer weighs and labels claims exactly as his terminal does | KPI-OOS-2 |
| RD-7 | Network search labels every GitHub claim, using Jeff's own check when he has one (2) | UC-7, C-OOS-8 |
| RD-8 | Claims made before owner-only signing are judged by the same rule | ADR-086 §7 |
| RD-9 | Inferred people show the authority of each repo claim they rest on | UC-10 |
| RD-10 | After Jeff renames his account, his claim about the old login reads third-party | G-2, A-14 |
| RD-11 | Asking for unattested claims without the weighted view is a usage mistake | P-OOS-1 |
| SA-1 | Priya's share post names only claims about repos she still owns | UC-21, P-OOS-2 |

**KPI traceability** (ruling R8): KPI-OOS-1 — WS-OOS-1, PS-2, PS-5, PS-6, RA-1, RA-2, RA-11a/b, CORE-OOS-4, plus the shipped review-app publish tests cited under S-OOS-8. KPI-OOS-2 — RD-2, RD-3, RD-4, RD-6, RD-10, IC-2, CORE-OOS-5. KPI-OOS-3 — PS-3 (active). KPI-OOS-4 — RA-1, RA-3, RA-4.

**Constraint enforcement outside acceptance** (ruling R9, F-06/F-07): C-OOS-8 (indexer gains no GitHub capability) — `xtask check-arch` `INDEXER_FORBIDDEN_DEPS` (DELIVER adds `adapter-github`), plus RD-7 at the port. C-OOS-4 (authority reader-local, never federated, insert-only) — `xtask check-arch` `repo_authority_checks_insert_only` + `authority_readers_hold_no_write_handle` (DESIGN component table) and the shipped `identity_link_checks_insert_only`, plus RA-4 / RA-16 at the port (nothing published, nothing recorded). C-OOS-7 additionally `imported_candidates_not_in_scoring_or_inference` + IC-2 / IC-5.

### Layer 2 (`owner_only_signing_core.rs`, all `@contract-shape:pure-function`)

| ID | Property | Binding | Traces to |
|---|---|---|---|
| CORE-OOS-1 | only disputes, withdrawals and links are weightless kinds | `todo!()` → `authority::kind` | ADR-086 §2 |
| CORE-OOS-2 | only GitHub assertions need authority; malformed `github:` never exempt | `todo!()` → `authority::requirement` | ADR-086 §1 |
| CORE-OOS-3 | attested exactly when the ADR-087 table says so | `todo!()` → `authority::classify` | ADR-087 §1-3 |
| CORE-OOS-4 / 4x | gate = decision table `GateRow` G1..G8 (ADR-086 §5 / ADR-087 §7); 4x anchors G3/G4/G8 by example | `todo!()` → CLI gate over local checks | ADR-086 §5, A-6 |
| CORE-OOS-5 | `score_attested == score(eligible)`; unmoved by weightless rows | `todo!()` → `score_attested`; oracle uses REAL `scoring::score` | KPI-OOS-2 |

Layer-2 CM-A exception: these call the pure core directly (Mandate 9 layer 2; ADR-007 functional core API); each is mirrored at layer 3 through the binary (module doc).

## 5. Adapter and driving-adapter coverage

| Adapter / port | @real-io scenario |
|---|---|
| `openlore` CLI — `claim add`, `claim counter`, `claim retract`, `claim publish` | WS-OOS-1, PS-2/3/4/7/8, RA-8/10/11a/11b/12b/15 |
| `openlore` CLI — `scrape github <owner/repo> --sign`, `scrape person --sign` | RA-1, RA-2, RA-3, RA-6, RA-13, PS-9 |
| `openlore` CLI — `infer people [--sign]` | PS-5, PS-6, RD-9, IC-5 |
| `openlore` CLI — `authority verify [--subject]` (NEW) | RA-4, RA-5, RA-7, RA-9, RA-14, RA-15, RA-16 |
| `openlore` CLI — `candidates list [--subject]` (NEW, slice 5) | IC-1, IC-2, IC-3, IC-4, IC-6 |
| `openlore` CLI — `graph query [--weighted] [--include-unattested] [--explain]`, `search`, `identity list` | RD-1..RD-5, RD-7, RD-8, RD-11, RA-12, RA-12b, every `WhatJeffSees` observation |
| viewer `/score`, `/project` | RD-6 |
| review app share preview | SA-1 |
| `adapter-github` `read_repo_authority` (+ redirect policy; exact policy pinned by the recorded-fixture test, U-OOS-8) | RA-1, RA-3, RA-4, RA-5, RA-8, RA-16 |
| `adapter-github` sign-time link refresh failing (rate limit / unreachable) | PS-9, RA-13 |
| `adapter-duckdb` v8 `repo_authority_checks` | RA-1, RA-4, RA-6, RA-10, RA-12, RA-12b |
| `adapter-duckdb` v8 `imported_candidates` | IC-1..IC-6 |
| `adapter-duckdb` v7 `identity_link_checks` (read; write via `--sign` refresh) | PS-4, PS-9, RA-12b, RA-13 |
| single publish path (`FakePds`) | WS-OOS-1, PS-3, RA-1, RA-11b |

## 6. Scenarios (Gherkin, extracted from the doc comments)

### `owner_only_signing.rs`

**WS-OOS-1** — `only_jeff_can_sign_what_is_said_about_jeffs_github_account`

```gherkin
@walking_skeleton @driving_port @driving_adapter @real-io @security @kpi-oos-1
@oos-d4 @c-oos-1 @s-oos-1 @contract-shape:bounded-change
Scenario Outline: Only Jeff can sign what is said about Jeff's GitHub account
  Given Jeff has publicly linked his GitHub account "jeffbailey" to his identity
  When Jeff signs and publishes "<account> adheres to memory-safety"
  Then <outcome>
  Examples:
    | account    | outcome                                                                                       |
    | jeffbailey | the claim is signed and reaches his published repo                                            |
    | rachelk    | nothing is signed or published, and he is told rachelk can sign it after linking her account |
```

**PS-2 (table over why Jeff holds no person authority)** — `a_person_claim_jeff_cannot_attest_is_refused_says_why_and_who_can_sign`

```gherkin
@driving_port @real-io @error @security @kpi-oos-1 @c6b @s-oos-1 @contract-shape:unbounded-preservation
Scenario Outline: A person claim Jeff cannot attest is refused, says why and who can sign
  Given <situation>
  When Jeff signs and publishes "<account> adheres to memory-safety"
  Then nothing is signed, published or recorded
  And he is told "<reason>" and who can sign it
  Examples:
    | situation                                                         | account    | reason                              | who can sign            |
    | Jeff has not linked any GitHub account                            | jeffbailey | you have no identity link to        | jeffbailey, after link  |
    | Jeff's bio stopped naming his identity and he re-checked his link | jeffbailey | your identity link is not verified  | jeffbailey, after link  |
    | Jeff linked "jeffbailey"                                          | tobiasw    | you have no identity link to        | tobiasw, after link     |
```

**PS-3 (table over the two exempt kinds; chained on WS-OOS-1; review H-1)** — `jeff_can_always_dispute_or_withdraw_whatever_authority_he_holds`

```gherkin
@driving_port @real-io @kpi-oos-3 @c-oos-2 @oos-d11 @guardrail @s-oos-2 @contract-shape:bounded-change
Scenario Outline: Jeff can always dispute or withdraw, whatever authority he holds
  Given <situation>
  When Jeff <acts>
  Then his published repo holds <record>
  Examples:
    | situation                                                                     | acts                                          | record                                                          |
    | Rachel published "rachelk adheres to memory-safety" and Jeff pulled it        | disputes Rachel's claim about her own account | his dispute of Rachel's claim, about github:rachelk             |
    | Jeff signed and published about "jeffbailey", then his bio stopped naming him | withdraws his own claim about "jeffbailey"    | his withdrawal of that claim, about github:jeffbailey           |
```

**PS-4 (boundary table over the age of Jeff's latest link check)** — `with_the_network_off_jeff_signs_about_himself_only_from_a_check_at_most_7_days_old`

```gherkin
@driving_port @real-io @boundary @c1b @i-9 @c-oos-9 @oos-d7 @security @s-oos-3 @contract-shape:bounded-change
Scenario Outline: With the network off, Jeff signs about himself only from a check at most 7 days old
  Given Jeff linked "jeffbailey" and his link was checked on 1 October at 09:00
  And the network is off
  When Jeff signs "jeffbailey adheres to memory-safety" at <time>
  Then <outcome>
  And GitHub is not asked anything
  Examples:
    | time                    | outcome                                                                       |
    | 3 October 09:00         | the claim is signed and kept locally                                          |
    | 8 October 09:00         | the claim is signed and kept locally (exactly 7 days)                         |
    | 8 October 09:01         | nothing is signed; he is told to run "openlore authority verify --subject github:jeffbailey" |
    | 20 September 09:00      | nothing is signed (a check dated after now counts as missing); same command  |
```

**PS-9 (ruling R3: GitHub cannot answer at signing time; chained on PS-4's Given)** — `when_github_cannot_answer_as_jeff_signs_from_an_import_his_latest_check_decides`

```gherkin
@driving_port @real-io @infrastructure-failure @c7a @i-9 @r-3 @security @s-oos-3 @contract-shape:bounded-change
Scenario Outline: When GitHub cannot answer as Jeff signs from an import, his latest check decides, as offline
  Given Jeff linked "jeffbailey" and his link was checked on 1 October at 09:00
  And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
  And <trouble> once the import has read his profile
  When Jeff reads "jeffbailey" as a person and signs his memory-safety candidate at <time>
  Then <outcome>
  And his linked identities read as before (the failed re-check recorded nothing)
  Examples:
    | trouble                                         | time            | outcome                                                                        |
    | GitHub says it is too busy to answer            | 3 October 09:00 | the claim is signed and reaches his published repo                             |
    | GitHub cannot be reached                        | 3 October 09:00 | the claim is signed and reaches his published repo                             |
    | GitHub says it is too busy to answer            | 8 October 09:01 | nothing is signed; he is told to re-check authority, with the command to run   |

@driving_port @real-io @security @s-oos-3 @r-3 @adr-087-6 @contract-shape:bounded-change
Scenario Outline: When GitHub answers as Jeff signs from an import, his own link is re-checked first, once
  Given Jeff linked "jeffbailey" and his link was checked on 1 October at 09:00
  And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
  And GitHub answers every profile read, BurntSushi's included
  When Jeff reads "<person>" as a person and signs candidate 1 on 21 October at 09:00
  Then <outcome>
  And GitHub was asked for Jeff's own profile <asked>
  Examples:
    | person     | outcome                                                                                              | asked                                                      |
    | jeffbailey | the claim is signed and reaches his published repo; his identities show one new check, dated today   | once for the import and exactly once more for the re-check |
    | BurntSushi | nothing is signed; he is told BurntSushi can sign it after linking; his identities read as before    | never                                                      |
```

The two success rows (added after DELIVER step 01-06) pin the sign-time refresh itself: the
1 October check is 20 days old, so without the refresh the `jeffbailey` row is refused as too
old. Deleting the `refresh_signers_own_link` call in `crates/cli/src/verbs/scrape_person.rs`
turns the test red at the row's primary Then (signed and published).

**PS-5 (table over whose adherence Jeff signs)** — `jeff_signs_an_inferred_adherence_only_about_himself`

```gherkin
@driving_port @real-io @security @kpi-oos-1 @uc-9 @a-8 @s-oos-1 @contract-shape:bounded-change
Scenario Outline: Jeff signs an inferred adherence only about himself
  Given Jeff linked "jeffbailey"
  And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
  And Jeff's inferred people list proposes memory-safety for both people
  When Jeff signs the inferred candidate about <person>
  Then <outcome>
  Examples:
    | person     | outcome                                                                                        |
    | jeffbailey | the adherence is signed and reaches his published repo                                         |
    | BurntSushi | nothing is signed or published; he is told BurntSushi can sign it after linking the account   |

@driving_port @real-io @security @kpi-oos-1 @uc-9 @a-8 @s-oos-1 @u-oos-12 @contract-shape:unbounded-preservation
Scenario: A stronger inference about BurntSushi is not signed, though Jeff signed the earlier one
  Given Jeff once held a verified link to BurntSushi's account
  And Jeff signed the inferred memory-safety adherence about BurntSushi from ripgrep alone
  And Jeff has since withdrawn that link
  And Jeff recorded that BurntSushi also builds rust-lang/regex, so the inference about him is stronger
  When Jeff signs the stronger candidate about BurntSushi
  Then nothing is signed or published; he is told BurntSushi can sign it after linking the account
  And his earlier adherence about BurntSushi reads as before
```

U-OOS-12 (supersedes half) lives here, not in PS-2: `claim add` carries no reference arguments, so the only verb that signs a `supersedes` claim is `infer people --sign` (a STRONGER candidate supersedes Jeff's earlier inference when the evidence grows). The Given uses real verbs only: Jeff could attest BurntSushi's account when he signed the earlier claim, then withdrew the link (`claim retract`), so the superseding claim meets `NoLink`. The Given checks that the listing shows the candidate as STRONGER, naming the earlier claim, before the When runs. Primary Thens: the refusal and the hand-off, plus `assert_jeff_sees_no_change`. The store guardrail is secondary. The case runs in the same `#[test]` after the two table rows.

**PS-6 (chained on PS-5's Given)** — `jeffs_inferred_people_say_who_can_sign_each_candidate_and_listing_writes_nothing`

```gherkin
@driving_port @real-io @kpi-oos-1 @adr-086-10 @s-oos-1 @contract-shape:unbounded-preservation
Scenario: Jeff's inferred people say who can sign each candidate, and listing writes nothing
  Given Jeff linked "jeffbailey"
  And Rachel's signed claims say two repos built by BurntSushi and jeffbailey are memory-safe
  When Jeff lists his inferred people
  Then the candidate about jeffbailey says "You can sign"
  And the candidate about BurntSushi says it can be signed by BurntSushi after "openlore identity link github BurntSushi"
  And nothing in his store or published repo changed
```

**PS-7** — `jeff_still_signs_about_his_account_when_its_bio_also_names_another_identity`

```gherkin
@driving_port @real-io @edge @security @uc-2 @adr-087-1 @guardrail @s-oos-1 @contract-shape:bounded-change
Scenario: Jeff still signs about his account when its bio also names another identity
  Given the "jeffbailey" bio names both Jeff's identity and Rachel's
  And Jeff and Rachel both published a link to "jeffbailey", and Jeff re-checked every link in his store (both verify)
  When Jeff signs and publishes "jeffbailey adheres to memory-safety"
  Then the claim is signed and reaches his published repo
```

**PS-8 (table over the old and the new login)** — `after_jeff_renames_his_github_account_he_signs_only_under_the_new_login`

```gherkin
@driving_port @real-io @edge @g-2 @a-14 @adr-087-1 @security @s-oos-1 @contract-shape:bounded-change
Scenario Outline: After Jeff renames his GitHub account, he signs only under the new login
  Given Jeff linked "jeffbailey", renamed the account to "jeff-b" and re-checked his link
  When Jeff signs and publishes "<login> adheres to memory-safety"
  Then <outcome>
  Examples:
    | login      | outcome                                                                    |
    | jeff-b     | the claim is signed and reaches his published repo                         |
    | jeffbailey | nothing is signed; he is told it is no longer the login of his linked account |
```

### `owner_only_signing_repos.rs`

**RA-1** — `jeff_imports_his_own_repo_and_signs_what_it_shows`

```gherkin
@driving_port @driving_adapter @real-io @kpi-oos-1 @kpi-oos-4 @adr-087-2 @oos-d5 @s-oos-4 @contract-shape:bounded-change
Scenario: Jeff imports his own repo and signs what it shows
  Given Jeff linked "jeffbailey", the account that owns jeffbailey/tidepool
  When Jeff imports jeffbailey/tidepool and signs candidate 1
  Then the claim is signed and reaches his published repo
  And his claim about tidepool reads "owner"
  And GitHub was asked once for tidepool's authority file
```

**RA-2 (table over what Jeff imports and tries to sign; Jeff is linked)** — `jeff_imports_anything_public_but_signing_what_he_cannot_attest_is_refused_with_a_hand_off`

```gherkin
@driving_port @real-io @error @security @kpi-oos-1 @adr-086-10 @oos-d12 @c-oos-7 @s-oos-4 @s-oos-1 @contract-shape:bounded-change
Scenario Outline: Jeff imports anything public, but signing what he cannot attest is refused with a hand-off
  Given Jeff linked "jeffbailey"
  And Rachel's signed claim says her repo rachelk/kelp is memory-safe, and Jeff recorded kelp's contributors (rachelk)
  When Jeff runs <import> and asks to sign candidate 1
  Then the candidates are still listed and nothing is signed or published
  And he is told "<reason>", who can sign it, and the command they would run
  Examples:
    | import                              | reason                                         | who can sign                                     |
    | scrape github acme-tools/anvil      | is owned by the organization                   | a DID listed in .github/openlore-authority       |
    | scrape github rachelk/kelp          | is not owned by your linked GitHub account     | rachelk after linking, or a listed DID           |
    | scrape person rachelk               | you have no identity link to                   | rachelk after linking                            |
```

**RA-3 (table over the authority file on acme-tools/anvil; Jeff has NO** — `an_organization_repo_is_signed_only_by_a_did_its_authority_file_lists_exactly`

```gherkin
@driving_port @real-io @security @c6a @adr-087-3 @kpi-oos-4 @s-oos-4 @contract-shape:bounded-change
Scenario Outline: An organization repo is signed only by an identity its authority file lists exactly
  Given acme-tools/anvil is owned by the organization acme-tools
  And its main branch holds <file>
  When Jeff imports acme-tools/anvil and signs candidate 1
  Then <outcome>
  Examples:
    | file                                                                    | outcome                                           |
    | an authority file listing Jeff's identity                               | signed and published                              |
    | an authority file listing Jeff's identity in capital letters            | refused: the organization has not listed him      |
    | an authority file listing an identity that merely starts like Jeff's    | refused: the organization has not listed him      |
    | an authority file where Jeff's identity is commented out                | refused: the organization has not listed him      |
    | a shortcut to another file, in place of the authority file              | refused: not a usable authority file              |
    | a link to another project, in place of the authority file               | refused: not a usable authority file              |
    | a folder instead of a file                                              | refused: not a usable authority file              |
    | an authority file of exactly 4 KiB that lists Jeff's identity           | signed and published                              |
    | an authority file over 4 KiB that lists Jeff's identity                 | refused: not a usable authority file              |
    | an authority file listing Jeff's identity that isn't plain text         | refused: not a usable authority file              |
    | Jeff's identity listed only in a file on a branch other than the main one | refused: the organization has not listed him    |
```

**RA-4** — `rechecking_authority_labels_each_peers_repo_claims_and_records_one_observation_per_repo`

```gherkin
@driving_port @real-io @adr-087-6 @c-oos-3 @c-oos-4 @kpi-oos-4 @s-oos-5 @contract-shape:bounded-change
Scenario: Re-checking authority labels each peer's repo claims from one look at each repo
  Given acme-tools/anvil's authority file lists Rachel's identity
  And Rachel published claims about acme-tools/anvil and jeffbailey/tidepool, and Jeff pulled both
  When Jeff re-checks authority
  Then her anvil claim reads "repo-listed" and her tidepool claim "third-party"
  And GitHub was asked once for each repo and once for each repo's authority file
  And nothing reached his published repo
```

**RA-5 (table over how GitHub fails; chained on RA-4's Given)** — `when_github_cannot_answer_rechecking_records_nothing_and_changes_nothing`

```gherkin
@driving_port @real-io @infrastructure-failure @security @c7a @adr-087-4 @s-oos-5 @contract-shape:unbounded-preservation
Scenario Outline: When GitHub cannot answer, re-checking records nothing and changes nothing
  Given Rachel published a claim about acme-tools/anvil and Jeff pulled it
  And <trouble>
  When Jeff re-checks the authority of acme-tools/anvil
  Then he is told GitHub could not be asked
  And every claim about anvil and his linked identities read as before, and nothing reached his published repo
  Examples:
    | trouble                                                  |
    | GitHub says it is too busy to answer                     |
    | GitHub cannot be reached                                 |
    | GitHub sends him to another site when he asks about anvil |
```

**RA-6 (table; ADR-087 §6 as corrected by design a600ad3: when the budget** — `when_githubs_budget_runs_out_mid_import_the_candidates_are_listed_and_his_latest_check_decides`

```gherkin
@driving_port @real-io @infrastructure-failure @c7a @c7b @adr-087-6 @r-3 @s-oos-5 @contract-shape:bounded-change
Scenario Outline: When GitHub's budget runs out mid-import, the candidates are listed and his latest check decides
  Given Jeff linked "jeffbailey" on 1 October at 09:00
  And <earlier check>
  And GitHub's budget runs out after the harvest of jeffbailey/tidepool, before its authority is read
  When Jeff imports jeffbailey/tidepool and asks to sign candidate 1 on 3 October
  Then the candidates are listed
  And <outcome>
  Examples:
    | earlier check                                      | outcome                                                                                       |
    | Jeff has never re-checked tidepool's authority     | nothing is signed or published; he is told GitHub could not be asked, when to retry, and the re-check command |
    | Jeff re-checked tidepool's authority on 1 October  | the claim is signed and published, on his 1 October checks                                    |
```

**RA-7 (chained on RA-1)** — `after_jeff_transfers_his_repo_his_earlier_claim_reads_third_party_and_nothing_is_withdrawn`

```gherkin
@driving_port @real-io @adr-086-6 @oos-d6 @c-oos-5 @s-oos-5 @contract-shape:bounded-change
Scenario: After Jeff transfers his repo, his earlier claim reads third-party and nothing is withdrawn
  Given Jeff signed and published a claim about jeffbailey/tidepool while he owned it
  And Jeff then transferred tidepool to the organization acme-tools
  When Jeff re-checks authority
  Then his claim reads "third-party"
  And his claim is not retracted, deleted or re-signed, and nothing is published
```

**RA-8 (table over the old and the new repo name)** — `after_jeff_renames_his_repo_he_signs_only_under_the_name_github_serves_now`

```gherkin
@driving_port @real-io @edge @security @a-10 @adr-087-4 @adr-089-2 @s-oos-4 @contract-shape:bounded-change
Scenario Outline: After Jeff renames his repo, he signs only under the name GitHub serves now
  Given Jeff linked "jeffbailey" and renamed jeffbailey/tidepool to jeffbailey/tidepool-rs
  And Jeff re-checked the authority of <name>
  When Jeff signs and publishes a claim about <name>
  Then <outcome>
  Examples:
    | name                    | outcome                                                                 |
    | jeffbailey/tidepool-rs  | the claim is signed and reaches his published repo                      |
    | jeffbailey/tidepool     | nothing is signed; he is told GitHub serves this repo as jeffbailey/tidepool-rs |
```

**RA-9 (observation order, review H-2)** — `a_listed_identity_stays_listed_whoever_observed_the_repo_last`

```gherkin
@driving_port @real-io @security @h-2 @adr-087-4 @s-oos-5 @contract-shape:bounded-change
Scenario: A listed identity stays listed whoever observed the repo last
  Given acme-tools/anvil's authority file lists Rachel's identity, and Rachel's claim about it was pulled
  And Jeff re-checked authority, then imported acme-tools/anvil himself
  When Jeff re-checks authority again
  Then Rachel's claim still reads "repo-listed"
```

**RA-14 (C4b: re-check with nothing to re-check)** — `with_no_github_claims_in_his_store_rechecking_authority_asks_github_nothing`

```gherkin
@driving_port @real-io @edge @c3 @c4b @adr-087-6 @s-oos-5 @contract-shape:unbounded-preservation
Scenario: With no GitHub claims in his store, re-checking authority asks GitHub nothing
  Given Jeff's store holds no claim about a GitHub account or repo
  When Jeff re-checks authority
  Then he is told there is nothing to re-check
  And GitHub is not asked anything, and nothing in his store changed
```

**RA-15 (C6b: the repo GitHub does not have)** — `a_repo_github_does_not_have_is_recorded_as_not_found_and_cannot_be_signed`

```gherkin
@driving_port @real-io @error @adr-087-4 @c6b @s-oos-3 @contract-shape:bounded-change
Scenario: A repo GitHub does not have is recorded as not found and cannot be signed
  Given Jeff re-checked the authority of acme-tools/vanished, which GitHub does not have
  And the network is off
  When Jeff signs a claim about acme-tools/vanished
  Then nothing is signed; he is told it was not found on GitHub
```

**RA-10 (table over the checks behind an offline repo signature; review** — `with_the_network_off_jeff_signs_a_repo_claim_only_from_fresh_checks`

```gherkin
@driving_port @real-io @boundary @c1b @i-9 @c-oos-9 @security @s-oos-3 @contract-shape:bounded-change
Scenario Outline: With the network off, Jeff signs a repo claim only from fresh checks
  Given <checks>
  And the network is off
  When Jeff signs a claim about <repo> at <time>
  Then <outcome>
  And GitHub is not asked anything
  Examples:
    | checks                                                                                    | repo                | time            | outcome                                             |
    | Jeff linked "jeffbailey" and re-checked tidepool's authority on 1 October                 | jeffbailey/tidepool | 3 October 09:00 | signed and kept locally                             |
    | anvil's authority file lists Jeff; Jeff re-checked anvil on 1 October (no identity link)  | acme-tools/anvil    | 3 October 09:00 | signed and kept locally                             |
    | anvil's authority file lists Jeff; Jeff re-checked anvil on 1 October (no identity link)  | acme-tools/anvil    | 8 October 09:00 | signed and kept locally (exactly 7 days)            |
    | anvil's authority file lists Jeff; Jeff re-checked anvil on 1 October (no identity link)  | acme-tools/anvil    | 8 October 09:01 | refused: checked over 7 days ago; names the re-check |
    | Jeff's link was checked on 1 October; he imported tidepool on 3 October                   | jeffbailey/tidepool | 8 October 09:01 | refused: the older check is over 7 days old         |
    | Jeff linked "jeffbailey" but tidepool was never observed                                  | jeffbailey/tidepool | 3 October 09:00 | refused: authority has not been checked             |
```

**RA-11a (table: online and network off; review Forge 5)** — `a_claim_jeff_signed_about_someone_elses_repo_before_owner_only_signing_is_never_published`

```gherkin
@driving_port @real-io @error @security @kpi-oos-1 @oos-d3 @adr-086-4 @i-9 @s-oos-6 @contract-shape:unbounded-preservation
Scenario Outline: A claim Jeff signed about someone else's repo before owner-only signing is never published
  Given Jeff's store was made before owner-only signing and holds his unpublished claim about BurntSushi/ripgrep
  When Jeff publishes that claim <how>
  Then nothing reaches his published repo
  And he is told the authority of BurntSushi/ripgrep has not been checked, with the command to check it
  Examples:
    | how                  |
    | with the network on  |
    | with the network off |
```

**RA-11b (table over a stored claim's history; chained on RA-1)** — `publishing_a_stored_claim_later_asks_for_authority_only_if_it_was_never_published`

```gherkin
@driving_port @real-io @c4a @kpi-oos-1 @adr-086-4 @s-oos-6 @contract-shape:bounded-change
Scenario Outline: Publishing a stored claim later asks for authority only if it was never published
  Given Jeff linked "jeffbailey" and re-checked tidepool's authority on 1 October
  And <history>
  When Jeff publishes that claim on 8 October
  Then <outcome>
  Examples:
    | history                                                                    | outcome                                                     |
    | he signed a claim about tidepool without publishing it                     | it reaches his published repo                               |
    | he signed it without publishing, then transferred tidepool and re-checked  | refused: the organization has not listed him; nothing published |
    | he signed and published it, then transferred tidepool and re-checked       | succeeds again, and his published repo is unchanged         |
```

**RA-12 (upgrade; ruling R1: CLI behaviour is the primary evidence)** — `opening_a_store_made_before_owner_only_signing_keeps_everything_in_it`

```gherkin
@driving_port @real-io @c4a @adr-087-5 @s-oos-6 @contract-shape:unbounded-preservation
Scenario: Opening a store made before owner-only signing keeps everything in it
  Given Jeff's store was made before identity links and owner-only signing
  When Jeff reads every claim about memory-safety, twice
  Then both reads list his claim about BurntSushi/ripgrep and Rachel's about rust-lang/regex
  And the second read is identical to the first
```

**RA-12b (upgrade from the frozen v7 store; review Forge 1, ruling R5)** — `jeffs_identity_link_checks_carry_over_when_his_store_gains_owner_only_signing`

```gherkin
@driving_port @real-io @c4a @adr-087-5 @i-9 @s-oos-6 @s-oos-3 @contract-shape:bounded-change
Scenario Outline: Jeff's identity-link checks carry over when his store gains owner-only signing
  Given Jeff's store was made after identity links but before owner-only signing
  And it holds his link to "jeffbailey", first checked on 20 September and re-checked on 1 October at 09:00
  When Jeff <acts>, with the network off
  Then <outcome>
  Examples:
    | acts                                                          | outcome                                                                   |
    | lists his linked identities on 3 October                      | they read exactly as the previous version printed them                    |
    | signs "jeffbailey adheres to memory-safety" on 3 October       | the claim is signed and kept locally (the 1 October check is 2 days old)  |
    | signs "jeffbailey adheres to memory-safety" on 8 October 09:01 | nothing is signed; he is told to re-check authority, with the command     |
```

**RA-13 (table over Jeff's own link at signing time; ruling R3 adds the** — `signing_from_an_import_rechecks_jeffs_own_link_first`

```gherkin
@driving_port @real-io @n-2 @adr-087-6 @security @c7a @r-3 @s-oos-4 @contract-shape:bounded-change
Scenario Outline: Signing from an import re-checks Jeff's own link first
  Given Jeff linked "jeffbailey" on 1 October at 09:00 and has not re-checked it since
  And <github>
  When Jeff imports jeffbailey/tidepool and signs candidate 1 on <day>
  Then GitHub was asked for jeffbailey's profile before signing
  And <outcome>
  Examples:
    | github                                                   | day        | outcome                                                                  |
    | the bio still names Jeff's identity                      | 21 October | the claim is signed and published                                        |
    | the bio no longer names Jeff's identity                  | 21 October | nothing is signed; he is told his identity link is not verified          |
    | GitHub says it is too busy to answer for his profile     | 3 October  | the claim is signed and published, on his 1 October check                |
    | GitHub cannot be reached for his profile                 | 3 October  | the claim is signed and published, on his 1 October check                |
    | GitHub says it is too busy to answer for his profile     | 21 October | nothing is signed; he is told to re-check authority, with the command    |
```

**RA-16 (table over GitHub's budget at the authority read; review Forge 2,** — `when_githubs_budget_is_gone_rechecking_says_so_records_nothing_and_never_shows_his_token`

```gherkin
@driving_port @real-io @infrastructure-failure @security @c7a @adr-087-6 @r-4 @s-oos-5 @contract-shape:unbounded-preservation
Scenario Outline: When GitHub's budget is gone, re-checking says so, records nothing and never shows his token
  Given Rachel published a claim about acme-tools/anvil and Jeff pulled it
  And <budget>
  When Jeff re-checks the authority of acme-tools/anvil
  Then he is told GitHub could not be asked
  And every claim about anvil and his linked identities read as before, and nothing reached his published repo
  And no line he sees shows his token
  Examples:
    | budget                                                                  |
    | he has no token and GitHub's budget for him runs out as it reads anvil   |
    | he has a token and its budget runs out as GitHub reads anvil             |
    | he has a token that GitHub no longer accepts                            |
```

**IC-1** — `jeff_reviews_what_he_imported_offline_one_repo_at_a_time`

```gherkin
@driving_port @real-io @oos-d12 @c-oos-7 @a-11 @s-oos-9 @contract-shape:unbounded-preservation
Scenario: Jeff reviews what he imported, offline, one repo at a time
  Given Jeff imported acme-tools/anvil and rachelk/kelp
  And the network is off
  When Jeff lists his imported candidates for acme-tools/anvil
  Then he sees anvil's dependency-pinning candidate and nothing about kelp
  And GitHub is not asked anything
```

**IC-2 (guardrail; review F-01)** — `importing_a_repo_keeps_its_candidates_locally_and_nowhere_else`

```gherkin
@driving_port @real-io @guardrail @c-oos-7 @kpi-oos-2 @s-oos-9 @contract-shape:bounded-change
Scenario: Importing a repo keeps its candidates locally and nowhere else
  Given anvil's authority file lists Rachel, Rachel claims anvil pins its dependencies, and Jeff re-checked anvil
  And Jeff's weighted view of dependency-pinning weighs anvil
  When Jeff imports acme-tools/anvil
  Then his imported candidates list anvil's dependency-pinning candidate
  And nothing reached his published repo, and he signed nothing about anvil
  And his weighted view of dependency-pinning is unchanged
```

**IC-3 (empty)** — `with_nothing_imported_the_list_says_so`

```gherkin
@driving_port @real-io @edge @c1a @s-oos-9 @contract-shape:unbounded-preservation
Scenario: With nothing imported, the list says so
  Given Jeff has imported nothing
  When Jeff lists his imported candidates
  Then he is told there are no imported candidates, and the command succeeds
  And what he sees about acme-tools/anvil is unchanged
```

**IC-4 (apply twice — ruling P-OOS-3, accepted 2026-10-20, resolves U-OOS-6)** — `importing_the_same_repo_twice_lists_each_candidate_once`

```gherkin
@driving_port @real-io @c4a @p-oos-3 @s-oos-9 @contract-shape:unbounded-preservation
Scenario: Importing the same repo twice lists each candidate once, as last imported
  Given Jeff imported acme-tools/anvil on 1 October and again on 3 October
  When Jeff lists his imported candidates
  Then anvil's dependency-pinning candidate is listed once, from the 3 October import
```

**IC-5 (review PO F-05; ACTIVE guard)** — `imported_candidates_never_feed_jeffs_inferred_people`

```gherkin
@driving_port @real-io @guardrail @c-oos-7 @a-11 @s-oos-9 @contract-shape:unbounded-preservation
Scenario: Imported candidates never feed Jeff's inferred people
  Given Rachel's signed claim says acme-tools/anvil is memory-safe, and Jeff recorded anvil's contributors, BurntSushi first
  And Jeff imported acme-tools/anvil, whose own candidate says it pins its dependencies
  When Jeff lists his inferred people
  Then BurntSushi is proposed for memory-safety, from Rachel's signed claim
  And no one is proposed for dependency-pinning
```

**IC-6 (addendum 2026-10-23, user decision D35; resolves U-OOS-D35)** — `reading_a_person_keeps_the_candidates_it_suggests_each_listed_once` — `#[ignore]`, DELIVER pending. RED today: `candidates list` prints "No imported candidates." after `scrape person jeffbailey` (row "jeff reads himself", 0 lines listed, 1 expected).

```gherkin
@driving_port @driving_adapter @real-io @c-oos-7 @p-oos-3 @oos-d35 @s-oos-9 @contract-shape:bounded-change
Scenario Outline: Reading a person keeps the candidates it suggests, each listed once
  Given <person> owns <repo>, which pins its dependencies
  When Jeff reads <person> as a person on 1 October
  Then his imported candidates list <repo>'s dependency-pinning candidate once, unsigned
  And nothing reached his published repo, and he signed nothing about <repo>
  When Jeff reads <person> as a person again on 3 October
  Then <repo>'s dependency-pinning candidate is still listed once, from the 3 October read
  And no one is proposed for dependency-pinning among his inferred people
  Examples:
    | person     | repo                |
    | jeffbailey | jeffbailey/tidepool |
    | rachelk    | rachelk/kelp        |
```

### `owner_only_signing_readers.rs`

**RD-1** — `every_claim_about_a_repo_is_shown_with_whose_word_it_is`

```gherkin
@driving_port @real-io @adr-086-8 @oos-d9 @c-oos-5 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: Every claim about a repo is shown with whose word it is
  Given Jeff signed a claim about his repo tidepool
  And acme-tools/anvil's authority file lists Rachel, who claims anvil, tidepool and rachelk/kelp
  And Jeff re-checked tidepool and anvil, but not kelp
  When Jeff reads every claim about memory-safety
  Then his own tidepool claim reads "owner", Rachel's anvil claim "repo-listed",
       Rachel's tidepool claim "third-party" and her kelp claim "authority not checked"
  And no claim is hidden
```

**RD-2 (chained on RD-1)** — `the_weighted_view_counts_only_attested_claims_and_says_what_it_left_out`

```gherkin
@driving_port @real-io @kpi-oos-2 @c-oos-6 @oos-d9 @adr-022 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: The weighted view counts only attested claims and says what it left out
  Given the store of RD-1
  When Jeff asks for the weighted view of memory-safety, explaining tidepool's weight
  Then tidepool and anvil are weighted, and kelp is not
  And tidepool's weight is built from Jeff's claim alone
  And he is told "2 claims not counted: 1 third-party, 1 authority not checked" with the command to re-check
```

**RD-3 (chained on RD-2)** — `jeff_can_see_the_claims_left_out_per_author_without_changing_the_weight`

```gherkin
@driving_port @real-io @kpi-oos-2 @c-oos-6 @oos-d9 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: Jeff can see the claims left out, per author, without changing the weight
  Given the store of RD-1, and the weighted view of memory-safety it gives
  When Jeff asks for the weighted view of memory-safety including unattested claims
  Then the weighted rows are exactly the same
  And a separate "Unattested claims, by author" section lists Rachel's tidepool and kelp claims under her name, and not her anvil claim
```

**RD-4 (table over what is added beside an attested claim; KPI-OOS-2, A-13)** — `nothing_but_an_attested_claim_moves_the_weight`

```gherkin
@driving_port @real-io @kpi-oos-2 @kpi-gdl-2 @a-13 @uc-16 @uc-20 @security @c-oos-6 @s-oos-7 @contract-shape:unbounded-preservation
Scenario Outline: Nothing but an attested claim moves the weight
  Given anvil's authority file lists Jeff, and Jeff re-checked anvil and signed and published "anvil is memory-safe"
  And Jeff noted the weighted view of memory-safety
  And Rachel published <addition>, Jeff pulled it and re-checked authority
  When Jeff asks for the weighted view of memory-safety
  Then it is exactly as Jeff noted it
  Examples:
    | addition                                                                 |
    | a dispute of Jeff's claim                                                |
    | a claim about anvil marked as disputing a claim nobody has (spoofed)     |
    | a claim about anvil, and her own withdrawal of it                        |
    | a link from her identity to her GitHub account                           |
    | a claim about anvil, which its authority file does not list her for     |
    | a withdrawal of Jeff's claim (only its author may withdraw it)           |
```

**RD-5 (chained on RD-4 row 1)** — `a_claim_marked_as_a_dispute_is_shown_as_a_dispute`

```gherkin
@driving_port @real-io @security @uc-16 @adr-086-2 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: A claim marked as a dispute is shown as a dispute, not as someone's word about the repo
  Given anvil's authority file lists Jeff, and Jeff re-checked anvil and signed and published "anvil is memory-safe"
  And Rachel published a claim about anvil marked as disputing Jeff's claim, and Jeff pulled it
  When Jeff reads every claim about acme-tools/anvil
  Then Rachel's claim is shown as disputing Jeff's claim
  And it carries no authority label
```

**RD-6 (chained on RD-1)** — `jeffs_viewer_weighs_and_labels_claims_exactly_as_his_terminal_does`

```gherkin
@driving_port @real-io @kpi-oos-2 @adr-086-8 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: Jeff's viewer weighs and labels claims exactly as his terminal does
  Given the store of RD-1
  When Jeff opens Rachel's score page and tidepool's project page in his viewer
  Then Rachel's page scores anvil and neither kelp nor tidepool, as the terminal's weighted view of her claims does
  And tidepool's page labels Jeff's claim "owner" and Rachel's "third-party"
```

**RD-7 (table: no local check vs a local check)** — `network_search_labels_every_github_claim_using_jeffs_own_check_when_he_has_one`

```gherkin
@driving_port @real-io @security @uc-7 @oos-d10 @c-oos-8 @s-oos-7 @contract-shape:unbounded-preservation
Scenario Outline: Network search labels every GitHub claim, using Jeff's own check when he has one
  Given the network index holds <claim>
  And <local>
  When Jeff searches the network for <subject>
  Then the row reads "<label>"
  Examples:
    | claim                                     | local                                                        | subject                 | label                       |
    | Sven's claim about rust-lang/regex        | Jeff has no check of rust-lang/regex                         | rust-lang/regex         | authority not checked here  |
    | Priya's claim about acme-tools/anvil      | anvil's file lists Priya and Jeff re-checked anvil           | acme-tools/anvil        | repo-listed                 |
```

**RD-8** — `claims_made_before_owner_only_signing_are_judged_by_the_same_rule`

```gherkin
@driving_port @real-io @oos-d3 @adr-086-7 @a-7 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: Claims made before owner-only signing are judged by the same rule, with no cut-off date
  Given Jeff's store was made before owner-only signing, with his claim about BurntSushi/ripgrep and Rachel's pulled claim about rust-lang/regex
  And Jeff re-checked authority
  When Jeff reads every claim about memory-safety
  Then both claims are still shown, and both read "third-party"
```

**RD-9** — `inferred_people_show_the_authority_of_each_repo_claim_they_rest_on`

```gherkin
@driving_port @real-io @uc-10 @adr-086-8 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: Inferred people show the authority of each repo claim they rest on
  Given anvil's authority file lists Rachel, Rachel claims anvil is memory-safe, and Jeff re-checked anvil
  And Jeff recorded anvil's contributors, BurntSushi first
  When Jeff lists his inferred people
  Then BurntSushi's candidate names Rachel's anvil claim as "repo-listed"
  And nothing in his store or published repo changed
```

**RD-10 (reader side of G-2; review F-02; chained on PS-8)** — `after_jeff_renames_his_account_his_claim_about_the_old_login_reads_third_party`

```gherkin
@driving_port @real-io @edge @security @g-2 @a-14 @kpi-oos-2 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: After Jeff renames his GitHub account, his claim about the old login reads third-party and stays published
  Given Jeff linked "jeffbailey" and signed and published "jeffbailey adheres to memory-safety"
  And Jeff then renamed the account to "jeff-b" and re-checked his link
  When Jeff reads every claim about memory-safety
  Then his claim about github:jeffbailey is still shown and reads "third-party"
  And nothing was withdrawn from his published repo
```

**RD-11 (C5a mode combination; ruling P-OOS-1 accepted 2026-10-20)** — `asking_for_unattested_claims_without_the_weighted_view_is_a_usage_mistake`

```gherkin
@driving_port @real-io @error @c5a @p-oos-1 @s-oos-7 @contract-shape:unbounded-preservation
Scenario: Asking for unattested claims without the weighted view is a usage mistake
  Given the store of RD-1
  When Jeff reads every claim about memory-safety including unattested claims, without asking for the weighted view
  Then he is told the unattested section needs the weighted view, and the command fails as a usage mistake
  And no claim is listed
```

### `owner_only_signing_review_app.rs`

**SA-1** — `priyas_share_post_names_only_claims_about_repos_she_still_owns`

```gherkin
@driving_port @real-io @uc-21 @adr-086-8 @p-oos-2 @s-oos-8 @contract-shape:unbounded-preservation
Scenario: Priya's share post names only claims about repos she still owns
  Given Priya published "tidepool is test-driven" and "kelp pins its dependencies"
  And she then transferred kelp to tidal-labs
  When she opens the share preview, which looks again at her bio and at who owns each repo
  Then the draft names test-driven and not dependency-pinning
  And the page lists kelp's claim as "not counted: authority not proven"
  And nothing has been posted
```

## 7. Step reuse (Mandate-12 criterion 4, informational)

Rust adaptation: invocations of `support/authority.rs` public functions in the three CLI scenario files ÷ distinct functions invoked = **411 / 67 = 6.13×** (plus the reused GDL `identity_links` and `people` steps, not counted). Natural ceiling; not a gate.
