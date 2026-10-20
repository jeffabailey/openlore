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
