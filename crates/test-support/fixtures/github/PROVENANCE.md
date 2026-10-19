# GitHub profile fixtures — provenance

Recorded live from the public, unauthenticated GitHub REST API (read-only
`GET`, no token, header `Accept: application/vnd.github+json`) for GitHub's
public demo account `octocat`. No real person's profile is recorded here.

| Fixture | Live URL | Recorded (UTC) |
|---|---|---|
| `users_profile.json` | https://api.github.com/users/octocat | 2026-10-16T07:42:50Z |
| `user_by_id.json` | https://api.github.com/user/583231 | 2026-10-16T07:42:50Z |

Both answered HTTP 200 with byte-identical bodies: `GET /user/{id}` serves the
same profile shape as `GET /users/{login}` (A-10, first half). The rename /
deleted-account halves of A-10 (current profile after a rename, 404 once
deleted) are not observable on a demo account and stay partly unverified.

Decoded by `crates/adapter-github/tests/github_profile_contract.rs` through
the strict production reshape `adapter_github::client::parse_account_profile`.

## Paged owned-repo listing (U-RRA-3, U-RRA-7)

`owned_repos_paged.json` holds four consecutive pages of GitHub's public
owned-repo listing for `octocat`, recorded live 2026-10-19T16:01:12Z the same
way (unauthenticated `GET`, `Accept: application/vnd.github+json`, all HTTP
200). `per_page=2` (production asks for 100) so octocat's 8 repos span four
pages and the 3-page cap can be observed on a demo account. Each page keeps its
verbatim `Link` header and body:

| Page | Live URL |
|---|---|
| 1 | https://api.github.com/users/octocat/repos?type=owner&sort=pushed&per_page=2 |
| 2 | https://api.github.com/user/583231/repos?type=owner&sort=pushed&per_page=2&page=2 |
| 3 | https://api.github.com/user/583231/repos?type=owner&sort=pushed&per_page=2&page=3 |
| 4 | https://api.github.com/user/583231/repos?type=owner&sort=pushed&per_page=2&page=4 |

Observed: the `rel="next"` links point at `/user/{id}/repos` (the account
number, not the login) on the same host, and the last page names no `next`.

Read by `crates/adapter-github/tests/github_listing_contract.rs` through the
production `GithubAdapter::list_account_repos` (3 pages read and reported
truncated; a last page without `next` reported complete; a `next` on another
host refused).

## Repo reads (U-RRA-3, ADR-089 §1 `read_repo`)

`repo_read_cases.json` holds GitHub's replies to `GET /repos/{owner}/{name}`,
recorded live 2026-10-19T16:33:06Z–16:33:17Z the same way (unauthenticated
`GET`, `Accept: application/vnd.github+json`), bodies verbatim. Only public
demo and well-known open-source repos are recorded.

| Case | Live URL(s) | Status |
|---|---|---|
| `user_200` | https://api.github.com/repos/octocat/Hello-World | 200 (owner `type: User`) |
| `org_200` | https://api.github.com/repos/github/gitignore | 200 (owner `type: Organization`) |
| `rename_301` | https://api.github.com/repos/rust-lang/rustup.rs → https://api.github.com/repositories/43192623 | 301, then 200 (`rust-lang/rustup`) |
| `transfer_301` | https://api.github.com/repos/carllerche/mio → https://api.github.com/repositories/23138984 | 301, then 200 (`tokio-rs/mio`, a new owner id) |
| `missing_404` | https://api.github.com/repos/octocat/this-repo-does-not-exist-openlore | 404 |
| `rate_limited_403` | — | 403, **documented, not recorded** |

Observed: a renamed or transferred repo answers `301` with `Location:
https://api.github.com/repositories/{id}` on the same host (not
`/repos/{new owner}/{new name}`); the final body names the current
`full_name` and owner. The 403 case is built from GitHub's documented
rate-limit reply (`x-ratelimit-remaining: 0`, "API rate limit exceeded")
because exhausting the live budget to record it is not worth it; the
`documented_not_recorded` flag marks it.

Read by `crates/adapter-github/tests/github_repo_contract.rs` through the
production `GithubAdapter::read_repo` (facts for a user's and an org's repo;
one same-host redirect followed for a rename and a transfer; 404 not found;
403 a rate limit; a redirect off the host or a second redirect refused).
