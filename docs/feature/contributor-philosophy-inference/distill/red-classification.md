# RED classification — contributor-philosophy-inference (pre-DELIVER gate)

Run 2026-09-27 against `main` (feature unimplemented):
`cargo test --no-fail-fast -p cli --test contributor_links --test infer_people --test infer_people_sign --test infer_people_evidence_grows --test scrape_person -- --ignored` (the un-ignored WS run separately).

Every GIVEN that uses only shipped verbs (`scrape github`, `claim add`, `claim retract`,
`peer add`/`pull`/`remove`, `PeerPds` retraction/counter/supersede markers) **succeeded**; no
scenario fails in setup, fixture, or compile. Classification per scenario:

| Scenario(s) | Failing step | Cause | Class |
|---|---|---|---|
| WS-CPI-1 | WHEN `infer people --sign 1` | `unrecognized subcommand 'infer'` | MISSING_FUNCTIONALITY ✅ |
| IP-1..IP-15, IS-2..IS-9 | WHEN `infer people …` | `unrecognized subcommand 'infer'` | MISSING_FUNCTIONALITY ✅ |
| EG-2/3, SP-1/2/5/8, IP-8, EG-4/5/6 | GIVEN "Maria signed the inference" (`infer people --sign`, a chained Pillar-2 step) | `unrecognized subcommand 'infer'` | MISSING_FUNCTIONALITY ✅ (chained on the slice-03 verb) |
| EG-9, EG-10 | WHEN `infer people` | `unrecognized subcommand 'infer'` | MISSING_FUNCTIONALITY ✅ |
| CL-3, CL-4, CL-11, CL-14 | WHEN `scrape github … --contributors N` | `unexpected argument '--contributors'` | MISSING_FUNCTIONALITY ✅ |
| CL-12 | THEN refusal says the flag is for `owner/repo` | clap's generic unknown-argument error lacks it | MISSING_FUNCTIONALITY ✅ (made non-vacuous this wave) |
| CL-1, CL-2, CL-5, CL-13 | THEN contributors block / links | no `Contributors recorded`, no `contribution_links` rows | MISSING_FUNCTIONALITY ✅ |
| CL-6, CL-7, CL-8 | THEN exit ≠ 0 | scrape never reads `/contributors`, exits 0 | MISSING_FUNCTIONALITY ✅ |
| CL-9, CL-10 | THEN `Contributors not recorded` notice | absent | MISSING_FUNCTIONALITY ✅ |
| EG-1, EG-8 | THEN hint `new inferred candidate` | absent | MISSING_FUNCTIONALITY ✅ |
| EG-7 | THEN `Contributors recorded: 1` (non-vacuity guard) | absent | MISSING_FUNCTIONALITY ✅ (made non-vacuous this wave) |
| SP-3, SP-6, SP-7 | THEN person-view text | today's user scrape prints "No candidate claims could be derived" | MISSING_FUNCTIONALITY ✅ |
| **SP-4** | — | passes today | GREEN-today regression guard (intended; shipped not-found behavior) |
| CL-15, IP-16, IS-10, EG-11, SP-9 | — | early return without `OPENLORE_LIVE_GITHUB=1` | live, opt-in (not part of the gate) |

Result: **0 BROKEN, 0 WRONG_ASSERTION** — gate passes. Only WS-CPI-1 is un-ignored.
