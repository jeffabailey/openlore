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

## Repo authority observations (U-OOS-8, ADR-087 §4 `read_repo_authority`)

Five single-case fixtures, one per ADR-087 Earned-Trust row, each holding the
production request and GitHub's replies in the order production asks for
them: the repo read (`GET /repos/{owner}/{name}`, with any same-host
redirect), then ALWAYS one `GET /repos/{full_name}/contents/.github/openlore-authority`
(no `ref`: the default branch), against the `full_name` the final repo body
names. Repo replies and the 404 file replies were recorded live
2026-10-20T17:27:09Z–17:27:10Z the same way (unauthenticated `GET`,
`Accept: application/vnd.github+json`), bodies verbatim. No public repo
carries `.github/openlore-authority` yet, so the two 200 file replies are
**hand-shaped, not recorded** (flag `hand_shaped: true` on the reply).

| Fixture | Repo read | Authority-file read |
|---|---|---|
| `repo_owner_user.json` | https://api.github.com/repos/octocat/Hello-World — 200, owner `type: User` (recorded) | 404 (recorded) |
| `repo_owner_org.json` | https://api.github.com/repos/github/gitignore — 200, owner `type: Organization` (recorded) | 404 (recorded) |
| `repo_renamed_redirect.json` | https://api.github.com/repos/rust-lang/rustup.rs — 301 to https://api.github.com/repositories/43192623, then 200 `rust-lang/rustup` (recorded) | `/repos/rust-lang/rustup/contents/…` — 404 (recorded) |
| `authority_file.json` | github/gitignore — 200 (the recorded org body) | 200 `type: file` (**hand-shaped**) |
| `authority_file_symlink.json` | octocat/Hello-World — 200 (the recorded user body) | 200 `type: symlink` (**hand-shaped**) |

How the two file replies were shaped:

- `authority_file.json`: the field set, order and encoding of a live contents
  reply for a regular file (https://api.github.com/repos/github/gitignore/contents/README.md,
  recorded 2026-10-20T17:27:10Z: `type: file`, `encoding: base64`, `size`,
  `content` base64 wrapped at 60 characters with a trailing newline, `sha`,
  `url`/`html_url`/`git_url`/`download_url`, `_links`), with the name, path,
  links and content replaced by a made-up authority file (a comment line, two
  example DIDs — no real person's — a blank line and an inline comment). `sha`
  is the real git blob id of that content.
- `authority_file_symlink.json`: GitHub's documented reply for a symlink
  whose target is not a regular file in the repo ("Get repository content",
  REST docs 2022-11-28: `type: symlink`, `target`, `size`, no `content`); a
  symlink to a file inside the repo is served as that file instead, which the
  API cannot tell apart and the adapter reads as a file.

Read by `crates/adapter-github/tests/github_authority_contract.rs` through
the production `GithubAdapter::read_repo_authority`: owner id, owner type,
`full_name`, `id` and the file's `type`, `encoding`, `size` and `content`
decode; every found repo is read exactly twice; a 404 repo
(`repo_read_cases.json` `missing_404`) is `RepoNotFound` with no file read; a
submodule, directory, over-4-KiB or non-UTF-8 file (variations of
`authority_file.json`) grants nothing; a shape error or a rate limit
(`rate_limited_403`) is an error, never an observation. It runs in the
workspace `cargo nextest run` of CI (`.github/workflows/ci.yml`), the GitHub
contract gate the other `*_contract` tests share.
