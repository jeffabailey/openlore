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
