# RCA: the "without GITHUB_TOKEN" note prints when the token is set

Date: 2026-11-05. With `GITHUB_TOKEN` exported, `./cli.sh scrape person <login>` still prints:

> Note: without GITHUB_TOKEN, GitHub allows 60 requests an hour; scraping 5 repos makes about 40. Set GITHUB_TOKEN to raise the limit to 5,000.

`scrape github` also reports `unauthenticated` with a token set.

## Root cause

The adapter decides whether a request was authenticated from a JSON `"auth"` key in the response body. Real GitHub never sends that key; only the test fake does.

- `parse_auth_report` returns `AuthReport::Anonymous` when the body has no `"auth"` key (`crates/adapter-github/src/client.rs:105-108`). It does not look at whether a token was configured or at the response headers.
- It is called on four response bodies (`crates/adapter-github/src/lib.rs:286, 313, 405, 556`), and the verbs read the result through `take_last_auth_report()`.
- `scrape person` prints the note when that result is `Anonymous` (`crates/cli/src/verbs/scrape_person.rs:97`). The condition is right; its input is wrong.
- Only FakeGithub adds `"auth": {"authenticated", "rate_remaining", "rate_limit"}` to its bodies (`crates/test-support/src/fake_github.rs:1491-1506`). GitHub reports the budget in the `X-RateLimit-Limit` and `X-RateLimit-Remaining` headers. The adapter's unit test uses the fake's made-up body (`lib.rs:1256`), so every test passed.

Ruled out:
- `cli.sh` passes the environment through to `openlore` (`exec`, no `env -i` or `unset`).
- The variable name is `GITHUB_TOKEN` (`client.rs:34`), read by both constructors and used by production wiring (`crates/cli/src/wiring.rs:189`).
- The token is sent as `Authorization: token …` (`lib.rs:643-644`).

## Fix (user-approved 2026-11-05: header-based report)

- A pure function in `client.rs` works out the report from whether a token was configured and the `X-RateLimit-Limit` / `X-RateLimit-Remaining` header values. It returns `Authenticated { remaining, limit }` only when a token was configured and both headers parse; otherwise `Anonymous`. A 401 already becomes `TokenRejected`.
- `send_get_once` records that report on every 200 response that carries both headers, so a response without them does not overwrite an earlier report. The four body-based calls go away.
- FakeGithub behaves like GitHub: rate-limit headers on every 200 (the person profile and repos list too), no `"auth"` body key.
- The verbs do not change.

Rejected: gating the note on the configured token alone. It changes the locked `GithubPort` contract (ADR-019 §1) and leaves `scrape github` saying `unauthenticated`.

## Regression check

- Acceptance: `scrape person` with a token set and an authenticated fake prints no note and prints `authenticated (`, next to `scrape_person_warns_about_the_unauthenticated_rate_limit` in `tests/acceptance/scrape_person_command.rs`. It fails today.
- Adapter: a real-shaped `/users/{u}` body with no `auth` key plus rate-limit headers, through an adapter with a token, reports `Authenticated`. It replaces the test at `lib.rs:1256`.
- A proptest for the pure function.
- SA-1 (`tests/acceptance/scrape_auth.rs`) still passes, now through headers.

Out of scope: treating a whitespace-only token as absent (`client.rs:134`).
