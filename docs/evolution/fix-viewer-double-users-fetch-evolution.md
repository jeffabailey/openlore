# Evolution: fix-viewer-double-users-fetch

- **Type**: bug fix (`/nw:bugfix`: RCA → user review → regression test + fix)
- **Date**: 2026-09-28
- **Commits**: `f7b66b4` (regression test V-S5 + fix), `9b700b3` (dead-code removal)
- **Origin**: follow-up recorded in
  [contributor-philosophy-inference](contributor-philosophy-inference-evolution.md)

## Defect

The read-only viewer's `POST /scrape` made **two** `GET /users/{user}` requests for a user
target. `propose_candidates` (`crates/adapter-http-viewer/src/lib.rs`) called
`GithubPort::resolve_target`, which already reads the profile, and then
`GithubPort::harvest_user`, which read it again and returned no signals. The CLI's
`scrape github <user>` had been fixed to a single request in contributor-philosophy-inference
step 05-01 (`770efe9`); the viewer had not.

## Root cause

- 05-01 moved the single fetch into the shared `resolve_target` but its `files_to_modify`
  covered only the CLI, so the second driving adapter (the viewer) kept the old two-step path.
- The "exactly one request" budget (slice-05 AC, SP-5) was tested only through the CLI. No
  viewer test used a user target or read FakeGithub's request log, so nothing failed.
- The second fetch had no useful side effect in the viewer: the auth report it recorded is
  never read there. It only spent rate budget and added a second failure point.

## Fix

1. **01-01**: regression **V-S5** `a_viewer_user_scrape_asks_github_only_for_the_profile_once`
   in `tests/acceptance/viewer_scrape.rs`. RED on the old code with
   `left: ["/users/BurntSushi", "/users/BurntSushi"]`; it also pins the previously unpinned
   user-target render ("No candidate claims could be derived"). The viewer's user arm now
   derives no signals without another request.
2. **01-02**: removed the now-dead `GithubPort::harvest_user` from `ports`, `adapter-github`
   and the FakeGithub docs. Its WD-64 purpose (a cross-repo user aggregate) is superseded by
   contributor-philosophy-inference D-4/D-6; noted in
   `docs/feature/openlore-github-scraper/design/wave-decisions.md`.

Viewer output is unchanged, except that a failure on what used to be the second fetch can no
longer turn a user scrape into an error page.

## Quality

- DES: both steps have complete traces. 01-01 skipped RED_UNIT (one-line wiring change driven
  by V-S5); 01-02 skipped both RED phases (dead-code removal guarded by the compiler and V-S5).
- CI green on both commits (runs 36510727558, 36516709691).
- Refactor, adversarial review and mutation testing were not run: the change is one match arm
  in an adapter plus an API removal, with no pure-core logic to mutate.

## Follow-ups

- **Repo targets still fetch `/repos/{o}/{r}` twice** (once in `resolve_target`, again in
  `harvest_repo`), in both the CLI and the viewer. This is within D-6, which only caps the
  extra `/contributors` request, but it is the same waste.
- **Request budgets are asserted per driving port.** SP-5 (CLI) and V-S5 (viewer) now cover
  user targets. A shared check across both ports would stop the next port-specific gap.
- Local viewer acceptance tests hit occasional transport errors ("error sending request") on
  the slow host; reruns and CI are green.
