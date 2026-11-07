# fix-review-app-repos-link — evolution

Delivered 2026-11-06 through /nw:bugfix. The RCA is in `docs/feature/fix-review-app-repos-link/rca.md`.

## Defect

The review app's signed-in queue had no link to "Choose repos" (`/repos`), where an owner ticks repos and adds a public repo by `owner/name`. Only typing the URL reached it. The journey and US-RRA-001 say the owner presses "Choose repos" on the queue, but the design and DISTILL pinned only `GET /repos`, and every acceptance step opened the page by URL, so no test noticed the missing link.

## Fix (deba179)

- **`views/queue.rs`:** `choose_repos_link()` renders `<a href="/repos">Choose repos</a>`. It is in the queue footer next to Settings for every signed-in state (linked or not), and inside the NothingToScan alert.
- **`views/repos.rs`:** `CHOOSE_REPOS_LABEL` names the page title and heading. The page already linked "Back to your review queue".
- **Unchanged:** routes, the store, budgets, the scan pipeline, htmx.

## Local preview (303a485)

`./review-app-preview.sh` runs the ignored `launch_local_preview` test (`tests/acceptance/review_app_local_preview.rs`). It starts the debug review app with the existing loopback test seam, the fake ATProto network (directory, PDS and OAuth server) and fake GitHub on 127.0.0.1, prints a sign-in link per fixture persona, and stops on Ctrl-C or stdin EOF. A non-ignored smoke test walks the same world: sign in, verify GitHub, queue, "Choose repos", scan.

- The app's own "Sign in with Bluesky" form does not work in a real browser on the preview. The CSP `form-action 'self' https:` (`crates/openlore-review-app/src/http.rs:53`) makes Chrome block the form's 303 to a plain-HTTP PDS. Production PDSes are https, so production is unaffected. The launcher adds a test-only sign-in link that POSTs the handle itself and redirects the browser with a GET. The harness Browser ignores CSP, so the suites can't see this.
- Chrome keeps the `__Host-` session cookie on http://127.0.0.1. Firefox and Safari were not tried.

## Evidence

- **RED, 01-01:** both new acceptance tests failed with `no button or link labelled "Choose repos"`. The two new page contracts failed on their assertions (32 passed, 2 failed).
- **RED, 01-02:** the smoke test failed in the stubbed setup (`not yet implemented: RED: the local preview world is not built yet`).
- **After:** `cargo test -p review-domain` all pass (page_contracts 34). `cargo nextest run -p openlore-review-app` 299 passed, 2 skipped (the launcher and one existing skip). Clippy, fmt, shellcheck and check-arch are clean.
- **By hand:** the preview answered `/healthz` and `/` with 200; Ctrl-C to the process group and stdin EOF both left no processes. A headless Chrome walk signed in, verified GitHub and reached `/repos`.
- **Mutation (per-feature):** 6 mutants on the changed review-domain code, all caught (4 by review-domain tests; the 2 whole-page `repos_page` replacements by the review app's acceptance tests).
- **Review:** REJECTED with 3 major and 5 minor items, no blockers. One revision (727c716) made the queue-state contract exhaustive over `ScanRefused` (a match that breaks on a new variant) and `ScanOverLimit`, documented which states render no notices, added timeouts and a thread per connection to the sign-in link, labelled the banner checks as formatting only, and made the script's help read the comment block. Ctrl-C cleanup had already been checked by hand; the RED text is recorded here. There was no second review.
- **DES integrity:** 2 of 2 steps have complete traces.

## Follow-ups (not done)

- The preview's sign-in form: a dev-only CSP relaxation behind the loopback seam would make the button work locally. It is a production code change, so it needs a decision.
- "0 repos read" for a run stored before the repos-and-accounts migration is misleading, and `scan.rs` discards `mark_repo_read` and `record_checks` errors with `let _ =` (RCA, out of scope). Whether the production screenshot is a pre-migration run is unverified.
- No browser-level test presses the link inside the NothingToScan alert; a page contract pins it.
- Killing only the launcher process (not its group) would orphan the review app child.

## Process

- The step logs use the 3-phase names (RED, GREEN, COMMIT), as in earlier features; `.nwave/des-config.json` lists 5.
- No DISTILL scaffold existed, so the crafter wrote the regression tests, with the orchestrator's approval.
