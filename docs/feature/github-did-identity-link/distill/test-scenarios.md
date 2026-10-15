# Acceptance Test Design — github-did-identity-link (DISTILL)

- **Wave**: DISTILL · **Date**: 2026-10-14 (final revision pass after iteration 2 and the user's rulings) · **Designer**: Quinn (nw-acceptance-designer)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (ADR-007)
- **Language**: Rust (`[lang-mode] rust`) · skip marker `#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]` · proptest at layer 2 only
- **Framework**: Rust std `#[test]` + subprocess, Gherkin in doc-comments (house convention: `infer_people_sign.rs`, `review_app_*.rs`)
- **Inherits**: DESIGN only (GDL-D1..D14, A-1..A-10, C-GDL-1..10, ADR-084/085, revised by ADR-086/087). No DISCUSS (AC derived from DESIGN), no DEVOPS (default environment).
- **Walking-skeleton strategy C**: REAL `openlore` binary, `adapter-github` over HTTP, `adapter-duckdb`, filesystem (and the REAL review app / indexer / viewer where a scenario drives them); FAKE (driven-external only) `FakeGithubAccounts`, `FakePds`, `FakeIdentity`, `PeerPds`, `FakeAtprotoNetwork`, `FakeIngestServer`.
- **Hand-off state, CI policy (D-d) — the single statement of it**: every scenario is `#[ignore]`d, **the walking skeleton included** (`#[ignore = "DISTILL github-did-identity-link: pending DELIVER; un-ignore with the first green step"]`), because a red test on trunk blocks every image job. Three scenarios that already hold today run as real regression guards: **IR-13**, **IR-17** (added under the same rule — it was found passing in the re-run) and **RA-3**. `walking-skeleton.md`, `red-classification.md` and `wave-decisions.md` refer here.

The `.rs` files are the scenario SSOT; this is the map.

## 1. Inputs read

| Input | Status |
|---|---|
| `docs/product/architecture/brief.md` (`## For Acceptance Designer`) | + |
| `docs/product/kpi-contracts.yaml` (KPI-GDL-1..4) | + |
| `docs/product/journeys/` (author-and-publish-claim, infer-contributor-philosophy, scrape-propose-sign) | + (context only) |
| `design/wave-decisions.md`, `design/architecture-design.md`, `design/upstream-changes.md` | + |
| ADR-084, ADR-085; ADR-086/087 where ADR-085 cites them | + |
| `discuss/`, `devops/` | - (not found) |

## 2. Test files

| File (`tests/acceptance/`) | Slice | Layer | Scenarios | Active today |
|---|---|---|---|---|
| `identity_link.rs` | 1 — link / verify / forget / composer guard / upgrade / re-link (incl. WS) | 3/4 subprocess | 32 (+1 fixture regenerator, not a scenario) | 0 |
| `identity_link_readers.rs` | 2 — association rule, person views, scoring, search label | 3/4 subprocess + viewer + indexer | 19 | 2 (IR-13, IR-17) |
| `identity_link_core.rs` | 1-2 pure core | 2 proptest (256 cases) | 10 | 0 |
| `review_app_identity_link.rs` | 3 — publish / retract in the review app | 4 HTTP | 9 | 1 (RA-3) |
| `review_app_identity_link_readers.rs` | 3 — the CLI reads app-written links | 4 HTTP + subprocess | 5 | 0 |
| **Total** | | | **75** | **3** |

Targets: four in `crates/cli/Cargo.toml`, `review_app_identity_link` in `crates/openlore-review-app/Cargo.toml`.
Vocabulary: `support/identity_links.rs`, `support/review_app/identity_link.rs`; `support/people.rs` `Invocation` gains `github_token`.
Double (test-support only): `FakeGithubAccounts` — `GET /user/{id}`, `OldLogin::{NotFound, Redirects, RedirectsToAnotherHost}`, `rename`, `delete_account`, `register`, `serve_stale_profile`/`end_stale_profile`, `request_paths`; 4 unit tests.

**Mix (75, informational only — not a gate)**: walking skeleton 1 · happy 15 · error 20 · edge 18 · boundary 1 · guardrail 10 · layer-2 property/catalogue 10. Non-happy = 49 / 65 example scenarios (75 %). Counted by the primary category in each doc comment's first line.

## 3. Traceability (H-5) — scenario → decision, with its elevator pitch

### identity_link.rs — slice 1 `@sizing-review-needed` (informational, Sentinel M-11: 30 scenarios)

| ID | Elevator pitch (test name, in words) | Traces to |
|---|---|---|
| WS-GDL-1 | Jeff's GitHub account is publicly linked to his DID and shows as verified | GDL-D1/D2/D4/D6/D10, A-1, C-GDL-1, I-8, KPI-GDL-1 |
| IL-2 | Jeff sees exactly what a link would publish before anything is written | GDL-D12(3), ADR-085 §7 plan-returning, I-7 |
| IL-3 | No link is made unless the bio names Jeff's exact DID (7 rows) | GDL-D4, ADR-076 §4, C-GDL-2 |
| IL-4 | Jeff's DID proves the account wherever it stands in his bio (4 rows) | ADR-076 §4 |
| IL-5 | Linking an account Jeff already linked is refused and points to re-checking | D-a (user-ratified) |
| IL-6 | When GitHub cannot be read, nothing is linked and nothing is recorded (3 rows) | ADR-085 §4, Earned Trust |
| IL-7 | A malformed GitHub username is refused before GitHub is asked (4 rows) | ADR-076 `is_github_login` |
| IL-8a | A hand-written "sameAs" claim is refused and Jeff is pointed to the link command | ADR-084 §4, N-1, C-GDL-2 |
| IL-8b | Editing an inferred claim into "sameAs" is refused at signing | ADR-084 §4, N-1 |
| IL-8c | Jeff can publicly dispute Rachel's link, and his dispute is never read as a link (wire: author ≠ subject) | D-b (user-ratified), OOS-D1, GDL-D3 |
| IL-9a | When the host of Jeff's published repo is unreachable, his link is kept signed and verified locally | single publish path (WS-10), ADR-085 §7 |
| IL-9b | A link kept locally reaches Jeff's published repo when he publishes it later | same |
| IL-10 | Jeff links a second GitHub account and both show as his | A-4, F1 |
| IL-11 | Linking and re-checking leave the contributors Jeff recorded untouched (observed in BurntSushi's person view) | UC-7, OD-CPI-2 |
| IL-12 | The identity verbs never repeat Jeff's bio or log his identity (3 rows) | GDL-D12(4), F2 |
| IL-13a | Opening a store made before identity links keeps everything already in it (frozen v6 fixture; v7 recorded once) | GDL-D10, DD-GDL-12 |
| IL-13b | Reopening an upgraded store changes nothing | GDL-D10 (idempotent) |
| IL-14 | After retracting his link, Jeff can link the same account again | D-g |
| IV-1 | Once Jeff's bio stops naming his DID, his link shows the proof is gone and stays published | GDL-D5, C-GDL-6, KPI-GDL-3 |
| IV-2 | When Jeff's DID is back, one re-check from one GitHub request restores the link | GDL-D10 insert-only, ≤ 2 requests |
| IV-3a | A renamed account stays verified under its new name | GDL-D9, A-10 |
| IV-3b | After a rename is recognised, the next re-check reads the new name with one request | GDL-D9 |
| IV-4 | Jeff stays verified under his new GitHub name, found from his old name alone | N-2 |
| IV-5 | The account number tells a re-registered login and a deleted account apart (3 rows) | GDL-D9, UC-10 |
| IV-6 | A GitHub failure during a re-check changes nothing (3 rows) | ADR-085 §4 |
| IV-7 | Re-checking can be narrowed to one person or one account (4 rows) | ADR-085 driving ports |
| IV-8 | With no links, a re-check says so and asks GitHub nothing | C3 |
| IV-9 | One re-check covers both of Jeff's GitHub accounts | A-4, F1 |
| IV-10 | An answer from GitHub that OpenLore will not follow leaves Jeff's link as it was (2 rows) | N-2 (≤ 1 same-host redirect), F8 |
| IF-1a | Forgetting Rachel erases only her proof history | C-GDL-5, M-4 |
| IF-1b | Forgetting Rachel a second time forgets nothing more | C-GDL-5 |
| IF-2 | Forgetting without a history or a valid DID changes nothing (3 rows) | C-GDL-5 |

### identity_link_readers.rs — slice 2 `@sizing-review-needed` (informational, Sentinel M-11: 19 scenarios)

| ID | Elevator pitch | Traces to |
|---|---|---|
| IR-1 | Rachel's link reads as her claim until Jeff checks it himself | GDL-D11, GDL-D14 |
| IR-2 | Once Jeff has checked Rachel's link himself, it shows as her GitHub account | GDL-D14, KPI-GDL-4 analog |
| IR-3 | A stranger's unchecked link to Jeff's account never displaces Jeff | GDL-D14, H-2 |
| IR-4 | A stranger's disproved link to Jeff's account never displaces Jeff | GDL-D14, H-2 |
| IR-5 | An account whose bio names two people who both linked it reads as contested | GDL-D14, A-4 |
| IR-6 | Rachel cannot link Tobias's GitHub account to Tobias | GDL-D3, C-GDL-1 |
| IR-7 | A link record with a flaw never links anyone (5 rows) | ADR-084 §3, N-4 |
| IR-8a | A link Jeff retracted no longer shows among his identities | GDL-D5, ADR-060 |
| IR-8b | A link Jeff retracted is no longer re-checked | GDL-D5 |
| IR-9 | Each link line says how old its check is, with a re-check hint after 30 days (5 rows) | N-3, ADR-085 §5 |
| IR-10 | Two link records for the same person and account read as one link naming both | GDL-D9 |
| IR-18 | Jeff can list one person's linked identities | `identity list --did`, F7 |
| IR-11 | The person view shows a verified link even with the network off | C-GDL-7 |
| IR-12 | Reading a person shows their link has lost its proof, at no extra GitHub cost | UC-8, KPI-GDL-3 |
| IR-13 | Reading a person whose bio names a DID never surfaces or suggests a link | GDL-D12(2), A-2 |
| IR-17 | A possible rename among inferred people never becomes a link | A-2, GDL-D12, F3 |
| IR-14 | A link never changes weighted results in the terminal | GDL-D8, ADR-022, KPI-GDL-2 |
| IR-15 | A link never changes a contributor's score page | GDL-D8, KPI-GDL-2 |
| IR-16 | Network search lists a link as "authority not checked here" | GDL-D11, C-GDL-8, D-c |

### identity_link_core.rs (layer 2)

| ID | Property | Binding | Traces to |
|---|---|---|---|
| CORE-1 / 1x | the bio proves a DID only by its exact token (generator + 9-row lie catalogue) | `todo!()` | ADR-076 §4, ADR-085 §1 |
| CORE-2 | a check decides over ≤ 2 reads and records nothing when GitHub fails; closed verdict set | `todo!()` | ADR-085 §4, UC-10 |
| CORE-3 | only the owner's well-formed, provenance-passed record projects (11 mutations × 4 provenances) | `todo!()` | ADR-084 §3, M-2, N-4 |
| CORE-4 | the production encoder's wire record → REAL decode/canonicalize/CID (deterministic) → project | `todo!()` encoder (returns the record JSON); real decoder | ADR-084 Earned Trust |
| CORE-5 / 5b | only this reader's sole Verified check associates; a squatter never changes another row | `todo!()` | ADR-085 §5, GDL-D14 |
| CORE-6 / 6x | `score(F ∪ sameAs) == score(F)` | REAL `scoring::score` | ADR-085 §6, KPI-GDL-2 |
| CORE-7 | reserved iff the predicate is exactly `sameAs` | `todo!()` | ADR-084 §4 |

### review_app_identity_link.rs

| ID | Elevator pitch | Traces to |
|---|---|---|
| RA-1a | Priya sees exactly what her link will publish before anything is written | GDL-D6, GDL-D12(3) |
| RA-1b | Priya's verified GitHub account is publicly linked to her DID from the app | GDL-D6, C-GDL-10, ADR-073, F2 |
| RA-2 | A link is never published on the strength of an old verification | C-GDL-10, ADR-085 §2 |
| RA-3 | Verifying GitHub and scanning never publish a link on their own | A-2 |
| RA-4 | Priya withdraws her link without erasing it | GDL-D5, C-GDL-6 |
| RA-5 | When GitHub cannot be read while publishing a link, nothing is published (2 rows) | ADR-085 §4 |
| RA-6 | A link confirmation that did not come from Priya's own page is refused (2 rows) | ADR-076 session/CSRF |
| RA-7 | Leaving the link preview publishes nothing and keeps the offer open | plan-returning preview |
| RA-8 | An ended session at confirmation publishes nothing and asks Priya to sign in again (3 rows: server-side 30-day expiry via clock skew, cookie cleared, repo-host session) | ADR-073 |

### review_app_identity_link_readers.rs

| ID | Elevator pitch | Traces to |
|---|---|---|
| RR-1a | Priya's app-published link reaches Maria as Priya's unchecked claim | ADR-084 Earned Trust, GDL-D11 |
| RR-1b | Once Maria checks it herself, Priya's link shows as Priya's GitHub account | KPI-GDL-4 analog |
| RR-2a | Priya can publish her link in the minute after removing her DID, before GitHub shows the change | ADR-085 §2 accepted limitation |
| RR-2b | Maria sees that a link published just before GitHub showed Priya's bio change has lost its proof | KPI-GDL-3, C-GDL-6 |
| RR-3 | A link of Priya's found in Sam's repository never links Priya | ADR-085 §5 `project(&claim, &Provenance)`, M-4 |

### Constraints and assumptions → scenarios (Sentinel M-8)

| Item | Coverage |
|---|---|
| A-1 the job (others know my GitHub work is me; nobody else can say so) | WS-GDL-1 (I can), IR-6 (no third party), IR-3/IR-4 (squatter cannot) |
| A-2 opt-in, owner only, never inferred | IR-13, IR-17, RA-3 |
| A-3 CLI first, review app second | sequencing, not behaviour: slices 1-2 are CLI (WS-GDL-1 …), slice 3 is the review app (RA-*) |
| A-4 one DID, several accounts; one account, several DIDs = contested | IL-10, IV-9; IR-5 |
| A-5 DID stays in the bio; removing it shows "proof gone" | IV-1, IR-12, RR-2b |
| A-6 display-only association | IR-14, IR-15, CORE-6 |
| A-7 `https://api.github.com/user/<id>` is the stable id URI | WS-GDL-1 and IL-4 (`assert_link_on_pds` evidence), CORE-3/CORE-4 |
| A-8 confidence fixed at 10000 bp | WS-GDL-1 / IL-9b / RA-1b (wire confidence 10000). "Not editable" is structural: `identity link` has no confidence flag or prompt (clap surface) |
| A-9 public, permanent; retraction withdraws, does not erase | IL-2, RA-1a (preview wording), RA-4 (record kept after retraction) |
| A-10 renames / re-registration | IV-3a, IV-3b, IV-4, IV-5, IV-10 |
| C-GDL-3 links keyed by numeric id; logins display-only | IV-3a (same id, new login stays verified), IV-5 (re-registered login ≠ my link), IR-10 (dedup by DID + id) |
| C-GDL-4 association display-only, needs this reader's own Verified check, scorer drops `sameAs` | IR-1/IR-2, IR-3/IR-4, IR-5, IR-14, IR-15, CORE-5, CORE-6 |
| C-GDL-9 read-only verbs hold no write / purge / signing handle | structural (xtask `identity_link_readers_hold_no_write_handle`, DELIVER); behavioural evidence: IR-1 (`identity list` asks GitHub nothing) and IL-13b (reopening changes nothing) |
| GDL-D7, GDL-D13 | structural (crate placement, `check-arch` rules) — DELIVER |

## 4. Driving-adapter coverage

| Driving port | Scenario(s) |
|---|---|
| `identity link github <login> [--yes]` | WS, IL-2..IL-12, IL-14 |
| `identity verify [--did \| --github]` | IV-1..IV-10, IR-2, IR-4, RR-1b, RR-2b |
| `identity list [--did]` | WS, IR-1..IR-18 (`--did`: IR-18) |
| `identity forget --did` | IF-1a, IF-1b, IF-2 |
| `claim retract` | IR-8a/b |
| `claim add`, `claim counter`, `infer people --sign` | IL-8a, IL-8c, IL-8b |
| `infer people` | IR-11, IR-17 |
| `scrape github <user>` | IR-12, IR-13 |
| `graph query --weighted` / viewer `/score` | IR-14 / IR-15 |
| `search` | IR-16 |
| review app "Publish this link" / "Retract this link" | RA-1a..RA-8, RR-2a/b |

## 5. Adapter coverage (Mandate 6)

| Driven adapter | Real-I/O scenario |
|---|---|
| `adapter-github` `read_person`, `read_person_by_id`, redirect policy, token | IV-3a (id read), IV-4 (redirect), IV-10 (chained / cross-host), IL-6 / IV-6 (401, anonymous rate limit, outage), IR-12 |
| `adapter-duckdb` v7 checks (read / insert / purge), migration | WS, IV-1/2, IF-1a, IL-13a/b (upgrade from the frozen v6 store) |
| single publish path → PDS | WS, IL-9a/b |
| `peer pull` (app-signed and self-attested links) | IR-1..IR-10, RR-1a, RR-3 |
| review app `UserRepoWritePort` | RA-1b, RA-4, RA-8 |
| indexer ingest + `search` | IR-16 |
| GitHub REST recorded-fixture contract | owned by DELIVER, slice 1 (see wave-decisions) — not an acceptance test |

## 6. Environment waivers (H-6)

The default DEVOPS matrix (clean / with-pre-commit / with-stale-config) describes install-time states; the identity verbs install nothing, run no hooks and read no config file, so those three are **N/A** for this feature. The environments that do change behaviour are covered instead:

| Platform environment | Scenario(s) |
|---|---|
| fresh store (clean) | WS-GDL-1 |
| populated pre-v7 store (upgrade) | IL-13a, IL-13b (frozen fixture `tests/fixtures/identity-link-pre-v7-store/`) |
| network disabled | IR-11, `identity list` in every scenario (no PDS wired) |
| GitHub anonymous rate limit / rejected token / unreachable | IL-6, IV-6, RA-5 |
| GitHub chained or cross-host redirect | IV-10 |
| PDS unreachable | IL-9a |
| app session expired (server-side and cookie) / repo-host session expired | RA-8 |
