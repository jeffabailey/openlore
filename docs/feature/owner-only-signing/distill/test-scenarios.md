# Acceptance Test Design — owner-only-signing (DISTILL)

- **Wave**: DISTILL · **Date**: 2026-10-20 · **Designer**: Quinn (nw-acceptance-designer)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (ADR-007)
- **Language**: Rust (`[lang-mode] rust`) · `[policy-mode] inherit` (one section appended) · `[port-mode] inherit` (`tests/common/state_delta.rs`)
- **Framework**: Rust std `#[test]` + subprocess, Gherkin in doc comments (house convention: `identity_link*.rs`, `infer_people_sign.rs`, `review_app_*.rs`); proptest at layer 2 only.
- **Skip marker**: `#[ignore = "DISTILL owner-only-signing <ID>: pending DELIVER …"]` on every scenario, the walking skeleton included (house rule D-d: a red test on trunk blocks every image job). The default run is green.
- **Inherits**: DESIGN only (OOS-D1..D15, A-1..A-14, C-OOS-1..9, ADR-086/087), reconciled against the shipped ADR-088/089 (see `wave-decisions.md` § Upstream Issues). No DISCUSS (stories derived below), no DEVOPS (project CI acceptance stage).

The `.rs` files are the scenario SSOT; this is the map.

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
| ⊘ | `docs/feature/owner-only-signing/discuss/` | **missing** — WARN: stories derived from DESIGN (§3) |
| ⊘ | `docs/feature/owner-only-signing/devops/` | **missing** — WARN: project CI acceptance stage, default environment |

## 2. Test files

| File (`tests/acceptance/`) | Slice | Layer | Scenario fns (rows) | Registered in |
|---|---|---|---|---|
| `owner_only_signing.rs` | 1 — person gate, WS, exemptions (PS-3 ACTIVE guard), offline window, inference | 3 subprocess | 8 (17) | `crates/cli/Cargo.toml` |
| `owner_only_signing_repos.rs` | 2 — repo authority, `authority verify`, `scrape --sign`, `claim publish`, upgrade; 5 — imported candidates | 3 subprocess | 16 + 4 (37 + 4) | `crates/cli/Cargo.toml` |
| `owner_only_signing_readers.rs` | 3 — labels, `score_attested`, `--include-unattested`, viewer, search | 3/4 subprocess + viewer + indexer | 10 (16) | `crates/cli/Cargo.toml` |
| `owner_only_signing_core.rs` | 1-3 pure core | 2 proptest (256 cases) | 5 properties + 1 example | `crates/cli/Cargo.toml` |
| `owner_only_signing_review_app.rs` | 4 remainder — share draft (UC-21) | 4 HTTP | 1 (1) | `crates/openlore-review-app/Cargo.toml` |
| **Total** | | | **45 test fns** (39 example scenarios, 76 example rows; 6 layer-2) | |

Vocabulary: `support/authority.rs` (NEW; typed domain module, wording pins, store universe), reusing `support/identity_links.rs` (`LinkWorld`, link steps) and `support/people.rs` (spawn, scrape, infer).
Double (test-support, additive): `FakeGithubAccounts` gains `AuthorityFile` (`Lists`, `Symlink`, `Submodule`, `Directory`, `Oversize`, `NotUtf8`, `OnlyOnAnotherBranch`) served at `/repos/{o}/{r}/contents/.github/openlore-authority` (base64, 60-column wrap, keyed by the repo's current path), `redirect_repo_to_another_host`, `exhaust_budget_at_authority_reads`, oracle `authority_file_reads_of`; one unit test.

**Mix** (primary category of each example scenario fn; rule: non-happy = error/refusal, boundary, edge, infrastructure-failure, guardrail/security): walking skeleton 1 · happy 11 · error/refusal 8 · boundary 3 · edge 10 · infrastructure-failure 2 · guardrail/security 4. **Non-happy: 27 / 39 fns (69 %)**; strict refusal-row share (rows whose Then is a refusal or "nothing written") ≈ 33 / 76 rows (43 %), review F-08. `@security` on 18 fns (forged/spoofed counter, cross-host redirect, symlink/submodule/directory/oversize/non-UTF-8 file, prefix-colliding and uppercase DIDs, offline window, renamed login).

## 3. Stories derived from DESIGN (no DISCUSS wave)

| Story | From | Scenarios |
|---|---|---|
| S-OOS-1 Only the owner of a GitHub account signs about it; others get the hand-off | A-1, A-2, OOS-D4, C-OOS-1, KPI-OOS-1, slice 1 | WS-OOS-1, PS-2, PS-5, PS-6, PS-7, PS-8, RA-2 (person row), CORE-OOS-2/3/4 |
| S-OOS-2 Anyone can always dispute; the author can always withdraw | OOS-D11, C-OOS-2, KPI-OOS-3 | PS-3, CORE-OOS-1/2 |
| S-OOS-3 Signing works offline from a check ≤ 7 days old, never otherwise | OOS-D7, C-OOS-9, I-9 exception, A-6 | PS-4, RA-10, RA-15, CORE-OOS-4/4x |
| S-OOS-4 Only the repo's owner or a DID its authority file lists signs about the repo; anyone imports | A-3, A-12, OOS-D5, ADR-087 §2-3, KPI-OOS-4 | RA-1, RA-2, RA-3, RA-8, RA-13, CORE-OOS-3 |
| S-OOS-5 Anyone re-checks authority from public data; failures change nothing | OOS-D6, ADR-087 §4-6, C-OOS-3/4 | RA-4, RA-5, RA-6, RA-7, RA-9, RA-14 |
| S-OOS-6 A never-published claim publishes only with a permit | OOS-D8, ADR-086 §4, OOS-D3 | RA-11a, RA-11b, RA-12 |
| S-OOS-7 Readers label every `github:` assertion and weigh only attested ones | OOS-D9/D10, C-OOS-5/6/8, KPI-OOS-2, UC-7/11/16/20 | RD-1..RD-10, CORE-OOS-5 |
| S-OOS-8 The review app's share draft quotes only claims whose authority holds | UC-21, ADR-086 §8 | SA-1 |
| S-OOS-9 Imported candidates are kept locally, never federated, never weighed | A-11, OOS-D12 B, C-OOS-7 | IC-1..IC-4 |

## 4. Traceability — scenario → decision (elevator pitch = test name in words)

| ID | Elevator pitch | Traces to |
|---|---|---|
| WS-OOS-1 | Only Jeff can sign what is said about Jeff's GitHub account (2 rows) | OOS-D4, C-OOS-1, KPI-OOS-1, slice 1 |
| PS-2 | A person claim Jeff cannot attest is refused, says why and who can sign (3 rows) | ADR-087 §1, ADR-086 §10 |
| PS-3 | Jeff can always dispute or withdraw, whatever authority he holds (2 rows) | OOS-D11, KPI-OOS-3 |
| PS-4 | With the network off, Jeff signs about himself only from a check at most 7 days old (4 rows) | ADR-086 §5, I-9, A-6 |
| PS-5 | Jeff signs an inferred adherence only about himself (2 rows) | UC-9, A-8 |
| PS-6 | Jeff's inferred people say who can sign each candidate, and listing writes nothing | ADR-086 §10, CPI D-1 |
| PS-7 | Jeff still signs about his account when its bio also names another identity | UC-2 |
| PS-8 | After Jeff renames his GitHub account, he signs only under the new login (2 rows) | ADR-087 §1, G-2, A-14 |
| RA-1 | Jeff imports his own repo and signs what it shows | ADR-087 §2, KPI-OOS-4 |
| RA-2 | Jeff imports anything public, but signing what he cannot attest is refused with a hand-off (3 rows) | ADR-086 §10, OOS-D12 |
| RA-3 | An organization repo is signed only by a DID its authority file lists exactly (11 rows) | ADR-087 §3, lie catalogue |
| RA-4 | Re-checking authority labels each peer's repo claims and records one observation per repo | ADR-087 §4/§6 |
| RA-5 | When GitHub cannot answer, re-checking records nothing and changes nothing (3 rows) | ADR-087 §4 redirect policy |
| RA-6 | When GitHub's budget runs out mid-import, the candidates are listed and signing waits for the reset | ADR-087 §6 budget |
| RA-7 | After Jeff transfers his repo, his earlier claim reads third-party and nothing is withdrawn | ADR-086 §6, C-OOS-5 |
| RA-8 | After Jeff renames his repo, he signs only under the name GitHub serves now (2 rows) | A-10, ADR-089 §2 (`RepoRenamed`) |
| RA-9 | A listed identity stays listed whoever observed the repo last | review H-2 |
| RA-10 | With the network off, Jeff signs a repo claim only from fresh checks (4 rows) | ADR-086 §5 (older check) |
| RA-11a | A claim Jeff signed about someone else's repo before owner-only signing is never published | OOS-D3, OOS-D8 |
| RA-11b | Publishing a stored claim later asks for authority only if it was never published (3 rows) | ADR-086 §4 (`published_at IS NULL`) |
| RA-12 | Opening a store made before owner-only signing keeps everything in it | ADR-087 §5 (v8) |
| RA-13 | Signing from an import re-checks Jeff's own link first (2 rows) | ADR-087 §6, N-2 |
| RA-14 | With no GitHub claims in his store, re-checking authority asks GitHub nothing | C3/C4b |
| RA-15 | A repo GitHub does not have is recorded as not found and cannot be signed | ADR-087 §4 (404) |
| IC-1 | Jeff reviews what he imported, offline, one repo at a time | OOS-D12 B |
| IC-2 | Importing a repo keeps its candidates locally and nowhere else | C-OOS-7 |
| IC-3 | With nothing imported, the list says so | C1a |
| IC-4 | Importing the same repo twice lists each candidate once (DISTILL proposal, U-OOS-6) | C4a |
| RD-1 | Every claim about a repo is shown with whose word it is | ADR-086 §8 |
| RD-2 | The weighted view counts only attested claims and says what it left out | OOS-D9, KPI-OOS-2 |
| RD-3 | Jeff can see the claims left out, per author, without changing the weight | OOS-D9 |
| RD-4 | Nothing but an attested claim moves the weight (6 rows incl. a foreign withdrawal; row 1 confirms A-13) | KPI-OOS-2, UC-16/20 |
| RD-5 | A claim marked as a dispute is shown as a dispute, not as someone's word about the repo | ADR-086 §2 |
| RD-6 | Jeff's viewer weighs and labels claims exactly as his terminal does | KPI-OOS-2 (viewer) |
| RD-7 | Network search labels every GitHub claim, using Jeff's own check when he has one (2 rows) | UC-7, OOS-D10, C-OOS-8 |
| RD-8 | Claims made before owner-only signing are judged by the same rule, with no cut-off date | OOS-D3, ADR-086 §7 |
| RD-9 | Inferred people show the authority of each repo claim they rest on | UC-10 |
| RD-10 | After Jeff renames his account, his claim about the old login reads third-party and stays published | G-2, A-14 (review F-02) |
| SA-1 | Priya's share post names only claims about repos she still owns | UC-21 |

### Layer 2 (`owner_only_signing_core.rs`)

| ID | Property | Binding | Traces to |
|---|---|---|---|
| CORE-OOS-1 | only disputes, withdrawals and links are weightless kinds; single marker → exact kind | `todo!()` → `authority::kind` | ADR-086 §2 |
| CORE-OOS-2 | only GitHub assertions need authority; malformed `github:` never exempt | `todo!()` → `authority::requirement` | ADR-086 §1, C-OOS-1 |
| CORE-OOS-3 | attested exactly when the ADR-087 table says so (exact DID token incl. uppercase / prefix / punctuation lies; org never Owner; latest check + current login) | `todo!()` → `authority::classify` | ADR-087 §1-3 |
| CORE-OOS-4 / 4x | permit iff exempt or freshly attested (older check ≥ 0 and ≤ 7 days); edges 0 / 7 d / 7 d + 1 min / −1 min by example | `todo!()` → CLI gate over local checks | ADR-086 §5, A-6 |
| CORE-OOS-5 | `score_attested == score(eligible)`; unchanged by added counters (incl. spoofed), retractions, links, unattested assertions | `todo!()` → `score_attested`; oracle uses the REAL `scoring::score` | KPI-OOS-2 |

## 5. Adapter and driving-adapter coverage

| Adapter / port | @real-io scenario |
|---|---|
| `openlore` CLI — `claim add`, `claim counter`, `claim retract`, `claim publish` | WS-OOS-1, PS-2/3/4/7/8, RA-8/10/11a/11b/15 |
| `openlore` CLI — `scrape github <owner/repo> --sign`, `scrape person --sign` | RA-1, RA-2, RA-3, RA-6, RA-13 |
| `openlore` CLI — `infer people [--sign]` | PS-5, PS-6, RD-9 |
| `openlore` CLI — `authority verify [--subject]` (NEW) | RA-4, RA-5, RA-7, RA-9, RA-14, RA-15 |
| `openlore` CLI — `candidates list [--subject]` (NEW, slice 5) | IC-1, IC-3, IC-4 |
| `openlore` CLI — `graph query [--weighted] [--include-unattested] [--explain]`, `search` | RD-1..RD-5, RD-7, RD-8 |
| viewer `/score`, `/project` | RD-6 |
| review app share preview | SA-1 |
| `adapter-github` `read_repo_authority` (+ redirect policy) | RA-1, RA-3, RA-4, RA-5 (cross-host), RA-8 (same-host rename) |
| `adapter-duckdb` v8 `repo_authority_checks` (read + insert-only write) | RA-1, RA-4, RA-6, RA-10, RA-12 |
| `adapter-duckdb` v8 `imported_candidates` | IC-1..IC-4 |
| `adapter-duckdb` v7 `identity_link_checks` (read; write via `--sign` refresh) | PS-4, RA-13 |
| single publish path (`FakePds`) | WS-OOS-1, RA-1, RA-11b |

The REQUIRED recorded-fixture contract test of the GitHub `/repos` + contents shapes (ADR-087 Earned Trust fixtures) is an adapter test owned by DELIVER, not acceptance (U-OOS-8).

## 6. Scenarios (Gherkin, extracted from the doc comments)

### `owner_only_signing.rs`

**WS-OOS-1** — `only_jeff_can_sign_what_is_said_about_jeffs_github_account`

```gherkin
@walking_skeleton @driving_port @driving_adapter @real-io @security @kpi-oos-1
@oos-d4 @c-oos-1 @contract-shape:bounded-change
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
@driving_port @real-io @error @security @kpi-oos-1 @c6b @contract-shape:unbounded-preservation
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

**PS-3 (table over the two exempt kinds; chained on WS-OOS-1)** — `jeff_can_always_dispute_or_withdraw_whatever_authority_he_holds`

```gherkin
@driving_port @real-io @kpi-oos-3 @c-oos-2 @oos-d11 @guardrail @contract-shape:bounded-change
Scenario Outline: Jeff can always dispute or withdraw, whatever authority he holds
  Given <situation>
  When Jeff <acts>
  Then the dispute or withdrawal is signed and reaches his published repo
  Examples:
    | situation                                                                       | acts                                                     |
    | Rachel published "rachelk adheres to memory-safety" and Jeff pulled it          | disputes Rachel's claim about her own account            |
    | Jeff signed and published about "jeffbailey", then his bio stopped naming him   | withdraws his own claim about "jeffbailey"               |
```

**PS-4 (boundary table over the age of Jeff's latest link check)** — `with_the_network_off_jeff_signs_about_himself_only_from_a_check_at_most_7_days_old`

```gherkin
@driving_port @real-io @boundary @c1b @i-9 @c-oos-9 @oos-d7 @security @contract-shape:bounded-change
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

**PS-5 (table over whose adherence Jeff signs)** — `jeff_signs_an_inferred_adherence_only_about_himself`

```gherkin
@driving_port @real-io @security @kpi-oos-1 @uc-9 @a-8 @contract-shape:bounded-change
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
```

**PS-6 (chained on PS-5's Given)** — `jeffs_inferred_people_say_who_can_sign_each_candidate_and_listing_writes_nothing`

```gherkin
@driving_port @real-io @kpi-oos-1 @adr-086-10 @contract-shape:unbounded-preservation
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
@driving_port @real-io @edge @security @uc-2 @adr-087-1 @contract-shape:bounded-change
Scenario: Jeff still signs about his account when its bio also names another identity
  Given the "jeffbailey" bio names both Jeff's identity and Rachel's
  And Jeff and Rachel both published a link to "jeffbailey", and Jeff re-checked every link in his store (both verify)
  When Jeff signs and publishes "jeffbailey adheres to memory-safety"
  Then the claim is signed and reaches his published repo
```

**PS-8 (table over the old and the new login)** — `after_jeff_renames_his_github_account_he_signs_only_under_the_new_login`

```gherkin
@driving_port @real-io @edge @g-2 @a-14 @adr-087-1 @security @contract-shape:bounded-change
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
@driving_port @driving_adapter @real-io @kpi-oos-1 @kpi-oos-4 @adr-087-2 @oos-d5 @contract-shape:bounded-change
Scenario: Jeff imports his own repo and signs what it shows
  Given Jeff linked "jeffbailey", the account that owns jeffbailey/tidepool
  When Jeff imports jeffbailey/tidepool and signs candidate 1
  Then the claim is signed and reaches his published repo
  And the repo's authority was observed once, as owned by a user with no authority file
```

**RA-2 (table over what Jeff imports and tries to sign; Jeff is linked)** — `jeff_imports_anything_public_but_signing_what_he_cannot_attest_is_refused_with_a_hand_off`

```gherkin
@driving_port @real-io @error @security @kpi-oos-1 @adr-086-10 @oos-d12 @c-oos-7 @contract-shape:bounded-change
Scenario Outline: Jeff imports anything public, but signing what he cannot attest is refused with a hand-off
  Given Jeff linked "jeffbailey"
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
@driving_port @real-io @security @c6a @adr-087-3 @kpi-oos-4 @contract-shape:bounded-change
Scenario Outline: An organization repo is signed only by a DID its authority file lists exactly
  Given acme-tools/anvil is owned by the organization acme-tools
  And its default branch holds <file>
  When Jeff imports acme-tools/anvil and signs candidate 1
  Then <outcome>
  Examples:
    | file                                                           | outcome                                          |
    | an authority file listing Jeff's identity                      | signed and published                             |
    | an authority file listing Jeff's identity in capitals          | refused: is owned by the organization            |
    | an authority file listing "did:plc:test-jeffrey"               | refused: is owned by the organization            |
    | an authority file where Jeff's identity is commented out       | refused: is owned by the organization            |
    | a symlink named like the authority file                        | refused: is not a valid authority file           |
    | a submodule named like the authority file                      | refused: is not a valid authority file           |
    | a directory named like the authority file                      | refused: is not a valid authority file           |
    | an authority file of exactly 4 KiB that lists Jeff's identity  | signed and published                             |
    | an authority file over 4 KiB that lists Jeff's identity        | refused: is not a valid authority file           |
    | an authority file listing Jeff's identity that is not plain text | refused: is not a valid authority file           |
    | Jeff's identity listed only on another branch                  | refused: is owned by the organization            |
```

**RA-4** — `rechecking_authority_labels_each_peers_repo_claims_and_records_one_observation_per_repo`

```gherkin
@driving_port @real-io @adr-087-6 @c-oos-3 @c-oos-4 @kpi-oos-4 @contract-shape:bounded-change
Scenario: Re-checking authority labels each peer's repo claims and records one observation per repo
  Given acme-tools/anvil's authority file lists Rachel's identity
  And Rachel published claims about acme-tools/anvil and jeffbailey/tidepool, and Jeff pulled both
  When Jeff re-checks authority
  Then her anvil claim reads "repo-listed" and her tidepool claim "third-party"
  And exactly one observation of each repo was recorded, from one repo read and one authority-file read each
  And nothing else in his store or published repo changed
```

**RA-5 (table over how GitHub fails; chained on RA-4's Given)** — `when_github_cannot_answer_rechecking_records_nothing_and_changes_nothing`

```gherkin
@driving_port @real-io @infrastructure-failure @security @c7a @adr-087-4 @contract-shape:unbounded-preservation
Scenario Outline: When GitHub cannot answer, re-checking records nothing and changes nothing
  Given Rachel published a claim about acme-tools/anvil and Jeff pulled it
  And <trouble>
  When Jeff re-checks the authority of acme-tools/anvil
  Then he is told GitHub could not be asked
  And nothing in his store or published repo changed
  Examples:
    | trouble                                                       |
    | GitHub says it is too busy to answer                          |
    | GitHub cannot be reached                                      |
    | GitHub answers the repo read by sending him to another site   |
```

**RA-6** — `when_githubs_budget_runs_out_mid_import_the_candidates_are_listed_and_signing_waits_for_the_reset`

```gherkin
@driving_port @real-io @infrastructure-failure @c7a @adr-087-6 @contract-shape:bounded-change
Scenario: When GitHub's budget runs out mid-import, the candidates are listed and signing waits for the reset
  Given Jeff linked "jeffbailey"
  And GitHub's budget runs out after the harvest of jeffbailey/tidepool, before its authority is read
  When Jeff imports jeffbailey/tidepool and asks to sign candidate 1
  Then the candidates are listed
  And nothing is signed or published, and no observation of tidepool is recorded
  And he is told GitHub could not be asked and when to retry
```

**RA-7 (chained on RA-1)** — `after_jeff_transfers_his_repo_his_earlier_claim_reads_third_party_and_nothing_is_withdrawn`

```gherkin
@driving_port @real-io @adr-086-6 @oos-d6 @c-oos-5 @contract-shape:bounded-change
Scenario: After Jeff transfers his repo, his earlier claim reads third-party and nothing is withdrawn
  Given Jeff signed and published a claim about jeffbailey/tidepool while he owned it
  And Jeff then transferred tidepool to the organization acme-tools
  When Jeff re-checks authority
  Then his claim reads "third-party"
  And his claim is not retracted, deleted or re-signed, and nothing is published
```

**RA-8 (table over the old and the new repo name)** — `after_jeff_renames_his_repo_he_signs_only_under_the_name_github_serves_now`

```gherkin
@driving_port @real-io @edge @security @a-10 @adr-087-4 @adr-089-2 @contract-shape:bounded-change
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
@driving_port @real-io @security @h-2 @adr-087-4 @contract-shape:pure-function
Scenario: A listed identity stays listed whoever observed the repo last
  Given acme-tools/anvil's authority file lists Rachel's identity, and Rachel's claim about it was pulled
  And Jeff re-checked authority, then imported acme-tools/anvil himself
  When Jeff re-checks authority again
  Then Rachel's claim still reads "repo-listed"
```

**RA-14 (C4b: re-check with nothing to re-check)** — `with_no_github_claims_in_his_store_rechecking_authority_asks_github_nothing`

```gherkin
@driving_port @real-io @edge @c3 @c4b @adr-087-6 @contract-shape:unbounded-preservation
Scenario: With no GitHub claims in his store, re-checking authority asks GitHub nothing
  Given Jeff's store holds no claim about a GitHub account or repo
  When Jeff re-checks authority
  Then he is told there is nothing to re-check
  And GitHub is not asked anything, and nothing in his store changed
```

**RA-15 (C6b: the repo GitHub does not have)** — `a_repo_github_does_not_have_is_recorded_as_not_found_and_cannot_be_signed`

```gherkin
@driving_port @real-io @error @adr-087-4 @c6b @contract-shape:bounded-change
Scenario: A repo GitHub does not have is recorded as not found and cannot be signed
  Given Jeff re-checked the authority of acme-tools/vanished, which GitHub does not have
  And the network is off
  When Jeff signs a claim about acme-tools/vanished
  Then nothing is signed; he is told it was not found on GitHub
```

**RA-10 (table over the checks behind an offline repo signature)** — `with_the_network_off_jeff_signs_a_repo_claim_only_from_fresh_checks`

```gherkin
@driving_port @real-io @boundary @c1b @i-9 @c-oos-9 @security @contract-shape:bounded-change
Scenario Outline: With the network off, Jeff signs a repo claim only from fresh checks
  Given <checks>
  And the network is off
  When Jeff signs a claim about <repo> at <time>
  Then <outcome>
  And GitHub is not asked anything
  Examples:
    | checks                                                                                      | repo                  | time        | outcome                                         |
    | Jeff linked "jeffbailey" and re-checked tidepool's authority on 1 October                   | jeffbailey/tidepool   | 3 October   | signed and kept locally                         |
    | anvil's authority file lists Jeff; Jeff re-checked anvil on 1 October (no identity link)    | acme-tools/anvil      | 3 October   | signed and kept locally                         |
    | Jeff's link was checked on 1 October; he imported tidepool on 3 October                     | jeffbailey/tidepool   | 8 Oct 09:01 | refused: the older check is over 7 days old     |
    | Jeff linked "jeffbailey" but tidepool was never observed                                    | jeffbailey/tidepool   | 3 October   | refused: authority has not been checked         |
```

**RA-11a** — `a_claim_jeff_signed_about_someone_elses_repo_before_owner_only_signing_is_never_published`

```gherkin
@driving_port @real-io @error @security @kpi-oos-1 @oos-d3 @adr-086-4 @contract-shape:unbounded-preservation
Scenario: A claim Jeff signed about someone else's repo before owner-only signing is never published
  Given Jeff's store was made before owner-only signing and holds his unpublished claim about BurntSushi/ripgrep
  When Jeff publishes that claim
  Then nothing reaches his published repo
  And he is told the authority of BurntSushi/ripgrep has not been checked, with the command to check it
```

**RA-11b (table over a stored claim's history; chained on RA-1)** — `publishing_a_stored_claim_later_asks_for_authority_only_if_it_was_never_published`

```gherkin
@driving_port @real-io @c4a @kpi-oos-1 @adr-086-4 @contract-shape:bounded-change
Scenario Outline: Publishing a stored claim later asks for authority only if it was never published
  Given Jeff linked "jeffbailey" and re-checked tidepool's authority on 1 October
  And <history>
  When Jeff publishes that claim on 8 October
  Then <outcome>
  Examples:
    | history                                                                        | outcome                                                |
    | he signed a claim about tidepool without publishing it                         | it reaches his published repo                          |
    | he signed it without publishing, then transferred tidepool and re-checked      | refused: is owned by the organization; nothing published |
    | he signed and published it, then transferred tidepool and re-checked           | succeeds again, and his published repo is unchanged    |
```

**RA-12 (upgrade)** — `opening_a_store_made_before_owner_only_signing_keeps_everything_in_it`

```gherkin
@driving_port @real-io @c4a @adr-087-5 @contract-shape:unbounded-preservation
Scenario: Opening a store made before owner-only signing keeps everything in it
  Given Jeff's store was made before identity links and owner-only signing
  When Jeff reads every claim about memory-safety, twice
  Then the store records the identity-link and repo-authority upgrades, once each
  And every claim, peer claim and contributor it held is unchanged
```

**RA-13 (table over Jeff's own link at signing time)** — `signing_from_an_import_rechecks_jeffs_own_link_first`

```gherkin
@driving_port @real-io @n-2 @adr-087-6 @security @contract-shape:bounded-change
Scenario Outline: Signing from an import re-checks Jeff's own link first
  Given Jeff linked "jeffbailey" on 1 October and has not re-checked it since
  And <bio>
  When Jeff imports jeffbailey/tidepool and signs candidate 1 on 21 October
  Then GitHub was asked for jeffbailey's profile before signing
  And <outcome>
  Examples:
    | bio                                     | outcome                                                       |
    | the bio still names Jeff's identity     | the claim is signed and published                             |
    | the bio no longer names Jeff's identity | nothing is signed; he is told his identity link is not verified |
```

**IC-1** — `jeff_reviews_what_he_imported_offline_one_repo_at_a_time`

```gherkin
@driving_port @real-io @oos-d12 @c-oos-7 @a-11 @contract-shape:pure-function
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
  Then his imported candidates grew
  And nothing reached his published repo, nothing was signed
  And his weighted view of dependency-pinning is unchanged
```

**IC-3 (empty)** — `with_nothing_imported_the_list_says_so`

```gherkin
@driving_port @real-io @edge @c1a @contract-shape:unbounded-preservation
Scenario: With nothing imported, the list says so
  Given Jeff has imported nothing
  When Jeff lists his imported candidates
  Then he is told there are no imported candidates, and the command succeeds
```

**IC-4 (apply twice — DISTILL proposal, upstream U-OOS-6)** — `importing_the_same_repo_twice_lists_each_candidate_once`

```gherkin
@driving_port @real-io @c4a @spec-ambiguity @contract-shape:pure-function
Scenario: Importing the same repo twice lists each candidate once
  Given Jeff imported acme-tools/anvil on 1 October and again on 3 October
  When Jeff lists his imported candidates
  Then anvil's dependency-pinning candidate is listed once
```


### `owner_only_signing_readers.rs`

**RD-1** — `every_claim_about_a_repo_is_shown_with_whose_word_it_is`

```gherkin
@driving_port @real-io @adr-086-8 @oos-d9 @c-oos-5 @contract-shape:pure-function
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
@driving_port @real-io @kpi-oos-2 @c-oos-6 @oos-d9 @adr-022 @contract-shape:unbounded-preservation
Scenario: The weighted view counts only attested claims and says what it left out
  Given the store of RD-1
  When Jeff asks for the weighted view of memory-safety, explaining tidepool's weight
  Then tidepool and anvil are weighted, and kelp is not
  And tidepool's weight is built from Jeff's claim alone
  And he is told "2 claims not counted: 1 third-party, 1 authority not checked" with the command to re-check
```

**RD-3 (chained on RD-2)** — `jeff_can_see_the_claims_left_out_per_author_without_changing_the_weight`

```gherkin
@driving_port @real-io @kpi-oos-2 @c-oos-6 @oos-d9 @contract-shape:unbounded-preservation
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
@driving_port @real-io @security @uc-16 @adr-086-2 @s-oos-7 @contract-shape:pure-function
Scenario: A claim marked as a dispute is shown as a dispute, not as someone's word about the repo
  Given anvil's authority file lists Jeff, and Jeff re-checked anvil and signed and published "anvil is memory-safe"
  And Rachel published a claim about anvil marked as disputing Jeff's claim, and Jeff pulled it
  When Jeff reads every claim about acme-tools/anvil
  Then Rachel's claim is shown as disputing Jeff's claim
  And it carries no authority label
```

**RD-6 (chained on RD-1)** — `jeffs_viewer_weighs_and_labels_claims_exactly_as_his_terminal_does`

```gherkin
@driving_port @real-io @kpi-oos-2 @adr-086-8 @contract-shape:unbounded-preservation
Scenario: Jeff's viewer weighs and labels claims exactly as his terminal does
  Given the store of RD-1
  When Jeff opens Rachel's score page and tidepool's project page in his viewer
  Then Rachel's page scores anvil and neither kelp nor tidepool, as the terminal's weighted view of her claims does
  And tidepool's page labels Jeff's claim "owner" and Rachel's "third-party"
```

**RD-7 (table: no local check vs a local check)** — `network_search_labels_every_github_claim_using_jeffs_own_check_when_he_has_one`

```gherkin
@driving_port @real-io @security @uc-7 @oos-d10 @c-oos-8 @contract-shape:pure-function
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
@driving_port @real-io @oos-d3 @adr-086-7 @a-7 @contract-shape:pure-function
Scenario: Claims made before owner-only signing are judged by the same rule, with no cut-off date
  Given Jeff's store was made before owner-only signing, with his claim about BurntSushi/ripgrep and Rachel's pulled claim about rust-lang/regex
  And Jeff re-checked authority
  When Jeff reads every claim about memory-safety
  Then both claims are still shown, and both read "third-party"
```

**RD-9** — `inferred_people_show_the_authority_of_each_repo_claim_they_rest_on`

```gherkin
@driving_port @real-io @uc-10 @adr-086-8 @contract-shape:unbounded-preservation
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


### `owner_only_signing_review_app.rs`

**SA-1** — `priyas_share_post_names_only_claims_about_repos_she_still_owns`

```gherkin
@driving_port @real-io @uc-21 @adr-086-8 @spec-ambiguity @contract-shape:pure-function
Scenario: Priya's share post names only claims about repos she still owns
  Given Priya published "tidepool is test-driven" and "kelp pins its dependencies"
  And she then transferred kelp to tidal-labs
  When she opens the share preview
  Then the draft names test-driven and not dependency-pinning
  And the page lists kelp's claim as "not counted: authority not proven"
  And nothing has been posted
```


## 7. Step reuse (Mandate-12 criterion 4, informational)

Rust adaptation: vocabulary-function invocations in the three CLI scenario files ÷ distinct `support/authority.rs` functions they call = **304 / 52 = 5.84×** (plus the reused GDL `identity_links` and `people` steps, not counted). Natural ceiling for a journey-rich, refusal-table-heavy feature; not a gate.
