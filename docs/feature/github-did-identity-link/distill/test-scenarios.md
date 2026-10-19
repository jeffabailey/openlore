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
| `identity_link.rs` | 1 — link / verify / forget / composer guard / upgrade / re-link (incl. WS) | 3/4 subprocess | 37 (+1 fixture regenerator, not a scenario; IL-12b, IL-15, IV-11..13 added 2026-10-16 for review D1/D4/D7) | 0 at DISTILL hand-off; the 5 review rows are active |
| `identity_link_readers.rs` | 2 — association rule, person views, scoring, search label, ADR-084 peer-pull round trip | 3/4 subprocess + viewer + indexer | 23 (IR-19 added 2026-10-16 for F14; IR-16b for U-7; IR-20/IR-20b 2026-10-17 for U-19) | 4 (IR-13, IR-17, IR-19, IR-16b); IR-20/IR-20b ignored pending DELIVER |
| `identity_link_core.rs` | 1-2 pure core | 2 proptest (256 cases) | 12 (CORE-2x, CORE-3x added for review D1) | 0 at hand-off; CORE-2x/3x active |
| `review_app_identity_link.rs` | 3 — publish / retract in the review app | 4 HTTP | 19 (RA-9..RA-14 and RA-5 rows 3-4 added 2026-10-17 for the slice-3 roadmap review; RA-4b and RA-12b added 2026-10-17 for the slice-3 re-review N2/N1) | 19 after slice-3 DELIVER (RA-12 active since the U-42 fix, RA-12b since the N1 fix) |
| `review_app_sign_in_and_ownership.rs` (review D1 rows only) | slice-1 fix — the ownership proof reads strictly | 4 HTTP | 2 (OW-4b, OW-4c) | 2 |
| `review_app_identity_link_readers.rs` | 3 — the CLI reads app-written links | 4 HTTP + subprocess | 5 | 0 |
| **Total** | | | **98** | **3 at hand-off (+9 review rows, IR-19 and IR-16b active)** |

Targets: four in `crates/cli/Cargo.toml`, `review_app_identity_link` in `crates/openlore-review-app/Cargo.toml`.
Vocabulary: `support/identity_links.rs`, `support/review_app/identity_link.rs`; `support/people.rs` `Invocation` gains `github_token`.
Double (test-support only): `FakeAtprotoNetwork` gains `WritePosture::StoreThenServerError` (RA-12 row 2; stores, then answers 500 until the posture changes). `FakeGithubAccounts` — `GET /user/{id}`, `OldLogin::{NotFound, Redirects, RedirectsToAnotherHost}`, `rename`, `delete_account`, `register`, `serve_stale_profile`/`end_stale_profile`, `request_paths`, `serve_flawed_profile(ProfileFlaw::{NoId, ZeroId, IdAsText, NoLogin})` (review D1); 5 unit tests.
Seeding (review D7): `given_a_check_recorded_seeing` inserts one `identity_link_checks` row directly — the only way to model a stored login that is not a GitHub login, which no verb can now write.

**Mix (75 at DISTILL hand-off, informational only — not a gate; the 9 review rows of 2026-10-16 are all error/guardrail)**: walking skeleton 1 · happy 15 · error 20 · edge 18 · boundary 1 · guardrail 10 · layer-2 property/catalogue 10. Non-happy = 49 / 65 example scenarios (75 %). Counted by the primary category in each doc comment's first line.

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
| IL-12 | The identity verbs never repeat Jeff's bio or log his identity (3 rows, success path) | GDL-D12(4), F2 |
| IL-12b | A refused or unreadable identity run never repeats Jeff's bio or logs his DID (5 rows: link refused for the bio, link/verify with GitHub unreachable, link/verify with an answer lacking the account number). The login MAY appear on stderr, but only in the one refusal line about the account he asked for (`… nothing recorded`) | review D4, GDL-D12(4), F2 |
| IL-13a | Opening a store made before identity links keeps everything already in it (frozen v6 fixture; v7 recorded once) | GDL-D10, DD-GDL-12 |
| IL-13b | Reopening an upgraded store changes nothing | GDL-D10 (idempotent) |
| IL-14 | After retracting his link, Jeff can link the same account again | D-g |
| IL-15 | When GitHub's answer names no account, nothing is linked and nothing is recorded (4 rows: no `id`, `id: 0`, `id` as text, no `login`; bio names the DID in every row) | review D1, ADR-085 §4 |
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
| IV-11 | A re-check whose answer names no account records nothing and asks no further (4 rows; only the login read, never `/user/{id}`; link still verified) | review D1, ADR-085 §4 |
| IV-12 | After a rename, an account-number answer that names no account is never read as a deleted account (2 rows: no `id`, no `login`; login read 404 → id read; nothing recorded, never `account_not_found`) | review D1, GDL-D9 |
| IV-13 | A stored username that is not a GitHub username is never sent to GitHub (2 rows; seeded check row via `given_a_check_recorded_seeing` — no verb can write one; the other link is still re-checked and recorded) | review D7, ADR-076 `is_github_login` |
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
| IR-8c | A link its peer author retracted no longer shows or is re-checked (was RED; green since the review fix) | slice-2 review **B2**, GDL-D5, ADR-060 D-RF-D3 |
| IR-8d | A link its peer author superseded no longer shows (was RED; green since the review fix) | slice-2 review **B2**, GDL-D9, DDD-7 |
| IR-20 | A link from a peer Jeff removed no longer shows or is re-checked: `identity list` shows no line, `identity verify` says there are none to verify, asks GitHub nothing and records nothing (RED, `#[ignore]` pending DELIVER) | **U-19**, slice-2 re-review F1 |
| IR-9 | Each link line says how old its check is, with a re-check hint after 30 days (5 rows) | N-3, ADR-085 §5 |
| IR-10 | Two link records for the same person and account read as one link naming both, at the end of the line as ` (records <id>, <id>)` in ascending order (`records_named`, U-13) | GDL-D9, U-13 |
| IR-18 | Jeff can list one person's linked identities | `identity list --did`, F7 |
| IR-18b | Listing one person never hides that their account is contested (was RED; green since the review fix) | slice-2 review **B1**, GDL-D14, A-4, F7 |
| IR-11 | The person view shows a verified link even with the network off | C-GDL-7 |
| IR-12 | Reading a person shows their link has lost its proof, at no extra GitHub cost | UC-8, KPI-GDL-3 |
| IR-12b | Reading a re-registered login records nothing (active: holds today) | slice-2 review **H2**(1), GDL-D9, A-10, UC-8 |
| IR-12c | Reading a renamed account refreshes its link once — written to the U-15 login scope: the rename is first seen by a re-check through GitHub's redirect (active: holds today) | slice-2 review **H2**(2), U-15, N-2, UC-8 |
| IR-20b | Reading a person shows no link from a peer Jeff removed, and records nothing (RED, `#[ignore]` pending DELIVER). Search's soft-removed cache (ADR-057) is out of this ruling and not asserted | **U-19**, UC-8 |
| IR-13 | Reading a person whose bio names a DID never surfaces or suggests a link | GDL-D12(2), A-2 |
| IR-17 | A possible rename among inferred people never becomes a link | A-2, GDL-D12, F3 |
| IR-14 | A link never changes weighted results in the terminal | GDL-D8, ADR-022, KPI-GDL-2 |
| IR-15 | A link never changes a contributor's score page; precondition: both pages weigh Rachel's memory-safety pairing, so the equality is never vacuous | GDL-D8, KPI-GDL-2, slice-2 review H1 |
| IR-16 | Network search lists a link as "authority not checked here", the label read inside Priya's own group | GDL-D11, C-GDL-8, D-c, slice-2 review L1 |
| IR-16b | Network search leaves a GitHub row that is not a link unlabelled (active guard: holds today and after 02-08). Slice 2 labels link rows only; widening `SEARCH_LABEL` to every `github:` row is a follow-up that would change this scenario | U-7, D-c, F11 |
| IR-19 | A self-attested link pulled from a peer keeps its record id and reads as a link (active: holds today) | ADR-084 Earned Trust, F14, U-11 |

### identity_link_core.rs (layer 2)

| ID | Property | Binding | Traces to |
|---|---|---|---|
| CORE-1 / 1x | the bio proves a DID only by its exact token (generator + 9-row lie catalogue) | `todo!()` | ADR-076 §4, ADR-085 §1 |
| CORE-2 / 2x | a check decides from its reads and records nothing when GitHub fails or answers naming no account (account 0 / invalid login, on the login read or the id read); closed verdict set. The read count is pinned at layer 3 (IV-2..IV-4, IV-11/12), not here (review D6). CORE-2x pins the no-account answers by example | `todo!()` | ADR-085 §4, UC-10, review D1 |
| CORE-3 / 3x | only the owner's well-formed, provenance-passed record projects (12 mutations × 5 provenances, incl. an id URI naming account 0 — review D1; CORE-3x pins `user/0` and `user/00` by example; every accepted provenance — incl. app-signed by another DID and self-attested elsewhere — reaches `project`, only `Failed` has no `Provenance` value) | `todo!()` | ADR-084 §3, M-2, N-4 |
| CORE-4 | the production encoder's wire record → REAL decode/canonicalize/CID (deterministic) → project | `todo!()` encoder (returns the record JSON); real decoder | ADR-084 Earned Trust |
| CORE-5 / 5b | only this reader's sole Verified check associates; a squatter never changes another row | `todo!()` | ADR-085 §5, GDL-D14 |
| CORE-6 / 6x | `score(F ∪ sameAs) == score(F)` | REAL `scoring::score` | ADR-085 §6, KPI-GDL-2 |
| CORE-7 (+ CORE-7x) | reserved iff the predicate is exactly `sameAs`; generator mixes the literal and its case/spacing near-misses, CORE-7x pins them by example | `todo!()` | ADR-084 §4 |

### review_app_identity_link.rs

| ID | Elevator pitch | Traces to |
|---|---|---|
| RA-1a | Priya sees exactly what her link will publish before anything is written | GDL-D6, GDL-D12(3) |
| RA-1b | Priya's verified GitHub account is publicly linked to her DID from the app | GDL-D6, C-GDL-10, ADR-073, F2 |
| RA-2 | A link is never published on the strength of an old verification | C-GDL-10, ADR-085 §2 |
| RA-3 | Verifying GitHub and scanning never publish a link on their own | A-2 |
| RA-4 | Priya withdraws her link without erasing it | GDL-D5, C-GDL-6 |
| RA-4b | A refused GitHub request keeps the offer to retract Priya's link (2 rows after her link is published: Verify for `priyaramn`, which has no account; a link preview naming the malformed `-priyaraman-`). The refusing page still offers "Retract this link" and the link is unchanged. Regression guard for F-4 (fixed in adbc648), active | slice-3 re-review **N2**, U-41 F-4 |
| RA-5 | When GitHub cannot be read while publishing a link, nothing is published (4 rows: rate limit, rejected token, this hour's checks used up, account not found; each ends "Nothing was published." and never shows the app's token) | ADR-085 §4, roadmap review F-10, U-31 |
| RA-6 | A link confirmation that did not come from Priya's own page is refused (2 rows) | ADR-076 session/CSRF |
| RA-7 | Leaving the link preview publishes nothing and keeps the offer open | plan-returning preview |
| RA-8 | An ended session at confirmation publishes nothing and asks Priya to sign in again (3 rows: server-side 30-day expiry via clock skew, cookie cleared, repo-host session) | ADR-073 |
| RA-9 | Only an owner the app has verified is offered "Publish this link" (5 rows: verified → offered; never verified, verify refused, verification lapsed, signed out → not offered) | roadmap review F-08, GDL-D12 |
| RA-10 | A link preview request that fails a guard asks GitHub nothing and keeps nothing (4 rows: no page token, forged page token → 403; this hour's checks used up → "Too many verification attempts in the last hour"; malformed username → "We couldn't find a GitHub account"). "Keeps nothing" is observed at the port: no confirm form is offered | roadmap review F-01, ADR-076 §2, U-26 |
| RA-10d | When GitHub cannot be read for a link preview, Priya is refused without seeing the app's token (3 rows: rate limit, rejected token, unreachable) | roadmap review F-01, ADR-085 §4 |
| RA-11a | Sam cannot publish Priya's GitHub account as his own (the offer form replayed naming `priyaraman`: no link shown or offered, nothing written) | roadmap review F-11, GDL-D3, C-GDL-1 |
| RA-11b | A bio naming someone else's DID is explained with that DID in full, and nothing is offered | roadmap review F-11, U-25, AC-002.3 |
| RA-12 | After a failed link write, Retry leaves exactly one link (2 rows: repo host answers 500; repo host stores the link but the answer is lost — `WritePosture::StoreThenServerError` in test-support, Retry's re-create answered "already exists" must read as success). Each row: the page says the link "may have been saved" and that Retry will confirm, never "Nothing was published." (U-42), plus Retry, then exactly one link, its address, status 200. Active since the U-42 wording fix | roadmap review F-08, ADR-071, **U-42** |
| RA-12b | When her repo host definitely did not take the link, Priya is told nothing was published (2 rows: the host refuses the link record with `400` — `WritePosture::RefuseCollection`; the host cannot be reached when she confirms — `set_host_reachable(.., false)` on the preview, since the confirm reads GitHub first and its first repo-host call is the write). Each row: "Nothing was published.", never "may have been saved", no link in her repo. Retry is deliberately not pinned. `#[ignore]` pending the N1 fix (today every write failure maps to `LinkNotPublished::NotWritten`) | slice-3 re-review **N1**, **U-42** |
| RA-13 | Priya's published link stays off her public profile and her share draft; her dependency-pinning claim still shows | **U-27** (user ruling: hide for now) |
| RA-14 | A link preview confirmed as an ordinary claim publish writes nothing and is used up (universe unchanged; the link confirm afterwards publishes nothing) | roadmap review F-03, **U-21 B** |

### review_app_sign_in_and_ownership.rs (review D1 rows; the ownership proof's home)

| ID | Elevator pitch | Traces to |
|---|---|---|
| OW-4b | A GitHub answer that names no account never proves ownership (4 rows; "GitHub could not be reached … Nothing was lost", not verified, no scan, no repo read) | review D1, ADR-076 |
| OW-4c | A scan stops before reading any repo when GitHub's answer names no account (re-verification fails closed → `interrupted`) | review D1, I-BRA-4 |

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

### Slice-2 review RED scenarios (2026-10-17)

The slice-2 adversarial review was rejected; its findings are pinned here before the fix.
B1/B2 scenarios fail today and carry
`#[ignore = "DISTILL github-did-identity-link: review B1/B2 — pending DELIVER fix"]`;
the H2 scenarios hold today and run un-ignored. Hardening of existing scenarios: IR-15 (H1),
IR-16 (L1), and `support::identity_links::link_checks` (M1: only a missing
`identity_link_checks` table reads as "no checks"; any other read failure panics, so the
"unchanged" guards cannot pass vacuously).

```gherkin
@driving_port @real-io @error @gdl-d14 @a-4 @f7 @review-b1 @contract-shape:pure-function
Scenario: Listing one person never hides that their account is contested            # IR-18b
  Given the bio of github "jeffbailey" names both Jeff's DID and Rachel's DID
  And both Jeff and Rachel linked github "jeffbailey", and Jeff re-checked every link
  When Jeff lists linked identities for did:plc:test-jeff only, and then for did:plc:rachel-test only
  Then each listing shows github:jeffbailey as contested, naming that person
  And neither listing shows that person as github:jeffbailey, verified

@driving_port @real-io @error @gdl-d5 @adr-060 @c2a @review-b2 @contract-shape:unbounded-preservation
Scenario: A link its peer author retracted no longer shows or is re-checked          # IR-8c
  Given Jeff follows Rachel, who published a link to github "rachelk" that her bio proves
  And Rachel retracted that link, and Jeff pulled her claims again
  When Jeff lists linked identities
  Then nothing links did:plc:rachel-test to github:rachelk
  And when Jeff re-checks his links he is told there are none to verify
  And GitHub was not asked anything, and nothing was recorded

@driving_port @real-io @error @gdl-d5 @gdl-d9 @c2a @review-b2 @contract-shape:pure-function
Scenario: A link its peer author superseded no longer shows                          # IR-8d
  Given Jeff follows Rachel, who published a link to github "rachelk"
  And Rachel replaced that link with a new one, and Jeff pulled her claims again
  When Jeff lists linked identities
  Then exactly one line shows Rachel's claim on github:rachelk, naming only the new record
  And the replaced record is not listed anywhere

@driving_port @real-io @adapter-integration @error @gdl-d9 @a-10 @uc-8 @review-h2 @contract-shape:unbounded-preservation
Scenario: Reading a re-registered login records nothing                              # IR-12b
  Given Jeff linked github "jeffbailey" and it is verified
  And he renamed his account, and a stranger took the login "jeffbailey"
  When Jeff reads the person github "jeffbailey"
  Then GitHub was asked once, for that login's profile
  And his linked identities read exactly as before
  And no check was recorded

@driving_port @real-io @adapter-integration @edge @gdl-d9 @n-2 @uc-8 @u-15 @review-h2 @contract-shape:bounded-change
Scenario: Reading a renamed account refreshes its link once                          # IR-12c
  Given Jeff linked github "jeffbailey"
  And he renamed his account to "jeff-b", and GitHub forwards the old name to the new one
  And Jeff re-checked his links, so his link was last seen as github:jeff-b
  And his bio no longer names his DID
  When Jeff reads the person github "jeff-b"
  Then GitHub was asked once, for jeff-b's profile
  And exactly one check was recorded: his link to github:jeff-b, proof gone because the bio names no DID
  And his linked identities show his claim on github:jeff-b as proof gone
```

Peer markers use production shapes: `peer_retraction_of` mirrors `claim_retract::run`
(same subject/predicate/object, no evidence, confidence 1.0, one `retracts` reference);
`peer_supersession_of` is the `LinkDraft` link re-stated with one `supersedes` reference, as
the production signer marks a successor (`infer_people::references_for`, DDD-7). Both travel
through the real `peer pull` path, signed by the peer's app key.

IR-12c is written to U-15 (accepted, DELIVER 02-07): the person-scrape refresh selects links
by their signed login or the login last seen by this reader's checks, so reading the new login
of a rename this reader has never observed refreshes nothing; `identity verify` (IV-4) is what
first observes the rename through the redirect.

## 4. Driving-adapter coverage

| Driving port | Scenario(s) |
|---|---|
| `identity link github <login> [--yes]` | WS, IL-2..IL-12b, IL-14, IL-15 |
| `identity verify [--did \| --github]` | IV-1..IV-13, IL-12b, IR-2, IR-4, IR-20, RR-1b, RR-2b |
| `identity list [--did]` | WS, IR-1..IR-18b, IR-20 (`--did`: IR-18, IR-18b) |
| `identity forget --did` | IF-1a, IF-1b, IF-2 |
| `claim retract` | IR-8a/b |
| `claim add`, `claim counter`, `infer people --sign` | IL-8a, IL-8c, IL-8b |
| `infer people` | IR-11, IR-17 |
| `scrape github <user>` | IR-12, IR-12b, IR-12c, IR-13, IR-20b |
| `peer remove` (soft) | IR-20, IR-20b (Given) |
| `graph query --weighted` / viewer `/score` | IR-14 / IR-15 |
| `search` | IR-16 |
| review app "Publish this link" / "Retract this link" | RA-1a..RA-14, RR-2a/b |

## 5. Adapter coverage (Mandate 6)

| Driven adapter | Real-I/O scenario |
|---|---|
| `adapter-github` `read_person`, `read_account` / `read_person_by_id` (strict shape), redirect policy, token | IV-3a (id read), IV-4 (redirect), IV-10 (chained / cross-host), IL-6 / IV-6 (401, anonymous rate limit, outage), IL-15 / IV-11 / IV-12 / OW-4b / OW-4c (200 answer naming no account — `FakeGithubAccounts::serve_flawed_profile`), IR-12 |
| `adapter-duckdb` v7 checks (read / insert / purge), migration | WS, IV-1/2, IF-1a, IL-13a/b (upgrade from the frozen v6 store) |
| single publish path → PDS | WS, IL-9a/b |
| `peer pull` (app-signed and self-attested links; a peer's retraction and supersession markers) | IR-1..IR-10, IR-8c, IR-8d, RR-1a, RR-3 |
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
| GitHub anonymous rate limit / rejected token / unreachable | IL-6, IV-6, RA-5, RA-10d |
| GitHub chained or cross-host redirect | IV-10 |
| PDS unreachable | IL-9a |
| app session expired (server-side and cookie) / repo-host session expired | RA-8 |

## Slice 4 — the viewer's linked-identity line (DISTILL 2026-10-19, revised after review cycle 1)

- **Designer**: Quinn · **Inherits**: `design/slice-4-viewer-linked-identity.md` (VL-1..19), design
  `wave-decisions.md` § Slice 4 (D-VL-1..11, C-VL-1..3; status PROPOSED, the user started the slice;
  open questions defaulted: no `/peers` line, no `/identities` page, legend and notice as proposed),
  `upstream-changes.md` UC-11 (stories US-VL-1..3 = the scope boundary), `deliver/upstream-issues.md`
  U-6, U-7, U-19/U-20, U-39.
- **Hand-off state**: every slice-4 scenario is `#[ignore = "DELIVER 04-NN: …"]`, the walking
  skeleton included (D-d unchanged), plus one ignored xtask case for the fault-seam guard. The
  `04-NN` numbers are DISTILL's proposed order; the roadmap may renumber. IR-15 was narrowed
  (D-VL-T5) and still passes.
- **Product-surface term (review F9)**: "identity list" in scenario text names the `openlore
  identity list` command the reader already uses to see his linked identities; it is a user-facing
  surface, not an implementation term, and is the stated parity reference (D-VL-5).

### Inputs read (slice 4)

| Input | Status |
|---|---|
| `design/slice-4-viewer-linked-identity.md`, `design/wave-decisions.md` § Slice 4, `design/upstream-changes.md` UC-11 | + |
| `distill/{test-scenarios,walking-skeleton,wave-decisions,red-classification}.md`, `deliver/upstream-issues.md` (U-6, U-7, U-19, U-20, U-39) | + |
| `docs/product/architecture/brief.md` (GDL "Driving ports" names the four slice-4 GETs) | + |
| `docs/product/kpi-contracts.yaml` | + — KPI-GDL-1 ("0 associations without this reader's Verified check") applies (VL-1, VL-4, VL-5); no new KPI |
| `docs/product/journeys/explore-the-graph.yaml` | + (context: the `/score` and traversal surfaces); no journey names the line |
| `discuss/`, `devops/` | - (not found) — stories UC-11 stand in; default environment matrix |
| Code: `identity_link_readers.rs`, `viewer_contributor_scoring*.rs`, `support/{mod,identity_links,people}.rs`, `claim-domain/src/identity_link.rs`, `viewer-domain/src`, `adapter-http-viewer/src/lib.rs`, `cli/src/verbs/{ui,identity_reads}.rs`, `cli/src/render/identity.rs`, `xtask/src/check_arch.rs` | + |

### Test files (slice 4)

| File | Layer | Test fns | Scenarios | Active today |
|---|---|---|---|---|
| `tests/acceptance/viewer_linked_identity.rs` | 3/4 subprocess (REAL `openlore ui` + DuckDB; REAL indexer for `/search`) | 13 | VL-1, 2, 3, 4, 5, 6, 7, 10, 11, 12, 15, 17a, 17b, 18, 19 | 0 |
| `tests/acceptance/viewer_linked_identity_invariants.rs` | 3/4 subprocess + layer 2 proptest (256 cases) | 5 | VL-8, 9, 13, 14, 16, 20 + VL-2p (parity property) | 0 |
| `xtask/tests/viewer_fail_seam.rs` (one case appended) | 1 (guard scan over a temp workspace) | 1 | VL-G (the fault seam is release-forbidden) | 0 |
| **Total** | | **19** | **21 VL scenarios + 1 property + 1 guard case** | **0** |

Vocabulary: `support/viewer_identities.rs` (`PersonPage`, `LinkRead`, `GithubStanding`, `ViewerOpts`,
`open_viewer`, `region_listing`, `what_jeff_lists`, region selection by heading/role). A page is read
back as an `identity list` listing, so the slice-2 `assert_shown` / `assert_check_age` / `Shown`
apply unchanged. `support/mod.rs`: `ViewerServer::start_with_env`, `startup_lines()`, stdout drained
on its own thread with a 20 s startup deadline; `encode_query_component_for_test` made `pub`.

### Traceability — scenario → story / decision

| ID | Outcome (test fn, in words) | Story | Traces to | Proposed step |
|---|---|---|---|---|
| VL-1 (WS) | Rachel's contributor page shows her GitHub account as verified | US-VL-1 | D-VL-1, D-VL-5, C-VL-2, KPI-GDL-1 | 04-01 |
| VL-2 + VL-18 | Every link standing reads in the viewer exactly as Jeff's identity list prints it (10 rows: verified today; 31 days + hint; contested, and contested past 30 days without hint; not checked naming two records; not checked naming one record; proof gone × 4 verdicts, each recorded by a real `identity verify`) | US-VL-1 | D-VL-5, U-8, U-9 | 04-02 |
| VL-2p | For every listing, the viewer's lines are the identity list lines without backticks (property) | US-VL-1 | D-VL-5 | 04-02 |
| VL-3 | A GitHub person's page shows the DID that linked the account, in any spelling | US-VL-2 | D-VL-6, U-6 | 04-03 |
| VL-17a | Until Jeff re-checks, a renamed account's line stays on its old login's page | US-VL-2 | D-VL-6 (known limit) | 04-03 |
| VL-17b | When Jeff re-checks, a renamed account's line moves to its new login's page | US-VL-2 | D-VL-6 | 04-03 |
| VL-4 | A contested account reads contested on every page about it | US-VL-1, US-VL-2 | GDL-D14, C-VL-3, KPI-GDL-1 | 04-03 |
| VL-5 | A stranger's link about Tobias and flawed link records never show as anyone's linked identity | US-VL-1 | C-GDL-1, ADR-084 §3, KPI-GDL-1 | 04-04 |
| VL-6 | A retracted or superseded link, and the retraction itself, never show | US-VL-1 | U-39, GDL-D5/D9 | 04-04 |
| VL-7 | A link from a peer Jeff removed does not show until he follows her again | US-VL-1, US-VL-2 | U-19, U-20 | 04-04 |
| VL-10 | A claim's detail page shows its author's linked identity | US-VL-1 | D-VL-1 | 04-05 |
| VL-11 + VL-19 | Network search shows each author's linked identity and labels link rows (index reachable / not configured) | US-VL-3 | D-VL-10, U-7, ADR-085 §8 | 04-06 |
| VL-9 + VL-14 | Pages still load and say so when linked identities cannot be read (3 per-request reads × 4 pages; the startup probe × 5 pages) | US-VL-1, US-VL-3 | D-VL-7 | 04-07 |
| VL-G | An ungated identity-link fault seam is refused by the release-build guard | — (enforcement) | D-VL-11 | 04-07 |
| VL-12 | A person with no link shows no linked-identities region (incl. a non-username login, a non-DID contributor, and an empty store) | US-VL-1, US-VL-2 | D-VL-1, design state table | 04-01 |
| VL-15 | A check dated in the future is ignored | US-VL-1 | Q-GDL-D4 / Q-RM-3 | 04-02 |
| VL-20 | A store made before identity links shows every page, and no linked identity it does not hold | US-VL-1, US-VL-2 | IL-13 fixture, platform review F-2 | 04-01 |
| VL-8 + VL-16 | Showing linked identities asks GitHub nothing, changes nothing, and reads the same with GitHub down | shared AC | D-VL-8, C-VL-1, C-VL-2 | 04-08 |
| VL-13 | Results updated in place carry the same linked identities as the full page | shared AC | ADR-033 | 04-08 |

Story coverage: **US-VL-1** VL-1, 2, 2p, 4, 5, 6, 7, 9, 10, 12, 14, 15, 18, 20 · **US-VL-2** VL-3,
4, 7, 12, 17a/b, 20 · **US-VL-3** VL-11, 19, VL-9/14 search rows · shared AC ("same words", "no
GitHub request", "nothing written", "only self-authored active non-removed links", "contested and
failed proofs are stated") VL-2/2p, VL-8/16, VL-5/6/7, VL-4.

Mix (informational; counted per scenario ID, VL-2p and VL-G excluded): walking skeleton 1 · happy
3 (VL-3, VL-10, VL-11) · error / edge / boundary / guardrail 17 (VL-2, 4, 5, 6, 7, 8, 9, 12, 13, 14,
15, 16, 17a, 17b, 18, 19, 20). Non-happy: 17 / 21 (81 %).

### Driving-adapter and adapter coverage (slice 4)

| Port | Scenario(s) |
|---|---|
| viewer `GET /score?contributor=` | VL-1, 2, 4, 5, 6, 7, 8, 9, 12, 13, 14, 15, 16, 20 |
| viewer `GET /project?subject=github:` | VL-3, 4, 5, 6, 7, 8, 9, 12, 13, 14, 16, 17a/b, 20 |
| viewer `GET /claims/{cid}` | VL-8, 9, 10, 13, 14, 16 |
| viewer `GET /search` | VL-9, 11, 13, 14, 19 |
| `IdentityLinkCheckReadPort` / `LinkCandidateReadPort` / `LinkLineageReadPort` (real DuckDB, viewer side) | every scenario; each failing per request: VL-9; probe: VL-14; upgraded store: VL-20 |
| `IndexQueryPort` (REAL `openlore-indexer`) | VL-9, 11, 13, 14 |
| GitHub (never asked by the viewer) | VL-1, VL-8 (request log of `FakeGithubAccounts` wired as the viewer's GitHub base) |
| `identity verify` (records the checks VL-2 reads) | VL-2, VL-4, VL-17b |

### Environments (default matrix — no DEVOPS wave)

| Environment | Scenario(s) |
|---|---|
| clean (fresh store, one link) | VL-1 (walking skeleton), VL-10, VL-12 |
| clean, empty (no claim, no link) | VL-12 row group 2 |
| populated pre-v7 store (upgrade) | VL-20 (frozen fixture `tests/fixtures/identity-link-pre-v7-store/`) |
| with-stale-config: checks older than 30 days | VL-2 rows "31 days later" (hint; contested without hint = VL-18) |
| with-stale-config: a check dated in the future | VL-15 (seeded row — no verb can date a check in the future) |
| with-stale-config: a damaged store (check table gone) | VL-14 (direct `DROP TABLE` — no verb damages a store) |
| with-pre-commit | **N/A** — the viewer runs no hooks and reads no hook config |
| GitHub unreachable | VL-16 |
| network index not configured | VL-19 |

**Walking skeleton and with-stale-config (review F5)**: documented N/A for the skeleton. VL-1 runs
in the clean environment only; the stale states are covered by VL-2 (31-day rows), VL-15 (future
check) and VL-14 (damaged store), each a focused scenario with its own Given.
