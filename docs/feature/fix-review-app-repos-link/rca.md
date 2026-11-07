# RCA: the queue has no way to reach "Choose repos"

Date: 2026-11-06. The signed-in queue ("Your suggestions") shows "Scan again", "Sign out" and "Settings", but nothing leads to `/repos`, where an owner ticks repos and adds a public repo (US-RRA-001, US-RRA-005). In production the page reads "Ownership re-checked ✓ · 0 new suggestions · 11 already published" and "0 repos read".

## Root cause

The entry link was specified but never built, and the acceptance step that stands for "opens Choose repos" types the URL. No test ever followed a link to the page.

- Specified. The journey step reads "On the queue, press 'Choose repos'" (`docs/feature/review-app-repos-and-accounts/discuss/journey-choose-repos-and-accounts.yaml:86`). US-RRA-001's "After" reads "Priya presses "Choose repos" on her queue" (`discuss/user-stories.md:130`).
- The design and DISTILL kept the route and dropped where you enter from. The page contract pins `GET /repos` (`distill/wave-decisions.md:57`, DWD-RRA-2; `distill/walking-skeleton.md:35,50`). The design lists `routes/repos.rs` but says nothing about a queue link (`design/architecture-design.md:76`, `design/component-boundaries.md:120`).
- Never built, never removed. `git log -S '/repos' -- crates/review-domain/src/views` finds only 170d81a, 592846f, f3d1273 and 417ef72. Each touches `views/repos.rs` only. f3d1273 changed `queue.rs` without adding `/repos`. `git log -S REPOS_PATH` finds the same two `repos.rs` commits. The only `href` to `REPOS_PATH` is the Cancel link on `/repos` itself (`views/repos.rs:198`). The queue's links are `/github` and `/settings` (`views/queue.rs:622, 712, 718, 728, 738`). Settings links only `/settings/forget` (`views/settings.rs:24`).
- Why no test caught it: the step `when_opens_choose_repos` is `browser.open("/repos")` (`tests/acceptance/support/review_app/repos_and_accounts.rs:791-793`). It is called 31 times, and three more tests `open("/repos")` directly (`review_app_repos_and_accounts_imports.rs:807,818`, `review_app_repos_and_accounts_several.rs:1163`). The harness can follow a link by its text: `Browser::press` falls back to `<a href>` links (`support/review_app/browser.rs:275-282`). No step uses it for this page.
- Why DELIVER didn't add it: the roadmap made `tests/acceptance/support/*` DISTILL-owned and forbade new helpers (`deliver/roadmap.json:16`). Nothing failed, so nothing asked for the link.
- Also, `/repos` has no link back to the queue. `views/repos.rs` has no `/review` href, even though `views/mod.rs:92-94` has a `back_to_queue()` helper.

## "0 repos read" is not the NothingToScan path

- Intended behaviour. The "Choose at least one repo, or name a public repo, before scanning." notice is only for an owner who saved an empty choice and has no imports (AC-002.5, `user-stories.md:303,363`). An owner who never chose gets today's top 5 by default (`user-stories.md:344-347`). The code does the same. `scan_intent` returns `NothingToRead` only when there are no imports and no account has anything to read (`crates/review-domain/src/repo_scope/plan.rs:116-118`). An account with no saved choice counts as having something to read unless it is known to have zero eligible repos (`plan.rs:131-135`). `scan.rs:187-193` always passes `eligible_seen: None`. So with no saved choice, a verified owner never sees NothingToScan. That is by design.
- What the screenshot most likely shows (Hypothesis, needs checking). The latest run was stored before the repos-and-accounts migration, not by a scan that has run since. Evidence:
  - "Ownership re-checked ✓" without a login means `latest_account_checks` returned nothing (`views/queue.rs:246-251`, `routes/review.rs:74-85`). A v2 scan always records each account's check (`scan.rs:445`). A pre-v2 row has `summary` NULL (`adapter-review-store/src/schema.rs`, `ADD_SCAN_RUN_SUMMARY`).
  - "0 repos read" counts `scan_run_repos` rows with status `read` (`scan_runs.rs:80-85`). A pre-v2 run has no plan rows.
  - "11 already published" can only come from `scan_repo` counts (`scan.rs:649-671`). A v2 run that reached those counts would also have marked at least one row read.
  - Check: query the latest `scan_runs` row for that DID (`summary IS NULL`? any `scan_run_repos` rows?). If a new run does exist, the fallback is that `record_checks` and `mark_repo_read` failed silently. Both results are thrown away with `let _ =` (`scan.rs:669`). That would be a separate defect.
- Small display gap (follow-up, not this fix): `repos_read_message` is shown for any completed run (`views/queue.rs:651-653`), including runs from before plans existed, where "0 repos read" is misleading.

## Fix proposal

- `views/repos.rs`: add `pub const CHOOSE_REPOS_LABEL: &str = "Choose repos";` and use it for the existing page title and `h1` (`repos.rs:279,281`). There is no such constant today.
- `views/queue.rs::review_page_of_scan`: add `p { a href=(REPOS_PATH) { (CHOOSE_REPOS_LABEL) } }` to the footer every signed-in owner sees, next to "Settings" (`queue.rs:738`). Show it always, not only once GitHub is verified. `GET /repos` needs only a session (`routes/repos.rs:33-36`), and imports work without an account (`queue.rs:711`, D-RRA-8).
- `views/queue.rs`: add the same link inside the NothingToScan alert (`queue.rs:639-641`), so the refusal's fix is one click away.
- `views/repos.rs::repos_page`: add `back_to_queue()` (make it `pub(crate)` if needed).
- Plain maud `<a href>`, no htmx. The queue's `hx-select` region (`#queue`) is not involved.

## Regression check

- Acceptance, in `tests/acceptance/review_app_repos_and_accounts.rs` next to the US-RRA-001 scenarios (around line 92): sign in a verified owner (`given_owner_verified`), `open("/review")`, then `press("Choose repos")` (follows the link by its text, `browser.rs:275`). Assert the page has the "Choose repos" heading and the "Save choice" form for the account. Then `press("Back to your review queue")` and assert "Your suggestions". Today it fails with `no button or link labelled "Choose repos"`. It must not use `open("/repos")`.
- A second case for an owner with no account and one import: the link is on the queue.
- Better still, a new support step `when_follows_choose_repos_from_queue`, used in place of `when_opens_choose_repos` in the walking skeleton. Support is DISTILL-owned (`roadmap.json:16`), so this goes to nw-acceptance-designer.
- Optional page contract in `crates/review-domain/tests/page_contracts.rs`: every `review_page_of_scan` state links `REPOS_PATH`.

Files: `crates/review-domain/src/views/queue.rs`, `crates/review-domain/src/views/repos.rs`, maybe `views/mod.rs` (helper visibility), `tests/acceptance/review_app_repos_and_accounts.rs`, maybe `tests/acceptance/support/review_app/repos_and_accounts.rs`.

Risk: low. The change only adds markup. Assertions that match an exact link list or exact page text could need updating. Check `page_contracts.rs` and `html::links` users. Opening `/repos` is counted (`ReposChoiceViewed`) and charged against the per-DID page-read allowance (`budget.rs:151`). A link makes visits more likely but changes no limit.

## Out of scope

- The pre-v2 "0 repos read" display, and the `let _ =` on `mark_repo_read` and `record_checks` (above).
- "Before anything is scanned, verify that your GitHub account is yours." on the unlinked queue (`queue.rs:716`), now that imports work without an account.
- A local preview (feasibility only). It is feasible. `FakeAtprotoNetwork` serves the directory and each PDS, which is also its OAuth server, on `127.0.0.1` (`crates/test-support/src/fake_atproto.rs:1-35`). `GET /oauth/authorize` approves by default (`ConsentPosture::Approve`, `fake_atproto.rs:413`) and answers with a 302 to the app's loopback `redirect_uri` (`fake_atproto.rs:1359-1385`). A real browser follows the whole sign-in with no consent screen. `ReviewApp::start` already launches the debug binary with `REVIEW_APP_ALLOW_LOOPBACK_HTTP=1` and all upstream env vars (`support/review_app/app.rs:329-346`). A release build refuses that variable (`config.rs:117`). The cheapest place for a launcher is an `#[ignore]` test registered in `crates/openlore-review-app/Cargo.toml`, run with `cargo test … -- --ignored --nocapture`. It would build `ReviewWorld::new()`, print the origin and a fixture handle, and block until Ctrl-C. That reuses `world.rs::settings_for` and the fixtures with no new dependencies. An xtask or an example would need dev-only `test-support` dependencies, and `tests/acceptance/support` is not a crate. Caveat to check: Secure or `__Host-` session cookies on `http://127.0.0.1`. Chrome and Firefox treat loopback as a secure context, but Safari may not.
