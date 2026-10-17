# Upstream issues — github-did-identity-link, slice 2

Raised by the solution architect while planning phase 02 (roadmap d4a811f; steps renumbered by revision d2672a2).
Rulings are the orchestrator's, 2026-10-17. The user can overturn any of them; each names the step it binds.

| # | Issue | Source | Ruling | Binds |
|---|---|---|---|---|
| U-1 | IR-1..IR-7, IR-8b already pass before any slice-2 code | `distill/red-classification.md` is stale for them | Un-ignore them as guards inside 02-01; no validation-only steps | 02-01 |
| U-2 | The slice-1 evolution doc listed `score(F) == score(F \ sameAs)` as shipped; scoring has no drop yet | evolution doc | Doc corrected to "intended, lands in 02-03" | — |
| U-3 | ADR-085 §6 (owner-only-signing revision) places the drop in `authority::kind` inside `score_attested`; neither exists yet | ADR-085 §6 | Drop in `scoring::score` now; it moves with `score_attested` when owner-only-signing lands. Same invariant, same tests (CORE-6/6x, IR-14/15) | 02-05 |
| U-4 | UC-8 "zero extra requests" needs the `/users/{user}` response, which `resolve_target` discards and reads leniently | UC-8, slice-1 D1 | Keep the already-fetched `/users/{user}` body and parse it strictly (`client::parse_account_profile`, the parser behind `read_account`); never a second GET, never the lenient `parse_person_profile`. The decision goes through `check_link`. *Amended after roadmap review F3.* | 02-07 |
| U-5 | A person scrape whose profile account number differs from the link's | unspecified | Record nothing; `identity verify` decides (it already handles renames and re-registrations) | 02-07 |
| U-6 | How person views match a link to a person | unspecified | By numeric account id when the view has it, else the case-insensitive last-seen login | 02-06 |
| U-7 | Search label scope: D-c says every `github:` row, ADR-085 §8 says every `github:` assertion; IR-16 tests link rows only | D-c, ADR-085 §8 | Label link rows only in slice 2; widening to all `github:` rows is a follow-up | 02-08 |
| U-8 | Does the 30-day re-check hint apply to "proof gone" lines? | IR-9 | Verified lines only; a "proof gone" line already tells the reader what happened | 02-04 |
| U-9 | GDL-D9 picks the duplicate with the newest `composedAt`, but `LinkCandidate` has no `composed_at` and IR-10 cannot observe the pick | GDL-D9 | One line naming every record ID (what IR-10 pins); the pick order is the record ID order, so it is deterministic. `composedAt` ordering is deferred until a scenario needs it | 02-03 |
| U-10 | `scrape person` shows link lines through the shared person view but does not refresh checks | — | Accepted for slice 2 | 02-06 |
| U-11 | The ADR-084 peer-pull CID round-trip fixture (F14) has no test | design wave-decisions F14 | Closed: written as IR-19 by the acceptance designer, using the production `self_attested_link_record`; a guard in 02-01 | 02-01 |

| U-12 | 02-07's new `github_profile_contract.rs` case and the check-arch synthetic-violation test fall outside `test_edit_allow_list` | roadmap revision | The crafter writes them, as in slice 1: the crafter owns adapter contract tests and xtask rule tests; the acceptance designer owns `tests/acceptance` | 02-07 |
| U-13 | 02-03 needed a read accessor in `verbs/identity_reads.rs` (not in its files_to_modify) so record IDs come from the same lineage read; and the record-ID wording ` (record <id>)` / ` (records <id>, <id>)` is not pinned by `support/identity_links.rs` | DELIVER 02-03 | Accepted (no second lineage read). The wording goes to the acceptance designer to pin in `support/identity_links.rs` before the slice-2 review | 02-03 |
| U-14 | 02-06 also edited `verbs/identity_list.rs` (`latest_of` moved to `identity_reads.rs`, not copied) and `verbs/scrape_github.rs` (`person_view`, shared by `scrape person` and `scrape github <user>`, now reads link lines through read-only handles). In the person view the lines sit under the repo links; in `infer people` they sit inside each candidate block | DELIVER 02-06 | Accepted: both read-only, no check write, refresh still only in 02-07 for `scrape github <user>` (U-10) | 02-06 |
