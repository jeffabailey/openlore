# Acceptance Test Design — bluesky-claim-review-app (DISTILL)

- **Wave**: DISTILL · **Date**: 2026-10-04 · **Designer**: Quinn (nw-acceptance-designer)
- **Language**: Rust (`[lang-mode] rust`) · skip marker `#[ignore = "DELIVER …: unskip one-at-a-time (…)"]` · PBT: proptest (layer 2 only)
- **Driving ports**: the REAL `openlore-review-app` (third composition root, ADR-072) over HTTP; the REAL `openlore` CLI + `openlore-indexer` + `openlore ui` for the reader path; pure `review-domain` / `claim-domain` functions (layer 2); the real workspace graph + repo files (xtask guardrails)
- **Faked (driven-external only)**: `FakeAtprotoNetwork` (PLC, handle resolution, the user's PDS + OAuth authorization server), `FakeGithubAccounts`
- **Decisions**: `wave-decisions.md` (DWD-1..13) · **WS**: `walking-skeleton.md` · **RED**: `red-classification.md` · **Review**: `acceptance-review.md`

The `.rs` files are the scenario SSOT; this is the map.

## 1. Test files

| File | Release / stories | Layer | Scenarios | Active | Ignored |
|---|---|---|---|---|---|
| `tests/acceptance/review_app_walking_skeleton.rs` | WS · US-BRA-000..004 (+ AC-004.3 reader) | 4/5 | 4 | **4** | 0 |
| `tests/acceptance/review_app_sign_in_and_ownership.rs` | WS · US-BRA-000/001/002 | 4 | 22 | 0 | 22 |
| `tests/acceptance/review_app_queue_and_publish.rs` | WS · US-BRA-003/004 | 4 | 18 | 0 | 18 |
| `tests/acceptance/review_app_consent_and_share.rs` | R1 · US-BRA-005..008 | 4 | 19 | 0 | 19 |
| `tests/acceptance/review_app_lifecycle.rs` | R2 · US-BRA-010/011/012 | 4 | 17 | 0 | 17 |
| `tests/acceptance/review_app_self_attested_readers.rs` | R2 · US-BRA-009 | 4 | 7 | 0 | 7 |
| `tests/acceptance/review_app_privacy_invariants.rs` | I-BRA-1..8 + DEVOPS operability | 4 | 13 | 0 | 13 |
| `tests/acceptance/review_app_core.rs` | pure cores (PBT) | 2 | 13 | 0 | 13 |
| `xtask/tests/review_app_architecture.rs` | check-arch + deploy guardrails | static | 10 | 0 | 10 |
| **Total** | | | **123** | **4** | **119** |

Shared harness: `tests/acceptance/support/review_app/` — `domain.rs` (typed vocabulary), `world.rs`
(the chained step vocabulary + PDS observations + universe), `app.rs` (the real process),
`browser.rs` + `html.rs` (JS-less browser). Doubles: `crates/test-support/src/{fake_atproto.rs,
fake_github_accounts.rs, review_http.rs}` (11 self-tests, green).

**Mix**: happy/journey 41 · error 31 · adversarial 14 · edge/boundary 13 · guardrail/infra 24.
**Error + adversarial + edge/boundary = 58/113 non-static = 51%** (≥ 40% target). Property-based:
13 (layer 2) + 6 invariant journeys tagged `@property` at layer 4 (example-pinned, Mandate 9).
Parametrised tables (finite Cartesian): OW-7, ED-1, ED-2, QP-10, SH-2, RD-4, OP-2, CORE-3.

## 2. Scenario inventory

### Walking skeleton (active) — `review_app_walking_skeleton.rs`

| ID | Scenario | Tags |
|---|---|---|
| WS-1 | Priya signs in with her Bluesky handle and Bluesky recognises the OpenLore review app | `@walking_skeleton @driving_port @US-BRA-000 @US-BRA-001` |
| WS-2 | Priya proves her GitHub account is hers with her DID in her bio | `@walking_skeleton @US-BRA-002 @I-BRA-4` |
| WS-3 | Priya sees private, evidence-backed suggestions from her own repos | `@walking_skeleton @US-BRA-003 @I-BRA-1 @I-BRA-4` |
| WS-4 | Priya approves a suggestion; it lands in her own PDS, accepted by OpenLore as self-attested | `@walking_skeleton @US-BRA-004 @US-BRA-009 @I-BRA-3/5/7 @kpi-bra-1 @kpi-bra-6` |

### US-BRA-000/001/002 — `review_app_sign_in_and_ownership.rs`

| ID | Scenario | Kind |
|---|---|---|
| SI-1 | Bluesky identifies the review app from its published client details | happy |
| SI-2 | An unreachable identity check is explained as temporarily unavailable and changes nothing | error |
| SI-3 | Every page tells the browser to use HTTPS only and never frame it | guardrail |
| SI-4 | Priya sees what the app will never do before she signs in | happy |
| SI-5 | Dmitri signs in through his self-hosted PDS | edge |
| SI-6 | A mistyped handle gets a helpful message and starts nothing | error |
| SI-7 | Cancelling authorization at her PDS changes nothing | error |
| SI-8 | A sign-in that returns a different account than the handle named is refused | adversarial |
| SI-9 | A failed sign-in exchange is explained and never takes the app down (SPIKE finding 3) | error/infra |
| SI-10 | A replayed sign-in return link signs nobody in (SPIKE finding 3) | adversarial |
| SI-11 | Signing out ends the session | happy |
| SI-12 | The session cookie is out of reach of page scripts and pages never carry tokens | guardrail |
| SI-13 | With only the broad permission, the app says why and still writes only claims | mode (C5) |
| OW-1 | Priya gets the exact DID to copy and where to put it | happy |
| OW-2 | Sam cannot claim someone else's GitHub account | adversarial |
| OW-3 | A different DID in the bio is explained | error |
| OW-4 | A GitHub rate limit is explained and retrying after the wait succeeds | error/infra |
| OW-5 | A GitHub username that does not exist is named in the message | error |
| OW-6 | A verified GitHub link belongs to one DID only | adversarial |
| OW-7 | Only the exact signed-in DID token proves ownership (7-row table) | boundary |
| OW-8 | A scan cannot be started before ownership is proven | adversarial |
| OW-9 | A renamed or re-registered GitHub account must be verified again | error |

### US-BRA-003/004 — `review_app_queue_and_publish.rs`

| ID | Scenario | Kind |
|---|---|---|
| QP-1 | Every suggestion card carries its signal and evidence | happy |
| QP-2 | Ownership is re-checked right before the first scan | error |
| QP-3 | An empty scan explains that forks and archived repos are skipped | edge |
| QP-4 | When GitHub is busy, partial results are kept and the scan resumes | infra |
| QP-5 | Every review page is navigable by keyboard with labelled controls | a11y |
| QP-6 | The preview shows exactly what will be written, and writes nothing | happy |
| QP-7 | The published record is the previewed record, field for field | property |
| QP-8 | The confirmation names the record and the way back, and the queue moves on | happy |
| QP-9 | Backing out of the preview writes nothing | edge |
| QP-10 | A failed publish leaves the suggestion pending with no partial record (3 postures) | error/infra |
| QP-11 | Retrying after a failure publishes exactly once | error |
| QP-12 | Confirming the same preview twice writes one record | adversarial (C4) |
| QP-13 | A confirmation for something never previewed is refused | adversarial |
| QP-14 | Dmitri's approval lands in his self-hosted PDS | edge |
| QP-15 | Publishing still works after the PDS access has quietly expired (SPIKE finding 4) | infra |
| QP-16 | Her queue survives signing out and the app restarting | interruption |
| QP-17 | The seventh scan in a day is politely refused | boundary |
| QP-18 | Two people scanning at the same time each get only their own suggestions | concurrency |

### Release 1 — `review_app_consent_and_share.rs`

| ID | Scenario | Kind |
|---|---|---|
| ED-1 | Priya publishes her edited claim (0.70 / 1.00 / 0.00 table) | happy/boundary |
| ED-2 | An invalid confidence is caught with guidance and blocks approval (6 inputs) | error |
| ED-3 | Cancelling an edit restores the suggestion | edge |
| ED-4 | Every vocabulary philosophy is offered when swapping | happy |
| DC-1 | Declining writes nothing public | happy/guardrail |
| DC-2 | A declined suggestion is not offered again | happy |
| DC-3 | Nobody else can see Priya's declines | adversarial |
| DC-4 | Undo restores a declined suggestion | happy |
| DC-5 | Declining twice is the same as declining once | edge (C4) |
| PR-1 | The profile shows only published claims, each labelled self-attested | happy |
| PR-2 | An empty profile is honest, with a call to action for its owner | edge |
| PR-3 | An unreachable PDS is stated, never replaced by stale claims | error/infra |
| PR-4 | A profile is found by handle or DID, and an unknown handle is plainly not found | error |
| SH-1 | Priya previews, edits and confirms her post | happy |
| SH-2 | Nothing is posted without consent ("Don't post" / leaving) | guardrail |
| SH-3 | The post never mentions unapproved items | guardrail |
| SH-4 | Sharing is unavailable with nothing published | edge |
| SH-5 | A failed post is explained and retryable, and the profile is unaffected | error |
| SH-6 | A post longer than Bluesky allows is caught before anything is sent | boundary |

### Release 2 — `review_app_lifecycle.rs` + `review_app_self_attested_readers.rs`

| ID | Scenario | Kind |
|---|---|---|
| RS-1 | A rescan offers only new suggestions, with a summary | happy |
| RS-2 | A rescan with nothing new says so | edge |
| RS-3 | Ownership is re-checked before every scrape | guardrail |
| RS-4 | A failed re-check blocks the scan and keeps published claims untouched | error |
| RS-5 | A hidden suggestion cannot be approved while ownership is unproven | adversarial |
| RS-6 | Re-verifying restores scanning and pending suggestions | happy |
| RT-1 | Priya retracts a published claim | happy |
| RT-2 | Cancelling a retraction writes nothing | edge |
| RT-3 | A failed retraction leaves the claim active, with a retry | error |
| RT-4 | A retracted claim never appears in the share post | guardrail |
| RT-5 | Retracting the same claim twice adds one retraction | edge (C4) |
| FG-1 | Forget me removes everything the app holds, and nothing in her PDS | happy |
| FG-2 | Cancelling forget me keeps everything | edge |
| FG-3 | Disconnecting revokes her grant, and an answer of 200 counts as done (SPIKE finding 1) | infra |
| FG-4 | Forget me still forgets when her PDS cannot be reached to revoke | error/infra |
| FG-5 | After forgetting, the app never uses the access her PDS already issued (SPIKE finding 2) | guardrail |
| FG-6 | The operator can forget a person on request, without their session | operator |
| RD-1 | Maria pulls Priya's self-attested claims | happy |
| RD-2 | The viewer labels self-attested claims, never unverified | happy |
| RD-3 | App-signed claims read exactly as before, side by side with self-attested ones | regression |
| RD-4 | Records that are not honest self-attestations are refused (tampered / foreign / malformed) | adversarial |
| RD-5 | A claim Priya retracted through the app reads as retracted | happy |
| RD-6 | The network index includes self-attested claims, attributed and marked | happy |
| RD-7 | A self-attested claim fetched through anything but the author's own PDS is not indexed | adversarial |

### Cross-cutting — `review_app_privacy_invariants.rs`

| ID | Scenario | Kind |
|---|---|---|
| PV-1 | Only Priya can see her queue — not Dmitri, not an anonymous visitor (replays every action) | `@property` I-BRA-1 |
| PV-2 | Pending suggestions are never exposed anywhere public | `@property` I-BRA-1 |
| PV-3 | Every write to her PDS follows one of her explicit confirms, and declines write nothing | `@property` I-BRA-2/3 |
| PV-4 | Writes only ever go to the author's own PDS, and nothing is ever updated or deleted | `@property` I-BRA-7/8 |
| PV-5 | No repository is ever read without a passing ownership check just before | `@property` I-BRA-4 |
| PV-6 | A confirmation without her page's anti-forgery token is refused | adversarial |
| OP-1 | A healthy app reports live and ready | smoke |
| OP-2 | The app refuses to start rather than run half-wired (3 arms) | infra |
| OP-3 | The image self-test passes with throwaway keys and no network | smoke |
| OP-4 | The operator is warned two weeks before the GitHub token expires | infra |
| OP-5 | The logs of a whole journey reveal nothing about anyone | privacy |
| OP-6 | The operator reads aggregate counts that identify no one | `@kpi` |
| OP-7 | A scan interrupted by a restart is offered for resume | interruption |

### Layer 2 pure cores — `review_app_core.rs` (proptest, 256 cases)

CORE-1 ownership verdict · CORE-1b its key examples · CORE-2 several DIDs/per-DID · CORE-3 ADR-071
provenance table · CORE-4 reconcile (+ idempotence) · CORE-5 confidence parse · CORE-6 publish plan
== record, rkey == CID · CORE-7 share text · CORE-8 lifecycle state machine · CORE-9 derived
visibility · CORE-10 budget · CORE-11 repo selection BR-3 (shipped fn, green today) · CORE-12 sign-in pin.

### Guardrails — `xtask/tests/review_app_architecture.rs`

AR-1 third composition root registered · AR-2 app cannot reach cli/indexer adapters · AR-3 only the
app reaches the OAuth + private-store adapters · AR-4 `review-domain` pure · AR-5 OAuth adapter
create-only · AR-6 no signing identity in the app · AR-7 owner-scoped SQL, owner-free KPI table ·
AR-8 compose mounts only data + secrets, no AWS creds · AR-9 Caddy HTTPS only · AR-10 log-field allowlist.

## 3. Traceability — every AC → scenario (70/70 = 100%)

| AC | Scenarios | AC | Scenarios |
|---|---|---|---|
| AC-000.1 | WS-1, SI-1 | AC-005.1 | ED-1, ED-4, CORE-5 |
| AC-000.2 | SI-3, AR-9 | AC-005.2 | ED-1 |
| AC-000.3 | SI-2 | AC-005.3 | ED-1 |
| AC-001.1 | WS-1 | AC-005.4 | ED-2, CORE-5 |
| AC-001.2 | SI-4 | AC-005.5 | ED-3 |
| AC-001.3 | SI-5 | AC-006.1 | DC-1 |
| AC-001.4 | SI-6 | AC-006.2 | DC-1, PV-3 |
| AC-001.5 | SI-7 | AC-006.3 | DC-3 |
| AC-001.6 | SI-8, CORE-12 | AC-006.4 | DC-2, CORE-4 |
| AC-001.7 | SI-11 | AC-006.5 | DC-4 |
| AC-002.1 | OW-1 | AC-007.1 | PR-1 |
| AC-002.2 | WS-2, OW-7, CORE-1 | AC-007.2 | PR-1, PV-2 |
| AC-002.3 | OW-2, OW-3, OW-4, OW-5 | AC-007.3 | PR-2 |
| AC-002.4 | OW-2, OW-8 | AC-007.4 | PR-3 |
| AC-002.5 | OW-6, CORE-2 | AC-007.5 | PR-1 |
| AC-002.6 | OW-7, CORE-1b | AC-008.1 | SH-1, SH-6 |
| AC-003.1 | WS-3, QP-2 | AC-008.2 | SH-1 |
| AC-003.2 | WS-3, QP-1 | AC-008.3 | SH-1, SH-3, CORE-7 |
| AC-003.3 | WS-3 | AC-008.4 | SH-2 |
| AC-003.4 | PV-1, DC-3 | AC-008.5 | SH-4 |
| AC-003.5 | WS-3, PV-2 | AC-008.6 | SH-5 |
| AC-003.6 | QP-3 | AC-009.1 | WS-4, RD-1 |
| AC-003.7 | QP-4 | AC-009.2 | RD-6 |
| AC-003.8 | QP-5 | AC-009.3 | RD-1, RD-2 |
| AC-004.1 | WS-4, QP-6, PV-3 | AC-009.4 | RD-3 (+ the shipped app-signed suites stay green) |
| AC-004.2 | WS-4, QP-14 | AC-009.5 | RD-4, CORE-3 |
| AC-004.3 | **WS-4**, RD-1 | AC-010.1 | RS-3, RS-4, PV-5 |
| AC-004.4 | WS-4, QP-6, QP-7, CORE-6 | AC-010.2 | RS-4, RS-5, CORE-9 |
| AC-004.5 | WS-4, QP-6, QP-8 | AC-010.3 | RS-4 |
| AC-004.6 | QP-9, QP-10, QP-11 | AC-010.4 | RS-6 |
| AC-004.7 | WS-4, QP-8 | AC-010.5 | RS-1, RS-2, CORE-4 |
| AC-011.1 | RT-1, RT-2 | AC-012.1 | FG-1 |
| AC-011.2 | RT-1, RD-5 | AC-012.2 | FG-1, FG-3, FG-6 |
| AC-011.3 | RT-1, RT-4 | AC-012.3 | FG-1, FG-5 |
| AC-011.4 | RT-3 | AC-012.4 | FG-2 |

### Invariants, NFRs, SPIKE findings, KPIs

| Item | Scenarios |
|---|---|
| I-BRA-1 pending private | WS-3, PV-1, PV-2, DC-3, SH-3 |
| I-BRA-2 declines private | DC-1, DC-3, PV-3 |
| I-BRA-3 write ⇐ confirm after preview | WS-4, QP-6, QP-12, QP-13, PV-3, PV-6 |
| I-BRA-4 no scrape without current proof | WS-2, WS-3, OW-2, OW-8, QP-2, RS-3, RS-4, PV-5 |
| I-BRA-5 self-attested first-class | WS-4, RD-1, RD-2, RD-3, RD-6, PR-1 |
| I-BRA-6 share opt-in, approved-only | SH-1, SH-2, SH-3, SH-4, RT-4, CORE-7 |
| I-BRA-7 writes only to own PDS | WS-4, QP-14, PV-4 |
| I-BRA-8 never modify/delete | RT-1, RS-4, FG-1, PV-4, AR-5, fake refuses + records delete/put/applyWrites everywhere |
| NFR-BRA-1 / -2 / -3 | PV-1 · SI-12, PV-6, OP-5, AR-10 · WS-1, SI-1, SI-13 |
| NFR-BRA-5 / -6 / -7 / -8 | QP-10, QP-15, QP-16 · QP-5, ED-2 · QP-6, CORE-6 · SI-4 |
| NFR-BRA-4 / -9 | not automated (SLO + runbook, DWD-12) · OP-1 |
| SPIKE findings 1 / 2 / 3 / 4 / 5 | FG-3 · FG-5 · SI-9, SI-10 · QP-15 · WS-3, QP-1 (0.25 default) |
| DEVOPS guardrails | OP-1..OP-7, AR-8, AR-9, AR-10 (compose mount guard, no AWS creds, log allowlist, token-expiry refusal, admin loopback-only) |
| KPI-BRA-1 / 4 / 4b / 5 / 6 / 7 / 8 | WS-4, OP-6 · PV-1..3 · DC-1, PV-3 · RS-3, PV-5 · RD-1, RD-6 · SH-1 · RS-1 |

## 4. One-at-a-time order for DELIVER

1. WS-1 → WS-2 → WS-3 → WS-4 (already active; the bootstrap slice turns them green in order).
2. Skeleton-focused (SI-*, OW-*, QP-*, PV-1/2/6, OP-1/2/3) and CORE-1/1b/2/6/10/11/12.
3. Release 1 (ED/DC/PR/SH, PV-3, OP-5/6, CORE-5/7/8).
4. Release 2 (RS/RT/FG/RD, PV-4/5, OP-7, CORE-3/4/9) — RD-* can start any time (independent of the app).
5. AR-* as each crate / deploy file lands (AR-8 in the same commit as the first compose file, DEVOPS rule).
