# Frozen v1 review-app store (review-app-repos-and-accounts MG-1 / MG-2)

A `review-app.duckdb` written by the shipped `openlore-review-app` whose store schema is **v1**
(one `github_links` row per DID), frozen on 2026-10-19 during DISTILL, BEFORE DELIVER 01-16 adds
the v2 migration (ADR-088). Made against `rra_github_fixtures()` and the shared ATProto network:
Priya signed in, verified `github.com/priyaraman`, scanned her default top 5, declined
`priyaraman/driftwood embodies dependency-pinning` and published
`priyaraman/kelp embodies dependency-pinning`.

| Path | What |
|---|---|
| `store/review-app.duckdb` + `.wal` | The store as the v1 app left it when killed (the WAL is part of the state; the migration's `CHECKPOINT` folds it in, ADR-088 §5.2) |
| `pending.txt` | The pending cards the v1 app showed Priya at freeze time — the golden queue MG-1 compares against |

The store's encrypted fields use the acceptance harness's fixed `data-key`
(`support/review_app/app.rs`), so any test app can open it.

Not covered here (DELIVER's adapter-level migration test, ADR-088 §5.2 item 6): a DID with
suggestions and no `github_links` row — the v1 app cannot produce one through its pages.

Schema v1 checked at freeze time (`duckdb -readonly … "SELECT max(version) FROM schema_version"` → 1).

**Do not regenerate after the v2 migration lands.** The regenerator
(`OPENLORE_REGEN_REVIEW_STORE_V1=1 cargo test -p openlore-review-app --test review_app_repos_and_accounts_fixture -- --ignored`)
is its own test target (never in a scenario run), needs the `duckdb` CLI, refuses unless the store
it made is schema v1 and its cards name no source account, and writes only `store.regenerated/`
and `pending.regenerated.txt` — never the committed `store/` or `pending.txt`.
