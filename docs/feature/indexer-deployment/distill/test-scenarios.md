# Test scenarios and traceability: indexer-deployment (DISTILL)

> The executable tests are the source of truth for scenarios. Each test carries its Gherkin in a doc
> comment (repo convention: Rust `#[test]` suites, no `.feature` runner). This file maps them to the
> stories, ACs, design-added ACs and binary changes. Every scenario is `#[ignore = "DELIVER <step>: …"]`
> at hand-off.

## Suites

| Suite (registered in) | Layer | Scenarios | Driving ports |
|---|---|---|---|
| `tests/acceptance/indexer_deployment_walking_skeleton.rs` (`crates/cli`) | 4, `@walking_skeleton` | 3 (WS-0..2) | `serve`, `trigger`, `openlore search`, `/healthz` |
| `tests/acceptance/indexer_deployment_passes.rs` (`crates/cli`) | 3 | 17 (PS-10..26) | `serve`, `trigger`, `openlore search`, public HTTP |
| `tests/acceptance/indexer_deployment_did_list.rs` (`crates/cli`) | 3 | 17 (DL-30..37, PG-36, PG-40..48) | `serve`, `trigger`, `openlore search` |
| `tests/acceptance/indexer_deployment_public_surface.rs` (`crates/cli`) | 3 | 6 (AS-50..56) | public HTTP, `serve` startup |
| `tests/acceptance/indexer_deployment_core.rs` (`crates/cli`) | 2, proptest | 12 (CORE-1..11 + 2 pinned examples) | pure functions (RED `sut_*` bindings) |
| `tests/acceptance/review_app_resource_caps.rs` (`crates/openlore-review-app`) | 4 | 1 (RAC-1, 8-row outline) | `openlore-review-app serve` |
| `xtask/tests/indexer_deployment_architecture.rs` | structural | 6 (XD-1..6) + 1 always-on scanner self-test | real workspace sources |
| `xtask/tests/indexer_deployment_platform.rs` | structural + real `bash` | 13 (XP-1..13) + 1 always-on helper self-test | real deploy files, `render-dids.sh`, `deploy.sh`, `health-timer.sh` |
| **Total** | | **75 ignored** (+2 always-on self-tests) | |

> DELIVER roadmap review (2026-10-07): XP-13 added (F3). CORE-8, CORE-9/9b and CORE-11 moved verbatim
> out of `indexer_deployment_core.rs` into their owning crates (F2): see `deliver/roadmap.json`
> `tag_renumbering`.

Error / edge / adversarial share: 31 of 74 (42 %). Walking skeletons: 3. Properties: 10 generators + 2 pinned.

## Scenario list

| ID | Scenario | Tags |
|---|---|---|
| WS-1 | Maria finds claims from authors on different PDSes after the scheduled pass | `@walking_skeleton @wiring_e2e @driving_port @kpi` AC-001.1, AC-002.4, KPI-IXD-1 |
| WS-0 | A fresh deployment answers before its first pass | `@walking_skeleton @edge` AC-001.4 |
| WS-2 | A claim Priya approves after a pass becomes searchable on the next pass | `@walking_skeleton @kpi` AC-002.1, KPI-IXD-2 |
| PS-10 | Search keeps answering within a second while a pass runs | AC-002.2, NFR-IXD-3 |
| PS-11 | A pass is never refused because search is busy (100 searches over a 40-author pass) | AC-002.3, NFR-IXD-7 |
| PS-12 | Searches overlapping the end of a pass still answer within a second | M3, B14 |
| PS-13 | Passes never overlap (second trigger coalesces, exit 0, one summary) | AC-002.4, C7c |
| PS-14 | The timer sees the pass's own exit code and exactly one summary (0, 0 with skip, 3, 2) | AC-004.1..3, B4, `@kpi` |
| PS-15 | The timer firing while serve is down reports that no pass ran (exit 4, no store opened) | ADR-080 ET, C4b, `@error` |
| PS-16 | The timer needs nothing but the control socket to start a pass | M4 |
| PS-17 | A pass that cannot store a claim ends with exit 2 and search keeps answering | AC-004.2, B4, `@error` |
| PS-18 | A pass that crashes frees the runner for the next pass | H2, `@error` |
| PS-19 | A pass that overruns its deadline ends with exit 2 and keeps what it saved (~60 s) | M1, C7b, `@error @slow` |
| PS-20 | A broken index makes serve stop instead of answering falsely | H2, AC-004.2, `@error` |
| PS-21 | A search that cannot read the index is reported as unavailable, never as no results | ADR-083 §2, `@error` |
| PS-22 | The health response shows when the last good pass ended, and only that | ADR-083 §2, KPI-IXD-3, `@kpi` |
| PS-23 | An index killed in the middle of a pass comes back with what it had saved | AC-002.5, AC-006.2, C7b, `@error` |
| PS-24 | A socket file left over from a crash does not stop the next start | ADR-080 ET, `@error` |
| PS-25 | A pass with nothing new on the network changes nothing Maria sees | C4a |
| PS-26 | What the index logs about its passes never contains claim content | I-IXD-4, AC-005.4 |
| DL-30 | A DID added to the list is indexed on the next pass without a restart | AC-003.1, KPI-IXD-4, `@kpi` |
| DL-31 | Two list edits in a row are picked up by two passes in a row | H1, AC-003.1 |
| DL-32 | A mistyped list refuses the pass, names the bad entry and keeps search serving (then recovers) | AC-003.3, AC-004.2, AC-004.4, `@error` |
| DL-33 | A list the index cannot read refuses the pass and touches nothing (missing / unreadable) | AC-003.4, C7a, `@error` |
| DL-34 | Line endings are tolerated, a hidden byte-order mark is refused loudly | ADR-081 ET, C6a, `@error` |
| DL-35 | Setting both a list and a list file is refused at start | ADR-081, C5a, `@error` |
| DL-37 | Each pass reports how old the DID list is | B15, M5 |
| PG-36 | An empty list never empties the index | AC-003.2, C3, `@edge` |
| PG-40 | Removing an author from the list removes their claims on the next pass | AC-003.2 |
| PG-41 | A removed author is purged even when every listed author is unreachable (exit 3) | AC-003.2, `@error` |
| PG-42 | An author who is still listed but unreachable keeps their claims | AC-003.2, ADR-078 am., `@error` |
| PG-43 | A list that drops an author but cannot be used purges nothing (typo / unreadable) | AC-003.2, `@error` |
| PG-44 | Without the purge setting a removed author's claims stay | ADR-082 opt-in, C5a |
| PG-45 | Only the removed author is purged, never one whose DID merely starts the same way | ADR-082 ET, `@adversarial` |
| PG-46 | A removed author's app-signed and self-attested claims are both purged | ADR-082 ET |
| PG-47 | Purging is done once, and listing the author again brings their claims back | C4a, R-IXD-D2 |
| PG-48 | A purge that fails ends the pass with exit 2 and finishes on the next pass | AC-004.2, C7b, `@error` |
| AS-50 | Anyone trying to write, or to reach anything but search and health, is refused (9 requests) | AC-001.3, FR-IXD-2, `@error @adversarial` |
| AS-52 | Oversized requests are refused at the boundary, not one byte early (8192/8193, 512/513) | NFR-IXD-7, C1b, `@error` |
| AS-53 | A search matching more than 1000 claims returns 1000 and logs the cut without the query | ADR-083 §3, `@slow` |
| AS-54 | A burst of 100 searches in 10 seconds is answered and leaves the index healthy | NFR-IXD-7 |
| AS-55 | The index refuses settings outside their range and accepts their limits (15 rows) | data-models §1, AC-006.5, C1b, `@error` |
| AS-56 | The production configuration starts ready and runs a pass (CI serve-smoke parity) | AC-001.6, AC-006.5 |
| CORE-1..3 | The purge plan is the set of indexed authors no longer listed (+ pinned look-alike) | `@property` AC-003.2 |
| CORE-4 | Reading the DID list loads it whole or names the first bad entry | `@property` AC-003.3 |
| CORE-5 | A pass's exit code puts local failures before outages before success (+ pinned table) | `@property` AC-004.1..3 |
| CORE-6 | The runner never runs two passes and always frees its slot (model-based) | `@property` AC-002.4, H2 |
| CORE-7 | A pass past its deadline fails whatever it would have said | `@property` M1 |
| CORE-8 | Each new setting accepts exactly its range | `@property` data-models §1 |
| CORE-9 / 9b | Only two routes exist on the public listener; requests are admitted exactly within bounds | `@property` AC-001.3, NFR-IXD-7 |
| CORE-10 | The health response is honest and minimal | `@property` ADR-083 §2 |
| CORE-11 | Purging an author changes only that author's claims and files (state delta, idempotent) | `@property` AC-003.2, ADR-082 |
| RAC-1 | The review app accepts database caps within their range and refuses the rest | B11, AC-006.4/5 |
| XD-1..6 | check-arch rules registered; search handler read-only; purge only in the runner; DELETE only in purge.rs; control channel Unix-only; purge port probed | §11, B6, B7 |
| XP-1..2 | Compose posture (indexer; review app B11) | AC-001.6, AC-003.5, AC-006.5 |
| XP-3 | Caddy: only POST search + GET /healthz, 8 KB cap, HTTPS only | AC-001.2, AC-001.3 |
| XP-4 | Timer every 15 min, Persistent, oneshot render-then-trigger, timeout ≥ deadline | AC-002.4, AC-002.5 |
| XP-5..6 | `render-dids.sh` (real run): rename the file, never the directory; failure keeps last good, exit 0 | H1, AC-003.1, AC-003.4 |
| XP-7..8 | `deploy.sh` (real run): non-digest refs, unsigned digests, red-CI shas refused before the host | AC-006.1 |
| XP-9..10 | Exactly 3 alarms on the existing topic with recoveries; filters/queries on structural events | AC-004.1..4, 004.6, AC-005.x |
| XP-11..12 | Host IAM least privilege; signed non-root distroless image from CI | AC-003.5, AC-001.6, AC-006.1 |
| XP-13 | `health-timer.sh` (real run): a failing FilterLogEvents makes the health line `not_live = 1` (fail closed) | AC-004.1, AC-005.1, DV-IXD-8, U-1, `@error` |

## AC traceability

| AC | Scenarios | Not automatable here (runbook / live) |
|---|---|---|
| AC-001.1 | WS-1 | nightly `index-smoke` (≥ 2 real hosts) |
| AC-001.2 | XP-3 | TLS certificate validity (post-deploy check, `index-smoke`) |
| AC-001.3 | AS-50, CORE-9, XP-3, XD-2, XD-5 | |
| AC-001.4 | WS-0, AS-56 | |
| AC-001.5 | (XP-1, XP-2: caps and OOM order) | PDS / review-app poller during the first deploy |
| AC-001.6 | XP-1, XP-8, XP-12, AS-56 | |
| AC-002.1 | WS-2, XP-4 | KPI-IXD-2 timed samples |
| AC-002.2 | PS-10, PS-11, PS-12 | |
| AC-002.3 | PS-11 | |
| AC-002.4 | PS-13, CORE-6, XP-4, WS-1 | |
| AC-002.5 | XP-4, PS-23 | reboot check at I-5 |
| AC-003.1 | DL-30, DL-31, XP-5 | |
| AC-003.2 (amended) | PG-36, PG-40..48, CORE-1..3, CORE-11, XD-3, XD-4 | |
| AC-003.3 | DL-32, DL-34, CORE-4 | |
| AC-003.4 | XP-6 (host keeps last good), DL-33 (binary never runs empty) | |
| AC-003.5 | XP-1, XP-11 | IMDS hop-limit check (`deploy.sh install`) |
| AC-004.1 | XP-9, PS-14, CORE-5 | A1 test-fire |
| AC-004.2 | XP-9, PS-14, PS-17..20, DL-32, PG-48 | A2 test-fire |
| AC-004.3 | XP-9, PS-14, CORE-5 | |
| AC-004.4 | XP-9, DL-32 (recovery pass) | |
| AC-004.5 | — | test-fire procedure (monitoring-alerting §3) |
| AC-004.6 | XP-9 (exactly 3 alarms) | cost table |
| AC-005.1..3 | XP-10, PS-22 | `deploy.sh status` against real CloudWatch |
| AC-005.4 | XP-10, PS-26 | ≤ 10 s timing |
| AC-006.1 | XP-7, XP-8, XP-1, XP-12 | |
| AC-006.2 | PS-23 (no restore after a kill) | auto-rollback drill (I-5) |
| AC-006.3 | (XP-1: init, 5 s grace) | ≤ 30 s downtime, PDS 200 throughout |
| AC-006.4 | RAC-1, AS-55, XP-2 | memory gate on the host |
| AC-006.5 | XP-1, XP-2, AS-55, AS-56 | OOM order under pressure |

## Design-added ACs (DESIGN handoff, ADR Earned Trust)

| Item | Scenarios |
|---|---|
| H1 two consecutive list edits, no restart | DL-31, XP-5 |
| H2 panic frees the runner; poisoned store → exit 2, `/healthz` never 200, search never empty 200 | PS-18, PS-20, PS-21, CORE-6, CORE-10 |
| M1 pass deadline → exit 2 `pass_deadline_exceeded`, committed kept | PS-19, CORE-7 |
| M3 search ≤ 1 s over the end-of-pass checkpoint | PS-12 |
| M4 `trigger` before config and store | PS-16, PS-15 |
| M5 stale list visible | DL-37 (+ A3 input in XP-10) |
| Empty list never purges | PG-36, CORE-1 |
| Malformed / unreadable list never purges | PG-43, DL-33 |
| Purge failure → exit 2 and alarm | PG-48 (+ A2 via XP-9) |
| One `pass_summary` per pass, including exit 2 | `assert_one_summary_matching` in every pass scenario; PS-14 |
| `trigger` coalescing | PS-13 |
| `/healthz` `last_successful_pass_at`, 503 | WS-0, PS-22, PS-20, CORE-10 |
| Search 500, never empty 200 | PS-21, PS-20 |
| ADR-080 ET: SIGKILL mid-pass, stale socket, unreachable → 4 | PS-23, PS-24, PS-15 |
| ADR-082 ET: both forms, references to purged CIDs kept, segment and prefix collisions, exit-3 pass | PG-46, CORE-11, PG-45, PG-41 |

## Binary changes → first scenario that needs them

B1 WS-1/PS-11 · B2 WS-1 · B3 WS-1, PS-13..16 · B4 PS-14, PS-17 · B5 DL-30..35 · B6 PG-* · B7 XD-2 ·
B8 WS-0, AS-50..54 · B9 AS-55 · B10 none (doc comment; review) · B11 RAC-1, XP-2 · B12 PS-18, PS-20,
PS-21 · B13 PS-10, PS-19 · B14 PS-12 · B15 DL-37.

## Suggested DELIVER order (one scenario enabled at a time)

1. WS-1 (B1-B5 minimal), WS-0 (healthz), WS-2.
2. PS-14, PS-13, PS-15, PS-16, PS-10, PS-11, PS-12, PS-25, PS-22.
3. DL-30, DL-31, DL-32, DL-33, DL-34, DL-35, DL-37.
4. CORE-1..3, CORE-11, then PG-40, PG-36, PG-42, PG-43, PG-41, PG-44, PG-45, PG-46, PG-47.
5. AS-50, AS-52, AS-55, AS-56, AS-54, AS-53.
6. PS-17, PS-18, PS-20, PS-21, PG-48, PS-19, PS-23, PS-24, PS-26 (robustness, needs the fault seam).
7. CORE-4..10 as each pure function lands; XD-1..6 with the check-arch rules.
8. RAC-1 (B11); XP-1..12 with the deploy artefacts.
