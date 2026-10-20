# DELIVER upstream issues — owner-only-signing

| ID | Step | Issue | Disposition |
|---|---|---|---|
| U-OOS-D1 | 01-03 | U-OOS-12 supersedes row cannot be built on `claim add` (no reference args). | Moved to PS-5 by DISTILL (4c5e7db); green in 01-05. |
| U-OOS-D2 | 01-05 | `infer people` listing says "You can sign" for any verified link, ignoring the 7-day window (listing runs on the real clock; PS-6 fixture check ~19 days old). Signing still enforces the window. | Accepted (orchestrator); revisit at review. |
| U-OOS-D3 | 01-06 | `infer people --sign` does not refresh the signer's link: shipped infer suites have no GitHub fake and a refresh reached real api.github.com. Only `scrape person --sign` refreshes. | Accepted; needs a GitHub fake in infer suites before adding. |
| U-OOS-D4 | 01-06 | PS-9 passed on activation; successful-refresh rows added by DISTILL (e65d144), mutation-verified. | Resolved. |
| U-OOS-D5 | 02-01 | `tests/acceptance/identity_link.rs` IL-13a pinned schema list ending at v7; now expects v7 then v8 (EXPECTATION, schema head moved). | Accepted (orchestrator). |
| U-OOS-D6 | 02-01 | No CLI-level test kills "probe skipped at startup" (same gap exists for the v7 identity-link probe). | Open: DISTILL scenario (break table → any verb → health.startup.refused) before mutation phase. |
| U-OOS-D7 | 02-03 | No AT kills "one GitHub read per claim instead of per repo" (RA-4 has one claim per repo). | Open: DISTILL row with two claims about one repo, before mutation phase. |
| U-OOS-D8 | 02-03 | CORE-OOS-3 oracle (unverified latest check -> LinkNotVerified regardless of login) disagrees with the may_sign_from_checks property (other login -> LoginNotCurrent); classify uses a reader-side override. Only the Unattested reason differs; permit decisions agree. | Accepted; revisit at review (unify the reason rule). |
| U-OOS-D9 | 02-03 | classify's new evidence inputs (LatestRepoObservation, RepoFinding, AuthorityFileSeen) are not in authority_evidence_only_from_adapters. | Open: add to the rule in refactor/review. |
| U-OOS-D10 | 02-05 | `review_app_repos_and_accounts_core.rs:457` exhaustive match on `permit.basis()` needed one `other => unreachable!` arm once PermitBasis gained Exempt/UngatedT1 (compile-only, no assertion changed). | Accepted (orchestrator). |
| U-OOS-D11 | 02-05 | `sign batch` now refuses a malformed `github:` subject (e.g. `github:a/b/c`) instead of signing it ungated — fail-closed per DWD-OOS-7, matches `claim add`. | Accepted (orchestrator): intended behaviour. |
