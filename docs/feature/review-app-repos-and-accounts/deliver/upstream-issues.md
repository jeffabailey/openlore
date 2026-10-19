# Upstream issues: review-app-repos-and-accounts (DELIVER)

| # | Issue | Source | Ruling | Binds |
|---|---|---|---|---|
| U-RRA-D1 | CORE-RRA-5's test loop feeds only `read` rows back between rounds, so a row skipped for a failed bio check comes back unread and the property reads it again. To pass, 01-07 kept a failed account's rows planned (read after a later passing check) instead of skipping them as unverified (design §6.2 step 8) | 01-07 (7406405) | DISTILL fixes the property to feed back every row's status; production returns to the design rule in 02-03 | 02-03 |
| U-RRA-D2 | Resumed rows lose their outside-default tag: `scan_run_repos` has no column for it, so KPI-RRA-1 (`suggestion.acted.outside_default`) undercounts suggestions from a resumed scan | 01-07 | Add the tag to the frozen plan row (ADR-088 table) in the next step that writes plan rows | 02-03 |
| U-RRA-D3 | The forks-only state offers an "Add repo" form posting to `/repos/imports`, which exists only from R3; R1 alone would serve a 404 | 01-03 | R1–R3 deploy together (no R1-only deploy); verify at R3 closure | 03-01 |
| U-RRA-D4 | Each Save lists every account's repos from GitHub again to compute the limit total (rate budget cost) | 01-04 | Review at the adversarial review; acceptable for v1 if within the per-scan budget | Review |
| U-RRA-D5 | Declines of outside-default suggestions are not counted by KPI-RRA-1 ("act on = approve, edit or decline") | 01-02 | Fix with the KPI registration | 03-06 |
