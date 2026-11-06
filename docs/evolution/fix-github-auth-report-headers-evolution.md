# fix-github-auth-report-headers — evolution

Delivered 2026-11-05 through /nw:bugfix. The RCA is in `docs/feature/fix-github-auth-report-headers/rca.md`.

## Defect

With `GITHUB_TOKEN` set, `scrape person` still printed "Note: without GITHUB_TOKEN, GitHub allows 60 requests an hour…", and `scrape github` reported `unauthenticated`. The adapter built its auth report from a JSON `"auth"` key in the response body. Only the test fake sent that key; real GitHub reports the budget in the `X-RateLimit-Limit` and `X-RateLimit-Remaining` headers. Every test ran against the fake, so none caught it.

## Fix (4a0ce70, tests strengthened in ef93c58)

- **`client::auth_report_of`** (pure): `Authenticated { remaining, limit }` only when a token was configured and both header values parse as `u32` after trimming; otherwise `Anonymous`. `parse_auth_report` is gone.
- **`observe_auth_report`** (in `send_get_once`): records that report on each 200 carrying both headers. The last such 200 wins; a 200 without both headers and any non-200 leave the earlier report alone. The 401 `TokenRejected` path is unchanged.
- **FakeGithub:** sends the rate-limit headers on every 200 (the person profile and repo list too). It reports the authenticated budget only when the request carried the token, and 60/59 otherwise. The `"auth"` body key is gone, also from `fake_github_accounts.rs`.
- **Unchanged:** the `GithubPort` contract (ADR-019 §1), the verbs and their output text, token handling.

## Evidence

- **RED:** `scrape_person_with_github_token_set_prints_no_unauthenticated_budget_note` (SP-4b) failed before the fix. Stdout showed `unauthenticated` and the note.
- **After the fix:** SP-4b asserts `authenticated (4982/5000 rate budget)`, no note, and that the fake saw the token. The adapter tests run over real TCP with a path-routed stub that records each request's `Authorization` header. A proptest and example cases cover `auth_report_of`.
- **Tests:** `cargo test -p adapter-github` 61 passed. The scoped nextest run (scrape, adapter-github, openlore-test-support) passed 238 of 238. A wider run over the GitHub, person and contributor binaries passed 350, with 3 skipped. Clippy, fmt and check-arch are clean. The full workspace suite was not run locally; CI runs it on the next push.
- **Mutation (per-feature):** cargo-mutants found 2 mutants on the new functions: 1 caught, 1 unviable. Of 4 hand mutants on the first commit, 3 were caught and one survived (recording without both headers). The revision added tests for it, and every hand mutant it checked was caught: dropping the `Authorization` header, recording without both headers, recording on any status, and removing `.trim()`.
- **Review:** REJECTED on test strength (D1 to D3, D5), with the production change judged correct. One revision pass fixed those four items. There was no second review.
- **DES integrity:** 1 of 1 step has a complete trace.

## Follow-ups (not done)

- **D4:** if a configured token gets a 200 without parseable rate-limit headers (for example behind a header-stripping proxy), the report is `Anonymous` and the note prints. Real GitHub always sends the headers. A possible fix is an `AuthReport::Unreported` variant rendered as "authenticated (budget not reported)". That changes output text, so it needs a decision.
- **D6:** the thread-local report depends on the current-thread runtime and is cleared only by reading it.
- **D7:** `auth_report_of` takes `Option<&str>` though its one caller always passes `Some`, and the crate-wide `#![allow(dead_code)]` hides dead code from the compiler.
- A whitespace-only `GITHUB_TOKEN` counts as set (`client.rs`), unlike an empty one.
- Tests rejected: the CONTRACT_SHAPE declaration markers (D0), which no Rust test in the repo uses.

## Process

- `.nwave/des-config.json` lists 5 TDD phases; the step was logged with the 3-phase names (RED, GREEN, COMMIT), as in earlier features.
- The roadmap's test command named `package(test-support)`; the package is `openlore-test-support`.
