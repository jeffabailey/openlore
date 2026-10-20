# Acceptance Review — owner-only-signing (DISTILL)

Reviewer: Sentinel (`nw-acceptance-designer-reviewer`, read-only), critique dimensions 1-10.
Two cycles maximum.

## Cycle 1 (2026-10-20) — needs_revision (2 blocker, 5 high, 6 medium, 11 low)

Strengths noted: refusal tables pin reason, who-can-sign and nothing-written with a fail-closed
universe; layer-2 oracles come from the ADR-087 table, CORE-OOS-5 calls the real scorer; RD-4 has
a non-vacuous precondition; the walking skeleton passes the litmus test; gaps disclosed honestly.

| Finding | Sev | Resolution |
|---|---|---|
| F-01 IC-2 weighted view vacuous (empty store) | blocker | Fixed: IC-2 seeds Rachel's attested dependency-pinning claim on anvil, asserts anvil is weighted before the import, asserts the imported-candidate count grew |
| F-02 no reader-side scenario for G-2 (old login reads third-party) | blocker | Fixed: RD-10 (`owner_only_signing_readers.rs`) — after the rename and re-check, Jeff's claim about `github:jeffbailey` is shown, reads `third-party`, nothing withdrawn |
| F-03 contested id: second DID's signing untested | high | Recorded U-OOS-13: the CLI harness signs only as Jeff's identity; CORE-OOS-3 requires Owner for each verified DID. PS-7's Given matches the code (`identity verify` re-checks every link in the store) |
| F-04 RD-4 When had three actions | high | Fixed: publish/pull/re-check moved to Givens; When = "Jeff asks for the weighted view" |
| F-05 non-author retraction not covered | high | Fixed: RD-4 row "a withdrawal of Jeff's claim (only its author may withdraw it)" — weight unchanged |
| F-06 Dim 7 storage assertions (RA-12 exact schema list; RA-1/RA-4 row dumps) | high | RA-12 now asserts v7 and v8 present once each (survives a v9). RA-1/RA-4 keep the observation rows as the documented SECONDARY guardrail (DWD-OOS-12); their primary Thens are the signed claim and the labels |
| F-07 REQUIRED GitHub fixture contract test untracked | high | U-OOS-8 now a tracked DELIVER blocker (adapter test in `crates/adapter-github`; untracked at slice-2 start = blocker) |
| F-08 error-share headline overstated | medium | Counting rule stated; strict refusal-row share (≈ 43 %) reported beside the 69 % fn share |
| F-09 one walking skeleton | medium | DWD-OOS-17 records the single-WS justification |
| F-10 import refusal rows do not pin the observation delta | medium | Accepted as documented (DWD-OOS-12); RA-1/RA-4/RA-6 pin observations where they are the point |
| F-11 no story tags in `.rs` | medium | Story map S-OOS-1..9 in `test-scenarios.md` §3; `@s-oos-7`/`@s-oos-9` added on rewritten scenarios; full tagging left to DELIVER when a DISCUSS exists (U-OOS-11) |
| F-12 reset instant not pinned | medium | Not fixed: the double's reset is "now + 1 h" at request time, so the instant is not stable across the run; the refusal must still say "retry after" (pinned) |
| F-13 two asserted phrases not in the pin list | medium | Fixed in DWD-OOS-3 |
| F-15 @security missing on PS-7, RD-7, RA-8 | low | Fixed |
| F-16 adapter jargon in RA-3 rows | low | "bytes not text" → "not plain text"; symlink/submodule/directory kept as the lie-catalogue names in Examples |
| F-18 red-classification.md missing | low | Written |
| F-19/F-20 environments, elevator-pitch continuity | low | U-OOS-14 |
| F-21 C7c credited on absence | low | C7c row cites the DESIGN data-model text; strict reading 13/14 applicable recorded |
| F-22 SA-1 locates the draft by field name | low | Kept: same selector as the shipped SH-1 (`review_app_consent_and_share.rs`) |
| F-23 IC-4 pins a proposal | low | Ignore reason names the U-OOS-6 ruling as a precondition for enabling |
| F-24 `authority verify --mine`, layer-3 supersedes refusal | low | U-OOS-12 (DELIVER adds a row each) |
| F-14, F-17 | low | Accepted (the link command is pinned in `CanSign`; slice 2 size is informational) |

Also found and fixed during the RED gate (not by the reviewer): `rows_about` read one line per
claim but readers print multi-line entries; two followed peers in one store fail `peer pull`
(DWD-OOS-18).

## Cycle 2 (2026-10-20) — APPROVED (0 blocker, 0 high)

F-01, F-02, F-04, F-05, F-06 verified fixed in the test bodies; F-03 accepted as recorded
(U-OOS-13); F-07 tracked as a DELIVER blocker (U-OOS-8); PS-3 verified active; `@contract-shape`
on all 39 example scenarios.

| Finding | Sev | Resolution |
|---|---|---|
| N-1 RA-12 Gherkin promised more than the test checks | low | Gherkin reworded: "the store records the identity-link and repo-authority upgrades, once each" |
| N-2 RED table predates the fixes | medium | Already current: the `red2` run after the fixes re-ran all of `owner_only_signing_readers` (incl. RD-4, RD-10) and RA-4 / IC-2 / RA-12 / RA-14; `red-classification.md` uses those lines. Default run after all edits: green (PS-3 passes) |
| N-3 counts not recounted | low | Informational; `@security` recounted by grep = 18 |

Review complete (2-cycle limit).

## Review iteration 1 (2026-10-20) — four-reviewer gate (PO, SA, Forge, Sentinel) rejected; fixed

Orchestrator rulings R1-R11 and the user's acceptance of P-OOS-1..3 (relayed by the coordinator)
are recorded in `wave-decisions.md` § Review iteration 1.

| Finding | Ruling | Fix |
|---|---|---|
| SA blockers 1-2, Sentinel B-1 — store universe as primary evidence | R1 | GDL D-e adopted exactly: `WhatJeffSees` port view (`graph query`, `identity list`, PDS wire) behind `assert_jeff_sees_no_change` / `assert_nothing_signed_or_published` / `assert_signed_and_kept_locally`; store universe renamed `guardrail_*`, labelled SECONDARY, slots renamed (`authority.repo_observations`, `candidates.imported.count`); explicit Mandate-8 waiver section |
| SA — RA-12 evidence from the schema table | R1 | primary = both pre-feature claims listed + second open prints identical output; schema versions and prior rows are secondary guards |
| SA blocker 3 — GitHub request-log assertions | R2 | kept, rationale recorded (external side effect with a budget, GDL D-VL-T13); only user-meaningful counts; RA-4 Gherkin now "asked once for each repo and once for its authority file"; exact redirect policy → U-OOS-8 fixture test |
| Forge 3 — sign-time GitHub outage | R3 + a600ad3 | PS-9 (person path, 3 rows) and RA-13 rows 3-5 (repo path): refresh fails → latest local check decides; RA-6 now two rows (never checked → refused with retry + re-check command; checked 1 Oct → signed). Double: `rate_limit_profile_after` / `drop_profile_after` + unit test |
| Forge 2 — token budget | R4 | RA-16 (anonymous exhausted, token exhausted, token rejected): "GitHub could not be asked", nothing recorded, token never shown |
| Forge 1 — no v7 store fixture | R5 | `tests/fixtures/owner-only-signing-v7-store/` generated from main (v7) with regenerator + self-check; RA-12b (3 rows: `identity list` matches the v7 golden; sign at 2 days passes; at 7 d + 1 min refused). EXPORT DATABASE dump, so no `.duckdb` and no ignore exception needed |
| Forge 5 — publish offline | R6 | RA-11a network-off row |
| Forge 8 — listed-repo window edges | R6 | RA-10 rows: anvil at 7 d (signed) and 7 d + 1 min (refused) |
| Forge 4 — downgrade | R6 | out of scope (project policy: forward-only stores) |
| Forge 6 — acceptance build time | R6 | DELIVER action recorded |
| SA high / Sentinel M-4 — spec-ambiguity gates | R7 → P-OOS-1..3 accepted | RD-11 added (P-OOS-1); SA-1 aligned (P-OOS-2); IC-4 aligned, asserts the latest import's date (P-OOS-3); `@spec-ambiguity` dropped; U-OOS-4/6/7 marked resolved |
| PO F-04 — review-app publish not covered | R8 | cited shipped tests (`no_request_publishes_a_suggestion_about_a_repo_the_owner_doesnt_own`, AP-1/2/4, permit-alone, GDL slice 3 prompt) under S-OOS-8 / KPI-OOS-1 |
| PO F-05 — imported candidates vs inference | R9 | IC-5 added, ACTIVE (green today) |
| PO F-06 / F-07 — C-OOS-8 / C-OOS-4 enforcement | R9 | xtask structural rules mapped in `test-scenarios.md` §4 |
| PO F-08 / Sentinel M-2 — story tags | R9 | `@s-oos-N` on all 44 scenario doc comments and the 6 core properties; IDs unchanged |
| Sentinel H-1 — PS-3 asserts only a count | R10 | wire body asserted: counter references Rachel's CID about `github:rachelk`; retraction references Jeff's CID about `github:jeffbailey`; author Jeff |
| Sentinel H-2 — CLI strings in Examples | R10 | RA-3 / RA-5 Examples in user terms ("a shortcut to another file", "a folder instead of a file", "a link to another project", "isn't plain text", "a branch other than the main one", "sends him to another site"); RA-3 outcomes "not listed" / "not a usable authority file" mapped to the pins in the doc comment |
| Sentinel H-3 — guards that hold today | R10 | PS-7 un-ignored (green); RD-5 re-checked, GIVEN-blocked, stays ignored; rule recorded in DWD-OOS-2 |
| Sentinel M-1 — first-failure lines | R10 | full re-run of all 47 ignored tests; full lines in `red-classification.md`, GIVEN-blocked marked; IC-1/3/4, RA-4, RA-14 fail on the missing verb in their WHEN |
| Sentinel M-3 / SA medium — contract-shape tags | R10 | no CLI scenario is `pure-function` any more (RA-9 → bounded-change; IC-1, IC-4, RD-1, RD-5, RD-7, RD-8, SA-1 → unbounded-preservation); core properties `pure-function` |
| Sentinel M-5 — CORE-OOS-4 oracle | R10 | decision table `GateRow` G1-G8 from ADR-086 §5 / ADR-087 §7, anchored by CORE-OOS-4x; layer-2 CM-A exception documented in the core module doc |
| Sentinel L-1 — one `authority verify` command | R10 | rationale recorded (one gate needs both check kinds; `identity verify` stays link-only) |
| Sentinel L-2 — repeat publish from one peer | R10 | verified: `a_link_its_peer_author_retracted_no_longer_shows_or_is_rechecked` (two publishes from Rachel) passes; RD-4 unchanged |
| SA low — completeness summary, counts | R11 | 14 of 14 applicable items pass (C7c N/A); 50 test fns, 44 example scenarios, 93 rows recomputed |

Wrong-reason failures found while re-running (fixed before hand-off): `jeffs_entries` counted
three lines per claim; PS-9 / RA-2 person row had no person candidate in their world; IC-5
matched the full philosophy id; RA-12b ran its permitted row first. Details in
`red-classification.md`.
